# Redirect Ownership Transfer Microbenchmark

This snapshot records the run used to validate commit `2d1993e`, before the
nonblocking enqueue change. The benchmark case source was already committed.

## Environment

- Date: 2026-07-12
- Commit: `cfb941029015e944358f9b8246795b6010fe9a6d`
- Command: `cargo bench --bench redirect_hot_path`
- Rust/Cargo: 1.97.0
- OS/architecture: Linux aarch64
- CPU: 4 vCPU, Arm Neoverse-N1
- Divan timer precision: 40 ns

## Complete observed result

| Case | Fastest | Slowest | Median | Mean | Samples | Iterations |
|---|---:|---:|---:|---:|---:|---:|
| `location_clone_long` | 39.33 ns | 4.159 us | 39.33 ns | 90.53 ns | 100 | 100 |
| `location_clone_short` | 16.98 ns | 17.45 ns | 17.14 ns | 17.21 ns | 100 | 25,600 |
| `location_parse_invalid` | 30.11 ns | 54.95 ns | 30.26 ns | 30.55 ns | 100 | 25,600 |
| `location_parse_long` | 574.3 ns | 989.3 ns | 574.3 ns | 580.6 ns | 100 | 800 |
| `location_parse_percent_encoded_unicode` | 71.83 ns | 73.08 ns | 72.45 ns | 72.29 ns | 100 | 6,400 |
| `location_parse_short` | 48.70 ns | 76.83 ns | 49.64 ns | 49.94 ns | 100 | 12,800 |
| `measured_lookup_result_shape` | 66.83 ns | 68.08 ns | 68.08 ns | 67.89 ns | 100 | 6,400 |
| `plain_lookup_result_shape` | 0 ns | 0.376 ns | 0.005 ns | 0.008 ns | 100 | 819,200 |
| `redirect_response_lean` | 96.83 ns | 101.2 ns | 99.33 ns | 99.14 ns | 100 | 6,400 |
| `redirect_response_with_timing_headers` | 839.3 ns | 4.079 us | 879.3 ns | 934.5 ns | 100 | 100 |
| `short_code_clone_common_length` | 18.39 ns | 43.70 ns | 18.70 ns | 18.89 ns | 100 | 25,600 |
| `short_code_clone_max_length` | 18.70 ns | 232.4 ns | 18.86 ns | 21.24 ns | 100 | 25,600 |
| `short_code_transfer_common_length` | 1.773 ns | 35.83 ns | 1.792 ns | 2.137 ns | 100 | 204,800 |
| `short_code_transfer_max_length` | 1.753 ns | 1.851 ns | 1.792 ns | 1.791 ns | 100 | 204,800 |

## Interpretation

For the common code, moving the existing allocation measured about 10.4 times
cheaper than cloning it by median (`18.70 / 1.792`). For the configured maximum
length, it measured about 10.5 times cheaper. The result justifies ownership
transfer locally; it does not claim an equivalent end-to-end throughput gain.

PostgreSQL 18 validation for the resulting implementation passed in
[performance run 29208561690](https://github.com/BTreeMap/Lynx/actions/runs/29208561690):
throughput, analytics impact, and CPU flamegraph jobs all succeeded.
