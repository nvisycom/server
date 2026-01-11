#!/usr/bin/env bash

# Waits for NATS to be ready.
set -euo pipefail

readonly URL="${NATS_URL:?NATS_URL is required}"
readonly HOST=$(echo "$URL" | sed -E 's|.*://([^:/]+).*|\1|')
readonly PORT=$(echo "$URL" | sed -E 's|.*://[^:]+:([0-9]+).*|\1|' | grep -E '^[0-9]+$' || echo "4222")
readonly MONITOR_PORT="${NATS_MONITOR_PORT:-8222}"
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

log_info "Waiting for NATS at ${HOST}:${PORT} (timeout: ${TIMEOUT}s)"

counter=0

check_nats() {
    if curl -sf "http://${HOST}:${MONITOR_PORT}/healthz" >/dev/null 2>&1; then
        return 0
    fi
    if command -v nc >/dev/null 2>&1; then
        nc -z "$HOST" "$PORT" 2>/dev/null
    else
        timeout 1 bash -c "echo >/dev/tcp/$HOST/$PORT" 2>/dev/null
    fi
}

until check_nats; do
    counter=$((counter + 1))

    if [ $counter -gt "$TIMEOUT" ]; then
        log_error "NATS not ready after ${TIMEOUT} seconds"
        exit 1
    fi

    if [ $((counter % 10)) -eq 0 ]; then
        log_warn "Still waiting... (${counter}/${TIMEOUT}s)"
    fi

    sleep 1
done

log_info "NATS is ready!"
