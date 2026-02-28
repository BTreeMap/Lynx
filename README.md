# Lynx URL Shortener

A high-performance URL shortener written in Rust with dual-server architecture, multi-backend storage support, and enterprise authentication.

## Features

- **URL Shortening**: Create short codes for long URLs with optional custom codes
- **Extensible Storage**: SQLite and PostgreSQL backends with automatic schema initialization
- **Access Control**: OAuth 2.0 and Cloudflare Zero Trust authentication with pass-through mode
- **Dual Server Architecture**: Separate API server (management) and redirect server (public-facing)
- **Analytics**: Click tracking with optional GeoIP-based visitor analytics ([Analytics Guide](docs/ANALYTICS.md))
- **Immutable URLs**: URLs can be deactivated/reactivated but not deleted or modified
- **Delete Protection**: Database-level triggers prevent accidental deletion ([Delete Protection](docs/DELETE_PROTECTION.md))
- **Multi-User Support**: User-specific link management with admin roles
- **Web Frontend**: React-based dashboard bundled into the binary
- **High Performance**: In-memory caching and write buffering ([Performance Optimizations](docs/PERFORMANCE_OPTIMIZATIONS.md))

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

**API Server** (default: port 8080)
- Serves the bundled React frontend at `/`
- API endpoints at `/api/...`
- Optional authentication (OAuth 2.0, Cloudflare Zero Trust, or disabled)
- Create URLs with auto-generated or custom codes
- Deactivate/reactivate URLs
- List and search capabilities

**Redirect Server** (default: port 3000)
- Public-facing URL redirects
- No authentication required
- Fast redirects with click tracking
- Handles deactivated links with appropriate HTTP status codes

This separation allows you to expose the redirect server publicly while keeping the API server internal, apply different security policies, and use separate domains via reverse proxy.

## Quick Start

### Docker (Recommended)

**For production deployments, use the stable release tag:**

```bash
# Pull the latest stable release
docker pull ghcr.io/btreemap/lynx:stable

# Run with SQLite (simplest setup)
docker run -d \
  -p 8080:8080 \
  -p 3000:3000 \
  -v $(pwd)/data:/data \
  -e DATABASE_BACKEND=sqlite \
  -e DATABASE_URL=sqlite:///data/lynx.db \
  -e AUTH_MODE=none \
  ghcr.io/btreemap/lynx:stable
```

**For testing unreleased features, use the latest development tag:**

```bash
# Pull the latest main branch build (updated on every commit, may be unstable)
docker pull ghcr.io/btreemap/lynx:latest
```

**Available Docker tags:**
- `:stable` - Latest stable release (recommended for production)
- `:latest` - Latest main branch build (unstable, for testing)
- `:v1.0.0` - Specific version tag

Access the web UI at `http://localhost:8080` and test a redirect:

```bash
# Create a short URL (no auth required with AUTH_MODE=none)
curl -X POST http://localhost:8080/api/urls \
  -H "Content-Type: application/json" \
  -d '{"url": "https://github.com/BTreeMap/Lynx", "custom_code": "gh"}'

# Access the shortened URL
curl -L http://localhost:3000/gh
```

### Pre-built Binaries

Download from [GitHub Releases](https://github.com/BTreeMap/Lynx/releases) for Linux, macOS, or Windows:

```bash
# Example: Linux
wget https://github.com/BTreeMap/Lynx/releases/download/v1.0.0/lynx-linux-amd64
chmod +x lynx-linux-amd64
./lynx-linux-amd64
```

### Building from Source

Requires Rust 1.93+ and Node.js 24+ for frontend compilation:

```bash
git clone https://github.com/BTreeMap/Lynx.git
cd Lynx
cargo build --release
./target/release/lynx
```

## Sample Deployments

### Development (SQLite, No Auth)

Simplest setup for local development:

```bash
docker run -d \
  -p 8080:8080 \
  -p 3000:3000 \
  -e DATABASE_BACKEND=sqlite \
  -e DATABASE_URL=sqlite:///tmp/lynx.db \
  -e AUTH_MODE=none \
  ghcr.io/btreemap/lynx:stable
```

Or with environment file:

```bash
# .env
DATABASE_BACKEND=sqlite
DATABASE_URL=sqlite://./lynx.db
AUTH_MODE=none
API_HOST=127.0.0.1
API_PORT=8080
REDIRECT_HOST=127.0.0.1
REDIRECT_PORT=3000
```

```bash
./lynx
```

### Production with PostgreSQL and OAuth 2.0

Enterprise deployment with OAuth authentication:

```bash
docker run -d \
  -p 8080:8080 \
  -p 3000:3000 \
  -e DATABASE_BACKEND=postgres \
  -e DATABASE_URL=postgresql://user:password@postgres:5432/lynx \
  -e AUTH_MODE=oauth \
  -e OAUTH_ISSUER_URL=https://auth.example.com/realms/lynx \
  -e OAUTH_AUDIENCE=lynx-api \
  -e API_HOST=0.0.0.0 \
  -e REDIRECT_HOST=0.0.0.0 \
  ghcr.io/btreemap/lynx:stable
```

See [OAuth Setup](#authentication) for detailed configuration.

### Production with Cloudflare Zero Trust

Use Cloudflare Access for authentication:

```bash
docker run -d \
  -p 8080:8080 \
  -p 3000:3000 \
  -e DATABASE_BACKEND=postgres \
  -e DATABASE_URL=postgresql://user:password@postgres:5432/lynx \
  -e AUTH_MODE=cloudflare \
  -e CLOUDFLARE_TEAM_DOMAIN=https://your-team.cloudflareaccess.com \
  -e CLOUDFLARE_AUDIENCE=your-aud-tag \
  -e API_HOST=0.0.0.0 \
  -e REDIRECT_HOST=0.0.0.0 \
  ghcr.io/btreemap/lynx:stable
```

See [Cloudflare Setup Guide](docs/CLOUDFLARE_SETUP.md) for complete instructions.

### Docker Compose with PostgreSQL

Complete stack with PostgreSQL:

```yaml
version: '3.8'

services:
  postgres:
    image: postgres:17-alpine
    environment:
      POSTGRES_USER: lynx
      POSTGRES_PASSWORD: secure_password
      POSTGRES_DB: lynx
    volumes:
      - postgres-data:/var/lib/postgresql/data
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U lynx"]
      interval: 10s
      timeout: 5s
      retries: 5

  lynx:
    image: ghcr.io/btreemap/lynx:stable
    ports:
      - "8080:8080"
      - "3000:3000"
    environment:
      DATABASE_BACKEND: postgres
      DATABASE_URL: postgresql://lynx:secure_password@postgres:5432/lynx
      AUTH_MODE: oauth
      OAUTH_ISSUER_URL: https://auth.example.com
      OAUTH_AUDIENCE: lynx-api
      API_HOST: 0.0.0.0
      REDIRECT_HOST: 0.0.0.0
    depends_on:
      postgres:
        condition: service_healthy

volumes:
  postgres-data:
```

## Authentication

Lynx supports three authentication modes configured via the `AUTH_MODE` environment variable.

### No Authentication (Development Only)

```bash
AUTH_MODE=none
```

All API endpoints are accessible without authentication. All users are automatically granted admin privileges. **Not recommended for production.**

### OAuth 2.0

Validates JWT Bearer tokens from any OpenID Connect provider (Keycloak, Auth0, Okta, etc.):

```bash
AUTH_MODE=oauth
OAUTH_ISSUER_URL=https://auth.example.com/realms/lynx
OAUTH_AUDIENCE=lynx-api
# Optional: OAUTH_JWKS_URL (if not using OIDC discovery)
# Optional: OAUTH_JWKS_CACHE_SECS=300
```

API clients must include a valid Bearer token:

```bash
curl -H "Authorization: Bearer <access-token>" \
  http://localhost:8080/api/urls
```

### Cloudflare Zero Trust

Validates Cloudflare Access JWT tokens when deployed behind Cloudflare Access:

```bash
AUTH_MODE=cloudflare
CLOUDFLARE_TEAM_DOMAIN=https://your-team.cloudflareaccess.com
CLOUDFLARE_AUDIENCE=your-aud-tag
# Optional: CLOUDFLARE_CERTS_CACHE_SECS=86400
```

See the [Cloudflare Setup Guide](docs/CLOUDFLARE_SETUP.md) for complete configuration including:
- Creating a Cloudflare Access application
- Obtaining your team domain and audience tag
- Configuring identity providers
- Setting up admin users

## Configuration

All configuration is done via environment variables. See `.env.example` for a complete reference.

### Core Settings

| Variable | Description | Default |
|----------|-------------|---------|
| `DATABASE_BACKEND` | Storage backend: `sqlite` or `postgres` | `sqlite` |
| `DATABASE_URL` | Database connection string | `sqlite://./lynx.db` |
| `DATABASE_MAX_CONNECTIONS` | Connection pool size | `30` |
| `API_HOST` | API server bind address | `127.0.0.1` |
| `API_PORT` | API server port | `8080` |
| `REDIRECT_HOST` | Redirect server bind address | `127.0.0.1` |
| `REDIRECT_PORT` | Redirect server port | `3000` |
| `SHORT_CODE_MAX_LENGTH` | Maximum length for custom short codes | `50` |
| `AUTH_MODE` | Authentication mode: `none`, `oauth`, or `cloudflare` | `none` |

### Performance Tuning

| Variable | Description | Default |
|----------|-------------|---------|
| `CACHE_MAX_ENTRIES` | Maximum entries in read cache | `500000` (~100MB) |
| `REDIRECT_STATUS_CODE` | HTTP status code for redirects: `301`, `302`, `303`, `307`, `308` | `308` |
| `ENABLE_TIMING_HEADERS` | Include diagnostic timing headers in redirect responses | `false` |

### Frontend

| Variable | Description |
|----------|-------------|
| `FRONTEND_STATIC_DIR` | Optional: Serve frontend from custom directory instead of embedded version |

For advanced configuration including analytics, see the [full documentation](docs/).

## Web Frontend

The React-based web frontend is bundled into the binary and served at the root path (`/`).

**Features:**
- Create short URLs with optional custom codes
- View statistics (clicks, status, creation date)
- User-specific URL filtering
- Admin panel for managing all links
- Deactivate/reactivate URLs

Access at `http://localhost:8080/` (or your configured API server address).

For custom frontend deployment or development, see the [Frontend README](frontend/README.md).

## API Reference

The API server exposes RESTful endpoints at `/api/*`. Authentication is required unless `AUTH_MODE=none`.

### Public Endpoints (no auth required)

```bash
GET  /api/health              # Health check
GET  /api/auth/mode           # Returns the configured authentication mode
```

### Protected Endpoints (auth required unless AUTH_MODE=none)

```bash
POST /api/urls                # Create short URL
GET  /api/urls                # List URLs (cursor-based pagination)
GET  /api/urls/search         # Search URLs by query string
GET  /api/urls/{code}         # Get URL details
PUT  /api/urls/{code}/deactivate   # Deactivate URL (admin only)
PUT  /api/urls/{code}/reactivate   # Reactivate URL (admin only)
GET  /api/user/info           # Get current user info
GET  /api/analytics/{code}           # Get analytics for a URL (admin only)
GET  /api/analytics/{code}/aggregate # Get aggregated analytics (admin only)
```

### Quick Examples

```bash
# Create short URL
curl -X POST http://localhost:8080/api/urls \
  -H "Content-Type: application/json" \
  -d '{"url": "https://example.com/very/long/url", "custom_code": "mycode"}'

# List URLs (cursor-based pagination, default limit=50)
curl http://localhost:8080/api/urls?limit=20

# Paginate using the next_cursor from the previous response
curl http://localhost:8080/api/urls?limit=20&cursor=<next_cursor>

# Get URL details
curl http://localhost:8080/api/urls/mycode

# Deactivate URL (admin only)
curl -X PUT http://localhost:8080/api/urls/mycode/deactivate

# Reactivate URL (admin only)
curl -X PUT http://localhost:8080/api/urls/mycode/reactivate
```

### Redirect Server

The redirect server handles public-facing redirects:

```bash
GET /{code}
→ 308 Permanent Redirect to original URL (configurable via REDIRECT_STATUS_CODE)

# Returns 410 Gone for deactivated URLs
# Returns 404 Not Found for non-existent codes
```

When `ENABLE_TIMING_HEADERS=true`, the redirect endpoint includes performance tracing headers:
- `X-Lynx-Cache-Hit`: Whether served from cache (`true`/`false`)
- `X-Lynx-Timing-Total-Ms`: Total request time in milliseconds
- `X-Lynx-Timing-Cache-Ms`: Cache lookup time
- `X-Lynx-Timing-Db-Ms`: Database lookup time (0 if cache hit)
- `X-Lynx-Timing-Handler-Ms`: Handler processing time

## Admin Management

Promote users to admin role using the CLI:

```bash
# Promote user to admin
./lynx admin promote <user-id> <auth-method>

# Example for Cloudflare
./lynx admin promote "google-oauth2|123456" cloudflare

# List admins
./lynx admin list

# Demote admin
./lynx admin demote <user-id> <auth-method>
```

Admin status from OAuth/Cloudflare JWT claims takes precedence over manual promotion.

## Deployment with Reverse Proxy

Example Nginx configuration:

```nginx
# API server (internal or with additional auth)
server {
    listen 443 ssl;
    server_name api.example.com;
    
    location / {
        proxy_pass http://localhost:8080;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }
}

# Redirect server (public)
server {
    listen 443 ssl;
    server_name short.example.com;
    
    location / {
        proxy_pass http://localhost:3000;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }
}
```

For Caddy, Apache, or advanced configurations, see the [deployment documentation](docs/).

## Development

### Running Tests

```bash
# Unit and integration tests
cargo test

# Bash integration tests (requires running service)
bash tests/integration_test.sh http://localhost:8080 http://localhost:3000

# Concurrent load tests
bash tests/concurrent_test.sh http://localhost:8080 http://localhost:3000 100
```

See [tests/README.md](tests/README.md) for comprehensive testing documentation.

### Running with Logging

```bash
RUST_LOG=debug cargo run
```

## Documentation

### Core Documentation
- [Performance Optimizations](docs/PERFORMANCE_OPTIMIZATIONS.md) - Caching strategies and actor pattern
- [Performance Benchmarks](docs/BENCHMARKS.md) - Benchmarking guide and methodology
- [Analytics Guide](docs/ANALYTICS.md) - GeoIP-based visitor analytics
- [Delete Protection](docs/DELETE_PROTECTION.md) - Database-level delete prevention
- [Cloudflare Setup Guide](docs/CLOUDFLARE_SETUP.md) - Cloudflare Zero Trust configuration

### Testing Documentation
- [Tests Overview](tests/README.md) - Integration and benchmark test documentation
- [Frontend README](frontend/README.md) - Frontend development and deployment

## Security Considerations

1. **Authentication**: Use OAuth or Cloudflare Zero Trust in production; never expose `AUTH_MODE=none` publicly
2. **HTTPS**: Always use HTTPS in production (configure via reverse proxy)
3. **Network Isolation**: Run the API server on a private network when possible
4. **Rate Limiting**: Implement rate limiting at the reverse proxy level
5. **Database Credentials**: Use strong passwords and restrict network access
6. **Token Rotation**: Rotate OAuth credentials and secrets regularly

## License

MIT License - see LICENSE file for details.

## Contributing

Contributions are welcome! Please submit a Pull Request.
