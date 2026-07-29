# Frontend Conventions

React 19 + TypeScript, built with Vite. Bundled into the Rust binary via
`rust-embed`, so a stale `frontend/dist/` ships stale UI — rebuild after
changes.

## Tooling

- Package manager: **npm** (`npm ci` / `npm run build` / `npm run lint` /
  `npm test`).
- Build: `tsc -b && vite build`. Lint: `eslint .`. Test: `vitest run`
  (`npm run test:watch` while working).

## Tests

`src/**/*.test.ts`, colocated with the module under test and configured in
[`vitest.config.ts`](../../frontend/vitest.config.ts). The suite covers the
**domain modules only** — reducers, parsers, selectors, codecs — which is why it
runs in a `node` environment with no DOM and no React plugin: that layer is
framework-free by construction, and it is where the invariants live.

Add a case for every new sum-type variant, every transition that must *not*
apply (a stale settlement, a dismissal mid-flight), and every rejection path of
a parser. Rendering is not covered; a component test needs a config that
supplies a renderer.

> CI's PR Quality Gate currently runs `npm run build` and `npm run lint` only.
> Until `npm test` is added there, run it locally before pushing.

## Platform baseline

Target current browsers (major releases from roughly the last two years) and use
platform APIs directly.

- **Never monkey-patch a built-in prototype**, and never hand-roll a method the
  engine already provides — a manual substitute is slower and drifts from the
  spec.
- **"Available" means Baseline _widely available_**, not merely shipped
  everywhere. A newly-available API on a critical path breaks browsers that are
  still well inside support; when one is tempting, use the widely-available
  primitive and leave a note naming the API to switch to later. See
  [`src/lib/base64.ts`](../../frontend/src/lib/base64.ts).
- If TypeScript's `lib` lags a shipped API, add an ambient *declaration* — a
  declaration, never an implementation. Build-time config is declared the same
  way in [`src/types/env.d.ts`](../../frontend/src/types/env.d.ts).

## Architecture

Domain logic is separated from rendering. The pattern throughout:

- **A pure module** holds the types, the reducer, and the selectors. It imports
  nothing from React and can be read (or tested) on its own — e.g.
  [`src/auth/model.ts`](../../frontend/src/auth/model.ts),
  [`src/components/urls/linkCollection.ts`](../../frontend/src/components/urls/linkCollection.ts),
  [`src/components/urls/urlActions.ts`](../../frontend/src/components/urls/urlActions.ts),
  [`src/components/urls/destinationEditor.ts`](../../frontend/src/components/urls/destinationEditor.ts).
- **A hook interprets it** against the network and the browser, owning
  cancellation and lifecycle — e.g. `useLinkCollection`, `useUrlActions`,
  `AuthProvider`.
- **Components render it.** Prefer deriving during render over storing; put an
  effect caused solely by a user action in the handler for that action.

Conventions that follow from it:

- **Model mutually exclusive states as a `readonly` discriminated union**, not
  as a bag of booleans and nullables. Match exhaustively and close the switch
  with `assertNever` from [`src/lib/assertNever.ts`](../../frontend/src/lib/assertNever.ts).
- **Reads go through `useRemoteQuery`**
  ([`src/hooks/useRemoteQuery.ts`](../../frontend/src/hooks/useRemoteQuery.ts)),
  which yields a `RemoteData<T>` (`idle | loading | success | failure`) and
  aborts the request when its key changes. Do not hand-roll `isLoading` flags.
- **Parse untrusted input at one boundary**: the auth-mode payload becomes a
  domain value in `parseServerConfig`, a short code from the address bar in
  `decodeShortCodeFromApi`. Downstream code receives a parsed value or `null`.

## HTTP access

- **Do not call `fetch` or `axios` directly from components.** Use the shared
  `apiClient` exported from [`frontend/src/api.ts`](../../frontend/src/api.ts).
  It centralizes the base URL, bearer-token injection, short-code encoding, and
  response typing.
- Add new endpoints as typed methods on `apiClient`; define request/response
  shapes in [`frontend/src/types.ts`](../../frontend/src/types.ts). Reads take
  an `AbortSignal`; pass query parameters as one object and let `undefined`
  members drop out.
- The bearer token lives behind
  [`src/auth/tokenStore.ts`](../../frontend/src/auth/tokenStore.ts) — never read
  `localStorage` for it directly.

## Code style

Formatting and lint rules are enforced by ESLint/TypeScript — do not hand-tune
style. Match surrounding code; let the linter be the source of truth. Keep files
under ~500 lines and split by concern, not by file size.

## Styling

Styling uses **Tailwind CSS v4** natively (CSS-first, no `tailwind.config.js`).
The plugin is wired in [`vite.config.ts`](../../frontend/vite.config.ts) and all
tokens live in [`frontend/src/index.css`](../../frontend/src/index.css):

- Brand palettes are declared in `@theme` (`baltic-blue` = primary, `pearl-aqua`
  = accent/success, `slate` = neutral).
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
  it out of the main bundle. Chart colors are passed as `var(--chart-N)`
  references (see
  [`src/components/analytics/chartPalette.ts`](../../frontend/src/components/analytics/chartPalette.ts))
  so the browser recolors them on theme change — do not sample computed styles.

## App icon

The mark is defined once as SVG paths in
[`src/components/layout/Logo.tsx`](../../frontend/src/components/layout/Logo.tsx)
and reproduced in [`public/favicon.svg`](../../frontend/public/favicon.svg).
`public/` also carries `apple-touch-icon.png`, `icon-192.png`, `icon-512.png`
and `manifest.json`, all wired up in `index.html`. Changing the mark means
regenerating the PNGs from the same geometry.
