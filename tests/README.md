# Lynx Integration and Data Consistency Tests

This directory contains comprehensive integration and data consistency tests for the Lynx URL shortener.

## Test Scripts

### `integration_test.sh`
Comprehensive integration test suite covering all API endpoints and functionality:

**Test Coverage (20 tests):**
1. Health check endpoint
2. Auth mode endpoint
3. URL creation with custom code
4. URL creation with auto-generated code
5. Get URL details
6. Redirect functionality
7. Click count verification
8. List URLs endpoint
9. URL deactivation
10. Deactivated URL returns proper error code
11. URL reactivation
12. Reactivated URL works correctly
13. Rapid URL creation (50 URLs stress test)
14. Concurrent redirects to same URL
15. URLs with special characters
16. Pagination functionality
17. Non-existent URL handling
18. Duplicate code rejection
19. Statistics accuracy
20. Data consistency verification

**Usage:**
```bash
bash tests/integration_test.sh [API_URL] [REDIRECT_URL]

# Examples:
bash tests/integration_test.sh http://localhost:8080 http://localhost:3000
bash tests/integration_test.sh http://api.example.com http://r.example.com
```

### `concurrent_test.sh`
Concurrent load and stress testing to verify data consistency under high load:

**Test Coverage (6 tests):**
1. Concurrent URL creation (configurable concurrency level)
2. Concurrent redirects to same URL (click counting accuracy)
3. Mixed concurrent operations (creates, gets, redirects)
4. Rapid state changes under concurrent load
5. High-frequency statistics updates (200 redirects)
6. List endpoint under concurrent load

**Usage:**
```bash
bash tests/concurrent_test.sh [API_URL] [REDIRECT_URL] [CONCURRENCY]

# Examples:
bash tests/concurrent_test.sh http://localhost:8080 http://localhost:3000 100
bash tests/concurrent_test.sh http://api.example.com http://r.example.com 200
```

## GitHub Actions Workflow

The `.github/workflows/integration-tests.yml` workflow automatically runs these tests:

**Triggers:**
- After successful Docker image build
- Manual workflow dispatch

**Test Matrix:**
- **SQLite Backend:** Full integration and concurrent tests
- **PostgreSQL Backend:** Full integration and concurrent tests

**Graceful Shutdown Tests:**
Both backends are tested with:
- SIGTERM signal handling
- SIGINT signal handling  
- Data persistence verification after restart
- Buffered write flush validation

**Requirements:**
- Docker image must be published to GHCR
- Tests run on ubuntu-24.04 (linux/amd64)

## Test Features

### Data Consistency Validation
- Verifies click counting accuracy under concurrent load
- Tests buffer flush behavior (Layer 1→2→3)
- Validates data persistence across restarts
- Checks graceful shutdown data integrity

### Concurrency Testing
- Simulates 100+ concurrent requests
- Tests race conditions in click counting
- Validates atomic operations
- Stress tests database connection pooling

### API Coverage
- All CRUD operations for URLs
- Authentication modes
- Pagination
- Error handling
- Special character handling

## Running Tests Locally

### Prerequisites
1. Lynx service running (or use Docker)
2. bash shell
3. curl command available

### With Docker
```bash
# Start Lynx with SQLite
docker run -d \
  -p 8080:8080 \
  -p 3000:3000 \
  -e DATABASE_BACKEND=sqlite \
  -e AUTH_MODE=none \
  ghcr.io/btreemap/lynx:latest

# Run tests
bash tests/integration_test.sh http://localhost:8080 http://localhost:3000
bash tests/concurrent_test.sh http://localhost:8080 http://localhost:3000 50
```

### With Local Binary
```bash
# Start Lynx
DATABASE_BACKEND=sqlite \
DATABASE_URL="sqlite://./test.db" \
AUTH_MODE=none \
API_HOST=127.0.0.1 \
API_PORT=8080 \
REDIRECT_HOST=127.0.0.1 \
REDIRECT_PORT=3000 \
./target/release/lynx

# Run tests
bash tests/integration_test.sh http://127.0.0.1:8080 http://127.0.0.1:3000
bash tests/concurrent_test.sh http://127.0.0.1:8080 http://127.0.0.1:3000 50
```

## Expected Output

### Successful Test Run
```
==========================================
Running Comprehensive Integration Tests
API URL: http://localhost:8080
Redirect URL: http://localhost:3000
==========================================

Test 1: Health Check
✓ Health check endpoint

Test 2: Get Auth Mode
✓ Auth mode endpoint

...

==========================================
All Integration Tests Passed!
==========================================
```

### Failed Test
Tests will exit with code 1 and show:
```
Test X: Description
✗ Error message explaining failure
```

## Troubleshooting

### Tests fail with connection errors
- Ensure Lynx service is running
- Check firewall settings
- Verify correct ports (default: 8080 for API, 3000 for redirects)

### Database errors
- For SQLite: Ensure write permissions to database directory
- For PostgreSQL: Verify connection string and database exists

### Timeout issues
- Tests have built-in delays for buffer flushes (2-6 seconds)
- Increase sleep times if running on slower systems
- Check CACHE_FLUSH_INTERVAL_SECS and ACTOR_FLUSH_INTERVAL_MS settings

## Contributing

When adding new tests:
1. Follow existing test numbering and format
2. Add descriptive test names
3. Include both positive and negative test cases
4. Test concurrent scenarios where applicable
5. Update this README with new test coverage
