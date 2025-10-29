# Analytics Performance Optimization Guide

## Overview

This document explains the performance optimizations implemented for the analytics feature to ensure minimal impact on redirect throughput.

## Design Goals

1. **Zero-impact on hot path**: Analytics recording should not block URL redirect responses
2. **Minimal overhead**: Target <3% throughput reduction with analytics enabled
3. **Accurate data**: Ensure all visitor events are captured and properly aggregated
4. **Scalability**: Handle 200k+ requests per second under peak load

## Key Optimizations

### 1. Deferred GeoIP Lookups

**Problem**: GeoIP database lookups are CPU-intensive (MMDB file parsing, tree traversal)

**Solution**: Defer GeoIP lookups to background flush task

```rust
// HOT PATH: Record lightweight event (fast!)
fn record_analytics(...) {
    let event = AnalyticsEvent {
        short_code: code.to_string(),
        timestamp: now(),
        client_ip: extract_ip(...), // Fast IP extraction only
    };
    aggregator.record_event(event); // Non-blocking
}

// BACKGROUND TASK: Process events with GeoIP (off hot path)
async fn flush_task() {
    let events = aggregator.drain_events();
    for event in events {
        let geo = geoip.lookup(event.client_ip); // Expensive operation here
        aggregate_with_geo(event, geo);
    }
}
```

**Impact**: Reduced analytics overhead from 5.6% to 2.9%

### 2. Event Batching

Events are batched in memory and processed in bulk during periodic flushes:

- **Collection**: Events buffered in DashMap by short_code
- **Flush interval**: Configurable (default: 10 seconds)
- **Processing**: GeoIP lookups done in batch, aggregated, then flushed to database

This approach:
- Reduces per-request overhead
- Amortizes GeoIP lookup costs
- Enables efficient database batch writes

### 3. Lock-Free Data Structures

Analytics uses DashMap for concurrent access without locks:

```rust
pub struct AnalyticsAggregator {
    // Lock-free concurrent hash map
    event_buffer: Arc<DashMap<String, Vec<AnalyticsEvent>>>,
    aggregates: Arc<DashMap<AnalyticsKey, AnalyticsValue>>,
}
```

**Benefits**:
- No lock contention under high concurrency
- Multiple threads can record events simultaneously
- Scales linearly with CPU cores

### 4. Minimal Allocations

Event recording minimizes heap allocations:

```rust
pub struct AnalyticsEvent {
    pub short_code: String,     // Reuses from request
    pub timestamp: i64,          // Stack-allocated
    pub client_ip: IpAddr,       // Stack-allocated (16 bytes max)
}
```

Only one String allocation per event vs. multiple for full GeoLocation data.

## Configuration

### Environment Variables

```bash
# Enable analytics
ANALYTICS_ENABLED=true

# GeoIP database paths
ANALYTICS_GEOIP_CITY_DB_PATH=/path/to/GeoLite2-City.mmdb
ANALYTICS_GEOIP_ASN_DB_PATH=/path/to/GeoLite2-ASN.mmdb

# Privacy settings
ANALYTICS_IP_ANONYMIZATION=true

# Performance tuning
ANALYTICS_FLUSH_INTERVAL_SECS=10  # How often to process events
```

### Performance vs. Accuracy Trade-offs

**Flush Interval**:
- Lower (5s): More real-time data, slightly higher overhead
- Higher (60s): Less overhead, data delayed up to 1 minute
- Recommended: 10-30 seconds for production

**IP Anonymization**:
- Enabled: Better privacy, slightly faster (no IP storage)
- Disabled: Full IP tracking, marginally slower
- Recommended: Enable for GDPR compliance

## Monitoring

### Key Metrics

Monitor these to ensure analytics performance:

1. **Event Buffer Size**: Should stay under 10k entries between flushes
2. **Flush Duration**: Should complete in <1 second
3. **GeoIP Lookup Time**: Should be <1ms per IP on average
4. **Database Write Time**: Batch inserts should take <100ms

### Debug Logging

Enable detailed analytics logging:

```bash
RUST_LOG=debug cargo run
```

Look for log lines:
- `Processing X analytics event buffers` - Events being flushed
- `Flushing X analytics aggregates` - Aggregates written to DB
- `Failed to flush analytics` - Errors to investigate

## Performance Testing

### Benchmark Analytics Impact

```bash
# Test without analytics
ANALYTICS_ENABLED=false wrk -t8 -c1000 -d15s http://localhost:3000/test

# Test with analytics
ANALYTICS_ENABLED=true wrk -t8 -c1000 -d15s http://localhost:3000/test
```

Expected overhead: <3% throughput reduction

### Verify Data Accuracy

After benchmark, check database:

```sql
SELECT short_code, SUM(visit_count) as total 
FROM analytics 
GROUP BY short_code;
```

Total should match number of successful requests from benchmark.

## Troubleshooting

### High Memory Usage

**Symptom**: Memory grows during high traffic

**Cause**: Events accumulating faster than flush can process

**Solutions**:
1. Reduce flush interval (e.g., 5 seconds)
2. Increase database write performance
3. Add more application instances

### Missing Analytics Data

**Symptom**: Visit counts lower than expected

**Cause**: Events dropped due to buffer overflow or crashes

**Solutions**:
1. Check logs for flush errors
2. Increase event buffer capacity (requires code change)
3. Ensure flush task is running (check for shutdown logs)

### GeoIP Lookup Errors

**Symptom**: No geographic data in analytics

**Cause**: GeoIP database files missing or corrupted

**Solutions**:
1. Verify database file paths are correct
2. Check file permissions (readable by application)
3. Download fresh GeoLite2 databases from MaxMind

## Best Practices

1. **Always test with analytics enabled** during performance benchmarks
2. **Monitor flush task latency** to detect bottlenecks early
3. **Use PostgreSQL** for production (better concurrent write performance than SQLite)
4. **Enable IP anonymization** unless full IP tracking is required
5. **Set appropriate flush interval** based on your data freshness requirements

## Advanced: Custom Aggregation

For custom analytics needs, you can modify the aggregation key:

```rust
pub struct AnalyticsKey {
    pub short_code: String,
    pub time_bucket: i64,        // Hour-level granularity
    pub country_code: Option<String>,
    pub region: Option<String>,  // Add/remove fields as needed
    pub city: Option<String>,
    // ... custom dimensions
}
```

Lower granularity (e.g., daily instead of hourly) = better performance, less detailed data.

## References

- [Main Performance Optimizations](PERFORMANCE_OPTIMIZATIONS.md)
- [Benchmark Results](PERFORMANCE_OPTIMIZATION_RESULTS.md)
- [Analytics Guide](ANALYTICS.md)
