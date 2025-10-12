# Lynx - URL Shortener

Lynx is a URL shortener backend API written in Rust with support for multiple storage backends (SQLite and PostgreSQL), access control, and separate API/client-facing servers.

## Features

- 🔗 **URL Shortening**: Create short codes for long URLs with optional custom codes
- 🗄️ **Extensible Storage**: Support for both SQLite and PostgreSQL backends
- 🔐 **Access Control**: OAuth 2.0 and Cloudflare Zero Trust authentication with configurable pass-through mode
- 🚀 **Dual Server Architecture**: Separate ports for API management and client redirects
- 📊 **Analytics**: Track click counts for each shortened URL
- 🔒 **Immutable URLs**: URLs are immutable and can only be deactivated, not deleted or modified
- 🔄 **Deactivation**: URLs can be deactivated and reactivated (e.g., for policy violations)
- 👥 **Multi-User Support**: Each user can manage their own links; admins can manage all links
- 🖥️ **Web Frontend**: React-based dashboard for managing URLs and viewing statistics
- ⚡ **High Performance**: In-memory caching and write buffering for optimal performance (see [Performance Optimizations](docs/PERFORMANCE_OPTIMIZATIONS.md))

## Frontend

Lynx includes a modern React-based web frontend for managing short URLs. The frontend is **bundled into the binary at compile time** and served directly from the API server.

### Accessing the Frontend

The frontend is automatically available at the API server's root path:

- **Frontend UI**: `http://localhost:8080/` (default)
- **API endpoints**: `http://localhost:8080/api/...`

### Frontend Features

- OAuth 2.0 Bearer token authentication
- Creating short URLs with optional custom codes
- Viewing URL statistics (clicks, status, creation date)
- User-specific URL filtering (users see only their own links)
- Admin panel for managing all users' links
- Admin-only deactivation/reactivation of URLs

### Serving from Custom Directory

You can optionally serve frontend files from a custom directory instead of using the embedded version:

```bash
export FRONTEND_STATIC_DIR=/path/to/frontend/dist
```

This is useful for:

- Serving a custom frontend without recompiling
- Static hosting scenarios where you extract the frontend separately
- Development with hot-reload (point to your dev server's output)

### Standalone Frontend Archive

A separate `frontend-static.tar.gz` archive is available in releases and CI artifacts. Extract and serve with any static file server:

```bash
# Extract the frontend
tar -xzf frontend-static.tar.gz -C /var/www/lynx-frontend

# Serve with nginx, Apache, or any static file server
# Point FRONTEND_STATIC_DIR to the extracted directory
export FRONTEND_STATIC_DIR=/var/www/lynx-frontend
```

See the [frontend README](frontend/README.md) for development setup.

## Architecture

Lynx runs two separate HTTP servers:

1. **API Server** (default: port 8080): For management operations and frontend serving

- Serves the bundled React frontend at `/`
- API endpoints available at `/api/...`
- Optional OAuth 2.0 authentication (can be disabled)
- Create URLs with auto-generated or custom codes
- Deactivate/reactivate URLs
- List and search capabilities

2. **Redirect Server** (default: port 3000): For client-facing URL redirects
   - No authentication required
   - Fast redirects
   - Click tracking
   - Handles deactivated links

This separation allows you to:

- Expose the redirect server publicly while keeping the API server internal
- Use different domains for each server via reverse proxy
- Apply different rate limiting and security policies
- Serve the management frontend only to authorized networks

## Installation

### Download Pre-built Binaries

Pre-built binaries are available for download:

**Latest main branch builds** (updated on every commit):

- Available as artifacts from [GitHub Actions runs](https://github.com/BTreeMap/Lynx/actions)

**Release builds** (stable versions):

- Download from [GitHub Releases](https://github.com/BTreeMap/Lynx/releases)
- Available for Linux (amd64, arm64), macOS (Intel, Apple Silicon), and Windows

```bash
# Example: Download and run on Linux
wget https://github.com/BTreeMap/Lynx/releases/download/v1.0.0/lynx-linux-amd64
chmod +x lynx-linux-amd64
./lynx-linux-amd64
```

### Using Docker

Docker images are automatically published to GitHub Container Registry:

```bash
# Pull latest main branch build
docker pull ghcr.io/btreemap/lynx:latest

# Or pull a specific release
docker pull ghcr.io/btreemap/lynx:v1.0.0

# Run with default SQLite database
docker run -p 8080:8080 -p 3000:3000 ghcr.io/btreemap/lynx:latest

# Run with custom environment variables
docker run -p 8080:8080 -p 3000:3000 \
  -e DATABASE_BACKEND=postgres \
  -e DATABASE_URL=postgresql://user:password@host/db \
  -e AUTH_MODE=oauth \
  ghcr.io/btreemap/lynx:latest
```

### Building from Source

#### Prerequisites

- Rust 1.70+ (install from [rustup.rs](https://rustup.rs))
- For PostgreSQL: A running PostgreSQL instance

#### Building

```bash
cargo build --release
```

## Configuration

Copy the example environment file and edit it:

```bash
cp .env.example .env
```

### Environment Variables

- `DATABASE_BACKEND`: Storage backend (`sqlite` or `postgres`)
- `DATABASE_URL`: Database connection string
  - SQLite: `sqlite://./lynx.db`
  - PostgreSQL: `postgresql://user:password@localhost/lynx`
- `DATABASE_MAX_CONNECTIONS`: Database connection pool size (default: `30`)
- `CACHE_MAX_ENTRIES`: Maximum number of entries in the read cache (default: `500000`, approximately 100MB)
- `API_HOST`: API server host (default: `127.0.0.1`)
- `API_PORT`: API server port (default: `8080`)
- `REDIRECT_HOST`: Redirect server host (default: `127.0.0.1`)
- `REDIRECT_PORT`: Redirect server port (default: `3000`)
- `AUTH_MODE`: Authentication mode (`none`, `oauth`, or `cloudflare`, default: `none`)
- `DISABLE_AUTH`: Legacy override alias for `AUTH_MODE=none`

**OAuth Configuration** (when `AUTH_MODE=oauth`):
- `OAUTH_ISSUER_URL`: OpenID Connect issuer URL
- `OAUTH_AUDIENCE`: Expected audience claim for incoming tokens
- `OAUTH_JWKS_URL`: Optional JWKS endpoint override (defaults to issuer discovery document)
- `OAUTH_JWKS_CACHE_SECS`: JWKS cache TTL in seconds (default: `300`)

**Cloudflare Zero Trust Configuration** (when `AUTH_MODE=cloudflare`):
- `CLOUDFLARE_TEAM_DOMAIN`: Your Cloudflare team domain (e.g., `https://your-team.cloudflareaccess.com`)
- `CLOUDFLARE_AUDIENCE`: Application Audience (AUD) tag from your Access Application
- `CLOUDFLARE_CERTS_CACHE_SECS`: Certificate cache TTL in seconds (default: `86400`)

## Running

```bash
cargo run --release
```

Or run the binary directly:

```bash
./target/release/lynx
```

## API Endpoints

### API Server (Port 8080)

All endpoints require a valid OAuth 2.0 Bearer token in the `Authorization` header (unless authentication is disabled via `AUTH_MODE=none`).

#### Health Check

```bash
GET /health
```

#### Create Shortened URL

```bash
POST /urls
Content-Type: application/json
Authorization: Bearer <access-token>

{
  "url": "https://example.com/very/long/url",
  "custom_code": "mycode"  // Optional
}

Response: 201 Created
{
  "id": 1,
  "short_code": "mycode",
  "original_url": "https://example.com/very/long/url",
  "created_at": 1704067200,
  "created_by": null,
  "clicks": 0,
  "is_active": true
}
```

#### Get URL Details

```bash
GET /urls/:code
Authorization: Bearer <access-token>

Response: 200 OK
{
  "id": 1,
  "short_code": "mycode",
  "original_url": "https://example.com/very/long/url",
  "created_at": 1704067200,
  "created_by": null,
  "clicks": 42,
  "is_active": true
}
```

#### Deactivate URL

```bash
PUT /urls/:code/deactivate
Content-Type: application/json
Authorization: Bearer <access-token>

{
  "reason": "Policy violation"  // Optional
}

Response: 200 OK
{
  "message": "URL deactivated successfully"
}
```

#### Reactivate URL

```bash
PUT /urls/:code/reactivate
Authorization: Bearer <access-token>

Response: 200 OK
{
  "message": "URL reactivated successfully"
}
```

#### List URLs

```bash
GET /urls?limit=50&offset=0
Authorization: Bearer <access-token>

Response: 200 OK
[
  {
    "id": 1,
    "short_code": "abc123",
    "original_url": "https://example.com",
    "created_at": 1704067200,
    "created_by": null,
    "clicks": 10,
    "is_active": true
  },
  ...
]
```

### Redirect Server (Port 3000)

No authentication required.

#### Health Check

```bash
GET /health
```

#### Redirect to Original URL

```bash
GET /:code

Response: 301 Permanent Redirect
Location: https://example.com/original/url
```

## Example Usage

### Using SQLite (Development)

```bash
# .env file
DATABASE_BACKEND=sqlite
DATABASE_URL=sqlite://./lynx.db
API_HOST=127.0.0.1
API_PORT=8080
REDIRECT_HOST=127.0.0.1
REDIRECT_PORT=3000
AUTH_MODE=none
```

### Using PostgreSQL with Cloudflare Zero Trust (Production)

```bash
# .env file
DATABASE_BACKEND=postgres
DATABASE_URL=postgresql://lynx_user:secure_password@localhost/lynx
API_HOST=0.0.0.0
API_PORT=8080
REDIRECT_HOST=0.0.0.0
REDIRECT_PORT=3000
AUTH_MODE=cloudflare
CLOUDFLARE_TEAM_DOMAIN=https://your-team.cloudflareaccess.com
CLOUDFLARE_AUDIENCE=abc123def456...your-aud-tag
```

See [Cloudflare Zero Trust Setup Guide](docs/CLOUDFLARE_SETUP.md) for detailed configuration instructions.

### Using PostgreSQL with OAuth (Production)

```bash
# .env file
DATABASE_BACKEND=postgres
DATABASE_URL=postgresql://lynx_user:secure_password@localhost/lynx
API_HOST=0.0.0.0
API_PORT=8080
REDIRECT_HOST=0.0.0.0
REDIRECT_PORT=3000
AUTH_MODE=oauth
OAUTH_ISSUER_URL=https://auth.yourdomain.com/realms/lynx
OAUTH_AUDIENCE=lynx-api
# Optional if discovery document exposes JWKS endpoint
# OAUTH_JWKS_URL=https://auth.yourdomain.com/realms/lynx/protocol/openid-connect/certs
```

### Example API Calls

```bash
# Create a shortened URL
curl -X POST http://localhost:8080/urls \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <access-token>" \
  -d '{
    "url": "https://github.com/BTreeMap/Lynx",
    "custom_code": "lynx"
  }'

# Access the shortened URL (will redirect)
curl -L http://localhost:3000/lynx

# Get URL statistics
curl http://localhost:8080/urls/lynx \
  -H "Authorization: Bearer <access-token>"

# List all URLs
curl http://localhost:8080/urls?limit=10 \
  -H "Authorization: Bearer <access-token>"

# Deactivate a URL
curl -X PUT http://localhost:8080/urls/lynx/deactivate \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <access-token>" \
  -d '{}'

# Reactivate a URL
curl -X PUT http://localhost:8080/urls/lynx/reactivate \
  -H "Authorization: Bearer <access-token>"
```

### Running with Authentication Disabled

For secure environments where the control plane is already protected:

```bash
# .env file
DATABASE_BACKEND=sqlite
DATABASE_URL=sqlite://./lynx.db
API_HOST=127.0.0.1
API_PORT=8080
REDIRECT_HOST=127.0.0.1
REDIRECT_PORT=3000
AUTH_MODE=none
```

With `AUTH_MODE=none` (or `DISABLE_AUTH=true` for backward compatibility), no authentication is required for management endpoints. All users are automatically granted admin privileges and URLs are associated with a legacy user account.

## Admin Management

When using `AUTH_MODE=oauth` or `AUTH_MODE=cloudflare`, you can manually promote users to admin using the CLI:

```bash
# Promote a user to admin
./lynx admin promote <user-id> <auth-method>

# Example for Cloudflare
./lynx admin promote "google-oauth2|123456" cloudflare

# List all manually promoted admins
./lynx admin list

# Demote a user from admin
./lynx admin demote <user-id> <auth-method>
```

**Note:** Admin status from OAuth/Cloudflare JWT claims takes precedence. Manual promotion only applies when the JWT doesn't grant admin status.

For detailed Cloudflare Zero Trust setup, see [Cloudflare Setup Guide](docs/CLOUDFLARE_SETUP.md).

## Deployment with Reverse Proxy

You can use a reverse proxy (like Nginx or Caddy) to expose the two servers on different domains:

### Example Nginx Configuration

```nginx
# API server (internal only, or with additional authentication)
server {
    listen 443 ssl;
    server_name api.yourdomain.com;
    
    location / {
        proxy_pass http://localhost:8080;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }
}

# Redirect server (public)
server {
    listen 443 ssl;
    server_name short.yourdomain.com;
    
    location / {
        proxy_pass http://localhost:3000;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }
}
```

## Database Migrations

The application automatically creates the necessary database tables on startup. No manual migration steps are required.

## Development

### Building for Development

```bash
cargo build
```

### Running Tests

```bash
cargo test
```

### Running with Logging

```bash
RUST_LOG=debug cargo run
```

## Security Considerations

1. **OAuth Scopes**: Delegate least-privilege scopes to Lynx API clients and rotate credentials regularly
2. **HTTPS**: Use HTTPS in production (configure via reverse proxy)
3. **Network Isolation**: Consider running the API server on a private network
4. **Rate Limiting**: Implement rate limiting at the reverse proxy level
5. **Database Credentials**: Use strong database passwords and restrict network access

## License

This project is open source and available under the MIT License.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
