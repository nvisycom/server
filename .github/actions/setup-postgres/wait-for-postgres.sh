#!/usr/bin/env bash

# Exits on log_error.
set -euo pipefail

# Get parameters:
readonly HOST="${POSTGRES_HOST:-localhost}"
readonly PORT="${POSTGRES_PORT:-5432}"
readonly USER="${POSTGRES_USER:-postgres}"
readonly TIMEOUT="${POSTGRES_TIMEOUT:-60}"

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

if ! command -v pg_isready >/dev/null 2>&1; then
    log_error "pg_isready command not found. Please install PostgreSQL tools."
    exit 1
fi

log_info "Waiting for PostgreSQL at ${HOST}:${PORT} (timeout: ${TIMEOUT}s)"

counter=0
until pg_isready -h "$HOST" -p "$PORT" -U "$USER" -q; do
    counter=$((counter + 1))
    if [ $counter -gt "$TIMEOUT" ]; then
        log_error "PostgreSQL not ready after ${TIMEOUT} seconds"
        exit 1
    fi

    echo "Waiting for database... (${counter}/${TIMEOUT})"
    sleep 1
done

log_info "PostgreSQL is ready and accepting connections!"
