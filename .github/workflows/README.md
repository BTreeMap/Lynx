# GitHub Actions Workflows

This repository includes GitHub Actions workflows for automated building and publishing of binaries and Docker containers.

## Workflows

### 1. Docker Image Publishing (`docker-publish.yml`)

**Triggers:**
- Push to `main` branch
- Scheduled builds (twice daily at 4:00 AM and 4:00 PM UTC)

**What it does:**
- Builds Docker images for `linux/amd64` and `linux/arm64` platforms
- Publishes images to GitHub Container Registry (GHCR)
- Creates multi-platform manifests with multiple tags:
  - `latest` - Latest main branch build
  - `YYYY-MM-DD` - Build date
  - `YYYY-MM-DD.HH-MM-SS` - Build date and time
  - `<commit-sha>` - Git commit SHA
  - Combined tags with SHA and date

**Usage:**
```bash
# Pull the latest main branch image
docker pull ghcr.io/btreemap/lynx:latest

# Pull a specific commit
docker pull ghcr.io/btreemap/lynx:<commit-sha>

# Run the container
docker run -p 8080:8080 -p 3000:3000 ghcr.io/btreemap/lynx:latest
```

### 2. Binary Artifacts (`build-binaries.yml`)

**Triggers:**
- Push to `main` branch

**What it does:**
- Builds release binaries for multiple platforms:
  - Linux (amd64, arm64)
  - macOS (amd64, arm64)
  - Windows (amd64)
- Uploads binaries as GitHub Actions artifacts
- Artifacts are retained for 30 days

**Supported Platforms:**
- `lynx-linux-amd64` - Linux x86_64
- `lynx-linux-arm64` - Linux ARM64
- `lynx-macos-amd64` - macOS x86_64 (Intel)
- `lynx-macos-arm64` - macOS ARM64 (Apple Silicon)
- `lynx-windows-amd64.exe` - Windows x86_64

### 3. Release Publishing (`release.yml`)

**Triggers:**
- When a GitHub release is published

**What it does:**
- Builds release binaries for all supported platforms
- Attaches binaries to the GitHub release as downloadable assets
- Builds and publishes Docker images tagged with the release version
- Creates multi-platform Docker manifests

**Usage:**
To create a release:
1. Create a new tag: `git tag v1.0.0`
2. Push the tag: `git push origin v1.0.0`
3. Create a GitHub release from the tag
4. The workflow will automatically build and attach binaries

**Download binaries from releases:**
```bash
# Example: Download v1.0.0 for Linux amd64
wget https://github.com/BTreeMap/Lynx/releases/download/v1.0.0/lynx-linux-amd64
chmod +x lynx-linux-amd64
./lynx-linux-amd64
```

**Pull release Docker images:**
```bash
# Pull a specific release
docker pull ghcr.io/btreemap/lynx:v1.0.0
```

## Docker Image Details

The Docker images are based on Debian slim and include:
- Pre-built Lynx binary
- Runtime dependencies (ca-certificates, libssl3)
- Non-root user (`lynx`)
- Default SQLite database configuration
- Exposed ports: 8080 (API), 3000 (Redirect)

### Environment Variables

The Docker images support the following environment variables (see `.env.example` for more):

- `DATABASE_BACKEND` - Database type (`sqlite` or `postgres`)
- `DATABASE_URL` - Database connection string
- `API_HOST` - API server host (default: `0.0.0.0`)
- `API_PORT` - API server port (default: `8080`)
- `REDIRECT_HOST` - Redirect server host (default: `0.0.0.0`)
- `REDIRECT_PORT` - Redirect server port (default: `3000`)
- `AUTH_MODE` - Authentication mode (`none`, `oauth`, or `cloudflare`)

## Development

To build locally:

```bash
# Build binary
cargo build --release

# Build Docker image
docker build -t lynx:local .
```
