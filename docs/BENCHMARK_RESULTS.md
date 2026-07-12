# Interpreting Native Benchmark Results

The Rust-native harness writes one JSON document and one Markdown summary for
each label. The report is intended for comparable runs, not a release gate with
fixed universal throughput targets: hosted runners, PostgreSQL state, kernel
limits, and background load all affect results.

## JSON Schema

The report has `schema_version: 1` and contains origin metadata plus a list of
stages. A stage has its workload, worker count, duration, and a `snapshot`.

```json
{
  "schema_version": 1,
  "label": "postgres",
  "runs": [
    {
      "name": "redirect-hot-1000",
      "workload": "redirect-hot",
      "concurrency": 1000,
      "duration_seconds": 30,
      "snapshot": {
        "total": 0,
        "success": 0,
        "unexpected_status": 0,
        "client_error": 0,
        "server_error": 0,
        "transport_error": 0,
        "error_rate": 0.0,
        "requests_per_second": 0.0,
        "p50_ms": 0.0,
        "p95_ms": 0.0,
        "p99_ms": 0.0,
        "latency_samples": 0
      }
    }
  ]
}
```

The values above are structural examples, not measurements.

## Reading a Stage

- **RPS** is `total / measured wall-clock seconds`.
- **Success** means the status matches the workload contract: redirect traffic
  requires a 3xx response; management traffic requires the appropriate 2xx
  response; create requires 201.
- **Error rate** includes all non-success statuses and transport failures.
  Inspect its typed subcounts before attributing a regression to application
  performance.
- **p50/p95/p99** are percentiles over sampled request latency. Sampling avoids
  unbounded memory and collector contention under high request rates, so they
  are representative rather than a complete latency histogram.

## Comparing Runs

The analytics-enabled report can take `BENCHMARK_COMPARE_BASELINE` and emits a
comparison table for matching stage names. Interpret a regression only when:

1. both reports use the same stage duration and worker cap;
2. database/backend/cache configuration matches;
3. the runner and network mode match;
4. the error rate did not change materially.

Prefer repeated measurements. Investigate changed p99 and error composition
before acting on small RPS changes.

## Diagnostics

The CI artifact also includes service logs. A low-RPS run with low errors may
be runner or kernel limited; a high client/transport-error count points to
connection/descriptor capacity; a high server-error count points to an
application or database failure. The companion flamegraph artifact is the next
step for CPU-path diagnosis.

See [Native Rust Benchmarks](BENCHMARKS.md) for workload definitions.
