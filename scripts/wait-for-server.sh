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
    echo "[INFO] $(date '+%Y-%m-%d %H:%M:%S') - $1"
}

# Pretty logging function for errors.
log_error() {
    echo "[ERROR] $(date '+%Y-%m-%d %H:%M:%S') - $1" >&2
}

# Function to check if server is ready.
check_server() {
    curl -s "$URL" > /dev/null 2>&1
}

# Main execution:
main() {
    log_info "Waiting for server to be ready at: $URL"
    log_info "Timeout: ${TIMEOUT_SECONDS}s, Check interval: ${SLEEP_INTERVAL}s"

    local elapsed=0

    while [ $elapsed -lt "$TIMEOUT_SECONDS" ]; do
        if check_server; then
            log_info "Server is ready! (took ${elapsed}s)"
            exit 0
        fi

        sleep "$SLEEP_INTERVAL"
        elapsed=$((elapsed + SLEEP_INTERVAL))

        # Log progress every 10 seconds:
        if [ $((elapsed % 10)) -eq 0 ]; then
            log_info "Still waiting... (${elapsed}/${TIMEOUT_SECONDS}s)"
        fi
    done

    log_error "Timeout reached! Server at $URL is not responding after ${TIMEOUT_SECONDS}s"
    exit 1
}

main "$@"
