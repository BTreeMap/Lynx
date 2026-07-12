# Rust Performance Profiling with Flamegraphs

Lynx generates CPU flamegraphs with a Rust-native, in-process profiling
harness. Flamegraphs visualize sampled call stacks: frame width represents the
relative CPU time in a function and its callees, while height represents stack
depth.

## Architecture

The ignored integration test in `tests/performance_harness.rs` starts the real
API and redirect Axum routers on ephemeral loopback ports, generates load with
Tokio and Reqwest, and samples the entire process with `pprof`. PostgreSQL
remains external so database behavior is production-representative.

The `profiling` Cargo feature is the compile-time capability boundary. It
enables the optional `pprof` dependency only for explicit profiling builds.
Normal tests, release binaries, and Docker images neither compile nor link the
profiler.

No `perf`, `cargo-flamegraph`, `wrk`, Lua, Bash lifecycle script, root
permissions, or kernel setting changes are required.

## Profiling Configuration

The dedicated Cargo profile inherits release optimizations while retaining
enough symbols for useful stacks:

```toml
[profile.profiling]
inherits = "release"
debug = 1
strip = "none"
```

The normal release profile is unchanged. CI sets
`RUSTFLAGS=-C force-frame-pointers=yes` only while compiling the profiling
harness so sampled stacks remain reliable.

Configuration uses positive, typed values. Zero, malformed, or overflowing
values fail before either server starts:

| Variable | Default | Meaning |
|---|---:|---|
| `PERF_FLAMEGRAPH_OUTPUT_DIR` | `target/flamegraphs` | Artifact directory |
| `PERF_FLAMEGRAPH_FREQUENCY_HZ` | `99` | Samples per second |
| `PERF_FLAMEGRAPH_DURATION` | `15s` | Duration per scenario (`s` or `m`) |
| `PERF_FLAMEGRAPH_REDIRECT_CONCURRENCY` | `256` | Cached redirect workers |
| `PERF_FLAMEGRAPH_API_CONCURRENCY` | `64` | Mixed API workers |

## Running Locally

Build the embedded frontend, provide PostgreSQL, and run the ignored harness:

```bash
cd frontend && npm ci && npm run build && cd ..
export DATABASE_URL=postgresql://lynx:lynx_password@localhost:5432/lynx
RUSTFLAGS="-C force-frame-pointers=yes" \
PERF_FLAMEGRAPH_DURATION=30s \
cargo test --profile profiling --features profiling \
  --test performance_harness representative_hot_path_flamegraphs \
  -- --ignored --nocapture
```

The exhaustive `ProfileScenario` enum runs both representative workloads:

- `redirect-cached`: deadline-driven workers repeatedly read one warmed short
  code, exercising routing, Moka lookup, response construction, and the
  click-counter actor.
- `api-mixed`: deadline-driven workers alternate URL creation and detail reads,
  exercising validation, SQLx, PostgreSQL pooling, and serialization.

The harness writes these artifacts:

- `target/flamegraphs/flamegraph-redirect-cached.svg`
- `target/flamegraphs/flamegraph-api-operations.svg`
- `target/flamegraphs/README.md`

Because servers and load generators deliberately share one process, graphs
contain both Lynx hot-path and harness client frames. Search for Lynx handler,
storage, cache, SQLx, and Tokio frames when investigating server behavior.

## Interpreting Flamegraphs

Open an SVG in a browser to zoom and search. Prioritize:

1. Wide plateaus, which identify CPU-heavy call paths.
2. Allocator frames, which may reveal avoidable allocation or copying.
3. Scheduler and synchronization frames, which may reveal contention.
4. SQLx and PostgreSQL frames in database-backed paths.
5. Moka and response-construction frames in the cached redirect path.

Expected high-level paths include:

```text
tokio runtime → axum redirect handler → moka cache → response
tokio runtime → axum API handler → sqlx → PostgreSQL → serialization
```

Flamegraphs measure CPU samples, not wall-clock latency. Pair them with the
throughput and latency benchmark artifacts before drawing conclusions about
network or database waits.

## Continuous Profiling

The `CPU Flamegraphs (PostgreSQL)` job in the Performance Benchmarks workflow
runs after a successful Docker publish and on manual dispatch. It is separate
from throughput and analytics jobs and profiles the triggering commit.

The job:

1. Runs the ignored integration test with release optimizations, the explicit
   `profiling` feature, debug symbols, and frame pointers.
2. Starts both Axum servers and generates both workloads in-process against a
   PostgreSQL 17 service at 99 Hz.
3. Fails on setup, request, sampling, SVG generation, or validation errors.
4. Uploads the two interactive SVGs and the generated interpretation guide in
   a commit-addressed artifact retained for 90 days.
5. Reports workload metadata, graph sizes, and the artifact link in the GitHub
   Actions job summary.

## Troubleshooting

### PostgreSQL connection failure

Confirm that `DATABASE_URL` points to a reachable, empty test database. The
harness initializes Lynx's schema before seeding deterministic fixtures.

### Missing or unresolved stack frames

Compile the dedicated profile with its feature and frame pointers:

```bash
RUSTFLAGS="-C force-frame-pointers=yes" \
cargo test --profile profiling --features profiling \
  --test performance_harness --no-run
```

### Sparse graph

Increase `PERF_FLAMEGRAPH_DURATION` to collect more samples. If profiling
overhead affects results, reduce `PERF_FLAMEGRAPH_FREQUENCY_HZ`; the default
99 Hz balances resolution and perturbation.

## Best Practices

1. Profile representative load rather than startup.
2. Compare the same scenario and configuration before and after a change.
3. Optimize wide application frames, not incidental narrow stacks.
4. Confirm suspected improvements with throughput and latency benchmarks.
5. Keep profiling opt-in so instrumentation never reaches release artifacts.

## Additional Resources

- [pprof-rs](https://github.com/tikv/pprof-rs)
- [Flame Graphs](https://www.brendangregg.com/flamegraphs.html)
- [Rust Performance Book](https://nnethercote.github.io/perf-book/)

## Related Documentation

- [Performance Optimizations](PERFORMANCE_OPTIMIZATIONS.md)
- [Benchmarks](BENCHMARKS.md)
- [Benchmark Results](BENCHMARK_RESULTS.md)
