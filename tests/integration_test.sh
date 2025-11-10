#!/bin/bash
set -e

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

API_URL="${1:-http://localhost:8080}"
REDIRECT_URL="${2:-http://localhost:3000}"

echo "=========================================="
echo "Running Comprehensive Integration Tests"
echo "API URL: $API_URL"
echo "Redirect URL: $REDIRECT_URL"
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

# Function to check JSON response contains a field
check_json_field() {
    local response="$1"
    local field="$2"
    local expected="$3"
    
    if echo "$response" | grep -q "\"$field\""; then
        if [ -n "$expected" ]; then
            if echo "$response" | grep -q "\"$field\":.*\"$expected\""; then
                return 0
            else
                echo "Field $field found but value doesn't match expected: $expected"
                return 1
            fi
        fi
        return 0
    else
        echo "Field $field not found in response: $response"
        return 1
    fi
}

# Test 1: Health Check
echo ""
echo "Test 1: Health Check"
response=$(curl -s "$API_URL/api/health")
check_json_field "$response" "message" "OK"
print_result $? "Health check endpoint"

# Test 2: Auth Mode
echo ""
echo "Test 2: Get Auth Mode"
response=$(curl -s "$API_URL/api/auth/mode")
check_json_field "$response" "mode"
print_result $? "Auth mode endpoint"

# Test 3: Create URL with custom code
echo ""
echo "Test 3: Create URL with custom code"
response=$(curl -s -X POST "$API_URL/api/urls" \
    -H "Content-Type: application/json" \
    -d '{"url": "https://github.com/BTreeMap/Lynx", "custom_code": "lynx-test"}')
check_json_field "$response" "short_code" "lynx-test"
print_result $? "Create URL with custom code"

# Test 4: Create URL with auto-generated code
echo ""
echo "Test 4: Create URL with auto-generated code"
response=$(curl -s -X POST "$API_URL/api/urls" \
    -H "Content-Type: application/json" \
    -d '{"url": "https://example.com"}')
auto_code=$(echo "$response" | grep -o '"short_code":"[^"]*"' | cut -d'"' -f4)
check_json_field "$response" "short_code"
print_result $? "Create URL with auto-generated code (code: $auto_code)"

# Test 5: Get URL details
echo ""
echo "Test 5: Get URL details"
response=$(curl -s "$API_URL/api/urls/lynx-test")
check_json_field "$response" "short_code" "lynx-test"
check_json_field "$response" "original_url"
print_result $? "Get URL details"

# Test 6: Test redirect
echo ""
echo "Test 6: Test redirect"
redirect_response=$(curl -s -o /dev/null -w "%{http_code}" -L "$REDIRECT_URL/lynx-test")
if [ "$redirect_response" = "200" ]; then
    print_result 0 "Redirect works"
else
    print_result 1 "Redirect failed (HTTP $redirect_response)"
fi

# Test 7: Verify click count increased
echo ""
echo "Test 7: Verify click count"
sleep 1  # Wait for click to be flushed
response=$(curl -s "$API_URL/api/urls/lynx-test")
clicks=$(echo "$response" | grep -o '"clicks":[0-9]*' | cut -d':' -f2)
if [ "$clicks" -ge 1 ]; then
    print_result 0 "Click count increased (clicks: $clicks)"
else
    print_result 1 "Click count not increased"
fi

# Test 8: List URLs
echo ""
echo "Test 8: List URLs"
response=$(curl -s "$API_URL/api/urls?limit=10")
if echo "$response" | grep -q "lynx-test"; then
    print_result 0 "List URLs endpoint"
else
    print_result 1 "List URLs endpoint (lynx-test not found)"
fi

# Test 9: Deactivate URL
echo ""
echo "Test 9: Deactivate URL"
response=$(curl -s -X PUT "$API_URL/api/urls/lynx-test/deactivate" \
    -H "Content-Type: application/json" \
    -d '{}')
check_json_field "$response" "message"
print_result $? "Deactivate URL"

# Test 10: Verify URL is deactivated (redirect should fail)
echo ""
echo "Test 10: Verify deactivated URL returns 410"
redirect_response=$(curl -s -o /dev/null -w "%{http_code}" -L "$REDIRECT_URL/lynx-test")
if [ "$redirect_response" = "410" ] || [ "$redirect_response" = "404" ]; then
    print_result 0 "Deactivated URL returns $redirect_response"
else
    print_result 1 "Deactivated URL should return 410 or 404 (got HTTP $redirect_response)"
fi

# Test 11: Reactivate URL
echo ""
echo "Test 11: Reactivate URL"
response=$(curl -s -X PUT "$API_URL/api/urls/lynx-test/reactivate")
check_json_field "$response" "message"
print_result $? "Reactivate URL"

# Test 12: Verify URL is reactivated
echo ""
echo "Test 12: Verify reactivated URL works"
redirect_response=$(curl -s -o /dev/null -w "%{http_code}" -L "$REDIRECT_URL/lynx-test")
if [ "$redirect_response" = "200" ]; then
    print_result 0 "Reactivated URL works"
else
    print_result 1 "Reactivated URL failed (HTTP $redirect_response)"
fi

# Test 13: Create multiple URLs rapidly
echo ""
echo "Test 13: Create multiple URLs rapidly (stress test)"
success_count=0
for i in {1..50}; do
    response=$(curl -s -X POST "$API_URL/api/urls" \
        -H "Content-Type: application/json" \
        -d "{\"url\": \"https://example.com/test$i\", \"custom_code\": \"rapid-$i\"}")
    if check_json_field "$response" "short_code" "rapid-$i" >/dev/null 2>&1; then
        success_count=$((success_count + 1))
    fi
done
if [ $success_count -eq 50 ]; then
    print_result 0 "Created 50 URLs rapidly"
else
    print_result 1 "Only created $success_count/50 URLs"
fi

# Test 14: Test concurrent redirects to same URL
echo ""
echo "Test 14: Concurrent redirects to same URL"
for i in {1..20}; do
    curl -s -o /dev/null -L "$REDIRECT_URL/rapid-1" &
done
wait
sleep 2  # Wait for clicks to be flushed

response=$(curl -s "$API_URL/api/urls/rapid-1")
clicks=$(echo "$response" | grep -o '"clicks":[0-9]*' | cut -d':' -f2)
if [ "$clicks" -ge 20 ]; then
    print_result 0 "Concurrent redirects counted correctly (clicks: $clicks)"
else
    print_result 1 "Concurrent redirects not counted correctly (expected >=20, got $clicks)"
fi

# Test 15: Test URL with special characters
echo ""
echo "Test 15: Create URL with special characters"
response=$(curl -s -X POST "$API_URL/api/urls" \
    -H "Content-Type: application/json" \
    -d '{"url": "https://example.com?param1=value1&param2=value2#anchor", "custom_code": "special-test"}')
check_json_field "$response" "short_code" "special-test"
print_result $? "URL with special characters"

# Test 16: Test pagination
echo ""
echo "Test 16: Test pagination"
response=$(curl -s "$API_URL/api/urls?limit=5&offset=0")
if echo "$response" | grep -q '\['; then
    print_result 0 "Pagination works"
else
    print_result 1 "Pagination failed"
fi

# Test 17: Test invalid code (should return 404)
echo ""
echo "Test 17: Test non-existent URL"
response=$(curl -s -o /dev/null -w "%{http_code}" "$API_URL/api/urls/non-existent-code")
if [ "$response" = "404" ]; then
    print_result 0 "Non-existent URL returns 404"
else
    print_result 1 "Non-existent URL should return 404 (got HTTP $response)"
fi

# Test 18: Test duplicate code (should fail)
echo ""
echo "Test 18: Test duplicate code creation"
response=$(curl -s -X POST "$API_URL/api/urls" \
    -H "Content-Type: application/json" \
    -d '{"url": "https://example.com/duplicate", "custom_code": "lynx-test"}')
http_code=$(curl -s -o /dev/null -w "%{http_code}" -X POST "$API_URL/api/urls" \
    -H "Content-Type: application/json" \
    -d '{"url": "https://example.com/duplicate", "custom_code": "lynx-test"}')
if [ "$http_code" = "409" ] || [ "$http_code" = "400" ]; then
    print_result 0 "Duplicate code rejected (HTTP $http_code)"
else
    print_result 1 "Duplicate code should be rejected (got HTTP $http_code)"
fi

# Test 19: Test statistics accuracy
echo ""
echo "Test 19: Test statistics accuracy"
# Create a new URL with unique code (must be <= 20 chars)
# Use timestamp-based code to ensure uniqueness and reasonable length
unique_code="st$(date +%s%N | cut -c10-18)"
create_response=$(curl -s -X POST "$API_URL/api/urls" \
    -H "Content-Type: application/json" \
    -d "{\"url\": \"https://example.com/stats\", \"custom_code\": \"$unique_code\"}")

# Verify URL was created successfully
if ! echo "$create_response" | grep -q "\"short_code\""; then
    echo "Failed to create URL for stats test. Response: $create_response"
    print_result 1 "Failed to create URL for stats test"
fi

# Make 10 redirects
for i in {1..10}; do
    curl -s -o /dev/null -L "$REDIRECT_URL/$unique_code"
done
sleep 5  # Wait for flush
# Check stats
response=$(curl -s "$API_URL/api/urls/$unique_code")
clicks=$(echo "$response" | grep -o '"clicks":[0-9]*' | cut -d':' -f2)
if [ -n "$clicks" ] && [ "$clicks" -ge 10 ]; then
    print_result 0 "Statistics accurate (clicks: $clicks)"
else
    echo "Full response: $response"
    print_result 1 "Statistics inaccurate (expected >=10, got ${clicks:-0})"
fi

# Test 20: Verify all created URLs still exist
echo ""
echo "Test 20: Verify data consistency"
response=$(curl -s "$API_URL/api/urls?limit=100")
# Count how many of our test URLs exist
count=0
for code in lynx-test rapid-1 special-test; do
    if echo "$response" | grep -q "\"$code\""; then
        count=$((count + 1))
    fi
done
if [ $count -eq 3 ]; then
    print_result 0 "All test URLs still exist in database"
else
    print_result 1 "Some URLs missing (found $count/3)"
fi

echo ""
echo -e "${GREEN}=========================================="
echo "All Integration Tests Passed!"
echo -e "==========================================${NC}"
