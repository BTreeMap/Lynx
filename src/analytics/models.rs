//! Data models for analytics

use serde::{Deserialize, Serialize};
use std::net::IpAddr;

/// Geographic location information derived from IP address
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeoLocation {
    /// ISO country code (e.g., "US", "GB")
    pub country_code: Option<String>,

    /// Country name
    pub country_name: Option<String>,

    /// Region/state/province
    pub region: Option<String>,

    /// City name
    pub city: Option<String>,

    /// Autonomous System Number
    pub asn: Option<u32>,

    /// ASN organization name
    pub asn_org: Option<String>,

    /// IP version (4 or 6)
    pub ip_version: u8,
}

impl Default for GeoLocation {
    fn default() -> Self {
        Self {
            country_code: None,
            country_name: None,
            region: None,
            city: None,
            asn: None,
            asn_org: None,
            ip_version: 4,
        }
    }
}

/// Analytics record for a single visit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsRecord {
    /// Short code that was accessed
    pub short_code: String,

    /// Timestamp of the visit (Unix timestamp)
    pub timestamp: i64,

    /// Geographic location information
    pub geo_location: GeoLocation,

    /// Original client IP (optional, may be anonymized or omitted)
    pub client_ip: Option<IpAddr>,
}

/// Lightweight analytics event for hot path recording
/// GeoIP lookup is deferred until flush time for better performance
#[derive(Debug, Clone)]
pub struct AnalyticsEvent {
    /// Short code that was accessed
    pub short_code: String,

    /// Timestamp of the visit (Unix timestamp)
    pub timestamp: i64,

    /// Client IP address (for deferred GeoIP lookup)
    pub client_ip: IpAddr,
}

/// Aggregated analytics key for grouping
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct AnalyticsKey {
    /// Short code
    pub short_code: String,

    /// Time bucket (hour granularity - Unix timestamp truncated to hour)
    pub time_bucket: i64,

    /// Country code
    pub country_code: Option<String>,

    /// Region
    pub region: Option<String>,

    /// City
    pub city: Option<String>,

    /// ASN
    pub asn: Option<u32>,

    /// IP version
    pub ip_version: u8,
}

impl AnalyticsKey {
    /// Create a new analytics key from a record
    pub fn from_record(record: &AnalyticsRecord) -> Self {
        // Truncate timestamp to hour boundary
        let time_bucket = (record.timestamp / 3600) * 3600;

        Self {
            short_code: record.short_code.clone(),
            time_bucket,
            country_code: record.geo_location.country_code.clone(),
            region: record.geo_location.region.clone(),
            city: record.geo_location.city.clone(),
            asn: record.geo_location.asn,
            ip_version: record.geo_location.ip_version,
        }
    }

    /// Create a new analytics key from an event and geo location
    pub fn from_event(event: &AnalyticsEvent, geo_location: &GeoLocation) -> Self {
        // Truncate timestamp to hour boundary
        let time_bucket = (event.timestamp / 3600) * 3600;

        Self {
            short_code: event.short_code.clone(),
            time_bucket,
            country_code: geo_location.country_code.clone(),
            region: geo_location.region.clone(),
            city: geo_location.city.clone(),
            asn: geo_location.asn,
            ip_version: geo_location.ip_version,
        }
    }
}

/// Aggregated analytics value
#[derive(Debug, Clone, Default)]
pub struct AnalyticsValue {
    /// Count of visits
    pub count: u64,
}
