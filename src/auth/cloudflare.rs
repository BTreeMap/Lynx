use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{anyhow, bail, Context, Result};
use jsonwebtoken::{decode, decode_header, DecodingKey, Validation};
use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;
use tokio::sync::RwLock;
use tracing::{debug, warn};

use crate::config::CloudflareConfig;

/// Cloudflare Zero Trust validator with stale-while-revalidate caching
#[derive(Clone)]
pub struct CloudflareValidator {
    team_domain: String,
    audience: String,
    certs_uri: String,
    client: Client,
    keys: Arc<RwLock<HashMap<String, Arc<DecodingKey>>>>,
    last_refresh: Arc<RwLock<Option<Instant>>>,
    cache_ttl: Duration,
}

impl CloudflareValidator {
    pub async fn from_config(config: &CloudflareConfig) -> Result<Self> {
        let client = Client::builder()
            .user_agent("lynx-cloudflare-validator/0.1.0")
            .timeout(Duration::from_secs(10))
            .build()
            .context("failed to build HTTP client for Cloudflare validation")?;

        let team_domain = config.team_domain.trim_end_matches('/').to_string();
        let certs_uri = format!("{}/cdn-cgi/access/certs", team_domain);

        let validator = Self {
            team_domain: team_domain.clone(),
            audience: config.audience.clone(),
            certs_uri,
            client,
            keys: Arc::new(RwLock::new(HashMap::new())),
            last_refresh: Arc::new(RwLock::new(None)),
            cache_ttl: Duration::from_secs(config.certs_cache_ttl_secs.max(3600)),
        };

        // Prime the cache so the first request doesn't incur latency
        if let Err(e) = validator.refresh_keys().await {
            warn!("Failed to prime Cloudflare key cache: {}", e);
            // Don't fail initialization - we'll try again on first request
        }

        Ok(validator)
    }

    pub async fn validate(&self, token: &str) -> Result<Value> {
        let header = decode_header(token).context("failed to parse token header")?;

        let kid = header
            .kid
            .ok_or_else(|| anyhow!("token header missing 'kid'"))?;

        let key = self.get_decoding_key(&kid).await?;

        let mut validation = Validation::new(header.alg);
        validation.set_audience(&[&self.audience]);
        validation.set_issuer(&[&self.team_domain]);

        let data = decode::<Value>(token, key.as_ref(), &validation)
            .context("token failed signature or structural validation")?;
        let claims = data.claims;

        Ok(claims)
    }

    async fn get_decoding_key(&self, kid: &str) -> Result<Arc<DecodingKey>> {
        // Try stale-while-revalidate pattern
        self.ensure_fresh_keys(Some(kid)).await;

        let keys_guard = self.keys.read().await;
        if let Some(key) = keys_guard.get(kid) {
            return Ok(Arc::clone(key));
        }

        drop(keys_guard);

        // If key not found, try refreshing synchronously
        if let Err(e) = self.refresh_keys().await {
            warn!(
                "Failed to refresh Cloudflare keys when kid was missing: {}",
                e
            );
        }

        let keys_guard = self.keys.read().await;
        keys_guard
            .get(kid)
            .cloned()
            .ok_or_else(|| anyhow!("no certificate found for key id '{kid}'"))
    }

    async fn ensure_fresh_keys(&self, kid: Option<&str>) {
        let needs_refresh = {
            let last_guard = self.last_refresh.read().await;
            match *last_guard {
                Some(last) => last.elapsed() > self.cache_ttl,
                None => true,
            }
        };

        if needs_refresh {
            debug!("Refreshing Cloudflare certs cache due to expiration (background)");
            // Spawn background refresh - don't block on it
            let self_clone = self.clone();
            tokio::spawn(async move {
                if let Err(e) = self_clone.refresh_keys().await {
                    warn!("Background Cloudflare key refresh failed: {}", e);
                }
            });
            return;
        }

        if let Some(k) = kid {
            let missing = {
                let keys_guard = self.keys.read().await;
                !keys_guard.contains_key(k)
            };
            if missing {
                debug!(
                    "Refreshing Cloudflare certs cache because key {k} was missing (background)"
                );
                let self_clone = self.clone();
                tokio::spawn(async move {
                    if let Err(e) = self_clone.refresh_keys().await {
                        warn!("Background Cloudflare key refresh failed: {}", e);
                    }
                });
            }
        }
    }

    async fn refresh_keys(&self) -> Result<()> {
        let response = self
            .client
            .get(&self.certs_uri)
            .send()
            .await
            .context("failed to request Cloudflare certs")?
            .error_for_status()
            .context("Cloudflare certs endpoint returned an error status")?;

        let certs: CloudflareCerts = response
            .json()
            .await
            .context("failed to parse Cloudflare certs response")?;

        let mut new_keys: HashMap<String, Arc<DecodingKey>> = HashMap::new();

        // Process JWK format keys
        for jwk in certs.keys {
            let Some(kid) = jwk.kid else {
                warn!("Skipping Cloudflare cert without 'kid'");
                continue;
            };

            match jwk.kty.as_str() {
                "RSA" => {
                    let n = jwk
                        .n
                        .as_deref()
                        .ok_or_else(|| anyhow!("Cloudflare RSA key missing modulus"))?;
                    let e = jwk
                        .e
                        .as_deref()
                        .ok_or_else(|| anyhow!("Cloudflare RSA key missing exponent"))?;
                    let key = DecodingKey::from_rsa_components(n, e)
                        .context("failed to build RSA decoding key from Cloudflare cert")?;
                    new_keys.insert(kid, Arc::new(key));
                }
                other => {
                    warn!("Skipping unsupported Cloudflare key type: {other}");
                }
            }
        }

        if new_keys.is_empty() {
            bail!("Cloudflare certs response did not contain any usable keys");
        }

        let mut keys_guard = self.keys.write().await;
        *keys_guard = new_keys;
        let mut last_guard = self.last_refresh.write().await;
        *last_guard = Some(Instant::now());

        Ok(())
    }
}

#[derive(Debug, Deserialize)]
struct CloudflareCerts {
    keys: Vec<CloudflareJwk>,
}

#[derive(Debug, Deserialize)]
struct CloudflareJwk {
    kid: Option<String>,
    #[serde(default)]
    kty: String,
    #[serde(default)]
    n: Option<String>,
    #[serde(default)]
    e: Option<String>,
}
