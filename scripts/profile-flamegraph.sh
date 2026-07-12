#!/usr/bin/env bash
set -Eeuo pipefail

readonly SCENARIO=${1:-}
readonly OUTPUT=${2:-}
readonly DURATION=${3:-30s}
readonly API_URL=${API_URL:-http://127.0.0.1:8080}
readonly REDIRECT_URL=${REDIRECT_URL:-http://127.0.0.1:3000}
readonly LYNX_BIN=${LYNX_BIN:-target/profiling/lynx}
readonly FLAMEGRAPH_FREQUENCY=${FLAMEGRAPH_FREQUENCY:-499}
readonly LOG_DIR=${LOG_DIR:-$(dirname "${OUTPUT:-.}")/logs}
readonly SEED_URL_COUNT=20

profile_pid=
workload_file=

usage() {
    cat <<'EOF'
Usage: scripts/profile-flamegraph.sh <scenario> <output.svg> [duration]

Scenarios:
  redirect-cached  Warm-cache redirect and click-counting hot path
  api-mixed        Even mix of URL creation and URL detail reads

Environment:
  API_URL, REDIRECT_URL, LYNX_BIN, FLAMEGRAPH_FREQUENCY, LOG_DIR
EOF
}

fail() {
    echo "error: $*" >&2
    exit 1
}

cleanup() {
    local status=$?
    local pid=$profile_pid
    profile_pid=

    if [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null; then
        kill -TERM -- "-$pid" 2>/dev/null || true
        wait "$pid" 2>/dev/null || true
    fi

    if [[ -n "$workload_file" ]]; then
        rm -f "$workload_file"
    fi

    exit "$status"
}
trap cleanup EXIT INT TERM

case "$SCENARIO" in
    redirect-cached)
        readonly CODE_PREFIX=prof-redirect
        ;;
    api-mixed)
        readonly CODE_PREFIX=prof-api
        ;;
    -h | --help)
        usage
        exit 0
        ;;
    *)
        usage >&2
        fail "scenario must be 'redirect-cached' or 'api-mixed'"
        ;;
esac

[[ -n "$OUTPUT" ]] || fail "an output SVG path is required"
[[ "$OUTPUT" == *.svg ]] || fail "output path must end in .svg"
[[ "$DURATION" =~ ^[1-9][0-9]*(s|m)$ ]] || fail "duration must be a positive integer followed by s or m"
[[ "$FLAMEGRAPH_FREQUENCY" =~ ^[1-9][0-9]*$ ]] || fail "FLAMEGRAPH_FREQUENCY must be a positive integer"
[[ -x "$LYNX_BIN" ]] || fail "profiling binary is not executable: $LYNX_BIN"

for command in curl flamegraph setsid wrk; do
    command -v "$command" >/dev/null 2>&1 || fail "required command is unavailable: $command"
done

mkdir -p "$(dirname "$OUTPUT")" "$LOG_DIR"
rm -f "$OUTPUT"
readonly profile_log="$LOG_DIR/$SCENARIO.log"

export DATABASE_BACKEND=${DATABASE_BACKEND:-postgres}
export DATABASE_MAX_CONNECTIONS=${DATABASE_MAX_CONNECTIONS:-50}
export AUTH_MODE=none
export API_HOST=${API_HOST:-127.0.0.1}
export API_PORT=${API_PORT:-8080}
export REDIRECT_HOST=${REDIRECT_HOST:-127.0.0.1}
export REDIRECT_PORT=${REDIRECT_PORT:-3000}
export CACHE_MAX_ENTRIES=${CACHE_MAX_ENTRIES:-500000}
export CACHE_FLUSH_INTERVAL_SECS=${CACHE_FLUSH_INTERVAL_SECS:-5}
export ACTOR_BUFFER_SIZE=${ACTOR_BUFFER_SIZE:-1000000}
export ACTOR_FLUSH_INTERVAL_MS=${ACTOR_FLUSH_INTERVAL_MS:-100}
export RUST_LOG=${RUST_LOG:-warn}
export PROFILE_CODE_PREFIX=$CODE_PREFIX
export PROFILE_SEED_URL_COUNT=$SEED_URL_COUNT

notes="Lynx $SCENARIO workload; duration=$DURATION; frequency=${FLAMEGRAPH_FREQUENCY}Hz"
setsid flamegraph \
    --freq "$FLAMEGRAPH_FREQUENCY" \
    --notes "$notes" \
    --output "$OUTPUT" \
    -- "$LYNX_BIN" >"$profile_log" 2>&1 &
profile_pid=$!

ready=false
for _ in $(seq 1 60); do
    if curl --fail --silent --show-error "$API_URL/api/health" >/dev/null; then
        ready=true
        break
    fi
    if ! kill -0 "$profile_pid" 2>/dev/null; then
        break
    fi
    sleep 1
done

if [[ "$ready" != true ]]; then
    tail -n 100 "$profile_log" >&2 || true
    fail "Lynx did not become healthy"
fi

for index in $(seq 1 "$SEED_URL_COUNT"); do
    curl --fail --silent --show-error \
        --request POST "$API_URL/api/urls" \
        --header 'Content-Type: application/json' \
        --data "{\"url\":\"https://example.com/profile-target-$index\",\"custom_code\":\"$CODE_PREFIX-$index\"}" \
        >/dev/null
done

case "$SCENARIO" in
    redirect-cached)
        for _ in $(seq 1 100); do
            curl --fail --silent --output /dev/null "$REDIRECT_URL/$CODE_PREFIX-1"
        done
        wrk --threads 4 --connections 256 --duration "$DURATION" \
            "$REDIRECT_URL/$CODE_PREFIX-1"
        ;;
    api-mixed)
        workload_file=$(mktemp)
        cat >"$workload_file" <<'LUA'
local counter = 0
local json_headers = { ["Content-Type"] = "application/json" }
local code_prefix = os.getenv("PROFILE_CODE_PREFIX")
local seed_url_count = tonumber(os.getenv("PROFILE_SEED_URL_COUNT"))

request = function()
    counter = counter + 1
    if counter % 2 == 0 then
        local body = string.format(
            '{"url":"https://example.com/profile-api-%d"}',
            counter
        )
        return wrk.format("POST", "/api/urls", json_headers, body)
    end

    local code = string.format("%s-%d", code_prefix, counter % seed_url_count + 1)
    return wrk.format("GET", "/api/urls/" .. code)
end
LUA
        wrk --threads 4 --connections 64 --duration "$DURATION" \
            --script "$workload_file" "$API_URL"
        ;;
esac

set +e
kill -INT -- "-$profile_pid" 2>/dev/null
wait "$profile_pid"
profile_status=$?
set -e
profile_pid=

if [[ $profile_status -ne 0 && $profile_status -ne 130 ]]; then
    tail -n 100 "$profile_log" >&2 || true
    fail "flamegraph exited with status $profile_status"
fi

[[ -s "$OUTPUT" ]] || fail "flamegraph did not produce a non-empty SVG"
grep -q '<svg' "$OUTPUT" || fail "flamegraph output is not an SVG"

echo "Generated $OUTPUT ($(du -h "$OUTPUT" | cut -f1))"
