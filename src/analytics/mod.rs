//! Visitor IP analytics module
//!
//! This module provides optional, high-performance visitor IP analytics
//! using MaxMind GeoLite2 MMDB for geolocation lookups.
//!
//! The feature is designed to be optional (via runtime configuration)
//! and does not affect core URL redirection performance when disabled.

pub mod aggregator;
pub mod geoip;
pub mod ip_extractor;
pub mod models;
pub mod storage;

// Constants for analytics
pub const DROPPED_DIMENSION_MARKER: &str = "<dropped>";
pub const DEFAULT_IP_VERSION: i32 = 4; // IPv4

// Re-export commonly used types
pub use aggregator::AnalyticsAggregator;
pub use geoip::GeoIpService;
pub use ip_extractor::extract_client_ip;
pub use models::{AnalyticsEvent, AnalyticsRecord, GeoLocation};
pub use storage::{AnalyticsAggregate, AnalyticsEntry, AnalyticsGroupBy, AnalyticsQuery};
