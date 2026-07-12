use anyhow::{bail, ensure, Context, Result};
use reqwest::StatusCode;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::task::JoinSet;

use super::client::{ExternalService, UrlRecord};
use super::config::ExternalConfig;

const EVENTUAL_TIMEOUT: Duration = Duration::from_secs(20);
const POLL_INTERVAL: Duration = Duration::from_millis(100);

/// Names generated for one external test run. The time-based namespace keeps
/// manual reruns against a persistent service disjoint without adding UUID-only
/// test dependencies or weakening endpoint typing.
pub struct CodeFactory {
    namespace: String,
    next: AtomicU64,
}

impl CodeFactory {
    pub fn new(max_length: usize) -> Result<Self> {
        const MINIMUM_TEST_CODE_LENGTH: usize = 20;
        ensure!(
            max_length >= MINIMUM_TEST_CODE_LENGTH,
            "external harness needs SHORT_CODE_MAX_LENGTH >= {MINIMUM_TEST_CODE_LENGTH}, got {max_length}"
        );
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("system clock predates Unix epoch")?
            .as_nanos();
        Ok(Self {
            namespace: format!("t{now:x}"),
            next: AtomicU64::new(0),
        })
    }

    pub fn code(&self) -> String {
        let sequence = self.next.fetch_add(1, Ordering::Relaxed);
        format!("{}{:x}", self.namespace, sequence)
    }
}

pub async fn run_functional_suite(
    service: &ExternalService,
    config: &ExternalConfig,
) -> Result<()> {
    service.wait_until_ready(config.readiness_timeout).await?;
    let auth = service.auth_mode().await?;
    ensure!(
        auth.mode == "none",
        "external suite requires AUTH_MODE=none"
    );
    let codes = CodeFactory::new(auth.short_code_max_length)?;

    let primary_code = codes.code();
    let primary_target = "https://example.com/functional-primary";
    let primary = service.create(primary_target, Some(&primary_code)).await?;
    assert_url(&primary, &primary_code, primary_target)?;

    let automatic = service
        .create("https://example.com/automatic", None)
        .await
        .context("create a server-generated short code")?;
    ensure!(
        !automatic.short_code.is_empty(),
        "server returned an empty generated short code"
    );

    let loaded = service.get(&primary_code).await?;
    assert_url(&loaded, &primary_code, primary_target)?;

    let redirect = service.redirect(&primary_code).await?;
    ensure!(
        redirect.status.is_redirection(),
        "redirect returned {}, expected a 3xx response",
        redirect.status
    );
    ensure!(
        redirect.location.as_deref() == Some(primary_target),
        "redirect Location was {:?}, expected {primary_target}",
        redirect.location
    );
    wait_for_clicks_exact(service, &primary_code, 1, EVENTUAL_TIMEOUT).await?;

    let first_page = service.list(10, None).await?;
    ensure!(
        first_page
            .urls
            .iter()
            .any(|url| url.short_code == primary_code),
        "first URL page does not contain the created primary URL"
    );

    let deactivated = service.deactivate(&primary_code).await?;
    ensure!(
        deactivated.message.contains("deactivated"),
        "unexpected deactivate message {:?}",
        deactivated.message
    );
    let inactive_redirect = service.redirect(&primary_code).await?;
    ensure!(
        inactive_redirect.status == StatusCode::GONE,
        "deactivated redirect returned {}, expected 410 Gone",
        inactive_redirect.status
    );

    let reactivated = service.reactivate(&primary_code).await?;
    ensure!(
        reactivated.message.contains("reactivated"),
        "unexpected reactivate message {:?}",
        reactivated.message
    );
    ensure!(
        service
            .redirect(&primary_code)
            .await?
            .status
            .is_redirection(),
        "reactivated URL did not return a redirect"
    );

    let rapid_codes = create_urls_concurrently(service, &codes, 50, "rapid").await?;
    ensure!(
        rapid_codes.len() == 50,
        "rapid creation did not return 50 URLs"
    );

    let hot_code = rapid_codes
        .first()
        .context("rapid creation unexpectedly produced no URLs")?;
    run_redirect_burst(service, hot_code, 20).await?;
    wait_for_clicks_exact(service, hot_code, 20, EVENTUAL_TIMEOUT).await?;

    let special_code = codes.code();
    let special_target = "https://example.com/?param1=value1&param2=value2#anchor";
    assert_url(
        &service.create(special_target, Some(&special_code)).await?,
        &special_code,
        special_target,
    )?;

    let paginated = service.list(5, None).await?;
    ensure!(paginated.urls.len() <= 5, "page exceeds requested limit");
    ensure!(
        paginated.has_more,
        "expected a second page after creating many URLs"
    );
    let cursor = paginated
        .next_cursor
        .as_deref()
        .context("paginated response says it has more items but has no cursor")?;
    let second_page = service.list(5, Some(cursor)).await?;
    ensure!(
        second_page
            .urls
            .iter()
            .all(|url| !url.short_code.is_empty()),
        "second page contains an empty short code"
    );

    let missing_code = codes.code();
    ensure!(
        service.get_status(&missing_code).await? == StatusCode::NOT_FOUND,
        "unknown URL did not return 404"
    );
    ensure!(
        service
            .duplicate_status("https://example.com/duplicate", &primary_code)
            .await?
            == StatusCode::CONFLICT,
        "duplicate short code was not rejected with 409"
    );

    let stats_code = codes.code();
    service
        .create("https://example.com/exact-clicks", Some(&stats_code))
        .await?;
    run_redirect_burst(service, &stats_code, 10).await?;
    wait_for_clicks_exact(service, &stats_code, 10, EVENTUAL_TIMEOUT).await?;

    let all_urls = service.list(100, None).await?;
    for code in [&primary_code, hot_code, &special_code, &stats_code] {
        ensure!(
            all_urls.urls.iter().any(|url| url.short_code == *code),
            "URL {code} was absent from the consistency page"
        );
    }

    let search = service.search("rapid").await?;
    ensure!(
        search.items.len() >= 50 || search.has_more,
        "rapid search returned too few matches"
    );
    ensure!(
        search.items.iter().any(|url| url.short_code == *hot_code),
        "rapid search omitted its hot URL"
    );
    for empty_query in ["", "   "] {
        ensure!(
            service.search_status(empty_query).await? == StatusCode::BAD_REQUEST,
            "empty search query {empty_query:?} was accepted"
        );
    }

    let history_code = codes.code();
    service
        .create("https://example.com/history-v1", Some(&history_code))
        .await?;
    let updated = service
        .update(&history_code, "https://example.com/history-v2")
        .await?;
    ensure!(
        updated.original_url == "https://example.com/history-v2",
        "updated destination was not returned"
    );
    let history = service.history(&history_code).await?;
    let previous = history
        .iter()
        .find(|entry| entry.historic_url == "https://example.com/history-v1")
        .context("history omitted the previous destination")?;
    let restored = service.restore(&history_code, previous.id).await?;
    ensure!(
        restored.original_url == "https://example.com/history-v1",
        "restore did not reinstate the historic destination"
    );
    ensure!(
        service.update_status(&history_code, "   ").await? == StatusCode::BAD_REQUEST,
        "blank destination update was accepted"
    );

    if config.analytics_expected {
        wait_for_analytics_exact(service, &stats_code, 10, EVENTUAL_TIMEOUT).await?;
    }

    Ok(())
}

pub async fn run_concurrency_suite(
    service: &ExternalService,
    config: &ExternalConfig,
) -> Result<()> {
    service.wait_until_ready(config.readiness_timeout).await?;
    let auth = service.auth_mode().await?;
    let codes = CodeFactory::new(auth.short_code_max_length)?;
    let concurrency = config.concurrency.get();

    let created = create_urls_concurrently(service, &codes, concurrency, "concurrent").await?;
    ensure!(
        created.len() == concurrency,
        "concurrent creation returned {}/{} URLs",
        created.len(),
        concurrency
    );

    let redirect_code = codes.code();
    service
        .create(
            "https://example.com/concurrent-redirect",
            Some(&redirect_code),
        )
        .await?;
    run_redirect_burst(service, &redirect_code, concurrency).await?;
    wait_for_clicks_exact(
        service,
        &redirect_code,
        concurrency as i64,
        EVENTUAL_TIMEOUT,
    )
    .await?;

    run_mixed_operations(service, &codes, concurrency / 3).await?;

    let state_code = codes.code();
    service
        .create("https://example.com/state-changes", Some(&state_code))
        .await?;
    let redirects = {
        let service = service.clone();
        let code = state_code.clone();
        tokio::spawn(async move { run_state_change_redirect_burst(&service, &code, 20).await })
    };
    for _ in 0..5 {
        service.deactivate(&state_code).await?;
        service.reactivate(&state_code).await?;
    }
    redirects
        .await
        .context("state-change redirect burst panicked")??;
    ensure!(
        service.get(&state_code).await?.is_active,
        "final URL state is inactive"
    );

    let high_frequency_code = codes.code();
    service
        .create(
            "https://example.com/high-frequency-clicks",
            Some(&high_frequency_code),
        )
        .await?;
    run_redirect_burst(service, &high_frequency_code, 200).await?;
    wait_for_clicks_exact(service, &high_frequency_code, 200, EVENTUAL_TIMEOUT).await?;

    let mut reads = JoinSet::new();
    for _ in 0..30 {
        let service = service.clone();
        reads.spawn(async move { service.list(50, None).await });
    }
    while let Some(result) = reads.join_next().await {
        let page = result.context("concurrent list task panicked")??;
        ensure!(
            !page.urls.is_empty(),
            "concurrent list response unexpectedly contained no URLs"
        );
    }

    Ok(())
}

pub async fn run_redirect_burst(
    service: &ExternalService,
    code: &str,
    requests: usize,
) -> Result<()> {
    let mut redirects = JoinSet::new();
    for _ in 0..requests {
        let service = service.clone();
        let code = code.to_owned();
        redirects.spawn(async move { service.redirect(&code).await });
    }

    while let Some(result) = redirects.join_next().await {
        let response = result.context("redirect task panicked")??;
        ensure!(
            response.status.is_redirection(),
            "redirect burst received {}, expected a 3xx response",
            response.status
        );
    }
    Ok(())
}

async fn run_state_change_redirect_burst(
    service: &ExternalService,
    code: &str,
    requests: usize,
) -> Result<()> {
    let mut redirects = JoinSet::new();
    for _ in 0..requests {
        let service = service.clone();
        let code = code.to_owned();
        redirects.spawn(async move { service.redirect(&code).await });
    }

    while let Some(result) = redirects.join_next().await {
        let response = result.context("state-change redirect task panicked")??;
        ensure!(
            response.status.is_redirection() || response.status == StatusCode::GONE,
            "state-change redirect received {}, expected a 3xx or 410 response",
            response.status
        );
    }
    Ok(())
}

pub async fn wait_for_clicks_exact(
    service: &ExternalService,
    code: &str,
    expected: i64,
    timeout: Duration,
) -> Result<()> {
    let deadline = tokio::time::Instant::now() + timeout;
    let mut observed = None;
    while tokio::time::Instant::now() < deadline {
        let clicks = service.get(code).await?.clicks;
        if clicks == expected {
            return Ok(());
        }
        if clicks > expected {
            bail!("click count for {code} exceeded exact expectation: {clicks} > {expected}");
        }
        observed = Some(clicks);
        tokio::time::sleep(POLL_INTERVAL).await;
    }
    bail!(
        "click count for {code} did not reach {expected}; last observed {}",
        observed.unwrap_or_default()
    )
}

pub async fn wait_for_analytics_exact(
    service: &ExternalService,
    code: &str,
    expected: i64,
    timeout: Duration,
) -> Result<()> {
    let deadline = tokio::time::Instant::now() + timeout;
    let mut observed = None;
    while tokio::time::Instant::now() < deadline {
        let visits: i64 = service
            .analytics(code)
            .await?
            .entries
            .iter()
            .map(|entry| entry.visit_count)
            .sum();
        if visits == expected {
            return Ok(());
        }
        if visits > expected {
            bail!("analytics count for {code} exceeded exact expectation: {visits} > {expected}");
        }
        observed = Some(visits);
        tokio::time::sleep(POLL_INTERVAL).await;
    }
    bail!(
        "analytics count for {code} did not reach {expected}; last observed {}",
        observed.unwrap_or_default()
    )
}

async fn create_urls_concurrently(
    service: &ExternalService,
    codes: &CodeFactory,
    count: usize,
    path: &str,
) -> Result<Vec<String>> {
    let mut requests = JoinSet::new();
    let mut expected_codes = Vec::with_capacity(count);
    for index in 0..count {
        let service = service.clone();
        let code = codes.code();
        let target = format!("https://example.com/{path}/{index}");
        expected_codes.push(code.clone());
        requests.spawn(async move { service.create(&target, Some(&code)).await });
    }

    let mut created = Vec::with_capacity(count);
    while let Some(result) = requests.join_next().await {
        let url = result.context("concurrent create task panicked")??;
        created.push(url.short_code);
    }
    created.sort_unstable();
    expected_codes.sort_unstable();
    ensure!(
        created == expected_codes,
        "concurrent creation did not return exactly the requested short codes"
    );
    Ok(created)
}

async fn run_mixed_operations(
    service: &ExternalService,
    codes: &CodeFactory,
    rounds: usize,
) -> Result<()> {
    let base_code = codes.code();
    service
        .create("https://example.com/mixed-base", Some(&base_code))
        .await?;

    let mut operations = JoinSet::new();
    for index in 0..rounds {
        let create_service = service.clone();
        let code = codes.code();
        operations.spawn(async move {
            create_service
                .create(&format!("https://example.com/mixed/{index}"), Some(&code))
                .await
                .map(|_| ())
        });

        let get_service = service.clone();
        let get_code = base_code.clone();
        operations.spawn(async move { get_service.get(&get_code).await.map(|_| ()) });

        let redirect_service = service.clone();
        let redirect_code = base_code.clone();
        operations.spawn(async move {
            let response = redirect_service.redirect(&redirect_code).await?;
            ensure!(
                response.status.is_redirection(),
                "mixed redirect was not a 3xx"
            );
            Ok(())
        });
    }

    while let Some(result) = operations.join_next().await {
        result.context("mixed operation task panicked")??;
    }
    Ok(())
}

fn assert_url(url: &UrlRecord, expected_code: &str, expected_target: &str) -> Result<()> {
    ensure!(
        url.short_code == expected_code,
        "response code {:?}, expected {expected_code}",
        url.short_code
    );
    ensure!(
        url.original_url == expected_target,
        "response target {:?}, expected {expected_target}",
        url.original_url
    );
    Ok(())
}
