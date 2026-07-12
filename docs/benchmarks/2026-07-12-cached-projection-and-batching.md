# Cached Redirect Projection and Batching Microbenchmark

This snapshot accompanies the cache-projection, shared analytics-code, and
batched-persistence changes. Database batching itself is validated by backend
integration and PostgreSQL CI rather than local SQLite timing.

## Environment

- Date: 2026-07-12
- Base commit: `9866edc` plus working-tree changes
- Command: `cargo bench --bench redirect_hot_path`
- Rust/Cargo: 1.97.0
- OS/architecture: Linux aarch64
- CPU: 4 vCPU, Arm Neoverse-N1
- Divan timer precision: 40 ns

## Complete observed result

| Case | Fastest | Slowest | Median | Mean | Samples | Iterations |
|---|---:|---:|---:|---:|---:|---:|
| `analytics_short_code_arc_clone` | 12.39 ns | 54.89 ns | 12.39 ns | 12.91 ns | 100 | 51,200 |
| `click_enqueue_bounded_available` | 72.78 ns | 153.4 ns | 73.41 ns | 74.43 ns | 100 | 6,400 |
| `click_enqueue_full_merge_existing` | 49.66 ns | 50.60 ns | 49.97 ns | 50.00 ns | 100 | 12,800 |
| `location_clone_long` | 17.32 ns | 41.69 ns | 17.63 ns | 17.79 ns | 100 | 25,600 |
| `location_clone_short` | 17.78 ns | 18.25 ns | 18.02 ns | 18.02 ns | 100 | 25,600 |
| `location_parse_invalid` | 30.28 ns | 43.41 ns | 30.44 ns | 30.58 ns | 100 | 25,600 |
| `location_parse_long` | 574.6 ns | 3.574 us | 579.6 ns | 607.4 ns | 100 | 800 |
| `location_parse_percent_encoded_unicode` | 72.16 ns | 128.4 ns | 72.78 ns | 73.08 ns | 100 | 6,400 |
| `location_parse_short` | 49.35 ns | 95.28 ns | 49.66 ns | 50.27 ns | 100 | 12,800 |
| `measured_lookup_result_shape` | 69.03 ns | 69.66 ns | 69.66 ns | 69.58 ns | 100 | 6,400 |
| `plain_lookup_result_shape` | 0.332 ns | 0.747 ns | 0.337 ns | 0.341 ns | 100 | 819,200 |
| `redirect_response_lean` | 99.03 ns | 185.9 ns | 100.2 ns | 101.3 ns | 100 | 6,400 |
| `redirect_response_with_timing_headers` | 864.6 ns | 1.329 us | 869.6 ns | 874.4 ns | 100 | 800 |
| `short_code_clone_common_length` | 18.72 ns | 19.03 ns | 18.88 ns | 18.88 ns | 100 | 25,600 |
| `short_code_clone_max_length` | 19.03 ns | 31.53 ns | 19.19 ns | 19.45 ns | 100 | 25,600 |
| `short_code_transfer_common_length` | 2.105 ns | 8.941 ns | 2.124 ns | 2.189 ns | 100 | 204,800 |
| `short_code_transfer_max_length` | 2.105 ns | 2.183 ns | 2.124 ns | 2.122 ns | 100 | 204,800 |

## Interpretation

- A cached short destination uses an 18.02 ns header clone instead of a
  49.66 ns parse, about 2.8 times cheaper by median.
- A cached long destination uses a 17.63 ns clone instead of a 579.6 ns parse,
  about 32.9 times cheaper by median.
- Sharing the cache-resident analytics code costs 12.39 ns versus 18.88 ns for
  a fresh common-length `String` clone. More importantly, `Arc<str>` avoids a
  per-event heap allocation and allocator contention.
- Single-flight misses, click batches, analytics batches, retry semantics, and
  cache coherence are behavioral/concurrency properties. They are covered by
  integration tests and must be judged by PostgreSQL 18 CI, not nanosecond
  SQLite measurements.
