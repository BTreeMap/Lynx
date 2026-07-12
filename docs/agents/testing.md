# Testing & Verification

All commands run from the repository root unless noted. The frontend must be
built before `cargo build`/`cargo test` because `build.rs` embeds
`frontend/dist/` (it attempts the build automatically when Node is present).

## Frontend

```bash
cd frontend
npm ci            # use `npm install` if package-lock.json is absent
npm run build     # → frontend/dist/
npm run lint
```

## Rust quality gate (matches CI)

```bash
cargo fmt --all -- --check
cargo clippy --all-features -- -D warnings -A clippy::too_many_arguments
cargo test
```

## Integration tests

```bash
# SQLite (minimum required before pushing)
DATABASE_BACKEND=sqlite cargo test --tests

# PostgreSQL (requires a running instance)
DATABASE_BACKEND=postgres \
DATABASE_URL=postgresql://lynx:lynx_password@localhost:5432/lynx \
cargo test --tests
```

Run a single integration test:

```bash
DATABASE_BACKEND=sqlite cargo test --test storage_integration_test <test_name>
```

## End-to-end (against a running instance)

Start Lynx (Docker or `cargo run`) with the test-only `AUTH_MODE=none`, then
run the typed external harness. Set `LYNX_E2E_CONTAINER` when the service is a
Docker container so the suite can verify SIGTERM/SIGINT persistence exactly.

```bash
LYNX_E2E_CONTAINER=lynx \
LYNX_E2E_CONCURRENCY=100 \
cargo test --test external_harness -- --ignored --test-threads=1 --nocapture
```

The suite owns HTTP requests, JSON validation, retries, concurrent traffic,
and lifecycle effects in Rust. See [tests/README.md](../../tests/README.md) for
configuration and native benchmark commands.

## Documentation drift

```bash
bash scripts/check-docs-drift.sh   # README must document all routes + env vars
```

## Notes

- Tests use `AUTH_MODE=none` for simplicity. **Never** use `AUTH_MODE=none` in
  production.
- The **PR Quality Gate** workflow (`.github/workflows/pr-quality-gate.yml`)
  must be green before work is considered complete. Do not edit workflow files
  unless explicitly asked.
