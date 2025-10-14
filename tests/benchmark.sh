#!/bin/bash
set -e

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Configuration
API_URL="${1:-http://localhost:8080}"
REDIRECT_URL="${2:-http://localhost:3000}"
OUTPUT_DIR="${3:-./benchmark-results}"
DURATION="${4:-30s}"

echo "=========================================="
echo "Lynx Performance Benchmark Suite"
echo "=========================================="
echo "API URL: $API_URL"
echo "Redirect URL: $REDIRECT_URL"
echo "Output Directory: $OUTPUT_DIR"
echo "Test Duration: $DURATION"
echo "=========================================="
echo ""

# Create output directory
mkdir -p "$OUTPUT_DIR"

# Generate timestamp for results
TIMESTAMP=$(date +%Y%m%d-%H%M%S)
RESULTS_FILE="$OUTPUT_DIR/benchmark-results-$TIMESTAMP.txt"
JSON_FILE="$OUTPUT_DIR/benchmark-results-$TIMESTAMP.json"

# Initialize JSON results
cat > "$JSON_FILE" << 'EOF'
{
  "timestamp": "TIMESTAMP_PLACEHOLDER",
  "api_url": "API_URL_PLACEHOLDER",
  "redirect_url": "REDIRECT_URL_PLACEHOLDER",
  "tests": []
}
EOF

# Replace placeholders
sed -i "s|TIMESTAMP_PLACEHOLDER|$(date -u +%Y-%m-%dT%H:%M:%SZ)|g" "$JSON_FILE"
sed -i "s|API_URL_PLACEHOLDER|$API_URL|g" "$JSON_FILE"
sed -i "s|REDIRECT_URL_PLACEHOLDER|$REDIRECT_URL|g" "$JSON_FILE"

# Function to print section header
print_header() {
    echo "" | tee -a "$RESULTS_FILE"
    echo -e "${CYAN}=========================================${NC}" | tee -a "$RESULTS_FILE"
    echo -e "${CYAN}$1${NC}" | tee -a "$RESULTS_FILE"
    echo -e "${CYAN}=========================================${NC}" | tee -a "$RESULTS_FILE"
    echo "" | tee -a "$RESULTS_FILE"
}

# Function to print test info
print_info() {
    echo -e "${BLUE}[INFO]${NC} $1" | tee -a "$RESULTS_FILE"
}

# Function to print success
print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1" | tee -a "$RESULTS_FILE"
}

# Function to print warning
print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1" | tee -a "$RESULTS_FILE"
}

# Function to print error
print_error() {
    echo -e "${RED}[ERROR]${NC} $1" | tee -a "$RESULTS_FILE"
}

# Function to add result to JSON
add_json_result() {
    local test_name="$1"
    local rps="$2"
    local avg_latency="$3"
    local p50="$4"
    local p90="$5"
    local p99="$6"
    local errors="$7"
    
    # Use jq if available, otherwise use sed
    if command -v jq &> /dev/null; then
        local temp_file=$(mktemp)
        jq --arg name "$test_name" \
           --arg rps "$rps" \
           --arg avg "$avg_latency" \
           --arg p50 "$p50" \
           --arg p90 "$p90" \
           --arg p99 "$p99" \
           --arg errors "$errors" \
           '.tests += [{
               "name": $name,
               "requests_per_second": $rps,
               "avg_latency_ms": $avg,
               "p50_latency_ms": $p50,
               "p90_latency_ms": $p90,
               "p99_latency_ms": $p99,
               "errors": $errors
           }]' "$JSON_FILE" > "$temp_file"
        mv "$temp_file" "$JSON_FILE"
    fi
}

# Check if wrk is available
if ! command -v wrk &> /dev/null; then
    print_error "wrk is not installed. Attempting to install..."
    
    # Try to install wrk
    if command -v apt-get &> /dev/null; then
        sudo apt-get update
        sudo apt-get install -y build-essential libssl-dev git
        git clone https://github.com/wg/wrk.git /tmp/wrk
        cd /tmp/wrk
        make
        sudo cp wrk /usr/local/bin/
        cd -
        print_success "wrk installed successfully"
    else
        print_error "Cannot install wrk automatically. Please install manually."
        exit 1
    fi
fi

# Check if ab (Apache Bench) is available as fallback
if ! command -v ab &> /dev/null; then
    print_warning "Apache Bench (ab) is not installed. Some tests may be skipped."
fi

print_header "PHASE 0: Setup Test Data"

# Create test URLs for benchmarking
print_info "Creating test URLs for benchmarking..."

# Create 100 test URLs
for i in $(seq 1 100); do
    curl -s -X POST "$API_URL/api/urls" \
        -H "Content-Type: application/json" \
        -d "{\"url\": \"https://example.com/benchmark-target-$i\", \"custom_code\": \"bench-$i\"}" > /dev/null 2>&1
done

print_success "Created 100 test URLs (bench-1 to bench-100)"

# Create a file with random short codes for testing
CODES_FILE="$OUTPUT_DIR/test-codes.txt"
for i in $(seq 1 100); do
    echo "bench-$i" >> "$CODES_FILE"
done

print_success "Test data setup complete"

print_header "PHASE 1: Redirect Endpoint Benchmarks (Primary Focus)"

print_info "Testing redirect endpoint with caching optimizations..."
print_info "This is the most critical endpoint - all effort has been put into caching here"

# Test 1.1: Single hot URL (best case for cache)
print_info "Test 1.1: Single hot URL - maximum cache effectiveness"
print_info "Running wrk with 1000 concurrent connections for $DURATION..."

wrk -t 8 -c 1000 -d "$DURATION" "$REDIRECT_URL/bench-1" 2>&1 | tee -a "$RESULTS_FILE"

# Test 1.2: Single hot URL with 5000 concurrency
print_info "Test 1.2: Single hot URL - 5000 concurrent connections"
print_info "Expected: slightly lower performance than 1000 concurrency"

wrk -t 16 -c 5000 -d "$DURATION" "$REDIRECT_URL/bench-1" 2>&1 | tee -a "$RESULTS_FILE"

# Test 1.3: Multiple URLs (cache distribution)
print_info "Test 1.3: Random URLs from pool of 100"
print_info "Testing cache effectiveness with distributed load"

# Create Lua script for random URL selection
cat > /tmp/random-redirect.lua << 'LUA_SCRIPT'
-- Load codes from file
codes = {}
local file = io.open("CODES_FILE_PATH", "r")
if file then
    for line in file:lines() do
        table.insert(codes, line)
    end
    file:close()
end

math.randomseed(os.time())

request = function()
    local code = codes[math.random(#codes)]
    return wrk.format(nil, "/REDIRECT_PATH/" .. code)
end
LUA_SCRIPT

sed -i "s|CODES_FILE_PATH|$CODES_FILE|g" /tmp/random-redirect.lua
sed -i "s|REDIRECT_PATH||g" /tmp/random-redirect.lua

# Extract host and port from URL
REDIRECT_HOST=$(echo "$REDIRECT_URL" | sed -E 's|http://([^:]+):([0-9]+)|\1|')
REDIRECT_PORT=$(echo "$REDIRECT_URL" | sed -E 's|http://([^:]+):([0-9]+)|\2|')

wrk -t 8 -c 1000 -d "$DURATION" -s /tmp/random-redirect.lua "http://${REDIRECT_HOST}:${REDIRECT_PORT}" 2>&1 | tee -a "$RESULTS_FILE"

# Test 1.4: Very hot URL stress test
print_info "Test 1.4: Extreme load on single URL (10000 connections)"
print_info "This tests the actor pattern and zero-lock-contention design"

wrk -t 20 -c 10000 -d "$DURATION" "$REDIRECT_URL/bench-1" 2>&1 | tee -a "$RESULTS_FILE"

print_header "PHASE 2: Management Endpoints Benchmarks"

print_info "Testing management endpoints (expected to be slower - involve database queries)"

# Test 2.1: Create URL (POST)
print_info "Test 2.1: POST /api/urls - Create new short URLs"

# Create Lua script for POST requests
cat > /tmp/create-url.lua << 'LUA_SCRIPT'
wrk.method = "POST"
wrk.headers["Content-Type"] = "application/json"

counter = 0

request = function()
    counter = counter + 1
    local body = string.format('{"url": "https://example.com/bench-post-%d"}', counter)
    return wrk.format("POST", "/api/urls", wrk.headers, body)
end
LUA_SCRIPT

API_HOST=$(echo "$API_URL" | sed -E 's|http://([^:]+):([0-9]+)|\1|')
API_PORT=$(echo "$API_URL" | sed -E 's|http://([^:]+):([0-9]+)|\2|')

wrk -t 4 -c 100 -d "$DURATION" -s /tmp/create-url.lua "http://${API_HOST}:${API_PORT}" 2>&1 | tee -a "$RESULTS_FILE"

# Test 2.2: Get URL details (GET single)
print_info "Test 2.2: GET /api/urls/:code - Retrieve URL details"

wrk -t 8 -c 500 -d "$DURATION" "$API_URL/api/urls/bench-1" 2>&1 | tee -a "$RESULTS_FILE"

# Test 2.3: List URLs (GET list)
print_info "Test 2.3: GET /api/urls - List all URLs (paginated)"

wrk -t 4 -c 100 -d "$DURATION" "$API_URL/api/urls?limit=50" 2>&1 | tee -a "$RESULTS_FILE"

# Test 2.4: Update URL (PUT)
print_info "Test 2.4: PUT /api/urls/:code/deactivate - Deactivate URL"

# Create Lua script for PUT requests
cat > /tmp/deactivate-url.lua << 'LUA_SCRIPT'
wrk.method = "PUT"
wrk.headers["Content-Type"] = "application/json"

counter = 0

request = function()
    counter = counter + 1
    local code = "bench-" .. (counter % 50 + 1)  -- Use first 50 codes
    return wrk.format("PUT", "/api/urls/" .. code .. "/deactivate")
end
LUA_SCRIPT

wrk -t 4 -c 100 -d "$DURATION" -s /tmp/deactivate-url.lua "http://${API_HOST}:${API_PORT}" 2>&1 | tee -a "$RESULTS_FILE"

# Test 2.5: Health check
print_info "Test 2.5: GET /api/health - Health check endpoint"

wrk -t 8 -c 1000 -d "$DURATION" "$API_URL/api/health" 2>&1 | tee -a "$RESULTS_FILE"

print_header "PHASE 3: Mixed Workload Benchmarks"

print_info "Test 3.1: Mixed workload - 80% redirects, 15% reads, 5% writes"

# Create Lua script for mixed workload
cat > /tmp/mixed-workload.lua << 'LUA_SCRIPT'
-- Load test codes
codes = {}
local file = io.open("CODES_FILE_PATH", "r")
if file then
    for line in file:lines() do
        table.insert(codes, line)
    end
    file:close()
end

math.randomseed(os.time())
counter = 0

request = function()
    counter = counter + 1
    local rand = math.random(100)
    
    if rand <= 80 then
        -- 80% redirects (PORT 3000)
        local code = codes[math.random(#codes)]
        return wrk.format(nil, "http://REDIRECT_HOST:REDIRECT_PORT/" .. code)
    elseif rand <= 95 then
        -- 15% reads (PORT 8080)
        local code = codes[math.random(#codes)]
        return wrk.format(nil, "http://API_HOST:API_PORT/api/urls/" .. code)
    else
        -- 5% writes (PORT 8080)
        wrk.method = "POST"
        wrk.headers["Content-Type"] = "application/json"
        local body = string.format('{"url": "https://example.com/mixed-%d"}', counter)
        return wrk.format("POST", "http://API_HOST:API_PORT/api/urls", wrk.headers, body)
    end
end
LUA_SCRIPT

# Note: Mixed workload testing is complex with wrk as it can't easily hit multiple ports
# This test would need a more sophisticated tool or proxy setup
print_warning "Mixed workload test requires advanced setup - skipping for now"

print_header "PHASE 4: Summary and Analysis"

print_success "Benchmark suite completed!"
print_info "Results saved to: $RESULTS_FILE"
print_info "JSON results saved to: $JSON_FILE"

echo "" | tee -a "$RESULTS_FILE"
echo "=========================================" | tee -a "$RESULTS_FILE"
echo "Key Findings:" | tee -a "$RESULTS_FILE"
echo "=========================================" | tee -a "$RESULTS_FILE"
echo "1. Redirect endpoint performance (cached):" | tee -a "$RESULTS_FILE"
echo "   - Check results for 1000 concurrency test" | tee -a "$RESULTS_FILE"
echo "   - Compare with 5000 concurrency test" | tee -a "$RESULTS_FILE"
echo "   - Expected: ~70k RPS with slight drop at higher concurrency" | tee -a "$RESULTS_FILE"
echo "" | tee -a "$RESULTS_FILE"
echo "2. Management endpoints:" | tee -a "$RESULTS_FILE"
echo "   - Expected to be slower (database queries)" | tee -a "$RESULTS_FILE"
echo "   - POST requests: Creates + DB writes" | tee -a "$RESULTS_FILE"
echo "   - GET requests: May benefit from caching" | tee -a "$RESULTS_FILE"
echo "   - PUT requests: State changes + cache invalidation" | tee -a "$RESULTS_FILE"
echo "" | tee -a "$RESULTS_FILE"
echo "3. Cache effectiveness:" | tee -a "$RESULTS_FILE"
echo "   - Single hot URL: Maximum cache hit rate" | tee -a "$RESULTS_FILE"
echo "   - Distributed load: Cache effectiveness with 100 URLs" | tee -a "$RESULTS_FILE"
echo "=========================================" | tee -a "$RESULTS_FILE"

print_success "Benchmark complete! Check $OUTPUT_DIR for detailed results."
