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
fn record_analytics(
    short_code: &str,
    headers: &HeaderMap,
    socket_ip: std::net::IpAddr,
    config: &AnalyticsConfig,
    geoip: &GeoIpService,
    aggregator: &AnalyticsAggregator,
) {
    let event = AnalyticsEvent {
        short_code: code.to_string(),
        timestamp: now(),
        client_ip: extract_ip(headers, socket_ip, config),
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

### 2. Actor Pattern for Zero Lock Contention

**Problem**: DashMap with hot keys causes lock contention

**Solution**: Use actor pattern with mpsc channel (similar to ClickCounterActor)

```rust
// 2-Layer Architecture:
// Layer 1: Local HashMap in actor (single-threaded, zero locks)
// Layer 2: Shared DashMap for flush task access

struct AnalyticsActor {
    receiver: mpsc::Receiver<ActorMessage>,
    buffer: HashMap<String, Vec<AnalyticsEvent>>,  // Layer 1
    shared_buffer: Arc<DashMap<...>>,               // Layer 2
    fast_flush_interval: Duration,                  // 100ms default
}

// HOT PATH: Lock-free channel send
pub fn record_event(&self, event: AnalyticsEvent) {
    self.actor_tx.try_send(ActorMessage::RecordEvent(event));
}

// ACTOR: Single-threaded accumulation (no locks!)
ActorMessage::RecordEvent(event) => {
    self.buffer.entry(event.short_code)
        .or_insert_with(Vec::new)
        .push(event);
}

// Fast flush: Layer 1 → Layer 2 every 100ms
_ = fast_flush_ticker.tick() => {
    for (short_code, events) in self.buffer.drain() {
        self.shared_buffer.entry(short_code)
            .and_modify(|existing| existing.extend(events))
            .or_insert(events);
    }
}
```

**Benefits**:
- Zero lock contention even with hot keys (popular URLs)
- mpsc channel is lock-free and highly optimized
- Single-threaded actor eliminates synchronization overhead
- Layer 1 accumulates events with zero locks
- Layer 2 provides concurrent read access for flush task

**Impact**: Eliminates DashMap contention bottleneck on hot keys

### 3. Event Batching

Events are batched in memory and processed in bulk during periodic flushes:

- **Collection**: Events buffered via actor pattern
- **Fast flush**: Layer 1 → Layer 2 every 100ms
- **Slow flush**: Layer 2 → Database every 10 seconds (configurable)
- **Processing**: GeoIP lookups done in batch, aggregated, then flushed to database

This approach:
- Reduces per-request overhead
- Amortizes GeoIP lookup costs
- Enables efficient database batch writes
- Handles burst traffic without blocking requests

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

1. **Event Buffer Size**: Should stay under 100k entries (channel capacity)
2. **Flush Duration**: Should complete in <1 second
3. **GeoIP Lookup Time**: Should be <1ms per IP on average
4. **Database Write Time**: Batch inserts should take <100ms
5. **Actor Channel**: Watch for "Analytics event buffer full" warnings

### Debug Logging

Enable detailed analytics logging:

```bash
RUST_LOG=debug cargo run
```

Look for log lines:
- `Processing X analytics event buffers` - Events being flushed from shared buffer
- `Flushing X analytics aggregates` - Aggregates written to DB
- `Failed to flush analytics` - Errors to investigate
- `Analytics event buffer full` - Channel capacity reached (increase buffer size)

## Performance Testing

### Test Actor Pattern Performance

Use the dedicated analytics performance test script:

```bash
# Test with analytics enabled (actor pattern)
bash tests/analytics_performance_test.sh http://localhost:3000 ./results 15s 1000
```

This script specifically tests:
1. **Single hot URL** - Validates zero contention on popular short codes
2. **Distributed load** - Tests multiple URLs simultaneously
3. **Extreme concurrency** - Stress test with 5000 connections

### Compare With/Without Analytics

```bash
# Test without analytics
ANALYTICS_ENABLED=false cargo run --release &
sleep 5
bash tests/analytics_performance_test.sh

# Restart with analytics
pkill lynx
ANALYTICS_ENABLED=true cargo run --release &
sleep 5
bash tests/analytics_performance_test.sh
```

Expected overhead: <3% throughput reduction

### Hot Key Contention Test

To specifically test the actor pattern's ability to handle hot keys:

```bash
# This hits the same URL repeatedly - tests actor pattern
wrk -t8 -c1000 -d30s http://localhost:3000/popular-url

# Watch for these indicators of good performance:
# - Requests/sec: >50k
# - Latency p99: <100ms
# - No "buffer full" warnings in logs
```

### Benchmark Analytics Impact

```bash
# Create a test URL first
curl -X POST http://localhost:8080/api/urls \
  -H "Content-Type: application/json" \
  -d '{"url": "https://example.com", "custom_code": "test"}'

# Test without analytics
ANALYTICS_ENABLED=false wrk -t8 -c1000 -d15s http://localhost:3000/test

# Test with analytics
ANALYTICS_ENABLED=true wrk -t8 -c1000 -d15s http://localhost:3000/test
```
- `Processing X analytics event buffers` - Events being flushed
- `Flushing X analytics aggregates` - Aggregates written to DB
- `Failed to flush analytics` - Errors to investigate

## Performance Testing

### Benchmark Analytics Impact

```bash
# Create a test URL first
curl -X POST http://localhost:8080/api/urls \
  -H "Content-Type: application/json" \
  -d '{"url": "https://example.com", "custom_code": "test"}'

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
