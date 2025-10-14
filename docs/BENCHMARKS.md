# Performance Benchmarks

This document describes the performance benchmark suite for Lynx URL shortener.

## Overview

The performance benchmark suite is designed to validate the caching optimizations and measure real-world performance characteristics of the Lynx service under various load conditions.

## Benchmark Focus

### Primary Focus: Redirect Endpoint

The redirect endpoint (`GET /:code`) has received the most optimization effort:

- **Moka read cache**: In-memory caching of URL lookups (500k entries, 5-minute TTL)
- **Actor pattern write buffering**: Zero-lock-contention click counting
- **Dual-layer flush system**: 100ms actor flush + 5s database flush
- **Non-blocking writes**: Database operations don't block redirect responses

**Target Performance**: ~70,000 requests/second at 1000 concurrency, with slight degradation at 5000 concurrency.

### Secondary Focus: Management Endpoints

Management API endpoints are expected to have lower performance as they involve database queries:

- `POST /api/urls` - Create short URL (write operation)
- `GET /api/urls/:code` - Get URL details (may benefit from read cache)
- `GET /api/urls` - List URLs (pagination, database query)
- `PUT /api/urls/:code/deactivate` - Deactivate URL (state change + cache invalidation)
- `PUT /api/urls/:code/reactivate` - Reactivate URL (state change + cache invalidation)

## Benchmark Architecture

### Test Scenarios

1. **Single Hot URL (1000 concurrency)**
   - Best case for cache effectiveness
   - Validates cache hit rate and performance
   - Tests actor pattern under concentrated load

2. **Single Hot URL (5000 concurrency)**
   - Higher concurrency test
   - Expected slight performance drop
   - Validates system stability under increased load

3. **Distributed Load (100 URLs)**
   - Tests cache with distributed access patterns
   - Validates cache eviction and refresh logic
   - More realistic traffic pattern

4. **Extreme Load (10000 connections)**
   - Stress test for actor pattern
   - Validates zero-lock-contention design
   - Tests system limits

5. **Management Endpoint Tests**
   - Individual tests for each CRUD operation
   - Validates database query performance
   - Measures cache invalidation overhead

## Tools Used

### wrk - HTTP Benchmarking Tool

**Primary tool** for performance testing:

- High-performance, multi-threaded HTTP benchmarking
- Lua scripting support for complex scenarios
- Detailed latency percentiles (p50, p90, p99, p99.9, p99.99)
- Coordinated omission correction for accurate measurements

**Installation**: Built from source in GitHub Actions

**Capabilities**:
- Can generate 100k+ requests/second
- Supports custom Lua scripts for complex request patterns
- Provides comprehensive latency histograms
- Thread-safe and efficient

### Apache Bench (ab)

**Fallback tool** for simple scenarios:

- Simpler interface
- Good for quick tests
- Limited to single URL per test
- Less accurate for high-performance testing

## Running Benchmarks

### Locally with Docker

```bash
# Start PostgreSQL
docker run -d \
  --name postgres \
  -e POSTGRES_USER=lynx \
  -e POSTGRES_PASSWORD=lynx_pass \
  -e POSTGRES_DB=lynx \
  -p 5432:5432 \
  postgres:16-alpine

# Start Lynx with host network (bypasses userland proxy)
docker run -d \
  --name lynx \
  --network host \
  -e DATABASE_BACKEND=postgres \
  -e DATABASE_URL=postgresql://lynx:lynx_pass@localhost:5432/lynx \
  -e DATABASE_MAX_CONNECTIONS=50 \
  -e AUTH_MODE=none \
  -e API_HOST=0.0.0.0 \
  -e API_PORT=8080 \
  -e REDIRECT_HOST=0.0.0.0 \
  -e REDIRECT_PORT=3000 \
  ghcr.io/btreemap/lynx:latest

# Wait for service to be ready
sleep 10

# Run benchmarks
bash tests/benchmark.sh http://localhost:8080 http://localhost:3000 ./results 30s
```

### In GitHub Actions

The benchmark runs automatically after the Docker image is built and published:

1. Triggered by: `docker-publish` workflow completion
2. Can also be triggered manually via workflow dispatch
3. Runs on: ubuntu-24.04 with GitHub Actions hosted runners
4. Network: Uses `--network host` to bypass Docker userland proxy
5. Duration: Configurable (default 30s per test)

**Manual Trigger**:

```bash
# Via GitHub CLI
gh workflow run performance-benchmark.yml

# With custom duration
gh workflow run performance-benchmark.yml -f duration=60s
```

## Configuration

### Environment Variables

The benchmark uses production-like configuration:

```bash
DATABASE_BACKEND=postgres
DATABASE_MAX_CONNECTIONS=50
CACHE_MAX_ENTRIES=500000
CACHE_FLUSH_INTERVAL_SECS=5
ACTOR_BUFFER_SIZE=1000000
ACTOR_FLUSH_INTERVAL_MS=100
```

### Test Duration

- **Default**: 30 seconds per test
- **Configurable**: Via workflow input or script parameter
- **Recommendation**: 
  - 30s for quick validation
  - 60s for accurate measurements
  - 120s+ for stress testing

## Output and Results

### Generated Files

1. **benchmark-results-TIMESTAMP.txt**
   - Raw wrk output
   - Detailed latency histograms
   - Request rate and throughput
   - Error counts and types

2. **benchmark-results-TIMESTAMP.json**
   - Structured results data
   - Programmatically parseable
   - Suitable for visualization tools
   - Schema documented in GRAPHS.txt

3. **PERFORMANCE_REPORT.md**
   - Human-readable summary
   - Configuration details
   - Key findings
   - Comparison with expected performance

4. **lynx-logs.txt**
   - Service logs during benchmark
   - Cache configuration at startup
   - Any warnings or errors
   - Useful for debugging

### Automated Visualization

The repository includes `tests/visualize_benchmarks.py` for automatic graph generation:

```bash
# Install dependencies (if not already installed)
pip3 install matplotlib numpy

# Generate graphs from JSON results
python3 tests/visualize_benchmarks.py benchmark-results-*.json -o ./graphs
```

This creates:
- RPS comparison bar chart
- Latency percentiles grouped chart
- Performance heatmap with normalized metrics
- Text summary with top performers

See [BENCHMARK_RESULTS.md](./BENCHMARK_RESULTS.md#visualizing-results) for more visualization options.

### Metrics Collected

For each test:

- **Requests per second (RPS)**: Total throughput
- **Average latency**: Mean response time
- **Latency percentiles**:
  - p50 (median)
  - p90 (90th percentile)
  - p99 (99th percentile)
  - p99.9 (99.9th percentile) - when available
  - p99.99 (99.99th percentile) - when available
- **Error count**: Failed requests
- **Transfer rate**: Data throughput (MB/s)

### Artifacts

All results are uploaded as GitHub Actions artifacts:

- **Retention**: 90 days
- **Name**: `performance-benchmark-results`
- **Size**: Typically < 10 MB

## Performance Targets

Based on `docs/PERFORMANCE_OPTIMIZATIONS.md`:

### Redirect Endpoint (Cached)

| Metric | Target | Notes |
|--------|--------|-------|
| RPS @ 1k concurrency | ~70,000 | Single hot URL, best case |
| RPS @ 5k concurrency | ~60,000+ | Expected slight drop |
| RPS @ 10k concurrency | Stable | Should not crash or degrade severely |
| p50 latency | < 1ms | Cached hits are memory operations |
| p99 latency | < 10ms | Should be very low for cached hits |

### Management Endpoints

| Endpoint | Expected RPS | Notes |
|----------|--------------|-------|
| POST /api/urls | 1,000 - 5,000 | Database write + validation |
| GET /api/urls/:code | 5,000 - 20,000 | May benefit from cache |
| GET /api/urls | 500 - 2,000 | Pagination query |
| PUT deactivate/reactivate | 1,000 - 3,000 | State change + cache invalidation |
| GET /api/health | 50,000+ | Simple status check |

*Note: These are estimates. Actual performance depends on hardware, database, and configuration.*

## Performance Considerations

### Userland Proxy

Docker's userland proxy significantly hurts performance. The benchmark **bypasses** it by using `--network host`:

- **With userland proxy**: ~10-20k RPS
- **Without userland proxy**: ~70k+ RPS

This is a 3-5x performance difference for high-throughput scenarios.

### Database Connection Pooling

- **Default**: 50 connections
- **Impact**: Higher pool size can improve concurrent database operations
- **Recommendation**: 
  - SQLite: Keep at 30 or lower (limited concurrent writes)
  - PostgreSQL: Scale to 50-100 based on load

### Cache Configuration

- **Max entries**: 500,000 (approximately 100 MB)
- **TTL**: 5 minutes
- **Eviction**: LRU (Least Recently Used)
- **Thread-safety**: Lock-free concurrent access

### Actor Buffer

- **Size**: 1,000,000 messages
- **Flush interval**: 100ms (Layer 1 → Layer 2)
- **Backpressure**: Blocking channel prevents message loss
- **Performance**: ~500k increments/second with minimal overhead

## Troubleshooting

### Low RPS Numbers

1. **Check userland proxy**: Use `--network host` to bypass
2. **Database connections**: Increase pool size if seeing connection timeouts
3. **System resources**: Ensure sufficient CPU and memory
4. **File descriptors**: Check `ulimit -n` (should be > 100,000)

### High Latency

1. **Cold cache**: First requests will be slower (cache miss)
2. **Database slow**: Check PostgreSQL performance and indexes
3. **Network**: Ensure low-latency connection to database
4. **Buffer flushing**: Check if database flush is blocking (shouldn't happen)

### Errors in Results

1. **Connection refused**: Service not ready, increase wait time
2. **Timeout**: Increase test duration or reduce concurrency
3. **502/503 errors**: Service overloaded, check logs
4. **Database errors**: Check connection string and credentials

## Continuous Monitoring

### Baseline Establishment

After running benchmarks several times, establish baseline metrics:

1. Record p50, p90, p99 latencies for each test
2. Document RPS for redirect endpoint at different concurrency levels
3. Note any anomalies or variations
4. Update performance targets based on actual results

### Regression Detection

Monitor for performance regressions:

1. Compare benchmark results across commits
2. Alert if RPS drops > 10% for redirect endpoint
3. Alert if p99 latency increases > 20%
4. Investigate any new errors or timeouts

### Performance Tracking

Track trends over time:

1. Plot RPS over commits/releases
2. Monitor latency percentiles
3. Track cache hit rates (from logs)
4. Analyze database query patterns

## Related Documentation

- [Interpreting Benchmark Results](./BENCHMARK_RESULTS.md) - How to read and analyze benchmark output
- [Performance Optimizations](./PERFORMANCE_OPTIMIZATIONS.md) - Detailed explanation of caching and optimization strategies
- [Integration Tests](../tests/README.md) - Functional correctness tests
- [Concurrent Tests](../tests/concurrent_test.sh) - Concurrency and data consistency tests

## Contributing

When modifying the benchmark suite:

1. **Add new tests** for new endpoints or features
2. **Update targets** if optimizations improve performance
3. **Document changes** in this file and commit messages
4. **Validate locally** before pushing to CI
5. **Consider test duration** - balance thoroughness with CI time

---

*For questions or issues with the benchmark suite, please open a GitHub issue.*
