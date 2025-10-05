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

1. Obtain an OAuth 2.0 bearer token from your OAuth provider
2. Enter the token in the login page
3. The token will be stored in your browser's localStorage
4. All API requests will include this token in the Authorization header

## Admin Features

Users with the `admin` role (detected from the OAuth token's `roles` array or `role` field) have access to:

- View all users' short URLs
- Deactivate inappropriate or policy-violating URLs
- Reactivate previously deactivated URLs

Regular users can:
- Create short URLs with optional custom codes
- View their own URLs and statistics
- See click counts and active status

## Technology Stack

- React 18
- TypeScript
- Vite
- Axios for API calls
- React Router DOM
