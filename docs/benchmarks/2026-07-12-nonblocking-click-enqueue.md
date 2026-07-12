# Nonblocking Click Enqueue Microbenchmark

This snapshot adds direct cases for the redirect click enqueue. Redirect
handlers no longer await bounded-channel capacity and do not create a Tokio task
per request.

## Environment

- Date: 2026-07-12
- Base commit: `cfb941029015e944358f9b8246795b6010fe9a6d` plus working-tree change
- Command: `cargo bench --bench redirect_hot_path`
- Rust: `rustc 1.97.0 (2d8144b78 2026-07-07)`
- Cargo: `cargo 1.97.0 (c980f4866 2026-06-30)`
- OS/architecture: Linux 6.17.0-35-generic aarch64
- CPU: 4 vCPU, Arm Neoverse-N1
- Divan timer precision: 40 ns

## Complete observed result

| Case | Fastest | Slowest | Median | Mean | Samples | Iterations |
|---|---:|---:|---:|---:|---:|---:|
| `click_enqueue_bounded_available` | 70.91 ns | 126.5 ns | 72.16 ns | 72.73 ns | 100 | 6,400 |
| `click_enqueue_full_merge_existing` | 49.35 ns | 50.60 ns | 49.97 ns | 49.93 ns | 100 | 12,800 |
| `location_clone_long` | 17.32 ns | 17.94 ns | 17.63 ns | 17.58 ns | 100 | 25,600 |
| `location_clone_short` | 17.47 ns | 18.10 ns | 17.78 ns | 17.78 ns | 100 | 25,600 |
| `location_parse_invalid` | 30.28 ns | 642.7 ns | 30.44 ns | 36.76 ns | 100 | 25,600 |
| `location_parse_long` | 574.6 ns | 11.54 us | 574.6 ns | 697.1 ns | 100 | 800 |
| `location_parse_percent_encoded_unicode` | 72.16 ns | 73.41 ns | 72.78 ns | 72.68 ns | 100 | 6,400 |
| `location_parse_short` | 49.97 ns | 109.3 ns | 50.28 ns | 50.98 ns | 100 | 12,800 |
| `measured_lookup_result_shape` | 67.78 ns | 68.41 ns | 68.41 ns | 68.23 ns | 100 | 6,400 |
| `plain_lookup_result_shape` | 0.332 ns | 4.624 ns | 0.337 ns | 0.384 ns | 100 | 819,200 |
| `redirect_response_lean` | 99.03 ns | 187.7 ns | 100.2 ns | 101.4 ns | 100 | 6,400 |
| `redirect_response_with_timing_headers` | 879.6 ns | 4.399 us | 879.6 ns | 938.4 ns | 100 | 100 |
| `short_code_clone_common_length` | 18.88 ns | 36.22 ns | 19.03 ns | 19.20 ns | 100 | 25,600 |
| `short_code_clone_max_length` | 19.19 ns | 19.50 ns | 19.35 ns | 19.29 ns | 100 | 25,600 |
| `short_code_transfer_common_length` | 2.105 ns | 5.503 ns | 2.124 ns | 2.159 ns | 100 | 204,800 |
| `short_code_transfer_max_length` | 2.105 ns | 7.066 ns | 2.124 ns | 2.182 ns | 100 | 204,800 |

## Interpretation

- Available queue: ownership transfer plus bounded `try_send` and immediate
  receive measured 72.16 ns median. The receive exists only to reset benchmark
  state, so this is conservative for the producer-side operation.
- Full queue: recovering the owned message and merging into an existing
  `DashMap` entry measured 49.97 ns median.
- These Divan cases are intentionally single-threaded mechanism tests. They do
  not measure same-code `DashMap` lock contention, actor flush work, or runtime
  scheduling under concurrent HTTP load.
- Neither path awaits, allocates a second short-code string, or invokes
  `tokio::spawn` per redirect.
- Queue saturation remains lossless: the fallback transfers the click into the
  same shared layer periodically persisted by the long-lived actor.
- The long-lived actor and periodic database flush may use startup/periodic
  tasks; no task is created per redirect.

The previous production version's async `send().await` is intentionally not
assigned one nanosecond value: whether it suspends depends on queue capacity and
runtime scheduling. The meaningful semantic improvement is removal of that
suspension point from the redirect handler. PostgreSQL 18 CI after this change
remains the end-to-end decision gate.
