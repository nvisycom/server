#!/usr/bin/env bash

# Exits on error.
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

log_warn() {
    YELLOW='\033[0;33m'
    NC='\033[0m'
    echo -e "${YELLOW}[$(date +'%T')] warning:${NC} $1" >&2
}

# Validate timeout is a positive integer
if ! [[ "$TIMEOUT" =~ ^[0-9]+$ ]] || [ "$TIMEOUT" -le 0 ]; then
    log_error "POSTGRES_TIMEOUT must be a positive integer, got: $TIMEOUT"
    exit 1
fi

# Validate port is a valid port number
if ! [[ "$PORT" =~ ^[0-9]+$ ]] || [ "$PORT" -lt 1 ] || [ "$PORT" -gt 65535 ]; then
    log_error "POSTGRES_PORT must be a valid port number (1-65535), got: $PORT"
    exit 1
fi

# Check if pg_isready is available
if ! command -v pg_isready >/dev/null 2>&1; then
    log_error "pg_isready command not found. Please install PostgreSQL client tools."
    log_error "Ubuntu/Debian: sudo apt-get install postgresql-client"
    log_error "macOS: brew install postgresql"
    log_error "RHEL/CentOS: sudo yum install postgresql"
    exit 1
fi

log_info "Waiting for PostgreSQL at ${HOST}:${PORT} (timeout: ${TIMEOUT}s)"

counter=0
last_error=""
consecutive_failures=0

until pg_isready -h "$HOST" -p "$PORT" -U "$USER" -q 2>/dev/null; do
    counter=$((counter + 1))
    consecutive_failures=$((consecutive_failures + 1))

    if [ $counter -gt "$TIMEOUT" ]; then
        log_error "PostgreSQL not ready after ${TIMEOUT} seconds"
        if [ -n "$last_error" ]; then
            log_error "Last error: $last_error"
        fi
        log_error "Troubleshooting:"
        log_error "  1. Check if PostgreSQL is running: docker ps | grep postgres"
        log_error "  2. Check if port $PORT is accessible: nc -zv $HOST $PORT"
        log_error "  3. Verify database user '$USER' exists"
        log_error "  4. Check PostgreSQL logs for errors"
        exit 1
    fi

    # Capture detailed error message every 10 seconds
    if [ $((counter % 10)) -eq 0 ]; then
        last_error=$(pg_isready -h "$HOST" -p "$PORT" -U "$USER" 2>&1 || true)
        log_warn "Still waiting... (${counter}/${TIMEOUT}s) - Last check: $last_error"

        # After 30 seconds of failures, suggest checking network/firewall
        if [ $consecutive_failures -ge 30 ]; then
            log_warn "Connection failing for ${consecutive_failures}s. Check network connectivity and firewall rules."
        fi
    else
        echo "Waiting for database... (${counter}/${TIMEOUT}s)"
    fi

    sleep 1
done

log_info "PostgreSQL is ready and accepting connections!"
exit 0
