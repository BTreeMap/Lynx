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

Start Lynx (Docker or `cargo run`), then:

```bash
bash tests/integration_test.sh http://localhost:8080 http://localhost:3000
bash tests/concurrent_test.sh   http://localhost:8080 http://localhost:3000 100
```

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
