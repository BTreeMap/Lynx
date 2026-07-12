# Retained Cached Redirect Projection Microbenchmark

This snapshot accompanies the ownership audit that changed `RedirectTarget` to
retain its `Arc<CachedUrl>` rather than independently clone the cache entry's
URL model and analytics code. It is a mechanism measurement, not an HTTP
throughput claim.

## Environment

- Date: 2026-07-12
- Base commit: `0a15bb5` plus working-tree changes
- Command: `cargo bench --bench redirect_hot_path`
- Rust/Cargo: 1.97.0
- OS/architecture: Linux aarch64
- CPU: 4 vCPU, Arm Neoverse-N1
- Divan timer precision: 40 ns

## Complete observed result

| Case | Fastest | Slowest | Median | Mean | Samples | Iterations |
|---|---:|---:|---:|---:|---:|---:|
| `analytics_short_code_arc_clone` | 12.71 ns | 37.16 ns | 12.78 ns | 13.13 ns | 100 | 51,200 |
| `click_enqueue_bounded_available` | 72.78 ns | 140.9 ns | 74.03 ns | 74.98 ns | 100 | 6,400 |
| `click_enqueue_full_merge_existing` | 49.35 ns | 50.28 ns | 49.66 ns | 49.59 ns | 100 | 12,800 |
| `location_clone_long` | 17.47 ns | 17.78 ns | 17.63 ns | 17.64 ns | 100 | 25,600 |
| `location_clone_short` | 17.32 ns | 32.47 ns | 17.47 ns | 17.68 ns | 100 | 25,600 |
| `location_parse_invalid` | 30.28 ns | 132 ns | 30.44 ns | 31.55 ns | 100 | 25,600 |
| `location_parse_long` | 574.6 ns | 584.6 ns | 574.6 ns | 577 ns | 100 | 800 |
| `location_parse_percent_encoded_unicode` | 71.53 ns | 122.7 ns | 72.78 ns | 72.95 ns | 100 | 6,400 |
| `location_parse_short` | 49.66 ns | 62.47 ns | 49.97 ns | 50.11 ns | 100 | 12,800 |
| `measured_lookup_result_shape` | 69.03 ns | 120.2 ns | 69.66 ns | 70.07 ns | 100 | 6,400 |
| `plain_lookup_result_shape` | 0.332 ns | 1.055 ns | 0.337 ns | 0.344 ns | 100 | 819,200 |
| `redirect_projection_clone_components` | 49.66 ns | 77.47 ns | 49.66 ns | 50.03 ns | 100 | 12,800 |
| `redirect_projection_retain_cache_entry` | 12.78 ns | 12.86 ns | 12.86 ns | 12.83 ns | 100 | 51,200 |
| `redirect_response_lean` | 97.16 ns | 153.4 ns | 99.03 ns | 99.6 ns | 100 | 6,400 |
| `redirect_response_with_timing_headers` | 859.6 ns | 1.584 µs | 864.6 ns | 872.6 ns | 100 | 800 |
| `short_code_clone_common_length` | 19.03 ns | 19.35 ns | 19.19 ns | 19.23 ns | 100 | 25,600 |
| `short_code_clone_max_length` | 19.03 ns | 95.28 ns | 19.19 ns | 20.14 ns | 100 | 25,600 |
| `short_code_transfer_common_length` | 2.105 ns | 2.261 ns | 2.124 ns | 2.138 ns | 100 | 204,800 |
| `short_code_transfer_max_length` | 2.105 ns | 2.242 ns | 2.124 ns | 2.124 ns | 100 | 204,800 |

## Interpretation

- The old projection shape performed one cache-entry clone plus separate
  `Arc<ShortenedUrl>`, `Arc<str>`, and `HeaderValue` clones. Its isolated median
  was 49.66 ns.
- Retaining the cache entry as the redirect target costs 12.86 ns in the same
  fixture, a 36.80 ns reduction and about 3.9× lower local projection cost. The
  response still clones the `HeaderValue`, so this is specifically an ownership
  construction result, not a claim that the whole redirect is 3.9× faster.
- An analytics code `Arc<str>` clone measured 12.78 ns. It remains appropriate
  because an event must own its code after the handler returns and after a cache
  entry may be evicted. The benchmark is uncontended; cache-line contention for
  a viral code must be measured with representative multi-core PostgreSQL load.
- The lifecycle mutexes do not appear in these cases because graceful shutdown
  is not a serving-path operation. They use a synchronous short critical section
  only to transfer a `JoinHandle`; the guard is released before any `.await`.
  A stateful Tokio watch channel prevents shutdown requests from being lost
  between a flush task's state check and its next wait.

PostgreSQL 18 throughput, tail latency, and flamegraphs remain the authority for
keeping this optimization.
