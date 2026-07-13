use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use reqwest::{redirect::Policy, Client, Response, StatusCode};
use serde::{de::DeserializeOwned, Deserialize};
use serde_json::json;
use std::time::Duration;

use super::config::{Endpoint, ExternalConfig};

#[derive(Clone)]
pub struct ExternalService {
    api: Endpoint,
    redirect: Endpoint,
    client: Client,
}

#[derive(Debug, Deserialize)]
pub struct UrlRecord {
    pub id: i64,
    pub short_code: String,
    pub original_url: String,
    pub created_at: i64,
    pub created_by: Option<String>,
    pub clicks: i64,
    pub is_active: bool,
    pub redirect_base_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct HealthResponse {
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct AuthModeResponse {
    pub mode: String,
    pub short_code_max_length: usize,
}

#[derive(Debug, Deserialize)]
pub struct MessageResponse {
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Debug, Deserialize)]
pub struct UrlPage {
    pub urls: Vec<UrlRecord>,
    pub next_cursor: Option<String>,
    pub has_more: bool,
}

#[derive(Debug, Deserialize)]
pub struct SearchPage {
    pub items: Vec<UrlRecord>,
    pub next_cursor: Option<String>,
    pub has_more: bool,
}

#[derive(Debug, Deserialize)]
pub struct UrlHistoryEntry {
    pub id: i64,
    pub short_code: String,
    pub historic_url: String,
    pub changed_at: i64,
    pub changed_by: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AnalyticsEntry {
    pub id: i64,
    pub short_code: String,
    pub time_bucket: i64,
    pub country_code: Option<String>,
    pub region: Option<String>,
    pub city: Option<String>,
    pub asn: Option<i64>,
    pub ip_version: i32,
    pub visit_count: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Deserialize)]
pub struct AnalyticsResponse {
    pub entries: Vec<AnalyticsEntry>,
    pub total: usize,
    pub clicks: i64,
}

#[derive(Debug)]
pub struct RedirectResponse {
    pub status: StatusCode,
    pub location: Option<String>,
}

impl ExternalService {
    pub fn new(config: &ExternalConfig) -> Result<Self> {
        let client = Client::builder()
            // Redirect behavior is part of Lynx's public contract. The test
            // client must observe the redirect response rather than follow an
            // unrelated target on the public internet.
            .redirect(Policy::none())
            .connect_timeout(Duration::from_secs(5))
            .timeout(config.request_timeout)
            // Each native load worker has at most one live request. Keeping
            // enough idle connections avoids benchmark artifacts caused by
            // pool eviction between requests.
            .pool_max_idle_per_host(usize::MAX)
            .build()
            .context("build external test HTTP client")?;

        Ok(Self {
            api: config.api.clone(),
            redirect: config.redirect.clone(),
            client,
        })
    }

    pub fn api_base(&self) -> &str {
        self.api.as_str()
    }

    pub fn redirect_base(&self) -> &str {
        self.redirect.as_str()
    }

    pub async fn health(&self) -> Result<HealthResponse> {
        let response = self.client.get(self.api_url("api/health")?).send().await?;
        decode_expected(response, StatusCode::OK, "GET /api/health").await
    }

    pub async fn auth_mode(&self) -> Result<AuthModeResponse> {
        let response = self
            .client
            .get(self.api_url("api/auth/mode")?)
            .send()
            .await?;
        decode_expected(response, StatusCode::OK, "GET /api/auth/mode").await
    }

    pub async fn wait_until_ready(&self, timeout: Duration) -> Result<()> {
        let deadline = tokio::time::Instant::now() + timeout;
        let mut last_error = None;

        while tokio::time::Instant::now() < deadline {
            match self.health().await {
                Ok(health) if health.message == "OK" => return Ok(()),
                Ok(health) => {
                    last_error = Some(format!("unexpected health message {:?}", health.message));
                }
                Err(error) => last_error = Some(format!("{error:#}")),
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }

        bail!(
            "service did not become ready within {:?}: {}",
            timeout,
            last_error.unwrap_or_else(|| "no health response".to_owned())
        )
    }

    pub async fn create(&self, original_url: &str, custom_code: Option<&str>) -> Result<UrlRecord> {
        let response = self
            .client
            .post(self.api_url("api/urls")?)
            .json(&json!({ "url": original_url, "custom_code": custom_code }))
            .send()
            .await?;
        decode_expected(response, StatusCode::CREATED, "POST /api/urls").await
    }

    pub async fn get(&self, code: &str) -> Result<UrlRecord> {
        let response = self
            .client
            .get(self.api_code_url("api/urls", code)?)
            .send()
            .await?;
        decode_expected(response, StatusCode::OK, "GET /api/urls/{code}").await
    }

    pub async fn get_status(&self, code: &str) -> Result<StatusCode> {
        Ok(self
            .client
            .get(self.api_code_url("api/urls", code)?)
            .send()
            .await?
            .status())
    }

    pub async fn list(&self, limit: usize, cursor: Option<&str>) -> Result<UrlPage> {
        let limit = limit.to_string();
        let mut query = vec![("limit", limit.as_str())];
        if let Some(cursor) = cursor {
            query.push(("cursor", cursor));
        }
        let response = self
            .client
            .get(self.api_url_with_query("api/urls", &query)?)
            .send()
            .await?;
        decode_expected(response, StatusCode::OK, "GET /api/urls").await
    }

    pub async fn search(&self, query: &str) -> Result<SearchPage> {
        let response = self
            .client
            .get(self.api_url_with_query("api/urls/search", &[("q", query)])?)
            .send()
            .await?;
        decode_expected(response, StatusCode::OK, "GET /api/urls/search").await
    }

    pub async fn search_status(&self, query: &str) -> Result<StatusCode> {
        Ok(self
            .client
            .get(self.api_url_with_query("api/urls/search", &[("q", query)])?)
            .send()
            .await?
            .status())
    }

    pub async fn deactivate(&self, code: &str) -> Result<MessageResponse> {
        let response = self
            .client
            .put(self.api_code_action_url("api/urls", code, "deactivate")?)
            .json(&json!({}))
            .send()
            .await?;
        decode_expected(response, StatusCode::OK, "PUT /api/urls/{code}/deactivate").await
    }

    pub async fn reactivate(&self, code: &str) -> Result<MessageResponse> {
        let response = self
            .client
            .put(self.api_code_action_url("api/urls", code, "reactivate")?)
            .send()
            .await?;
        decode_expected(response, StatusCode::OK, "PUT /api/urls/{code}/reactivate").await
    }

    pub async fn update(&self, code: &str, original_url: &str) -> Result<UrlRecord> {
        let response = self
            .client
            .patch(self.api_code_url("api/urls", code)?)
            .json(&json!({ "url": original_url }))
            .send()
            .await?;
        decode_expected(response, StatusCode::OK, "PATCH /api/urls/{code}").await
    }

    pub async fn update_status(&self, code: &str, original_url: &str) -> Result<StatusCode> {
        Ok(self
            .client
            .patch(self.api_code_url("api/urls", code)?)
            .json(&json!({ "url": original_url }))
            .send()
            .await?
            .status())
    }

    pub async fn history(&self, code: &str) -> Result<Vec<UrlHistoryEntry>> {
        let response = self
            .client
            .get(self.api_code_action_url("api/urls", code, "history")?)
            .send()
            .await?;
        decode_expected(response, StatusCode::OK, "GET /api/urls/{code}/history").await
    }

    pub async fn restore(&self, code: &str, history_id: i64) -> Result<UrlRecord> {
        let response = self
            .client
            .post(self.api_code_action_url(
                "api/urls",
                code,
                &format!("history/{history_id}/restore"),
            )?)
            .send()
            .await?;
        decode_expected(
            response,
            StatusCode::OK,
            "POST /api/urls/{code}/history/{id}/restore",
        )
        .await
    }

    pub async fn redirect(&self, code: &str) -> Result<RedirectResponse> {
        let response = self.client.get(self.redirect_url(code)?).send().await?;
        let status = response.status();
        let location = response
            .headers()
            .get(reqwest::header::LOCATION)
            .map(|value| {
                value
                    .to_str()
                    .context("redirect Location header is not valid UTF-8")
            })
            .transpose()?
            .map(str::to_owned);
        Ok(RedirectResponse { status, location })
    }

    pub async fn analytics(&self, code: &str) -> Result<AnalyticsResponse> {
        let response = self
            .client
            .get(self.api_code_url("api/analytics", code)?)
            .send()
            .await?;
        decode_expected(response, StatusCode::OK, "GET /api/analytics/{code}").await
    }

    pub async fn redirect_status(&self, code: &str) -> Result<StatusCode> {
        Ok(self.redirect(code).await?.status)
    }

    pub async fn url_status_for_traffic(&self, code: &str) -> Result<StatusCode> {
        Ok(self
            .client
            .get(self.api_code_url("api/urls", code)?)
            .send()
            .await?
            .status())
    }

    pub async fn list_status_for_traffic(&self) -> Result<StatusCode> {
        Ok(self
            .client
            .get(self.api_url("api/urls?limit=50")?)
            .send()
            .await?
            .status())
    }

    pub async fn health_status_for_traffic(&self) -> Result<StatusCode> {
        Ok(self
            .client
            .get(self.api_url("api/health")?)
            .send()
            .await?
            .status())
    }

    pub async fn create_status_for_traffic(&self, sequence: u64) -> Result<StatusCode> {
        Ok(self
            .client
            .post(self.api_url("api/urls")?)
            .json(&json!({ "url": format!("https://example.com/native-load/{sequence}") }))
            .send()
            .await?
            .status())
    }

    pub async fn deactivate_status_for_traffic(&self, code: &str) -> Result<StatusCode> {
        Ok(self
            .client
            .put(self.api_code_action_url("api/urls", code, "deactivate")?)
            .send()
            .await?
            .status())
    }

    pub async fn analytics_status_for_traffic(&self, code: &str) -> Result<StatusCode> {
        Ok(self
            .client
            .get(self.api_code_url("api/analytics", code)?)
            .send()
            .await?
            .status())
    }

    pub async fn duplicate_status(&self, original_url: &str, code: &str) -> Result<StatusCode> {
        Ok(self
            .client
            .post(self.api_url("api/urls")?)
            .json(&json!({ "url": original_url, "custom_code": code }))
            .send()
            .await?
            .status())
    }

    fn api_url(&self, path: &str) -> Result<reqwest::Url> {
        self.api.url(path)
    }

    fn api_url_with_query(&self, path: &str, query: &[(&str, &str)]) -> Result<reqwest::Url> {
        let mut url = self.api_url(path)?;
        url.query_pairs_mut().extend_pairs(query);
        Ok(url)
    }

    fn api_code_url(&self, collection: &str, code: &str) -> Result<reqwest::Url> {
        self.api.url(&format!("{collection}/{}", encode_code(code)))
    }

    fn api_code_action_url(
        &self,
        collection: &str,
        code: &str,
        action: &str,
    ) -> Result<reqwest::Url> {
        self.api
            .url(&format!("{collection}/{}/{action}", encode_code(code)))
    }

    fn redirect_url(&self, code: &str) -> Result<reqwest::Url> {
        // Redirect routes consume the public short code directly. API routes
        // Base64URL-encode their path parameter so arbitrary codes remain one
        // management-route segment, but applying that representation here
        // makes the redirect lookup search for the wrong code.
        self.redirect.url(code)
    }
}

fn encode_code(code: &str) -> String {
    URL_SAFE_NO_PAD.encode(code)
}

async fn decode_expected<T: DeserializeOwned>(
    response: Response,
    expected: StatusCode,
    operation: &str,
) -> Result<T> {
    let status = response.status();
    let body = response.text().await.context("read HTTP response body")?;
    if status != expected {
        let detail = serde_json::from_str::<ErrorResponse>(&body)
            .map(|error| error.error)
            .unwrap_or(body);
        bail!("{operation} returned {status}, expected {expected}: {detail}");
    }
    serde_json::from_str(&body).with_context(|| format!("decode {operation} success response"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::num::NonZeroUsize;

    fn test_config() -> ExternalConfig {
        ExternalConfig {
            api: Endpoint::parse("http://api.test", "TEST_API_URL").unwrap(),
            redirect: Endpoint::parse("http://redirect.test", "TEST_REDIRECT_URL").unwrap(),
            request_timeout: Duration::from_secs(1),
            readiness_timeout: Duration::from_secs(1),
            concurrency: NonZeroUsize::new(1).unwrap(),
            container_name: None,
            analytics_expected: false,
        }
    }

    #[test]
    fn redirect_url_uses_the_raw_short_code() {
        let service = ExternalService::new(&test_config()).unwrap();

        assert_eq!(
            service.redirect_url("redirect-code").unwrap().as_str(),
            "http://redirect.test/redirect-code"
        );
    }
}
