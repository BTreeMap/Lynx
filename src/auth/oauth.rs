use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{anyhow, bail, Context, Result};
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;
use tokio::sync::RwLock;
use tracing::{debug, warn};

use crate::config::OAuthConfig;

#[derive(Clone)]
pub struct OAuthValidator {
    issuer: String,
    audience: String,
    jwks_uri: String,
    client: Client,
    keys: Arc<RwLock<HashMap<String, Arc<DecodingKey<'static>>>>>,
    last_refresh: Arc<RwLock<Option<Instant>>>,
    cache_ttl: Duration,
}

impl OAuthValidator {
    pub async fn from_config(config: &OAuthConfig) -> Result<Self> {
        let client = Client::builder()
            .user_agent("lynx-oauth-validator/0.1.0")
            .build()
            .context("failed to build HTTP client for OAuth validation")?;

        let jwks_uri = resolve_jwks_uri(config, &client).await?;
        let validator = Self {
            issuer: config.issuer_url.clone(),
            audience: config.audience.clone(),
            jwks_uri,
            client,
            keys: Arc::new(RwLock::new(HashMap::new())),
            last_refresh: Arc::new(RwLock::new(None)),
            cache_ttl: Duration::from_secs(config.jwks_cache_ttl_secs.max(60)),
        };

        // Prime the JWKS cache so the first request doesn't incur latency.
        validator.refresh_keys().await?;

        Ok(validator)
    }

    pub async fn validate(&self, token: &str) -> Result<Value> {
        let header = decode_header(token).context("failed to parse token header")?;
        if header.alg == Algorithm::None {
            bail!("unsigned tokens are not allowed");
        }

        let kid = header
            .kid
            .ok_or_else(|| anyhow!("token header missing 'kid'"))?;

        let key = self.get_decoding_key(&kid).await?;

        let mut validation = Validation::new(header.alg);
        validation.validate_aud = false;
        validation.validate_iss = false;

        let data = decode::<Value>(token, key.as_ref(), &validation)
            .context("token failed signature or structural validation")?;
        let claims = data.claims;

        let issuer = claims
            .get("iss")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("token missing 'iss' claim"))?;
        if issuer != self.issuer {
            bail!("token issuer '{}' does not match expected issuer", issuer);
        }

        if !audience_matches(claims.get("aud"), &self.audience) {
            bail!("token audience does not include expected value");
        }

        Ok(claims)
    }

    async fn get_decoding_key(&self, kid: &str) -> Result<Arc<DecodingKey<'static>>> {
        self.ensure_fresh_keys(Some(kid)).await?;

        let keys_guard = self.keys.read().await;
        if let Some(key) = keys_guard.get(kid) {
            return Ok(Arc::clone(key));
        }

        drop(keys_guard);
        // If the key still isn't available, refresh once more explicitly.
        self.refresh_keys().await?;
        let keys_guard = self.keys.read().await;
        keys_guard
            .get(kid)
            .cloned()
            .ok_or_else(|| anyhow!("no JWKS entry found for key id '{kid}'"))
    }

    async fn ensure_fresh_keys(&self, kid: Option<&str>) -> Result<()> {
        let needs_refresh = {
            let last_guard = self.last_refresh.read().await;
            match *last_guard {
                Some(last) => last.elapsed() > self.cache_ttl,
                None => true,
            }
        };

        if needs_refresh {
            debug!("Refreshing JWKS cache due to expiration");
            self.refresh_keys().await?;
            return Ok(());
        }

        if let Some(k) = kid {
            let missing = {
                let keys_guard = self.keys.read().await;
                !keys_guard.contains_key(k)
            };
            if missing {
                debug!("Refreshing JWKS cache because key {k} was missing");
                self.refresh_keys().await?;
            }
        }

        Ok(())
    }

    async fn refresh_keys(&self) -> Result<()> {
        let response = self
            .client
            .get(&self.jwks_uri)
            .send()
            .await
            .context("failed to request JWKS")?
            .error_for_status()
            .context("JWKS endpoint returned an error status")?;

        let jwks: JwkSet = response
            .json()
            .await
            .context("failed to parse JWKS response")?;

        let mut new_keys: HashMap<String, Arc<DecodingKey<'static>>> = HashMap::new();

        for jwk in jwks.keys {
            let Some(kid) = jwk.kid else {
                warn!("Skipping JWKS entry without 'kid'");
                continue;
            };

            match jwk.kty.as_str() {
                "RSA" => {
                    let n = jwk
                        .n
                        .as_deref()
                        .ok_or_else(|| anyhow!("JWKS RSA key missing modulus"))?;
                    let e = jwk
                        .e
                        .as_deref()
                        .ok_or_else(|| anyhow!("JWKS RSA key missing exponent"))?;
                    let key = DecodingKey::from_rsa_components(n, e)
                        .context("failed to build RSA decoding key from JWKS entry")?;
                    new_keys.insert(kid, Arc::new(key));
                }
                "oct" => {
                    let secret = jwk
                        .k
                        .as_deref()
                        .ok_or_else(|| anyhow!("JWKS symmetric key missing 'k'"))?;
                    let key = DecodingKey::from_base64_secret(secret)
                        .context("failed to build HMAC decoding key from JWKS entry")?;
                    new_keys.insert(kid, Arc::new(key));
                }
                other => {
                    warn!("Skipping unsupported JWKS key type: {other}");
                }
            }
        }

        if new_keys.is_empty() {
            bail!("JWKS response did not contain any usable keys");
        }

        let mut keys_guard = self.keys.write().await;
        *keys_guard = new_keys;
        let mut last_guard = self.last_refresh.write().await;
        *last_guard = Some(Instant::now());

        Ok(())
    }
}

fn audience_matches(aud_claim: Option<&Value>, expected: &str) -> bool {
    match aud_claim {
        Some(Value::String(aud)) => aud == expected,
        Some(Value::Array(entries)) => entries
            .iter()
            .filter_map(Value::as_str)
            .any(|entry| entry == expected),
        _ => false,
    }
}

async fn resolve_jwks_uri(config: &OAuthConfig, client: &Client) -> Result<String> {
    if let Some(url) = &config.jwks_url {
        return Ok(url.clone());
    }

    let issuer = config.issuer_url.trim_end_matches('/');
    let discovery_url = format!("{issuer}/.well-known/openid-configuration");
    let metadata: OpenIdProviderMetadata = client
        .get(&discovery_url)
        .send()
        .await
        .context("failed to request OpenID provider metadata")?
        .error_for_status()
        .context("OpenID provider metadata endpoint returned an error status")?
        .json()
        .await
        .context("failed to parse OpenID provider metadata")?;

    metadata
        .jwks_uri
        .ok_or_else(|| anyhow!("OpenID provider metadata did not include 'jwks_uri'"))
}

#[derive(Debug, Deserialize)]
struct OpenIdProviderMetadata {
    jwks_uri: Option<String>,
}

#[derive(Debug, Deserialize)]
struct JwkSet {
    keys: Vec<Jwk>,
}

#[derive(Debug, Deserialize)]
struct Jwk {
    kid: Option<String>,
    #[serde(default)]
    kty: String,
    #[serde(default)]
    alg: Option<String>,
    #[serde(default)]
    n: Option<String>,
    #[serde(default)]
    e: Option<String>,
    #[serde(default)]
    k: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audience_matching_handles_strings_and_arrays() {
        assert!(audience_matches(Some(&Value::String("abc".into())), "abc"));
        assert!(!audience_matches(Some(&Value::String("abc".into())), "def"));

        let array = Value::Array(vec![
            Value::String("def".into()),
            Value::String("ghi".into()),
        ]);
        assert!(audience_matches(Some(&array), "def"));
        assert!(!audience_matches(Some(&array), "abc"));

        assert!(!audience_matches(None, "abc"));
    }
}
