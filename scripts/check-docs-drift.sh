#!/usr/bin/env bash
# Drift-prevention: checks that README.md documents all API routes and
# key environment variables that are defined in the source code.
#
# Usage:
#   bash scripts/check-docs-drift.sh
#
# Exit code 0 = all checks pass, 1 = drift detected.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
README="$REPO_ROOT/README.md"
ENV_EXAMPLE="$REPO_ROOT/.env.example"
DRIFT=0

# ── 1. API route coverage ───────────────────────────────────────────
# Extract route paths from Rust source and verify each appears in README.
echo "=== Checking API route coverage in README ==="

# Extract route strings from api/routes.rs and redirect/routes.rs
ROUTE_FILES=(
  "$REPO_ROOT/src/api/routes.rs"
  "$REPO_ROOT/src/redirect/routes.rs"
)

for rf in "${ROUTE_FILES[@]}"; do
  if [[ ! -f "$rf" ]]; then
    echo "WARN: Route file not found: $rf"
    continue
  fi
  # Match .route("/path", ...) and extract the path
  while IFS= read -r route_path; do
    # Normalize {param} to a pattern we can search for
    search_path="$route_path"
    if ! grep -qF "$search_path" "$README"; then
      # Try also with :param style (some docs use :code instead of {code})
      alt_path=$(echo "$search_path" | sed 's/{[^}]*}/:[^/]*/g')
      if ! grep -qP "$alt_path" "$README" 2>/dev/null; then
        echo "DRIFT: Route '$route_path' (from $(basename "$rf")) not found in README.md"
        DRIFT=1
      fi
    fi
  done < <(grep -oP '\.route\(\s*"([^"]+)"' "$rf" | sed 's/\.route("//' | sed 's/"//')
done

# ── 2. Environment variable coverage ────────────────────────────────
# Check that key env vars read in config/mod.rs and main.rs appear in
# README.md or .env.example.
echo "=== Checking env var coverage in README + .env.example ==="

# Collect env var names from config source and main.rs
ENV_SOURCES=(
  "$REPO_ROOT/src/config/mod.rs"
  "$REPO_ROOT/src/main.rs"
)

KNOWN_VARS=()
for src in "${ENV_SOURCES[@]}"; do
  if [[ ! -f "$src" ]]; then continue; fi
  while IFS= read -r var; do
    KNOWN_VARS+=("$var")
  done < <(grep -oP 'std::env::var\(\s*"([^"]+)"' "$src" | sed 's/std::env::var("//' | sed 's/"//')
done

# De-duplicate
UNIQUE_VARS=($(printf '%s\n' "${KNOWN_VARS[@]}" | sort -u))

for var in "${UNIQUE_VARS[@]}"; do
  found=0
  grep -qF "$var" "$README" && found=1
  if [[ $found -eq 0 ]]; then
    grep -qF "$var" "$ENV_EXAMPLE" && found=1
  fi
  if [[ $found -eq 0 ]]; then
    echo "DRIFT: Env var '$var' (from source) not found in README.md or .env.example"
    DRIFT=1
  fi
done

# ── 3. Redirect status code default ────────────────────────────────
echo "=== Checking redirect status code documentation ==="
# The default redirect code is 308 (RedirectMode::default() = Permanent).
# README must not claim 301 as the default.
if grep -qP '→\s*301\s+Redirect' "$README"; then
  echo "DRIFT: README claims default redirect is 301 but code defaults to 308"
  DRIFT=1
fi

# ── 4. Pagination parameter ────────────────────────────────────────
echo "=== Checking pagination documentation ==="
# The list endpoint uses cursor-based pagination, not offset-based.
if grep -qP 'offset=\d+' "$README"; then
  echo "DRIFT: README references offset-based pagination but API uses cursor-based"
  DRIFT=1
fi

# ── Summary ─────────────────────────────────────────────────────────
if [[ $DRIFT -eq 0 ]]; then
  echo ""
  echo "✅ All documentation drift checks passed."
  exit 0
else
  echo ""
  echo "❌ Documentation drift detected. Please update docs to match source."
  exit 1
fi
