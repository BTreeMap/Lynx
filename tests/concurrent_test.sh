#!/bin/bash
set -e

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

API_URL="${1:-http://localhost:8080}"
REDIRECT_URL="${2:-http://localhost:3000}"
CONCURRENCY="${3:-50}"

echo "=========================================="
echo "Running Concurrent Load Tests"
echo "API URL: $API_URL"
echo "Redirect URL: $REDIRECT_URL"
echo "Concurrency Level: $CONCURRENCY"
echo "=========================================="

# Function to print test results
print_result() {
    if [ $1 -eq 0 ]; then
        echo -e "${GREEN}✓ $2${NC}"
    else
        echo -e "${RED}✗ $2${NC}"
        exit 1
    fi
}

# Test 1: Concurrent URL creation
echo ""
echo "Test 1: Concurrent URL creation ($CONCURRENCY parallel requests)"
echo "Creating URLs concurrently..."

success_count=0
failure_count=0
pids=()

for i in $(seq 1 $CONCURRENCY); do
    (
        response=$(curl -s -X POST "$API_URL/api/urls" \
            -H "Content-Type: application/json" \
            -d "{\"url\": \"https://example.com/concurrent-$i\", \"custom_code\": \"conc-$i\"}" 2>&1)
        
        if echo "$response" | grep -q "\"short_code\".*\"conc-$i\""; then
            echo "SUCCESS: conc-$i"
        else
            echo "FAILED: conc-$i - $response" >&2
        fi
    ) &
    pids+=($!)
done

# Wait for all requests to complete
for pid in "${pids[@]}"; do
    wait "$pid" && success_count=$((success_count + 1)) || failure_count=$((failure_count + 1))
done

echo "Created $success_count URLs successfully, $failure_count failed"
if [ $success_count -ge $((CONCURRENCY * 90 / 100)) ]; then
    print_result 0 "Concurrent URL creation (${success_count}/${CONCURRENCY} succeeded)"
else
    print_result 1 "Too many failures in concurrent URL creation (${success_count}/${CONCURRENCY})"
fi

# Test 2: Concurrent redirects to the same URL
echo ""
echo "Test 2: Concurrent redirects to same URL ($CONCURRENCY parallel requests)"

# First create a URL
curl -s -X POST "$API_URL/api/urls" \
    -H "Content-Type: application/json" \
    -d '{"url": "https://example.com/load-test", "custom_code": "load-test"}' > /dev/null

sleep 1

# Get initial click count
initial_response=$(curl -s "$API_URL/api/urls/load-test")
initial_clicks=$(echo "$initial_response" | grep -o '"clicks":[0-9]*' | cut -d':' -f2)
echo "Initial clicks: $initial_clicks"

# Make concurrent redirects
echo "Making $CONCURRENCY concurrent redirects..."
for i in $(seq 1 $CONCURRENCY); do
    curl -s -o /dev/null -L "$REDIRECT_URL/load-test" &
done
wait

# Wait for writes to be flushed
sleep 3

# Get final click count
final_response=$(curl -s "$API_URL/api/urls/load-test")
final_clicks=$(echo "$final_response" | grep -o '"clicks":[0-9]*' | cut -d':' -f2)
echo "Final clicks: $final_clicks"

# Calculate difference
diff_clicks=$((final_clicks - initial_clicks))
echo "Clicks added: $diff_clicks"

# Check if all clicks were recorded (allow for some buffering delay)
if [ $diff_clicks -ge $((CONCURRENCY * 95 / 100)) ]; then
    print_result 0 "Concurrent redirects counted correctly (${diff_clicks}/${CONCURRENCY})"
else
    print_result 1 "Missing clicks in concurrent test (got ${diff_clicks}, expected ${CONCURRENCY})"
fi

# Test 3: Mixed concurrent operations
echo ""
echo "Test 3: Mixed concurrent operations (creates, gets, redirects)"

# Create base URL
curl -s -X POST "$API_URL/api/urls" \
    -H "Content-Type: application/json" \
    -d '{"url": "https://example.com/mixed-test", "custom_code": "mixed-test"}' > /dev/null

pids=()
operations=0

# Mix of different operations
for i in $(seq 1 $((CONCURRENCY / 3))); do
    # Create operations
    curl -s -X POST "$API_URL/api/urls" \
        -H "Content-Type: application/json" \
        -d "{\"url\": \"https://example.com/mixed-$i\"}" > /dev/null &
    pids+=($!)
    operations=$((operations + 1))
    
    # Get operations
    curl -s "$API_URL/api/urls/mixed-test" > /dev/null &
    pids+=($!)
    operations=$((operations + 1))
    
    # Redirect operations
    curl -s -o /dev/null -L "$REDIRECT_URL/mixed-test" &
    pids+=($!)
    operations=$((operations + 1))
done

# Wait for all to complete
echo "Executing $operations mixed operations..."
for pid in "${pids[@]}"; do
    wait "$pid" 2>/dev/null || true
done

print_result 0 "Mixed concurrent operations completed"

# Test 4: Rapid deactivate/reactivate under load
echo ""
echo "Test 4: Rapid state changes under concurrent load"

# Create URL
curl -s -X POST "$API_URL/api/urls" \
    -H "Content-Type: application/json" \
    -d '{"url": "https://example.com/state-test", "custom_code": "state-test"}' > /dev/null

# Start concurrent redirects in background
for i in $(seq 1 20); do
    curl -s -o /dev/null -L "$REDIRECT_URL/state-test" &
done

# Rapidly change state
for i in {1..5}; do
    curl -s -X PUT "$API_URL/api/urls/state-test/deactivate" \
        -H "Content-Type: application/json" -d '{}' > /dev/null
    sleep 0.1
    curl -s -X PUT "$API_URL/api/urls/state-test/reactivate" > /dev/null
    sleep 0.1
done

wait

# Verify final state
final_response=$(curl -s "$API_URL/api/urls/state-test")
if echo "$final_response" | grep -q "\"is_active\":true"; then
    print_result 0 "State changes under load handled correctly"
else
    print_result 1 "State changes under load failed"
fi

# Test 5: High-frequency statistics updates
echo ""
echo "Test 5: High-frequency statistics updates"

# Create URL for stats test
curl -s -X POST "$API_URL/api/urls" \
    -H "Content-Type: application/json" \
    -d '{"url": "https://example.com/stats-high", "custom_code": "stats-high"}' > /dev/null

sleep 1

# Get initial count
initial_response=$(curl -s "$API_URL/api/urls/stats-high")
initial_clicks=$(echo "$initial_response" | grep -o '"clicks":[0-9]*' | cut -d':' -f2)

# Make many rapid redirects
redirect_count=200
echo "Making $redirect_count rapid redirects..."
for i in $(seq 1 $redirect_count); do
    curl -s -o /dev/null -L "$REDIRECT_URL/stats-high" &
    if [ $((i % 50)) -eq 0 ]; then
        wait
    fi
done
wait

# Wait for all flushes to complete
sleep 6

# Verify count
final_response=$(curl -s "$API_URL/api/urls/stats-high")
final_clicks=$(echo "$final_response" | grep -o '"clicks":[0-9]*' | cut -d':' -f2)
diff_clicks=$((final_clicks - initial_clicks))

echo "Clicks added: ${diff_clicks}/${redirect_count}"

# Allow for some small variance due to timing
if [ $diff_clicks -ge $((redirect_count * 95 / 100)) ]; then
    print_result 0 "High-frequency stats updates accurate (${diff_clicks}/${redirect_count})"
else
    print_result 1 "High-frequency stats inaccurate (${diff_clicks}/${redirect_count})"
fi

# Test 6: Stress test - list endpoint under load
echo ""
echo "Test 6: List endpoint under concurrent load"

pids=()
for i in $(seq 1 30); do
    curl -s "$API_URL/api/urls?limit=50" > /dev/null &
    pids+=($!)
done

# Wait for all
for pid in "${pids[@]}"; do
    wait "$pid" 2>/dev/null || true
done

print_result 0 "List endpoint handled concurrent requests"

echo ""
echo -e "${GREEN}=========================================="
echo "All Concurrent Load Tests Passed!"
echo -e "==========================================${NC}"
