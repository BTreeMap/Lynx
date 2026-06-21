# Database & Migration Integrity

**Read this before any change that touches persistence, models, or queries.**

Data integrity is the one place where "ruthless refactoring / no backward
compatibility" does **not** apply. You may freely rewrite Rust interfaces, but
you must **never** put existing user data at risk. Every schema-affecting code
change ships a correct migration.

## How schema is defined

Lynx has **no separate migration files**. Schema is created idempotently at
startup inside each backend's `init()`:

- SQLite: [`src/storage/sqlite.rs`](../../src/storage/sqlite.rs)
- PostgreSQL: [`src/storage/postgres.rs`](../../src/storage/postgres.rs)

Both use `CREATE TABLE IF NOT EXISTS`, `CREATE INDEX IF NOT EXISTS`, and
triggers so that `init()` is safe to run on every boot against an existing
database.

## Rules for changing the schema

1. **Mirror every change across both backends.** A change to `sqlite.rs::init()`
   must have an equivalent in `postgres.rs::init()` (mind dialect differences:
   `INTEGER PRIMARY KEY AUTOINCREMENT` vs `BIGSERIAL`, `?` vs `$1` binds, FTS5 vs
   `pg_trgm`, etc.).
2. **Additive and idempotent only.** New columns/tables/indexes must use
   `IF NOT EXISTS` (or an `ALTER TABLE ... ADD COLUMN` guarded so re-running is
   safe). Never drop or rename a column/table that may hold data without a
   data-preserving copy step.
3. **Backfill, don't break.** New non-null columns need a default or a backfill
   so existing rows remain valid. Existing rows must keep working after upgrade.
4. **Preserve the delete-protection invariant.** The `urls` table has a
   `prevent_urls_delete` trigger (SQLite) / equivalent rule (Postgres): URLs are
   **deactivated, never deleted**. Do not remove or weaken this. See
   [`docs/DELETE_PROTECTION.md`](../DELETE_PROTECTION.md).
5. **Validate on both engines.** Run the integration suite against SQLite *and*
   PostgreSQL (commands in `docs/agents/testing.md`) plus the graceful-shutdown
   persistence E2E check before declaring done.

## Tables (current)

`urls`, `users`, `admin_users`, `analytics`, plus FTS5 virtual tables for
search. Consult `init()` in each backend for the authoritative column list
rather than duplicating it here.

## Decision table

| You are... | Required action |
|---|---|
| Adding a column | Add to both `init()` bodies, additive + defaulted, backfill existing rows |
| Adding a table/index | `CREATE ... IF NOT EXISTS` in both backends |
| Renaming a field in Rust only | No migration needed (code-only); keep DB column name or do a guarded copy |
| Renaming/removing a DB column | Copy data to new shape first; never drop populated columns destructively |
| Changing the `urls` lifecycle | Keep deactivation semantics; never enable hard delete |
