# Redirect Hot-Path Architecture

This document records the performance model and remaining work for the redirect
service. The target workload is 100,000 requests/second with approximately
1,000 active connections and skewed short-code popularity.

## Successful cached redirect

1. Axum extracts the owned path `String`.
2. A startup-selected handler avoids disabled analytics/timing extractors.
3. `CachedStorage` reads a TTL-free Moka entry.
4. The entry returns an immutable URL model, prevalidated `HeaderValue`, and
   shared analytics short code.
5. The response clones the cached header.
6. The owned path code is synchronously offered to a bounded click actor.
7. If the actor queue is saturated, the increment is merged into the shared
   flush layer without awaiting or creating a task.
8. Analytics uses bounded `try_send`; saturation moves the event into its shared
   layer without awaiting or dropping it.

There is no database operation, task spawn, channel-capacity wait, destination
parse, or short-code allocation on the successful lean cached path.

## Read-cache policy

- Entries have no TTL or time-to-idle and therefore remain until explicit
  invalidation or configured capacity pressure.
- Positive and negative misses use Moka fallible single-flight. Concurrent first
  requests for one code perform one database read.
- Create populates the cache immediately.
- Deactivate, reactivate, destination update, and history restore invalidate the
  affected code.
- Bulk activation changes invalidate the whole cache because the storage API
  returns only a row count, not the changed code set.
- The database remains authoritative across process restarts.

`CACHE_MAX_ENTRIES` must exceed the expected active working set to achieve
virtually permanent residency. Truly unbounded retention is intentionally not
used because an attacker can generate unbounded negative codes.

## Click persistence

- Layer 1: actor-local `HashMap`, no shared lock.
- Layer 2: sharded `DashMap`, used for overflow and flush visibility.
- Layer 3: database.
- Each periodic flush snapshots nonzero counts into validated `ClickIncrement`
  values and submits one logical atomic batch.
- PostgreSQL applies a batch with one `UPDATE ... FROM UNNEST` statement.
- SQLite applies the batch in one transaction.
- Failed batches are merged back into Layer 2 for retry.
- Graceful shutdown joins actor and database flush tasks.

The queue-full fallback can contend on one DashMap shard for an extremely hot
code, but it runs only when the large bounded actor queue is saturated. The
normal path is message passing and actor-local aggregation.

## Analytics persistence

- Request handling only constructs a small event, clones an `Arc<str>`, and uses
  `try_send`.
- GeoIP work and multidimensional aggregation occur off the serving path.
- Queue saturation falls back to the shared event layer without awaiting.
- PostgreSQL writes a complete rollup batch with one `UNNEST` upsert.
- SQLite writes a complete rollup batch in one transaction.
- Failed rollup flushes are requeued in memory.

## Contention budget

| Operation | Normal path | Saturated/failure path |
|---|---|---|
| URL lookup | Moka concurrent cache read | One single-flight database future per code |
| Click record | Bounded `try_send` | One sharded-map entry update |
| Analytics record | Bounded `try_send` | One sharded-map vector append |
| Header construction | Clone cached `HeaderValue` | Invalid cached header logs and returns 500 |
| Database writes | None per request | Periodic aggregate batches |

No global mutex is acquired per redirect. Locks used for graceful shutdown are
not touched by request handling.

## Remaining optimization plan

1. Split oversized cache and analytics modules into cohesive actor/cache modules.
2. Add authoritative cache-hit/miss and overflow counters without enabling
   request timing on the lean route.
3. Evaluate a cache weigher based on destination/code bytes if link counts can
   exceed configured capacity.
4. Re-profile PostgreSQL 18 after every phase; retain only changes with neutral
   or improved throughput/tail latency.
5. Migrate shell benchmark orchestration incrementally:
   - replace curl-based functional/concurrency scripts with an external Rust E2E
     harness while retaining container-boundary coverage;
   - replace Bash parsing/Lua generation with typed Rust scenario and result
     models;
   - retain an external load generator until the Rust generator demonstrates it
     can drive 100k RPS without becoming the bottleneck;
   - keep the documentation drift script as shell because it is not a benchmark.

## Evidence

Microbenchmark cases and dated snapshots are under
[benchmarks/](benchmarks/README.md). PostgreSQL 18 CI throughput, latency, and
flamegraphs remain authoritative.
