//! Analytics API handlers

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::analytics::{AnalyticsAggregate, AnalyticsAggregator, AnalyticsEntry};
use crate::storage::Storage;

/// State for analytics handlers
pub struct AnalyticsState {
    pub storage: Arc<dyn Storage>,
    pub aggregator: Option<Arc<AnalyticsAggregator>>,
}

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
    pub clicks: i64,
}

#[derive(Debug, Serialize)]
pub struct AnalyticsAggregateResponse {
    pub aggregates: Vec<AnalyticsAggregate>,
    pub total: usize,
    pub clicks: i64,
}

/// Get analytics for a specific short code
pub async fn get_analytics(
    State(state): State<Arc<AnalyticsState>>,
    Path(short_code): Path<String>,
    Query(params): Query<AnalyticsQueryParams>,
) -> impl IntoResponse {
    let limit = params.limit.clamp(1, 1000);

    // Get click count first
    let clicks = match state.storage.get_authoritative(&short_code).await {
        Ok(Some(url)) => url.clicks,
        _ => 0,
    };

    match state
        .storage
        .get_analytics(&short_code, params.start_time, params.end_time, limit)
        .await
    {
        Ok(entries) => {
            let total = entries.len();
            Json(AnalyticsResponse {
                entries,
                total,
                clicks,
            })
            .into_response()
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
    State(state): State<Arc<AnalyticsState>>,
    Path(short_code): Path<String>,
    Query(params): Query<AnalyticsQueryParams>,
) -> impl IntoResponse {
    let limit = params.limit.clamp(1, 1000);
    let group_by = params.group_by.as_deref().unwrap_or("country");

    // Get aggregates from database
    let db_aggregates = match state
        .storage
        .get_analytics_aggregate(
            &short_code,
            params.start_time,
            params.end_time,
            group_by,
            limit,
        )
        .await
    {
        Ok(agg) => agg,
        Err(e) => {
            tracing::error!("Failed to get analytics aggregate: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to retrieve analytics aggregate",
            )
                .into_response();
        }
    };

    // If we have an analytics aggregator, get in-memory data for near real-time display
    let combined_aggregates = if let Some(aggregator) = &state.aggregator {
        // Get in-memory aggregates (pending data not yet in DB)
        let in_memory = aggregator.get_in_memory_aggregate(&short_code, group_by);

        // Combine database and in-memory data
        use std::collections::HashMap;
        let mut combined: HashMap<String, i64> = HashMap::new();

        // Add database aggregates
        for agg in db_aggregates {
            *combined.entry(agg.dimension).or_insert(0) += agg.visit_count;
        }

        // Add in-memory aggregates
        for (dimension, count) in in_memory {
            *combined.entry(dimension).or_insert(0) += count;
        }

        // Convert back to Vec
        let mut result: Vec<AnalyticsAggregate> = combined
            .into_iter()
            .map(|(dimension, visit_count)| AnalyticsAggregate {
                dimension,
                visit_count,
            })
            .collect();

        // Sort by visit_count descending
        result.sort_by(|a, b| b.visit_count.cmp(&a.visit_count));

        // Apply limit
        result.truncate(limit as usize);
        result
    } else {
        // No aggregator, just return database results
        db_aggregates
    };

    // Get click count
    let clicks = match state.storage.get_authoritative(&short_code).await {
        Ok(Some(url)) => url.clicks,
        _ => 0,
    };

    let total = combined_aggregates.len();
    Json(AnalyticsAggregateResponse {
        aggregates: combined_aggregates,
        total,
        clicks,
    })
    .into_response()
}
