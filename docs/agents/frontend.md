# Frontend Conventions

React 19 + TypeScript, built with Vite. Bundled into the Rust binary via
`rust-embed`, so a stale `frontend/dist/` ships stale UI — rebuild after
changes.

## Tooling

- Package manager: **npm** (`npm ci` / `npm run build` / `npm run lint`).
- Build: `tsc -b && vite build`. Lint: `eslint .`.

## HTTP access

- **Do not call `fetch` or `axios` directly from components.** Use the shared
  `apiClient` exported from [`frontend/src/api.ts`](../../frontend/src/api.ts).
  It centralizes the base URL, bearer-token injection, and response typing.
- Add new endpoints as typed methods on `apiClient`; define request/response
  shapes in [`frontend/src/types.ts`](../../frontend/src/types.ts).

## Code style

Formatting and lint rules are enforced by ESLint/TypeScript — do not hand-tune
style. Match surrounding code; let the linter be the source of truth.
