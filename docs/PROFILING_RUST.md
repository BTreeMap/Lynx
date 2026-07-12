# Rust Performance Profiling with Flamegraphs

This document explains how to profile Lynx performance using flamegraphs for detailed timing breakdowns and CPU profiling.

## What are Flamegraphs?

Flamegraphs are visual representations of profiled software, showing which code paths consume the most CPU time. They help identify performance bottlenecks by displaying:

- **Width**: Represents the total time spent in a function (including its callees)
- **Height**: Represents the call stack depth
- **Color**: Usually randomized for visual separation, but can encode other information

## Tools Used

### cargo-flamegraph

`cargo-flamegraph` is a Rust tool that simplifies the process of generating flamegraphs from Rust programs. It handles:

- Building the binary with appropriate symbols
- Running the profiler (perf on Linux, dtrace on macOS, others on different platforms)
- Collecting stack traces
- Generating SVG flamegraph visualizations

### perf (Linux)

On Linux, `cargo-flamegraph` uses `perf`, a powerful performance analysis tool that:

- Records CPU performance counters
- Captures stack traces at regular intervals
- Provides low-overhead profiling suitable for production-like benchmarks

## Prerequisites

### On Linux (Ubuntu/Debian)

```bash
# Install perf
sudo apt-get update
sudo apt-get install -y linux-tools-common linux-tools-generic linux-tools-$(uname -r)

# Install cargo-flamegraph
cargo install flamegraph

# Allow perf to run without sudo (optional but recommended for CI)
echo 'kernel.perf_event_paranoid = -1' | sudo tee -a /etc/sysctl.conf
sudo sysctl -p
```

### On macOS

```bash
# Install cargo-flamegraph (uses dtrace)
cargo install flamegraph

# Note: May require running with sudo due to dtrace privileges
```

## Profiling Configuration

### Cargo Profile

To get meaningful flamegraphs, the binary needs to include debug symbols while maintaining release-level optimizations. Add this to `Cargo.toml`:

```toml
[profile.profiling]
inherits = "release"
debug = true
```

This creates a `profiling` profile that:
- Inherits all optimizations from `release` profile
- Includes debug symbols for stack trace resolution
- Maintains performance characteristics close to production

The normal release profile is not modified. Release binaries, Docker images, and
downloadable artifacts continue to use `cargo build --release`, so they contain
neither profiling symbols nor profiling tools. CI additionally sets
`RUSTFLAGS=-C force-frame-pointers=yes` only while building the profiling binary
to make sampled stacks reliable.

### Building for Profiling

```bash
cargo build --profile profiling
```

## Running Flamegraph Profiling

The repository provides `scripts/profile-flamegraph.sh` so local and CI runs use
the same lifecycle, deterministic fixtures, and workload definitions. It owns an
isolated process group, waits for health, stops the profiler cleanly, and rejects
missing or malformed SVG output.

Build the frontend and profiling binary first:

```bash
cd frontend && npm ci && npm run build && cd ..
RUSTFLAGS="-C force-frame-pointers=yes" cargo build --profile profiling
```

With PostgreSQL already available and `perf`, `flamegraph`, and `wrk` installed,
run either representative scenario:

```bash
export DATABASE_URL=postgresql://lynx:lynx_password@localhost:5432/lynx

scripts/profile-flamegraph.sh \
	redirect-cached results/flamegraph-redirect-cached.svg 30s

scripts/profile-flamegraph.sh \
	api-mixed results/flamegraph-api-operations.svg 30s
```

The scenarios form a closed set; unknown names and malformed durations fail
before a server starts:

- `redirect-cached`: warms one short code, then drives 4 threads and 256
	connections through the dominant cache-hit and click-counting path.
- `api-mixed`: drives 4 threads and 64 connections with an even deterministic
	mix of URL creation and detail reads, exercising validation, SQLx, PostgreSQL,
	connection pooling, and serialization.

Linux performance counters are commonly unavailable in rootless development
containers. In that environment, build and test the profiling profile locally,
but rely on the hosted CI job to collect samples; do not attempt privileged
workarounds.

## Interpreting Flamegraphs

### Key Areas to Analyze

1. **Wide Plateaus**: Functions that appear wide indicate they consume significant CPU time
2. **Tall Stacks**: Deep call stacks may indicate recursion or complex call chains
3. **System Calls**: Look for time spent in syscalls (network I/O, disk I/O)
4. **Lock Contention**: Search for mutex/lock-related functions
5. **Allocation**: Time spent in allocator functions

### Common Patterns in Lynx

**Efficient Cache Path (Expected)**
```
tokio runtime → axum handler → moka cache lookup → immediate return
```
- Should be shallow and fast
- Minimal time in database-related functions

**Database Path**
```
tokio runtime → axum handler → sqlx query → postgres driver → network I/O
```
- Expected for cache misses
- Should be optimized but inherently slower than cache

**Actor Write Path**
```
tokio runtime → axum handler → actor channel send → background flush
```
- Non-blocking write path
- Most time should be in the background actor, not request handler

### Performance Targets

Based on Lynx's architecture:

- **Redirect (cached)**: Minimal time in handler, most in tokio runtime scheduling
- **Redirect (uncached)**: Significant time in sqlx and database I/O
- **Create URL**: Time split between validation, actor send, and response
- **List URLs**: Time in database query and serialization

## Continuous Profiling in CI/CD

The `CPU Flamegraphs (PostgreSQL)` job in the Performance Benchmarks workflow
runs after a successful Docker publish and on manual workflow dispatch. It is
independent of the throughput and analytics jobs and profiles the exact commit
that triggered the workflow.

The job:

1. Builds `target/profiling/lynx` once with release optimizations, debug symbols,
	and frame pointers.
2. Runs the cached-redirect and mixed-API scenarios against PostgreSQL 17 at a
	499 Hz sampling frequency.
3. Fails if service startup, load generation, profiling, or SVG validation fails.
4. Uploads both interactive SVGs, profiler/service logs, and an interpretation
	README in a commit-addressed `flamegraphs-*` artifact retained for 90 days.
5. Adds workload metadata, graph status and sizes, and a direct artifact link to
	the GitHub Actions job summary.

Open the downloaded SVGs in a browser to zoom and search. The separate
`performance-benchmark-results` artifact remains dedicated to throughput and
latency measurements.

## Troubleshooting

### "Permission denied" errors with perf

```bash
# Temporarily allow perf access
sudo sysctl -w kernel.perf_event_paranoid=-1

# Or permanently in /etc/sysctl.conf
echo 'kernel.perf_event_paranoid = -1' | sudo tee -a /etc/sysctl.conf
```

### Missing stack traces

Ensure the binary is built with debug symbols:
```bash
cargo clean
cargo build --profile profiling
```

### Flamegraph is empty or shows only `[unknown]`

This usually means:
- Binary lacks debug symbols
- Process exited before profiling could capture data
- Insufficient permissions to capture stack traces

### High overhead during profiling

Flamegraph profiling has minimal overhead (~1-5%) but if you notice issues:
- Reduce sampling frequency
- Use `--freq` parameter: `cargo flamegraph --freq 99`
- Profile for longer duration to amortize startup costs

## Best Practices

1. **Profile Under Load**: Always profile with realistic benchmark load
2. **Multiple Scenarios**: Generate separate flamegraphs for different workloads
3. **Compare Over Time**: Keep historical flamegraphs to track performance changes
4. **Focus on Wide Patterns**: Optimization wins come from addressing wide functions
5. **Verify Changes**: Always profile before and after optimization changes

## Additional Resources

- [cargo-flamegraph GitHub](https://github.com/flamegraph-rs/flamegraph)
- [Flamegraph Official Site](http://www.brendangregg.com/flamegraphs.html)
- [Perf Wiki](https://perf.wiki.kernel.org/)
- [Rust Performance Book](https://nnethercote.github.io/perf-book/)

## Related Documentation

- [Performance Optimizations](PERFORMANCE_OPTIMIZATIONS.md) - Architecture and caching strategies
- [Benchmarks](BENCHMARKS.md) - Benchmark methodology and metrics
- [Benchmark Results](BENCHMARK_RESULTS.md) - Interpreting benchmark output
