#!/usr/bin/env bash

# Exits on error.
set -euo pipefail

# Get parameters:
readonly HOST="${NATS_HOST:-localhost}"
readonly PORT="${NATS_PORT:-4222}"
readonly TIMEOUT="${NATS_TIMEOUT:-30}"

log_info() {
    GREEN='\033[0;32m'
    NC='\033[0m'
    echo -e "${GREEN}[$(date +'%T')]${NC} $1"
}

log_error() {
    RED='\033[0;31m'
    NC='\033[0m'
    echo -e "${RED}[$(date +'%T')] error:${NC} $1" >&2
}

log_warn() {
    YELLOW='\033[0;33m'
    NC='\033[0m'
    echo -e "${YELLOW}[$(date +'%T')] warning:${NC} $1" >&2
}

# Validate timeout is a positive integer
if ! [[ "$TIMEOUT" =~ ^[0-9]+$ ]] || [ "$TIMEOUT" -le 0 ]; then
    log_error "NATS_TIMEOUT must be a positive integer, got: $TIMEOUT"
    exit 1
fi

# Validate port is a valid port number
if ! [[ "$PORT" =~ ^[0-9]+$ ]] || [ "$PORT" -lt 1 ] || [ "$PORT" -gt 65535 ]; then
    log_error "NATS_PORT must be a valid port number (1-65535), got: $PORT"
    exit 1
fi

# Check if nats CLI is available
if ! command -v nats >/dev/null 2>&1; then
    log_error "nats command not found. Please install NATS CLI."
    log_error "Installation: https://github.com/nats-io/natscli"
    exit 1
fi

log_info "Waiting for NATS at ${HOST}:${PORT} (timeout: ${TIMEOUT}s)"

counter=0
last_error=""

until nats server check --server "nats://${HOST}:${PORT}" >/dev/null 2>&1; do
    counter=$((counter + 1))

    if [ $counter -gt "$TIMEOUT" ]; then
        log_error "NATS not ready after ${TIMEOUT} seconds"
        if [ -n "$last_error" ]; then
            log_error "Last error: $last_error"
        fi
        log_error "Troubleshooting:"
        log_error "  1. Check if NATS server is running: docker ps | grep nats"
        log_error "  2. Check if port $PORT is accessible: nc -zv $HOST $PORT"
        log_error "  3. Check NATS logs for errors"
        exit 1
    fi

    # Capture error message every 10 seconds
    if [ $((counter % 10)) -eq 0 ]; then
        last_error=$(nats server check --server "nats://${HOST}:${PORT}" 2>&1 || true)
        log_warn "Still waiting... (${counter}/${TIMEOUT}s) - Last check: $last_error"
    else
        echo "Waiting for NATS... (${counter}/${TIMEOUT}s)"
    fi

    sleep 1
done

log_info "NATS is ready and accepting connections!"
exit 0
