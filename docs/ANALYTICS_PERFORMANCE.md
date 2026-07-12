# Analytics Performance

Analytics must not make successful redirects wait for geolocation, aggregation,
or database persistence. Lynx records a lightweight `AnalyticsEvent` with a
shared `Arc<str>` short code, timestamp, and client address, then transfers it
to a bounded actor ingress with `try_send`.

## Concurrency and Durability Design

- **Normal path:** `try_send` transfers the event to the analytics actor with no
  await and no per-request task creation.
- **Saturated ingress:** the event is preserved in a sharded `DashMap` overflow
  buffer instead of being dropped.
- **Actor-local aggregation:** the actor owns its first-layer `HashMap`, so hot
  short codes do not contend on a global request lock.
- **Batch persistence:** periodic flush tasks aggregate events and use the
  storage batch API. PostgreSQL uses a set-based upsert; SQLite uses a
  transaction.
- **Shutdown:** stateful `watch` notification lets flush tasks observe shutdown
  even when they subscribe late. The main runtime awaits analytics actor and
  flush completion before cached click storage shutdown.

The isolated component suites exercise saturated event handling and exact
shutdown durability. The external suite can additionally require persisted
analytics for an isolated URL with `LYNX_E2E_EXPECT_ANALYTICS=true`.

## Benchmarking Analytics Impact

The `analytics` mode of the native Rust benchmark harness replaces the former
external traffic generator. It measures:

- hot redirects at 1,000, 5,000, and 10,000 workers;
- 100- and 500-URL distributed traffic;
- 80/20 hotspot and 70/30 power-law distributions;
- sustained distributed traffic;
- the protected analytics query endpoint.

Run a baseline and an analytics-enabled service with equivalent PostgreSQL 18,
cache, actor, and network configuration. The CI workflow performs this sequence
and writes a comparison report directly from typed JSON data.

```text
BENCHMARK_SUITE=analytics \
BENCHMARK_LABEL=analytics \
BENCHMARK_OUTPUT_DIR=benchmark-results-analytics \
BENCHMARK_DURATION_SECS=30 \
BENCHMARK_MAX_CONCURRENCY=10000 \
cargo test --test benchmark_harness native_external_benchmark -- --ignored --nocapture
```

To compare with a baseline artifact, pass its path through
`BENCHMARK_COMPARE_BASELINE`. See [Native Rust Benchmarks](BENCHMARKS.md) for
all configuration and [Benchmark Results](BENCHMARK_RESULTS.md) for
interpretation.

## Operational Signals

Monitor event-buffer pressure, flush duration, database batch failure/retry
logs, memory usage, and the difference between successful redirect totals and
persisted analytics totals during controlled shutdown tests. A benchmark error
rate is as important as RPS: a faster generator that loses requests is not a
valid result.
