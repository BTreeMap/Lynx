//! Visitor IP analytics module
//!
//! This module provides optional, high-performance visitor IP analytics
//! using MaxMind GeoLite2 MMDB for geolocation lookups.
//!
//! The feature is designed to be optional and does not affect core
//! URL redirection performance when disabled.

#[cfg(feature = "analytics")]
pub mod geoip;

#[cfg(feature = "analytics")]
pub mod ip_extractor;

#[cfg(feature = "analytics")]
pub mod models;

#[cfg(feature = "analytics")]
pub mod aggregator;

// Re-export commonly used types when analytics is enabled
#[cfg(feature = "analytics")]
pub use geoip::GeoIpService;
#[cfg(feature = "analytics")]
pub use ip_extractor::extract_client_ip;
#[cfg(feature = "analytics")]
pub use models::{AnalyticsRecord, GeoLocation};
#[cfg(feature = "analytics")]
pub use aggregator::AnalyticsAggregator;

// Stub implementations when analytics is disabled
#[cfg(not(feature = "analytics"))]
pub struct GeoIpService;

#[cfg(not(feature = "analytics"))]
impl GeoIpService {
    pub fn new(_path: &str) -> anyhow::Result<Self> {
        Ok(Self)
    }
}
