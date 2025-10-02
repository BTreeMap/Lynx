# Lynx - URL Shortener

Lynx is a URL shortener backend API written in Rust with support for multiple storage backends (SQLite and PostgreSQL), access control, and separate API/client-facing servers.

## Features

- 🔗 **URL Shortening**: Create short codes for long URLs with optional custom codes
- 🗄️ **Extensible Storage**: Support for both SQLite and PostgreSQL backends
- 🔐 **Access Control**: Optional API key-based authentication for management operations
- 🚀 **Dual Server Architecture**: Separate ports for API management and client redirects
- 📊 **Analytics**: Track click counts for each shortened URL
- 🔒 **Immutable URLs**: URLs are immutable and can only be deactivated, not deleted or modified
- 🔄 **Deactivation**: URLs can be deactivated and reactivated (e.g., for policy violations)

## Architecture

Lynx runs two separate HTTP servers:

1. **API Server** (default: port 8080): For creating and managing shortened URLs
   - Optional API key authentication (can be disabled)
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

## Installation

### Prerequisites

- Rust 1.70+ (install from [rustup.rs](https://rustup.rs))
- For PostgreSQL: A running PostgreSQL instance

### Building

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
- `API_HOST`: API server host (default: `127.0.0.1`)
- `API_PORT`: API server port (default: `8080`)
- `REDIRECT_HOST`: Redirect server host (default: `127.0.0.1`)
- `REDIRECT_PORT`: Redirect server port (default: `3000`)
- `DISABLE_AUTH`: Set to `true` to completely disable authentication (default: `false`)
- `API_KEYS`: Comma-separated list of API keys (leave empty for development mode)

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

All endpoints require an `X-API-Key` header (unless authentication is disabled via `DISABLE_AUTH=true` or running in development mode with no API keys configured).

#### Health Check
```bash
GET /health
```

#### Create Shortened URL
```bash
POST /urls
Content-Type: application/json
X-API-Key: your-api-key

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
X-API-Key: your-api-key

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
X-API-Key: your-api-key

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
X-API-Key: your-api-key

Response: 200 OK
{
  "message": "URL reactivated successfully"
}
```

#### List URLs
```bash
GET /urls?limit=50&offset=0
X-API-Key: your-api-key

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
API_KEYS=dev-key-123
```

### Using PostgreSQL (Production)

```bash
# .env file
DATABASE_BACKEND=postgres
DATABASE_URL=postgresql://lynx_user:secure_password@localhost/lynx
API_HOST=0.0.0.0
API_PORT=8080
REDIRECT_HOST=0.0.0.0
REDIRECT_PORT=3000
API_KEYS=prod-key-1,prod-key-2,prod-key-3
```

### Example API Calls

```bash
# Create a shortened URL
curl -X POST http://localhost:8080/urls \
  -H "Content-Type: application/json" \
  -H "X-API-Key: dev-key-123" \
  -d '{
    "url": "https://github.com/BTreeMap/Lynx",
    "custom_code": "lynx"
  }'

# Access the shortened URL (will redirect)
curl -L http://localhost:3000/lynx

# Get URL statistics
curl http://localhost:8080/urls/lynx \
  -H "X-API-Key: dev-key-123"

# List all URLs
curl http://localhost:8080/urls?limit=10 \
  -H "X-API-Key: dev-key-123"

# Deactivate a URL
curl -X PUT http://localhost:8080/urls/lynx/deactivate \
  -H "Content-Type: application/json" \
  -H "X-API-Key: dev-key-123" \
  -d '{}'

# Reactivate a URL
curl -X PUT http://localhost:8080/urls/lynx/reactivate \
  -H "X-API-Key: dev-key-123"
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
DISABLE_AUTH=true
```

With `DISABLE_AUTH=true`, no API key is required for any endpoint.

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

1. **API Keys**: Always use strong, randomly generated API keys in production
2. **HTTPS**: Use HTTPS in production (configure via reverse proxy)
3. **Network Isolation**: Consider running the API server on a private network
4. **Rate Limiting**: Implement rate limiting at the reverse proxy level
5. **Database Credentials**: Use strong database passwords and restrict network access

## License

This project is open source and available under the MIT License.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
