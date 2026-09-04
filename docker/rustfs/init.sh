#!/bin/sh
# Provisions the first-party bucket in RustFS (which does not auto-create
# buckets). Shared by docker-compose.dev.yml and docker-compose.yml, mounted
# read-only and run by the rustfs-init container. Idempotent: safe to re-run.
#
# Configuration comes from the environment (never interpolated into a shell
# string), so a credential containing shell metacharacters cannot alter the
# command:
#   S3_ENDPOINT           RustFS URL (e.g. http://rustfs:9000)
#   S3_ACCESS_KEY_ID      access key
#   S3_SECRET_ACCESS_KEY  secret key
#   S3_BUCKET             bucket to create
set -eu

: "${S3_ENDPOINT:?S3_ENDPOINT is required}"
: "${S3_ACCESS_KEY_ID:?S3_ACCESS_KEY_ID is required}"
: "${S3_SECRET_ACCESS_KEY:?S3_SECRET_ACCESS_KEY is required}"
: "${S3_BUCKET:?S3_BUCKET is required}"

rc alias set nvisy "$S3_ENDPOINT" "$S3_ACCESS_KEY_ID" "$S3_SECRET_ACCESS_KEY"

# The healthcheck already gates on /health/ready, but retry a few times so a
# brief readiness flap between the probe and this call does not fail the run.
attempt=1
max_attempts=10
until rc bucket create --ignore-existing "nvisy/$S3_BUCKET"; do
    if [ "$attempt" -ge "$max_attempts" ]; then
        echo "bucket create failed after $max_attempts attempts" >&2
        exit 1
    fi
    echo "bucket create attempt $attempt failed; retrying in 2s..." >&2
    attempt=$((attempt + 1))
    sleep 2
done
