#!/bin/bash
set -e

# Analytics Performance Comparison Script
# Tests the performance impact of analytics with the actor-based implementation
# Compares: Analytics OFF vs Analytics ON (with deferred GeoIP lookups)

# Color codes
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

# Configuration
REDIRECT_URL="${1:-http://localhost:3000}"
OUTPUT_DIR="${2:-./analytics-perf-results}"
DURATION="${3:-15s}"
CONCURRENCY="${4:-1000}"

echo "==========================================="
echo "Analytics Performance Comparison Test"
echo "==========================================="
echo "Redirect URL: $REDIRECT_URL"
echo "Output Directory: $OUTPUT_DIR"
echo "Test Duration: $DURATION"
echo "Concurrency: $CONCURRENCY"
echo "==========================================="
echo ""

# Create output directory
mkdir -p "$OUTPUT_DIR"

# Generate timestamp
TIMESTAMP=$(date +%Y%m%d-%H%M%S)
RESULTS_FILE="$OUTPUT_DIR/analytics-comparison-$TIMESTAMP.txt"

echo "Testing analytics performance impact with actor-based implementation" | tee -a "$RESULTS_FILE"
echo "This test requires restarting the server with different configurations" | tee -a "$RESULTS_FILE"
echo "" | tee -a "$RESULTS_FILE"

# Check if wrk is available
if ! command -v wrk &> /dev/null; then
    echo -e "${RED}ERROR: wrk is not installed${NC}"
    echo "Please install wrk: https://github.com/wg/wrk"
    exit 1
fi

# Create test URLs if needed
echo -e "${BLUE}Creating test URLs...${NC}"
curl -s -X POST http://localhost:8080/api/urls \
    -H "Content-Type: application/json" \
    -d '{"url": "https://example.com/test1", "custom_code": "perftest1"}' > /dev/null 2>&1 || true
curl -s -X POST http://localhost:8080/api/urls \
    -H "Content-Type: application/json" \
    -d '{"url": "https://example.com/test2", "custom_code": "perftest2"}' > /dev/null 2>&1 || true
echo -e "${GREEN}Test URLs ready${NC}"
echo ""

# Test 1: Single hot URL (contention test)
echo -e "${CYAN}=========================================${NC}" | tee -a "$RESULTS_FILE"
echo -e "${CYAN}Test 1: Single Hot URL (Contention Test)${NC}" | tee -a "$RESULTS_FILE"
echo -e "${CYAN}=========================================${NC}" | tee -a "$RESULTS_FILE"
echo "" | tee -a "$RESULTS_FILE"
echo "Testing $CONCURRENCY concurrent connections hitting the same URL" | tee -a "$RESULTS_FILE"
echo "This tests the actor pattern's ability to handle hot key contention" | tee -a "$RESULTS_FILE"
echo "" | tee -a "$RESULTS_FILE"

echo -e "${YELLOW}Running benchmark...${NC}"
wrk -t8 -c${CONCURRENCY} -d${DURATION} ${REDIRECT_URL}/perftest1 2>&1 | tee -a "$RESULTS_FILE"

echo "" | tee -a "$RESULTS_FILE"

# Test 2: Distributed load across multiple URLs
echo -e "${CYAN}=========================================${NC}" | tee -a "$RESULTS_FILE"
echo -e "${CYAN}Test 2: Distributed Load (Multiple URLs)${NC}" | tee -a "$RESULTS_FILE"
echo -e "${CYAN}=========================================${NC}" | tee -a "$RESULTS_FILE"
echo "" | tee -a "$RESULTS_FILE"
echo "Testing with load distributed across multiple URLs" | tee -a "$RESULTS_FILE"
echo "" | tee -a "$RESULTS_FILE"

# Create Lua script for distributed load
cat > /tmp/distributed_load.lua << 'EOF'
math.randomseed(os.time())
request = function()
   local id = math.random(1, 2)
   return wrk.format(nil, "/perftest" .. id)
end
EOF

echo -e "${YELLOW}Running benchmark...${NC}"
wrk -t8 -c${CONCURRENCY} -d${DURATION} -s /tmp/distributed_load.lua ${REDIRECT_URL} 2>&1 | tee -a "$RESULTS_FILE"

echo "" | tee -a "$RESULTS_FILE"

# Test 3: Extreme concurrency
echo -e "${CYAN}=========================================${NC}" | tee -a "$RESULTS_FILE"
echo -e "${CYAN}Test 3: Extreme Concurrency (5000 conn)${NC}" | tee -a "$RESULTS_FILE"
echo -e "${CYAN}=========================================${NC}" | tee -a "$RESULTS_FILE"
echo "" | tee -a "$RESULTS_FILE"
echo "Testing with extreme concurrency to stress the actor pattern" | tee -a "$RESULTS_FILE"
echo "" | tee -a "$RESULTS_FILE"

echo -e "${YELLOW}Running benchmark...${NC}"
wrk -t16 -c5000 -d${DURATION} ${REDIRECT_URL}/perftest1 2>&1 | tee -a "$RESULTS_FILE"

echo "" | tee -a "$RESULTS_FILE"

# Summary
echo -e "${GREEN}=========================================${NC}" | tee -a "$RESULTS_FILE"
echo -e "${GREEN}Analytics Performance Test Complete${NC}" | tee -a "$RESULTS_FILE"
echo -e "${GREEN}=========================================${NC}" | tee -a "$RESULTS_FILE"
echo "" | tee -a "$RESULTS_FILE"
echo "Results saved to: $RESULTS_FILE" | tee -a "$RESULTS_FILE"
echo ""  | tee -a "$RESULTS_FILE"
echo "Key Metrics to Review:" | tee -a "$RESULTS_FILE"
echo "1. Requests/sec - Should be >50k with analytics enabled" | tee -a "$RESULTS_FILE"
echo "2. Latency percentiles - P99 should be <100ms under normal load" | tee -a "$RESULTS_FILE"
echo "3. Single hot URL performance - Tests actor pattern effectiveness" | tee -a "$RESULTS_FILE"
echo "" | tee -a "$RESULTS_FILE"
echo "To test without analytics, restart server with ANALYTICS_ENABLED=false" | tee -a "$RESULTS_FILE"
echo "and run this script again for comparison." | tee -a "$RESULTS_FILE"

# Cleanup
rm -f /tmp/distributed_load.lua
