use std::sync::{Arc, LazyLock};
use std::time::Instant;

use axum::http::header::{HeaderMap, HeaderValue, LOCATION};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use dashmap::DashMap;
use divan::{black_box, Bencher};
use lynx::models::ShortenedUrl;
use lynx::storage::{LookupMetadata, LookupResult};
use tokio::sync::mpsc;

static SHORT_DESTINATION: &str = "https://example.com/target";
static LONG_DESTINATION: LazyLock<String> = LazyLock::new(|| {
    format!(
        "https://example.com/{}?campaign={}",
        "nested/".repeat(32),
        "redirect-hot-path".repeat(16)
    )
});
static UNICODE_DESTINATION: &str = "https://example.com/caf%C3%A9/%E6%9D%B1%E4%BA%AC";
static INVALID_DESTINATION: &str = "https://example.com/invalid\r\nlocation";
static SHORT_LOCATION: LazyLock<HeaderValue> =
    LazyLock::new(|| HeaderValue::try_from(SHORT_DESTINATION).unwrap());
static LONG_LOCATION: LazyLock<HeaderValue> =
    LazyLock::new(|| HeaderValue::try_from(LONG_DESTINATION.as_str()).unwrap());
static SHORT_CODE: LazyLock<String> = LazyLock::new(|| "abc12345".to_owned());
static SHARED_SHORT_CODE: LazyLock<Arc<str>> = LazyLock::new(|| Arc::from(SHORT_CODE.as_str()));
static MAX_LENGTH_CODE: LazyLock<String> = LazyLock::new(|| "x".repeat(50));
static CACHED_PROJECTION: LazyLock<Arc<BenchmarkCachedProjection>> = LazyLock::new(|| {
    Arc::new(BenchmarkCachedProjection {
        url: Arc::new(ShortenedUrl {
            id: 1,
            short_code: SHORT_CODE.clone(),
            original_url: SHORT_DESTINATION.to_owned(),
            created_at: 0,
            created_by: None,
            clicks: 0,
            is_active: true,
        }),
        location: (*SHORT_LOCATION).clone(),
        analytics_code: Arc::clone(&*SHARED_SHORT_CODE),
    })
});

/// Mirrors the relevant ownership shape of the cached redirect projection.
struct BenchmarkCachedProjection {
    url: Arc<ShortenedUrl>,
    location: HeaderValue,
    analytics_code: Arc<str>,
}

/// The projection shape used before `RedirectTarget` retained the cache entry.
#[allow(dead_code)]
struct ClonedRedirectProjection {
    url: Arc<ShortenedUrl>,
    location: HeaderValue,
    analytics_code: Arc<str>,
}

/// The projection shape used by the current lean redirect path.
#[allow(dead_code)]
struct RetainedRedirectProjection {
    cached: Arc<BenchmarkCachedProjection>,
}

fn main() {
    divan::main();
}

#[divan::bench]
fn location_parse_short() {
    black_box(HeaderValue::try_from(black_box(SHORT_DESTINATION)).unwrap());
}

#[divan::bench]
fn location_clone_short() {
    black_box(black_box(&*SHORT_LOCATION).clone());
}

#[divan::bench]
fn location_parse_long() {
    black_box(HeaderValue::try_from(black_box(LONG_DESTINATION.as_str())).unwrap());
}

#[divan::bench]
fn location_clone_long() {
    black_box(black_box(&*LONG_LOCATION).clone());
}

#[divan::bench]
fn location_parse_percent_encoded_unicode() {
    black_box(HeaderValue::try_from(black_box(UNICODE_DESTINATION)).unwrap());
}

#[divan::bench]
fn location_parse_invalid() {
    let _ = black_box(HeaderValue::try_from(black_box(INVALID_DESTINATION)));
}

#[divan::bench]
fn short_code_clone_common_length() {
    black_box(black_box(&*SHORT_CODE).clone());
}

#[divan::bench]
fn short_code_clone_max_length() {
    black_box(black_box(&*MAX_LENGTH_CODE).clone());
}

#[divan::bench]
fn analytics_short_code_arc_clone() {
    black_box(Arc::clone(black_box(&*SHARED_SHORT_CODE)));
}

#[divan::bench]
fn redirect_projection_clone_components() {
    let cached = Arc::clone(&CACHED_PROJECTION);
    black_box(ClonedRedirectProjection {
        url: Arc::clone(&cached.url),
        location: cached.location.clone(),
        analytics_code: Arc::clone(&cached.analytics_code),
    });
}

#[divan::bench]
fn redirect_projection_retain_cache_entry() {
    black_box(RetainedRedirectProjection {
        cached: Arc::clone(&CACHED_PROJECTION),
    });
}

#[divan::bench]
fn short_code_transfer_common_length(bencher: Bencher) {
    bencher
        .with_inputs(|| SHORT_CODE.clone())
        .bench_values(black_box);
}

#[divan::bench]
fn short_code_transfer_max_length(bencher: Bencher) {
    bencher
        .with_inputs(|| MAX_LENGTH_CODE.clone())
        .bench_values(black_box);
}

#[divan::bench]
fn click_enqueue_bounded_available(bencher: Bencher) {
    let (sender, mut receiver) = mpsc::channel(1);
    let mut message = Some((SHORT_CODE.clone(), 1_u64));
    bencher.bench_local(move || {
        sender.try_send(message.take().unwrap()).unwrap();
        message = Some(receiver.try_recv().unwrap());
        black_box(&message);
    });
}

#[divan::bench]
fn click_enqueue_full_merge_existing(bencher: Bencher) {
    let (sender, _receiver) = mpsc::channel(1);
    sender.try_send(("queued".to_owned(), 1_u64)).unwrap();
    let overflow = DashMap::new();
    overflow.insert(SHORT_CODE.clone(), 0_u64);

    bencher
        .with_inputs(|| SHORT_CODE.clone())
        .bench_values(move |short_code| {
            let (short_code, amount) = sender
                .try_send((short_code, 1_u64))
                .unwrap_err()
                .into_inner();
            overflow
                .entry(short_code)
                .and_modify(|count| *count += amount)
                .or_insert(amount);
            black_box(());
        });
}

#[divan::bench]
fn plain_lookup_result_shape() {
    black_box(Option::<Arc<lynx::models::ShortenedUrl>>::None);
}

#[divan::bench]
fn measured_lookup_result_shape() {
    let started = Instant::now();
    black_box(LookupResult {
        url: None,
        metadata: LookupMetadata {
            cache_hit: true,
            cache_duration: Some(black_box(started).elapsed()),
            db_duration: None,
        },
    });
}

#[divan::bench]
fn redirect_response_lean() {
    black_box(
        (
            StatusCode::PERMANENT_REDIRECT,
            [(LOCATION, black_box(&*SHORT_LOCATION).clone())],
        )
            .into_response(),
    );
}

#[divan::bench]
fn redirect_response_with_timing_headers() {
    let mut headers = HeaderMap::new();
    headers.insert(LOCATION, black_box(&*SHORT_LOCATION).clone());
    headers.insert("x-lynx-cache-hit", HeaderValue::from_static("true"));
    headers.insert("x-lynx-timing-total-ms", HeaderValue::from(1_u64));
    headers.insert("x-lynx-timing-cache-ms", HeaderValue::from(0_u64));
    headers.insert("x-lynx-timing-db-ms", HeaderValue::from(0_u64));
    headers.insert("x-lynx-timing-handler-ms", HeaderValue::from(1_u64));
    black_box((StatusCode::PERMANENT_REDIRECT, headers).into_response());
}
