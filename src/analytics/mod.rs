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

// Re-export commonly used types
pub use geoip::GeoIpService;
pub use ip_extractor::extract_client_ip;
pub use models::{AnalyticsRecord, GeoLocation};
pub use aggregator::AnalyticsAggregator;
pub use storage::{AnalyticsEntry, AnalyticsQuery, AnalyticsAggregate, AnalyticsGroupBy};
