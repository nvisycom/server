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

if ! command -v nats >/dev/null 2>&1; then
    log_error "nats command not found. Please install NATS CLI."
    exit 1
fi

log_info "Waiting for NATS at ${HOST}:${PORT} (timeout: ${TIMEOUT}s)"

counter=0
until nats server check --server "nats://${HOST}:${PORT}" >/dev/null 2>&1; do
    counter=$((counter + 1))
    if [ $counter -gt "$TIMEOUT" ]; then
        log_error "NATS not ready after ${TIMEOUT} seconds"
        exit 1
    fi

    echo "Waiting for NATS... (${counter}/${TIMEOUT})"
    sleep 1
done

log_info "NATS is ready and accepting connections!"
