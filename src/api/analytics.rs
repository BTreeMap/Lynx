//! Analytics API handlers

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::analytics::{AnalyticsAggregate, AnalyticsEntry};
use crate::storage::Storage;

#[derive(Debug, Deserialize)]
pub struct AnalyticsQueryParams {
    /// Start time (Unix timestamp)
    pub start_time: Option<i64>,
    
    /// End time (Unix timestamp)  
    pub end_time: Option<i64>,
    
    /// Group by dimension
    pub group_by: Option<String>,
    
    /// Limit results (default: 100, max: 1000)
    #[serde(default = "default_limit")]
    pub limit: i64,
}

fn default_limit() -> i64 {
    100
}

#[derive(Debug, Serialize)]
pub struct AnalyticsResponse {
    pub entries: Vec<AnalyticsEntry>,
    pub total: usize,
}

#[derive(Debug, Serialize)]
pub struct AnalyticsAggregateResponse {
    pub aggregates: Vec<AnalyticsAggregate>,
    pub total: usize,
}

/// Get analytics for a specific short code
pub async fn get_analytics(
    State(storage): State<Arc<dyn Storage>>,
    Path(short_code): Path<String>,
    Query(params): Query<AnalyticsQueryParams>,
) -> impl IntoResponse {
    let limit = params.limit.min(1000).max(1);
    
    match storage
        .get_analytics(&short_code, params.start_time, params.end_time, limit)
        .await
    {
        Ok(entries) => {
            let total = entries.len();
            Json(AnalyticsResponse { entries, total }).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to get analytics: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to retrieve analytics",
            )
                .into_response()
        }
    }
}

/// Get aggregated analytics for a specific short code
pub async fn get_analytics_aggregate(
    State(storage): State<Arc<dyn Storage>>,
    Path(short_code): Path<String>,
    Query(params): Query<AnalyticsQueryParams>,
) -> impl IntoResponse {
    let limit = params.limit.min(1000).max(1);
    let group_by = params.group_by.as_deref().unwrap_or("country");
    
    match storage
        .get_analytics_aggregate(&short_code, params.start_time, params.end_time, group_by, limit)
        .await
    {
        Ok(aggregates) => {
            let total = aggregates.len();
            Json(AnalyticsAggregateResponse { aggregates, total }).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to get analytics aggregate: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to retrieve analytics aggregate",
            )
                .into_response()
        }
    }
}
