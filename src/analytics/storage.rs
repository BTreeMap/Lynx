//! Analytics storage models

use serde::{Deserialize, Serialize};

/// Analytics record stored in database
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
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

/// Request for querying analytics
#[derive(Debug, Clone, Deserialize)]
pub struct AnalyticsQuery {
    /// Filter by short code
    pub short_code: Option<String>,
    
    /// Start time (Unix timestamp)
    pub start_time: Option<i64>,
    
    /// End time (Unix timestamp)
    pub end_time: Option<i64>,
    
    /// Group by field
    pub group_by: Option<AnalyticsGroupBy>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AnalyticsGroupBy {
    Country,
    Region,
    City,
    Asn,
    Hour,
    Day,
}

/// Aggregated analytics result
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct AnalyticsAggregate {
    pub dimension: String,
    pub visit_count: i64,
}
