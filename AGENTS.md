# AGENTS.md

## Project Overview

Lynx is a high-performance URL shortener built with a Rust backend (Axum) and a React (Vite + TypeScript) frontend that is bundled directly into the binary via `rust-embed`. It features a dual-server architecture: an API server (default port 8080) for management endpoints and a redirect server (default port 3000) for fast URL redirects. Lynx supports both SQLite and PostgreSQL as storage backends.

## Repo Layout

| Path | Description |
|---|---|
| `src/` | Rust backend source (API, redirect, storage, auth, analytics, config) |
| `frontend/` | React + TypeScript frontend (Vite build) |
| `tests/` | Integration and load test scripts (shell scripts and Rust integration tests) |
| `docs/` | Project documentation (analytics, benchmarks, profiling, etc.) |
| `Dockerfile` | Multi-stage Docker build (Rust builder → Debian slim runtime) |
| `build.rs` | Rust build script; also builds frontend via npm if available |
| `.github/workflows/` | CI/CD workflow files |

## Prerequisites

- **Rust** stable toolchain (with `rustfmt` and `clippy` components)
- **Node.js 24** for building the frontend
- **Docker** for end-to-end tests and PostgreSQL backend testing

## Key Commands

### Frontend

```bash
cd frontend
npm ci           # install deps (use npm install if no package-lock.json)
npm run build    # build frontend → frontend/dist/
npm run lint     # run ESLint
```

### Rust

```bash
# Formatting check
cargo fmt --all -- --check

# Linting (same flags as PR Quality Gate CI)
cargo clippy --all-features -- -D warnings -A clippy::too_many_arguments

# Unit tests
cargo test

# Integration tests — SQLite
DATABASE_BACKEND=sqlite cargo test --tests

# Integration tests — PostgreSQL (requires a running postgres instance)
DATABASE_BACKEND=postgres \
DATABASE_URL=postgresql://lynx:lynx_password@localhost:5432/lynx \
cargo test --tests
```

### End-to-End Tests

Start a Lynx instance (via Docker or cargo run), then:

```bash
# Comprehensive API tests
bash tests/integration_test.sh http://localhost:8080 http://localhost:3000

# Concurrent load test (100 concurrent requests)
bash tests/concurrent_test.sh http://localhost:8080 http://localhost:3000 100
```

## CI Workflows

| Workflow | File | Purpose |
|---|---|---|
| **PR Quality Gate** | `pr-quality-gate.yml` | Runs on every PR to main: frontend build + lint, cargo fmt/clippy/test, integration tests (SQLite & PostgreSQL), Docker build, and full E2E suite with graceful shutdown persistence tests. **Must be green before declaring work complete.** |
| **Build and Publish Docker image** | `docker-publish.yml` | Builds multi-platform Docker images on push to main and publishes to GHCR. |
| **Integration and Data Consistency Tests** | `integration-tests.yml` | Post-publish E2E tests against the published Docker image (SQLite & PostgreSQL). |
| **Build and Publish Binaries** | `build-binaries.yml` | Builds native binaries for Linux, macOS, and Windows on push to main. |
| **Release** | `release.yml` | Builds release binaries and Docker images on GitHub release events. |
| **Performance Benchmarks** | `performance-benchmark.yml` | Performance characterization (not part of PR gating). |

## PR Instructions

1. **Build and test locally before pushing:**
   - Build the frontend: `cd frontend && npm ci && npm run build && npm run lint`
   - Run Rust checks: `cargo fmt --all -- --check && cargo clippy --all-features -- -D warnings -A clippy::too_many_arguments && cargo test`
   - Run integration tests for at least the SQLite backend: `DATABASE_BACKEND=sqlite cargo test --tests`
2. **Add or update tests** when changing behavior.
3. **Do not modify existing CI workflow files** (under `.github/workflows/`) unless explicitly requested.
4. Ensure the **PR Quality Gate** workflow passes on your pull request before requesting review.

## Security Considerations

- **AUTH_MODE**: Lynx supports different authentication modes. Tests use `AUTH_MODE=none` for simplicity. Never use `AUTH_MODE=none` in production.
- **Credentials in logs**: Avoid printing `DATABASE_URL` or other secrets in CI output. The PR quality gate workflow does not use repository secrets.
- **PR CI safety**: The PR workflow uses `pull_request` (not `pull_request_target`), does not perform `docker login`, and does not push images. This prevents untrusted PR code from accessing secrets or publishing artifacts.

## Troubleshooting

| Problem | Solution |
|---|---|
| `frontend/dist` directory missing | Run `cd frontend && npm ci && npm run build` before `cargo build` or `cargo test`. The `build.rs` script attempts this automatically if npm is available. |
| PostgreSQL not ready | Ensure the PostgreSQL container is healthy before running tests. Use `pg_isready -U lynx -d lynx` or the readiness loop pattern from CI. |
| Ports already in use (8080, 3000, 5432) | Stop conflicting services or containers. Check with `lsof -i :8080` or `docker ps`. |
| Docker network issues in local E2E | Create a dedicated Docker network (`docker network create lynx-ci`) and attach both postgres and lynx containers to it, using the container name as the hostname in `DATABASE_URL`. |
| Cargo build fails with missing frontend | The build script (`build.rs`) expects `frontend/dist/` to exist. Build the frontend first or ensure Node.js is installed. |
