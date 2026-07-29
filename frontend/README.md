# Lynx Frontend

React-based frontend for the Lynx URL shortener with multi-user support and admin panel.

## Features

- 🔐 OAuth 2.0 Bearer token authentication
- 📊 Dashboard showing user's short URLs with statistics
- ➕ Create new short URLs (with optional custom codes)
- 👁️ View click statistics for each URL
- 🔒 Admin panel for managing all users' links
- ⚡ Admin-only URL deactivation/reactivation

## Setup

1. Install dependencies:
```bash
npm install
```

2. Create a `.env` file based on `.env.example`:
```bash
cp .env.example .env
```

3. Update the `.env` file with your configuration:
```
VITE_API_URL=http://localhost:8080
VITE_REDIRECT_URL=http://localhost:3000
```

## Development

Run the development server:
```bash
npm run dev
```

The app will be available at http://localhost:5173

## Building

Build for production:
```bash
npm run build
```

The built files will be in the `dist` directory.

## Authentication

To use the application:

1. Click "Sign in with OAuth" on the login page
2. Complete authentication with your OpenID Connect provider
3. Return to the frontend callback route for PKCE code exchange
4. A bearer token is stored in localStorage
5. All API requests include this token in the Authorization header

## Admin Features

Users with the `admin` role (detected from the OAuth token's `roles` array or `role` field) have access to:

- View all users' short URLs
- Deactivate inappropriate or policy-violating URLs
- Reactivate previously deactivated URLs

Regular users can:
- Create short URLs with optional custom codes
- View their own URLs and statistics
- See click counts and active status

## Project structure

Domain logic is kept out of components: a pure module holds the state machine, a hook
interprets it against the network, and components render the result.

```
src/
  api.ts                  Every HTTP call. Base URL, bearer injection, short-code encoding.
  types.ts                Wire types (declarations only — they validate nothing).
  auth/                   Auth mode + session machine, OIDC/PKCE flow, token storage.
  lib/                    Framework-free helpers: RemoteData, base64url, cn, assertNever.
  hooks/                  useRemoteQuery (abortable reads), useAuth, useTheme.
  components/
    ui/                   Presentational primitives (Button, Card, Dialog, Table, …).
    layout/               App shell: header, page scaffolding, logo.
    urls/                 Link collection + row actions + destination editing.
    analytics/            Dimension model, charts, analytics queries.
    dashboard/            Dashboard-only presentation.
```

See [docs/agents/frontend.md](../docs/agents/frontend.md) for the conventions these
follow.

## Technology Stack

- React 19
- TypeScript
- Vite (Rolldown)
- Tailwind CSS v4
- Axios for API calls
- React Router DOM
- Recharts for analytics charts
