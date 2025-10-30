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
OUTPUT_DIR="${3:-./benchmark-results-analytics}"
DURATION="${4:-30s}"
MODE="${5:-with-analytics}" # "with-analytics" or "without-analytics"

echo "=========================================="
echo "Lynx Analytics Performance Benchmark"
echo "=========================================="
echo "API URL: $API_URL"
echo "Redirect URL: $REDIRECT_URL"
echo "Output Directory: $OUTPUT_DIR"
echo "Test Duration: $DURATION"
echo "Mode: $MODE"
echo "=========================================="
echo ""

# Create output directory
mkdir -p "$OUTPUT_DIR"

# Generate timestamp for results
TIMESTAMP=$(date +%Y%m%d-%H%M%S)
RESULTS_FILE="$OUTPUT_DIR/analytics-benchmark-$MODE-$TIMESTAMP.txt"
JSON_FILE="$OUTPUT_DIR/analytics-benchmark-$MODE-$TIMESTAMP.json"

# Initialize JSON results
cat > "$JSON_FILE" << 'EOF'
{
  "timestamp": "TIMESTAMP_PLACEHOLDER",
  "mode": "MODE_PLACEHOLDER",
  "api_url": "API_URL_PLACEHOLDER",
  "redirect_url": "REDIRECT_URL_PLACEHOLDER",
  "tests": []
}
EOF

# Replace placeholders
sed -i "s|TIMESTAMP_PLACEHOLDER|$(date -u +%Y-%m-%dT%H:%M:%SZ)|g" "$JSON_FILE"
sed -i "s|MODE_PLACEHOLDER|$MODE|g" "$JSON_FILE"
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

print_header "Analytics Performance Benchmark - $MODE"

print_info "This benchmark suite tests the performance impact of the analytics module"
print_info "under various traffic patterns and load conditions."

print_header "PHASE 1: Single Hot URL Tests (Maximum Contention)"

print_info "Test 1.1: Peak load on single URL - 1000 concurrent connections"
print_info "This represents the highest contention scenario - all traffic to one URL"

wrk -t 8 -c 1000 -d "$DURATION" "$REDIRECT_URL/analytics-bench-1" 2>&1 | tee -a "$RESULTS_FILE"

print_info "Test 1.2: Peak load on single URL - 5000 concurrent connections"
print_info "Extreme contention test"

wrk -t 16 -c 5000 -d "$DURATION" "$REDIRECT_URL/analytics-bench-1" 2>&1 | tee -a "$RESULTS_FILE"

print_info "Test 1.3: Peak load on single URL - 10000 concurrent connections"
print_info "Maximum stress test for single URL"

wrk -t 20 -c 10000 -d "$DURATION" "$REDIRECT_URL/analytics-bench-1" 2>&1 | tee -a "$RESULTS_FILE"

print_header "PHASE 2: Distributed URL Tests (Low Contention, More Cache Misses)"

print_info "Test 2.1: Distributed load across 100 URLs - 1000 concurrent connections"
print_info "Tests cache effectiveness and memory spatial locality with diverse traffic"

# Create Lua script for random URL selection
cat > /tmp/analytics-random-redirect.lua << 'LUA_SCRIPT'
-- Generate random codes for 100 URLs
math.randomseed(os.time())

request = function()
    local code = "analytics-bench-" .. math.random(100)
    return wrk.format(nil, "/" .. code)
end
LUA_SCRIPT

# Extract host and port from URL
REDIRECT_HOST=$(echo "$REDIRECT_URL" | sed -E 's|http://([^:]+):([0-9]+)|\1|')
REDIRECT_PORT=$(echo "$REDIRECT_URL" | sed -E 's|http://([^:]+):([0-9]+)|\2|')

wrk -t 8 -c 1000 -d "$DURATION" -s /tmp/analytics-random-redirect.lua "http://${REDIRECT_HOST}:${REDIRECT_PORT}" 2>&1 | tee -a "$RESULTS_FILE"

print_info "Test 2.2: Distributed load across 500 URLs - 1000 concurrent connections"
print_info "Even more distributed traffic pattern"

cat > /tmp/analytics-random-redirect-500.lua << 'LUA_SCRIPT'
math.randomseed(os.time())

request = function()
    local code = "analytics-bench-" .. math.random(500)
    return wrk.format(nil, "/" .. code)
end
LUA_SCRIPT

wrk -t 8 -c 1000 -d "$DURATION" -s /tmp/analytics-random-redirect-500.lua "http://${REDIRECT_HOST}:${REDIRECT_PORT}" 2>&1 | tee -a "$RESULTS_FILE"

print_info "Test 2.3: Distributed load across 100 URLs - 5000 concurrent connections"
print_info "High concurrency with distributed traffic"

wrk -t 16 -c 5000 -d "$DURATION" -s /tmp/analytics-random-redirect.lua "http://${REDIRECT_HOST}:${REDIRECT_PORT}" 2>&1 | tee -a "$RESULTS_FILE"

print_header "PHASE 3: Mixed Traffic Patterns"

print_info "Test 3.1: Hot spot pattern - 80% on single URL, 20% distributed"
print_info "Simulates real-world scenario with a viral link"

cat > /tmp/analytics-hotspot.lua << 'LUA_SCRIPT'
math.randomseed(os.time())

request = function()
    local rand = math.random(100)
    if rand <= 80 then
        -- 80% traffic to hot URL
        return wrk.format(nil, "/analytics-bench-1")
    else
        -- 20% distributed across other URLs
        local code = "analytics-bench-" .. (math.random(99) + 1)
        return wrk.format(nil, "/" .. code)
    end
end
LUA_SCRIPT

wrk -t 8 -c 1000 -d "$DURATION" -s /tmp/analytics-hotspot.lua "http://${REDIRECT_HOST}:${REDIRECT_PORT}" 2>&1 | tee -a "$RESULTS_FILE"

print_info "Test 3.2: Power law distribution - Simulating realistic traffic"
print_info "Top 10 URLs get 70% of traffic, rest distributed"

cat > /tmp/analytics-powerlaw.lua << 'LUA_SCRIPT'
math.randomseed(os.time())

request = function()
    local rand = math.random(100)
    if rand <= 70 then
        -- 70% traffic to top 10 URLs
        local code = "analytics-bench-" .. math.random(10)
        return wrk.format(nil, "/" .. code)
    else
        -- 30% distributed across remaining 90 URLs
        local code = "analytics-bench-" .. (math.random(90) + 10)
        return wrk.format(nil, "/" .. code)
    end
end
LUA_SCRIPT

wrk -t 8 -c 1000 -d "$DURATION" -s /tmp/analytics-powerlaw.lua "http://${REDIRECT_HOST}:${REDIRECT_PORT}" 2>&1 | tee -a "$RESULTS_FILE"

print_header "PHASE 4: Sustained Load Tests"

print_info "Test 4.1: Sustained load - 1000 connections for extended duration"
print_info "Tests analytics aggregation and flush behavior over time"

# Use longer duration for sustained test (2x the base duration)
SUSTAINED_DURATION=$(echo "$DURATION" | sed 's/[0-9]*s/&/' | awk -F's' '{print $1*2"s"}')
if [[ ! "$SUSTAINED_DURATION" =~ ^[0-9]+s$ ]]; then
    SUSTAINED_DURATION="60s" # Fallback to 60s if calculation fails
fi

print_info "Sustained test duration: $SUSTAINED_DURATION"

wrk -t 8 -c 1000 -d "$SUSTAINED_DURATION" -s /tmp/analytics-random-redirect.lua "http://${REDIRECT_HOST}:${REDIRECT_PORT}" 2>&1 | tee -a "$RESULTS_FILE"

print_header "PHASE 5: API Analytics Endpoint Tests"

print_info "Test 5.1: GET analytics data - Testing analytics retrieval performance"

# Test analytics API endpoints if they exist
wrk -t 4 -c 100 -d "$DURATION" "$API_URL/api/analytics/urls/analytics-bench-1" 2>&1 | tee -a "$RESULTS_FILE" || print_warning "Analytics API endpoint may not be available"

print_header "Summary"

print_success "Analytics benchmark suite completed!"
print_info "Results saved to: $RESULTS_FILE"
print_info "JSON results saved to: $JSON_FILE"

echo "" | tee -a "$RESULTS_FILE"
echo "=========================================" | tee -a "$RESULTS_FILE"
echo "Analytics Performance Analysis ($MODE):" | tee -a "$RESULTS_FILE"
echo "=========================================" | tee -a "$RESULTS_FILE"
echo "1. Single Hot URL Performance:" | tee -a "$RESULTS_FILE"
echo "   - Measures impact of analytics on highly contended URLs" | tee -a "$RESULTS_FILE"
echo "   - Tests actor pattern effectiveness with analytics" | tee -a "$RESULTS_FILE"
echo "" | tee -a "$RESULTS_FILE"
echo "2. Distributed Traffic Performance:" | tee -a "$RESULTS_FILE"
echo "   - Measures cache miss impact with analytics enabled" | tee -a "$RESULTS_FILE"
echo "   - Tests memory and I/O patterns with diverse traffic" | tee -a "$RESULTS_FILE"
echo "" | tee -a "$RESULTS_FILE"
echo "3. Realistic Traffic Patterns:" | tee -a "$RESULTS_FILE"
echo "   - Hot spot and power law distributions" | tee -a "$RESULTS_FILE"
echo "   - Simulates real-world usage scenarios" | tee -a "$RESULTS_FILE"
echo "" | tee -a "$RESULTS_FILE"
echo "4. Expected Impact:" | tee -a "$RESULTS_FILE"
echo "   - Analytics adds GeoIP lookup and aggregation overhead" | tee -a "$RESULTS_FILE"
echo "   - Impact should be minimal due to async processing" | tee -a "$RESULTS_FILE"
echo "   - Buffered aggregation prevents database bottleneck" | tee -a "$RESULTS_FILE"
echo "=========================================" | tee -a "$RESULTS_FILE"

print_success "Benchmark complete! Check $OUTPUT_DIR for detailed results."
