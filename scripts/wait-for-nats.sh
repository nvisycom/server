#!/usr/bin/env bash

# Waits for NATS to be ready.
set -euo pipefail

readonly HOST="${NATS_HOST:-localhost}"
readonly PORT="${NATS_PORT:-4222}"
readonly MONITOR_PORT="${NATS_MONITOR_PORT:-8222}"
readonly TIMEOUT="${1:-${NATS_TIMEOUT:-30}}"

log_info() {
    echo -e "\033[0;32m[$(date +'%T')]\033[0m $1"
}

log_error() {
    echo -e "\033[0;31m[$(date +'%T')] error:\033[0m $1" >&2
}

log_warn() {
    echo -e "\033[0;33m[$(date +'%T')] warning:\033[0m $1" >&2
}

# Validate timeout
if ! [[ "$TIMEOUT" =~ ^[0-9]+$ ]] || [ "$TIMEOUT" -le 0 ]; then
    log_error "Timeout must be a positive integer, got: $TIMEOUT"
    exit 1
fi

log_info "Waiting for NATS at ${HOST}:${PORT} (timeout: ${TIMEOUT}s)"

counter=0

# Try health endpoint first (preferred), fall back to port check
check_nats() {
    # Try monitoring endpoint
    if curl -sf "http://${HOST}:${MONITOR_PORT}/healthz" >/dev/null 2>&1; then
        return 0
    fi
    # Fall back to TCP port check
    if command -v nc >/dev/null 2>&1; then
        nc -z "$HOST" "$PORT" 2>/dev/null
    elif command -v bash >/dev/null 2>&1; then
        timeout 1 bash -c "echo >/dev/tcp/$HOST/$PORT" 2>/dev/null
    else
        return 1
    fi
}

until check_nats; do
    counter=$((counter + 1))

    if [ $counter -gt "$TIMEOUT" ]; then
        log_error "NATS not ready after ${TIMEOUT} seconds"
        log_error "Troubleshooting:"
        log_error "  1. Check if NATS is running: docker ps | grep nats"
        log_error "  2. Check if port $PORT is accessible"
        log_error "  3. Check NATS logs: docker logs nvisy-nats-dev"
        exit 1
    fi

    if [ $((counter % 10)) -eq 0 ]; then
        log_warn "Still waiting... (${counter}/${TIMEOUT}s)"
    fi

    sleep 1
done

log_info "NATS is ready!"
exit 0
