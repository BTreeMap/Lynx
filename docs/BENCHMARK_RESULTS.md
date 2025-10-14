# Interpreting Benchmark Results

This guide helps you understand and analyze the performance benchmark results for Lynx.

## Reading wrk Output

The benchmark suite uses `wrk` as the primary benchmarking tool. Here's how to interpret its output:

### Example Output

```
Running 30s test @ http://localhost:3000/bench-1
  8 threads and 1000 connections
  Thread Stats   Avg      Stdev     Max   +/- Stdev
    Latency     2.15ms    1.34ms  45.67ms   89.23%
    Req/Sec     9.12k     1.23k   12.45k    76.89%
  2187654 requests in 30.10s, 456.78MB read
Requests/sec:  72685.43
Transfer/sec:     15.18MB
```

### Key Metrics Explained

#### Requests/sec (RPS)
- **What it is**: Total throughput of the service
- **Example**: `72685.43` requests per second
- **Target for redirects**: ~70,000 RPS @ 1000 concurrency
- **Higher is better**

#### Latency Stats
- **Average (Avg)**: Mean response time
  - Example: `2.15ms` average latency
  - For cached redirects: Should be < 5ms
  
- **Standard Deviation (Stdev)**: Variation in response times
  - Example: `1.34ms` variation
  - Lower is better (more consistent)
  
- **Max**: Worst-case latency observed
  - Example: `45.67ms` maximum latency
  - Outliers are normal but should be rare
  
- **+/- Stdev**: Percentage of requests within 1 standard deviation
  - Example: `89.23%` of requests within Avg ± Stdev
  - Higher is better (more predictable)

#### Thread Stats
- **Req/Sec**: Requests per second per thread
  - Example: `9.12k` average per thread
  - With 8 threads: 9.12k × 8 ≈ 72k total RPS
  
- **Stdev**: Variation between threads
  - Example: `1.23k` variation
  - Low variation means good load distribution

#### Transfer Rate
- **Transfer/sec**: Data throughput
  - Example: `15.18MB/sec`
  - Depends on response size
  - Not a primary metric for Lynx (redirects are small)

### Latency Percentiles

Some wrk versions show percentile latencies:

```
Latency Distribution
  50%    1.87ms
  75%    2.43ms
  90%    3.21ms
  99%    6.78ms
```

- **p50 (median)**: Half of all requests were faster than this
  - Target: < 2ms for cached redirects
  
- **p75**: 75% of requests were faster than this
  - Target: < 5ms for cached redirects
  
- **p90**: 90% of requests were faster than this
  - Target: < 10ms for cached redirects
  
- **p99**: 99% of requests were faster than this
  - Target: < 50ms for cached redirects
  - Most important for user experience

**Rule of thumb**: Focus on p99 latency. It represents the worst experience most users will have.

## Comparing Results

### Redirect Endpoint Performance

Expected performance for the redirect endpoint:

| Scenario | Expected RPS | Expected p99 |
|----------|--------------|--------------|
| Hot URL @ 1k concurrency | ~70,000 | < 10ms |
| Hot URL @ 5k concurrency | ~60,000 | < 20ms |
| Distributed (100 URLs) | ~50,000 | < 50ms |
| Extreme (10k connections) | Stable* | < 100ms |

*Stable means no crashes, errors < 1%

### Management Endpoint Performance

Expected performance for management endpoints:

| Endpoint | Expected RPS | Expected p99 |
|----------|--------------|--------------|
| POST /api/urls | 1,000-5,000 | < 100ms |
| GET /api/urls/:code | 5,000-20,000 | < 50ms |
| GET /api/urls (list) | 500-2,000 | < 200ms |
| PUT deactivate/reactivate | 1,000-3,000 | < 100ms |
| GET /api/health | 50,000+ | < 5ms |

## Performance Indicators

### Good Performance ✅

- **High RPS**: Close to or exceeding targets
- **Low latency**: p99 < 50ms for redirects
- **Consistent**: Low standard deviation
- **No errors**: 0% error rate
- **Linear scaling**: Performance scales with threads

### Signs of Issues ⚠️

- **Low RPS**: < 50% of target
  - Check: Database connection pool size
  - Check: Userland proxy (should be bypassed)
  - Check: System resources (CPU, memory)

- **High latency**: p99 > 100ms for redirects
  - Check: Cache hit rate (should be > 90%)
  - Check: Database performance
  - Check: Network latency

- **High variation**: Large standard deviation
  - Check: Database query performance
  - Check: GC pauses (check logs)
  - Check: System noise (other processes)

- **Errors**: Any error rate > 0.1%
  - Check: Service logs for details
  - Check: Connection limits
  - Check: Database connection pool

### Red Flags 🚨

- **Crash**: Service becomes unresponsive
  - Critical issue - investigate logs immediately
  
- **Degradation**: Performance drops over time
  - Possible memory leak or connection leak
  - Check resource usage trends
  
- **High errors**: > 1% error rate
  - Service is overloaded or misconfigured
  - Reduce concurrency and investigate

## Analyzing JSON Results

The benchmark generates a JSON file with structured results:

```json
{
  "timestamp": "2024-01-15T10:30:00Z",
  "api_url": "http://localhost:8080",
  "redirect_url": "http://localhost:3000",
  "tests": [
    {
      "name": "Single hot URL @ 1000 concurrency",
      "requests_per_second": "72685.43",
      "avg_latency_ms": "2.15",
      "p50_latency_ms": "1.87",
      "p90_latency_ms": "3.21",
      "p99_latency_ms": "6.78",
      "errors": "0"
    }
  ]
}
```

### Visualizing Results

You can use various tools to visualize the JSON data:

#### Automated Visualization Script

The repository includes a Python script for automatic graph generation:

```bash
# Install dependencies
pip3 install matplotlib numpy

# Generate visualizations
python3 tests/visualize_benchmarks.py benchmark-results.json -o ./graphs
```

This generates:
- `rps_comparison.png` - Bar chart of throughput across all tests
- `latency_percentiles.png` - Grouped bar chart of p50/p90/p99 latencies
- `performance_heatmap.png` - Normalized heatmap of all metrics
- `summary.txt` - Text summary with top performers

#### Python (matplotlib)

```python
import json
import matplotlib.pyplot as plt

with open('benchmark-results.json') as f:
    data = json.load(f)

tests = [t['name'] for t in data['tests']]
rps = [float(t['requests_per_second']) for t in data['tests']]

plt.bar(tests, rps)
plt.xlabel('Test Scenario')
plt.ylabel('Requests per Second')
plt.title('Lynx Performance Benchmarks')
plt.xticks(rotation=45)
plt.tight_layout()
plt.savefig('benchmark-rps.png')
```

#### Gnuplot

```bash
# Extract data
jq -r '.tests[] | "\(.name)\t\(.requests_per_second)"' results.json > data.tsv

# Create graph
gnuplot << EOF
set terminal png size 1200,800
set output 'benchmark.png'
set title 'Lynx Performance Benchmarks'
set xlabel 'Test Scenario'
set ylabel 'Requests per Second'
set style data histogram
set style fill solid
plot 'data.tsv' using 2:xtic(1) title 'RPS'
EOF
```

## Common Scenarios

### Scenario 1: Lower than Expected RPS

**Observation**: Getting 40k RPS instead of expected 70k

**Possible Causes**:
1. Userland proxy not bypassed (use `--network host`)
2. Database connection pool too small
3. Cache not warming up (run longer)
4. System resource limits (file descriptors, CPU)

**Investigation**:
```bash
# Check if using host network
docker inspect lynx | grep NetworkMode

# Check connection pool in logs
docker logs lynx | grep "max.*connections"

# Check file descriptor limit
ulimit -n
```

### Scenario 2: High p99 Latency

**Observation**: p99 latency is 200ms (expected < 50ms)

**Possible Causes**:
1. Cache misses (cold cache or distributed load)
2. Database slow queries
3. Connection pool exhausted
4. GC pauses

**Investigation**:
```bash
# Check cache configuration
docker logs lynx | grep cache

# Check database query times
# (PostgreSQL: pg_stat_statements)

# Check GC activity
docker logs lynx | grep -i "gc\|garbage"
```

### Scenario 3: Inconsistent Results

**Observation**: RPS varies significantly between runs

**Possible Causes**:
1. Cold cache vs warm cache
2. Background processes interfering
3. Database autovacuum running
4. Network instability

**Investigation**:
- Run benchmark multiple times, discard first run (cache warmup)
- Check system load during benchmark
- Use longer duration (60s instead of 30s)
- Monitor database activity

## Best Practices

### Running Benchmarks

1. **Warmup**: Run a short test first to warm up caches
2. **Duration**: Use at least 30s, preferably 60s for stability
3. **Isolation**: Close other applications, disable background tasks
4. **Consistency**: Run multiple times and average results
5. **Documentation**: Record system specs and configuration

### Comparing Results

1. **Same hardware**: Always compare on same hardware
2. **Same configuration**: Keep database, cache settings constant
3. **Baseline**: Establish baseline before making changes
4. **Small changes**: Test one change at a time
5. **Statistical significance**: Look for > 10% differences

### Reporting Issues

When reporting performance issues, include:

1. Full wrk output
2. System specifications (CPU, RAM, OS)
3. Configuration (cache size, connection pool, etc.)
4. Service logs (especially startup configuration)
5. Database type and version
6. Network mode (host vs bridge)

## Related Documentation

- [BENCHMARKS.md](./BENCHMARKS.md) - Full benchmark suite documentation
- [PERFORMANCE_OPTIMIZATIONS.md](./PERFORMANCE_OPTIMIZATIONS.md) - Optimization details
- [tests/README.md](../tests/README.md) - All test documentation

---

*For questions about interpreting results, open a GitHub issue with your benchmark output.*
