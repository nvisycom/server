#!/usr/bin/env bash

# Waits for HTTP server to be ready.
set -euo pipefail

readonly URL="${SERVER_URL:?SERVER_URL is required}"
readonly TIMEOUT="${1:-30}"

log_info() {
    echo -e "\033[0;32m[$(date +'%T')]\033[0m $1"
}

log_error() {
    echo -e "\033[0;31m[$(date +'%T')] error:\033[0m $1" >&2
}

log_warn() {
    echo -e "\033[0;33m[$(date +'%T')] warning:\033[0m $1" >&2
}

if ! [[ "$TIMEOUT" =~ ^[0-9]+$ ]] || [ "$TIMEOUT" -le 0 ]; then
    log_error "Timeout must be a positive integer, got: $TIMEOUT"
    exit 1
fi

if ! command -v curl >/dev/null 2>&1; then
    log_error "curl not found"
    exit 1
fi

log_info "Waiting for server at ${URL} (timeout: ${TIMEOUT}s)"

counter=0

check_server() {
    local code
    code=$(curl -s -o /dev/null -w "%{http_code}" --max-time 2 "$URL" 2>/dev/null || echo "000")
    [[ "$code" =~ ^[23][0-9][0-9]$ ]]
}

until check_server; do
    counter=$((counter + 1))

    if [ $counter -gt "$TIMEOUT" ]; then
        log_error "Server not ready after ${TIMEOUT} seconds"
        exit 1
    fi

    if [ $((counter % 10)) -eq 0 ]; then
        log_warn "Still waiting... (${counter}/${TIMEOUT}s)"
    fi

    sleep 1
done

log_info "Server is ready!"
