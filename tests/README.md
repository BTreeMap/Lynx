# Lynx Test Harnesses

Lynx uses typed Rust test targets for component, black-box, lifecycle, and
performance verification. Shell is deliberately limited to environment setup
such as starting a Docker image or checking documentation drift; it does not
parse HTTP responses, generate traffic, or encode test behavior.

## Test Layers

| Target | Scope | Normal invocation |
|---|---|---|
| Existing `tests/*_integration.rs` targets | Storage, routing, cache, analytics, and API-component behavior | `cargo test --tests` |
| `external_harness` | Real HTTP API/redirect behavior, concurrent operations, and exact restart durability | Ignored; requires a running service |
| `benchmark_harness` | Deadline-bound native Rust traffic, latency sampling, and JSON/Markdown reports | Ignored; requires a running service |
| `performance_harness` | In-process CPU-flamegraph capture | Ignored; run in profiling CI |

## External HTTP and Lifecycle Harness

The external harness uses a non-following `reqwest` client, strongly typed JSON
models, base64url route encoding, bounded eventual assertions, and native
Tokio concurrency. It replaces the former curl/grep/sleep scripts.

It verifies, without tolerances for isolated URLs:

- health, auth mode, URL CRUD, cursor pagination, search, history, and restore;
- redirect status and exact `Location` header;
- exact click totals after concurrent redirects;
- all concurrent creates succeed for distinct codes;
- mixed reads/writes/redirects, state changes under traffic, and concurrent lists;
- SIGTERM and SIGINT process shutdown: exit success, `Shutdown complete`, restart,
  and exact persisted click totals.

### Run against a locally started service

`AUTH_MODE=none` is test-only. Start Lynx or a Docker image with test settings,
then run:

```text
LYNX_E2E_CONTAINER=lynx \
LYNX_E2E_CONCURRENCY=100 \
cargo test --test external_harness -- --ignored --test-threads=1 --nocapture
```

Configuration is typed and validated before test execution:

| Variable | Default | Meaning |
|---|---:|---|
| `LYNX_E2E_API_URL` | `http://127.0.0.1:8080` | API origin |
| `LYNX_E2E_REDIRECT_URL` | `http://127.0.0.1:3000` | Redirect origin |
| `LYNX_E2E_CONTAINER` | none | Required for SIGTERM/SIGINT lifecycle verification |
| `LYNX_E2E_CONCURRENCY` | `100` | Concurrent functional-load workers; must be non-zero |
| `LYNX_E2E_REQUEST_TIMEOUT_SECS` | `15` | Per-request timeout |
| `LYNX_E2E_READINESS_TIMEOUT_SECS` | `45` | Bounded health-check wait |
| `LYNX_E2E_EXPECT_ANALYTICS` | `false` | Additionally require exact persisted analytics for the isolated stats URL |

## Native Traffic Benchmarks

`benchmark_harness` replaces `wrk`, Lua scripts, Apache Bench fallback, and
shell report parsing. It uses deadline-bound Tokio workers with a shared
`reqwest` connection pool. Request counters use relaxed atomics; latency is
sampled every 256 requests to keep measurement overhead bounded. Workers
self-stop at the deadline and a bounded join aborts stuck stages.

The standard suite covers hot and distributed redirects, URL creation/read/list,
deactivation, health, and the previously skipped 80/15/5 mixed workload. The
analytics suite covers hot, distributed, hotspot, power-law, sustained, and
analytics-API traffic. Every run emits a typed JSON report and a Markdown
summary in the chosen output directory.

```text
BENCHMARK_SUITE=standard \
BENCHMARK_LABEL=local \
BENCHMARK_OUTPUT_DIR=benchmark-results \
BENCHMARK_DURATION_SECS=30 \
BENCHMARK_MAX_CONCURRENCY=10000 \
cargo test --profile profiling --locked --test benchmark_harness \
  native_external_benchmark -- --ignored --nocapture
```

Use `BENCHMARK_SUITE=analytics` for analytics traffic. A second run can compare
itself to a baseline report without shell parsing:

```text
BENCHMARK_SUITE=analytics \
BENCHMARK_LABEL=analytics \
BENCHMARK_COMPARE_BASELINE=benchmark-results/native-benchmark-baseline.json \
cargo test --profile profiling --locked --test benchmark_harness \
  native_external_benchmark -- --ignored --nocapture
```

## CI

- The PR quality gate builds a Docker image, then runs `external_harness` on
  SQLite and PostgreSQL 18.
- The published-image integration workflow runs the same suites against the
  commit-addressed GHCR image.
- The performance workflow runs the Rust-native standard and analytics suites
  and retains the independent Rust flamegraph harness.

No browser/Playwright tests are required by these harnesses.
