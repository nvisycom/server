#!/usr/bin/env bash

set -euo pipefail

# Configuration via environment variables:
readonly DEFAULT_URL="http://localhost:4321"
readonly DEFAULT_TIMEOUT=30
readonly DEFAULT_SLEEP_INTERVAL=1

# Get parameters:
readonly URL="${1:-${SERVER_URL:-$DEFAULT_URL}}"
readonly TIMEOUT_SECONDS="${2:-${SERVER_TIMEOUT:-$DEFAULT_TIMEOUT}}"
readonly SLEEP_INTERVAL="${3:-${SERVER_SLEEP_INTERVAL:-$DEFAULT_SLEEP_INTERVAL}}"

# Pretty logging function for info.
log_info() {
    GREEN='\033[0;32m'
    NC='\033[0m'
    echo -e "${GREEN}[INFO]${NC} $(date '+%Y-%m-%d %H:%M:%S') - $1"
}

# Pretty logging function for errors.
log_error() {
    RED='\033[0;31m'
    NC='\033[0m'
    echo -e "${RED}[ERROR]${NC} $(date '+%Y-%m-%d %H:%M:%S') - $1" >&2
}

# Pretty logging function for warnings.
log_warn() {
    YELLOW='\033[0;33m'
    NC='\033[0m'
    echo -e "${YELLOW}[WARN]${NC} $(date '+%Y-%m-%d %H:%M:%S') - $1" >&2
}

# Validate inputs
validate_inputs() {
    if ! [[ "$TIMEOUT_SECONDS" =~ ^[0-9]+$ ]] || [ "$TIMEOUT_SECONDS" -le 0 ]; then
        log_error "Timeout must be a positive integer, got: $TIMEOUT_SECONDS"
        exit 1
    fi

    if ! [[ "$SLEEP_INTERVAL" =~ ^[0-9]+$ ]] || [ "$SLEEP_INTERVAL" -le 0 ]; then
        log_error "Sleep interval must be a positive integer, got: $SLEEP_INTERVAL"
        exit 1
    fi

    if ! command -v curl >/dev/null 2>&1; then
        log_error "curl command not found. Please install curl."
        exit 1
    fi
}

# Function to check if server is ready.
check_server() {
    local response_code
    local http_response

    # Try to get HTTP response with timeout
    http_response=$(curl -s -o /dev/null -w "%{http_code}" --max-time 5 --connect-timeout 2 "$URL" 2>&1 || echo "000")

    # Accept 2xx and 3xx status codes as "ready"
    if [[ "$http_response" =~ ^[23][0-9][0-9]$ ]]; then
        return 0
    fi

    return 1
}

# Main execution:
main() {
    validate_inputs

    log_info "Waiting for server to be ready at: $URL"
    log_info "Timeout: ${TIMEOUT_SECONDS}s, Check interval: ${SLEEP_INTERVAL}s"

    local elapsed=0
    local last_error=""
    local consecutive_failures=0

    while [ $elapsed -lt "$TIMEOUT_SECONDS" ]; do
        if check_server; then
            log_info "Server is ready! (took ${elapsed}s)"
            exit 0
        fi

        consecutive_failures=$((consecutive_failures + 1))

        sleep "$SLEEP_INTERVAL"
        elapsed=$((elapsed + SLEEP_INTERVAL))

        # Log progress every 10 seconds:
        if [ $((elapsed % 10)) -eq 0 ]; then
            last_error=$(curl -s -o /dev/null -w "HTTP %{http_code}" --max-time 5 --connect-timeout 2 "$URL" 2>&1 || echo "Connection failed")
            log_warn "Still waiting... (${elapsed}/${TIMEOUT_SECONDS}s) - Last response: $last_error"

            # After 20 seconds, provide troubleshooting tips
            if [ $elapsed -ge 20 ]; then
                log_warn "Server not responding. Check if the process is running and listening on the correct port."
            fi
        fi
    done

    log_error "Timeout reached! Server at $URL is not responding after ${TIMEOUT_SECONDS}s"
    log_error "Troubleshooting:"
    log_error "  1. Check if server process is running: ps aux | grep server"
    log_error "  2. Check if port is open: netstat -tulpn | grep <port>"
    log_error "  3. Check server logs for startup errors"
    log_error "  4. Verify URL is correct: $URL"
    exit 1
}

main "$@"
