//! Visitor IP analytics module
//!
//! This module provides optional, high-performance visitor IP analytics
//! using MaxMind GeoLite2 MMDB for geolocation lookups.
//!
//! The feature is designed to be optional (via runtime configuration)
//! and does not affect core URL redirection performance when disabled.

pub mod geoip;
pub mod ip_extractor;
pub mod models;
pub mod aggregator;
pub mod storage;

// Constants for analytics pruning and alignment
pub const DROPPED_DIMENSION_MARKER: &str = "<dropped>";
pub const DEFAULT_IP_VERSION: i32 = 4; // IPv4
pub const DROPPED_TIME_BUCKET: i64 = 0;

// Re-export commonly used types
pub use geoip::GeoIpService;
pub use ip_extractor::extract_client_ip;
pub use models::{AnalyticsRecord, AnalyticsEvent, GeoLocation};
pub use aggregator::AnalyticsAggregator;
pub use storage::{AnalyticsEntry, AnalyticsQuery, AnalyticsAggregate, AnalyticsGroupBy};
