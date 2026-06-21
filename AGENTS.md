# AGENTS.md

Lynx is a high-performance URL shortener: a Rust/Axum backend with a React
(Vite + TypeScript) frontend embedded into the binary via `rust-embed`. It runs
a dual-server setup (API on 8080, redirects on 3000) over SQLite **or**
PostgreSQL.

This file is the always-on operating manual for coding agents. It is
intentionally short; load the linked domain docs just-in-time for deeper work.

## Tooling & Commands

- Backend: `cargo` (stable, with `rustfmt` + `clippy`). Frontend: `npm`
  (Node 24). Docker is needed for PostgreSQL and E2E tests.
- Build frontend before backend — `build.rs` embeds `frontend/dist/`.

```bash
# Frontend
cd frontend && npm ci && npm run build && npm run lint

# Backend quality gate (matches CI)
cargo fmt --all -- --check
cargo clippy --all-features -- -D warnings -A clippy::too_many_arguments
cargo test
DATABASE_BACKEND=sqlite cargo test --tests
```

Full test/E2E/PostgreSQL commands → [docs/agents/testing.md](docs/agents/testing.md).

## Boundaries & Constraints

- **Never delete URL records.** The `urls` table is delete-protected by design;
  deactivate instead. Keep the `prevent_urls_delete` trigger intact.
- **Never break the database to refactor code.** You may rewrite/rename/delete
  Rust interfaces freely, but every schema-affecting change MUST ship a correct,
  idempotent, data-preserving migration applied to **both** SQLite and Postgres.
  → [docs/agents/database.md](docs/agents/database.md).
- **Never edit CI workflows** under `.github/workflows/` unless explicitly asked.
- **Never use `AUTH_MODE=none` outside tests**, and never print `DATABASE_URL`
  or other secrets to logs/CI.
- **Frontend: don't call `fetch`/`axios` directly.** Use the shared `apiClient`
  from `frontend/src/api.ts`. → [docs/agents/frontend.md](docs/agents/frontend.md).
- **Reach for the right backend abstraction**, not escape hatches: persistence
  goes through the `Storage` trait (`src/storage/`); don't open ad-hoc DB
  connections in handlers.

## Definition of Done

Work is complete only when the quality-gate commands above pass, schema changes
are validated on both backends, and tests are added/updated for changed
behavior. The **PR Quality Gate** workflow must be green before review.

## Agent Operating Contract

Every session follows the standards in
[docs/agents/engineering-standards.md](docs/agents/engineering-standards.md).
Key always-on rules:

- **Operate autonomously.** Make the most reasonable assumption on ambiguity,
  document it, and proceed; only pause for destructive/irreversible actions.
  Announce explicit completion — do not stop silently.
- **Make invalid states unrepresentable**; push invariants into the type system.
- **Refactor ruthlessly** (no internal backward-compat duty) and prune dead
  code — except the database, where data integrity is non-negotiable.
- **Keep files ≤ ~500 lines**; split into cohesive submodules as they grow.
- **Handle errors idiomatically** (`Result`/`Option`, `?`, `thiserror`,
  `anyhow`); never swallow them.
- **Avoid reflexive `.clone()`/`Rc`/`Arc`/`Box<dyn _>`** to appease the borrow
  checker — redesign data flow instead. Justified shared ownership (e.g.
  `Arc<Pool>`) and the deliberate `Storage` trait object remain fine.

## Domain Documentation (load on demand)

| When working on… | Read |
|---|---|
| Persistence, models, schema, migrations | [docs/agents/database.md](docs/agents/database.md) |
| Tests, CI gate, E2E, drift checks | [docs/agents/testing.md](docs/agents/testing.md) |
| React/TypeScript frontend | [docs/agents/frontend.md](docs/agents/frontend.md) |
| Full engineering standards & output contract | [docs/agents/engineering-standards.md](docs/agents/engineering-standards.md) |
