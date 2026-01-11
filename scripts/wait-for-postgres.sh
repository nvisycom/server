#!/usr/bin/env bash

# Waits for PostgreSQL to be ready.
set -euo pipefail

readonly URL="${POSTGRES_URL:?POSTGRES_URL is required}"
readonly HOST=$(echo "$URL" | sed -E 's|.*@([^:/]+).*|\1|')
readonly PORT=$(echo "$URL" | sed -E 's|.*:([0-9]+)/.*|\1|')
readonly USER=$(echo "$URL" | sed -E 's|.*://([^:]+):.*|\1|')
readonly TIMEOUT="${1:-60}"

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

if ! command -v pg_isready >/dev/null 2>&1; then
    log_error "pg_isready not found. Install postgresql-client."
    exit 1
fi

log_info "Waiting for PostgreSQL at ${HOST}:${PORT} (timeout: ${TIMEOUT}s)"

counter=0

until pg_isready -h "$HOST" -p "$PORT" -U "$USER" -q 2>/dev/null; do
    counter=$((counter + 1))

    if [ $counter -gt "$TIMEOUT" ]; then
        log_error "PostgreSQL not ready after ${TIMEOUT} seconds"
        exit 1
    fi

    if [ $((counter % 10)) -eq 0 ]; then
        log_warn "Still waiting... (${counter}/${TIMEOUT}s)"
    fi

    sleep 1
done

log_info "PostgreSQL is ready!"
