# Test and Benchmark Harness Summary

Lynx uses Rust-native, type-checked test harnesses for external HTTP behavior,
container lifecycle verification, and performance traffic generation.

## External Harness

`tests/external_harness.rs` is an ignored black-box integration target. Its
reusable modules validate origins and environment configuration before sending
requests. It owns typed JSON decoding, base64url path encoding, readiness
retries, concurrent request groups, exact click assertions, and Docker signal
control.

The harness covers functional URL management, redirect contract, pagination,
search, history/restore, concurrent creates, concurrent click accounting,
mixed operations, state changes under traffic, and exact restart durability for
both SIGTERM and SIGINT. CI runs it against SQLite and PostgreSQL 18 with
analytics enabled, so graceful shutdown proves exact click and analytics
persistence after restart.

## Native Benchmark Harness

`tests/benchmark_harness.rs` replaces the retired external load generator and
its Lua/shell reporting. Deadline-bound Tokio workers generate typed requests
through a non-following `reqwest` client. Relaxed atomic counters and sparse
latency sampling avoid measurement contention; bounded joins prevent a stuck
request from hanging the job.

The standard suite measures hot/distributed redirects, management endpoints,
and a real mixed workload. The analytics suite measures hot, distributed,
hotspot, power-law, sustained, and analytics-query traffic. Every run emits
versioned JSON plus Markdown; analytics-enabled reports compare themselves with
typed baseline JSON without shell parsing.

## CI and Documentation

The PR and published-image workflows use the external harness. The performance
workflow uses the native benchmark target and retains the separate Rust
flamegraph harness. Current commands, configuration, and report interpretation
are documented in [tests/README.md](tests/README.md),
[docs/BENCHMARKS.md](docs/BENCHMARKS.md), and
[docs/BENCHMARK_RESULTS.md](docs/BENCHMARK_RESULTS.md).
