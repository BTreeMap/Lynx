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

## Styling

Styling uses **Tailwind CSS v4** natively (CSS-first, no `tailwind.config.js`).
The plugin is wired in [`vite.config.ts`](../../frontend/vite.config.ts) and all
tokens live in [`frontend/src/index.css`](../../frontend/src/index.css):

- Brand palettes are declared in `@theme` (`baltic-blue` = primary, `porcelain`
  = success, `cherry-rose` = danger, `dark-raspberry` = accent).
- Semantic, theme-aware tokens (`bg`, `surface`, `fg`, `fg-muted`, `border`,
  `primary`, `success`, `danger`, …) are CSS variables registered via
  `@theme inline`. Prefer these utilities (`bg-surface`, `text-fg`,
  `border-border`) over raw palette shades so light/dark both work.
- **Light + dark mode** is class-based: the `.dark` class is toggled on `<html>`
  by [`ThemeProvider`](../../frontend/src/components/ThemeProvider.tsx); never
  hard-code colors that don't adapt.
- Reusable primitives live in `frontend/src/components/ui/`; compose those
  (Button, Card, Dialog, Table, Badge, …) instead of bespoke markup. Merge
  classes with the `cn()` helper from `frontend/src/lib/cn.ts`.
- Analytics charts use **Recharts**; the analytics route is lazy-loaded to keep
  it out of the main bundle.
