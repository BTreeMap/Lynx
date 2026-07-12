# Microbenchmark Evidence

Microbenchmark cases are source-controlled in
[`benches/redirect_hot_path.rs`](../../benches/redirect_hot_path.rs). Dated
snapshots in this directory preserve observed results that justified hot-path
changes.

Run every case with:

```text
cargo bench --bench redirect_hot_path
```

## Evidence rules

- Case source is the reproducible specification; snapshots are observations.
- Snapshots record the commit, toolchain, architecture, and complete result
  table. Results from different machines are not directly comparable.
- Divan isolates synchronous mechanisms. It does not model Axum scheduling,
  network traffic, PostgreSQL, or queue contention at production concurrency.
- PostgreSQL 18 CI throughput, latency, and flamegraphs are authoritative for
  retaining a production optimization.
- Add a dated snapshot whenever a microbenchmark is used to justify a code
  change. Do not replace historical snapshots.

## Current cases and rationale

| Case family | Question answered |
|---|---|
| `short_code_clone_*` / `short_code_transfer_*` | What allocation cost is avoided by transferring Axum's owned path string? |
| `click_enqueue_bounded_available` | What is the uncontended cost of ownership transfer through a bounded Tokio channel? |
| `click_enqueue_full_merge_existing` | What is the synchronous lossless fallback cost when that channel is saturated? |
| `plain_lookup_result_shape` / `measured_lookup_result_shape` | What local work is removed when timing metadata is disabled? |
| `redirect_response_*` | What local response-construction cost comes from timing headers? |
| `location_parse_*` / `location_clone_*` | Would caching a validated `HeaderValue` plausibly avoid useful work? |

The transfer input is created outside Divan's timed region. This is deliberate:
the path `String` already exists after Axum extraction, so the production choice
is between moving that allocation and creating a second allocation with
`String::clone`.
