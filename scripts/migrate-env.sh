#!/usr/bin/env bash

# Migrates .env to match .env.example: adds keys new to the template (with their
# default), keeps values already set locally, and prunes keys the template no
# longer defines. The template's comments, sections, and ordering are adopted.
set -euo pipefail

readonly EXAMPLE="${1:-.env.example}"
readonly ENV="${2:-.env}"

log_info() {
    echo -e "\033[0;32m[migrate]\033[0m $1"
}

log_error() {
    echo -e "\033[0;31m[migrate] error:\033[0m $1" >&2
}

log_add() {
    echo -e "\033[0;32m[migrate]\033[0m + added $1"
}

log_remove() {
    echo -e "\033[0;33m[migrate]\033[0m - removed $1 (no longer in template)"
}

if [ ! -f "$EXAMPLE" ]; then
    log_error "$EXAMPLE not found (run 'make generate-env-example' first)"
    exit 1
fi

# No existing .env: the template is the migration.
if [ ! -f "$ENV" ]; then
    cp "$EXAMPLE" "$ENV"
    log_info "$ENV created from $EXAMPLE"
    exit 0
fi

# The env-var key of a `KEY=value` or `# KEY=value` line, else empty. Accepts a
# leading `# ` so commented-out optionals are matched too.
key_of() {
    echo "$1" | sed -nE 's/^#? ?([A-Za-z_][A-Za-z0-9_]*)=.*/\1/p'
}

# Every key currently set (uncommented) in the existing .env. `|| true` so a
# .env with no set keys (grep finds nothing, exits 1) does not trip the errexit.
existing_keys=$(grep -E '^[A-Za-z_][A-Za-z0-9_]*=' "$ENV" | sed -E 's/=.*//' | sort -u || true)

added=0
removed=0
output=""

# Rebuild .env from the template, line by line: comments and sections are taken
# verbatim, and each key line keeps the local value when one is set.
while IFS= read -r line || [ -n "$line" ]; do
    key=$(key_of "$line")
    if [ -z "$key" ]; then
        output+="$line"$'\n'
        continue
    fi

    if grep -qE "^${key}=" "$ENV"; then
        # Preserve the locally-set value verbatim.
        output+="$(grep -E "^${key}=" "$ENV" | head -n1)"$'\n'
    else
        # New key (or one only present commented-out locally): take the template
        # line, which carries the default (or a blank commented-out optional).
        output+="$line"$'\n'
        if [[ "$line" != \#* ]]; then
            log_add "$key"
            added=$((added + 1))
        fi
    fi
done < "$EXAMPLE"

# Report keys dropped: set locally but absent from the template. `|| true` so an
# empty template does not trip the errexit.
template_keys=$(grep -E '^#? ?[A-Za-z_][A-Za-z0-9_]*=' "$EXAMPLE" | sed -E 's/^#? ?//; s/=.*//' | sort -u || true)
for key in $existing_keys; do
    if ! echo "$template_keys" | grep -qx "$key"; then
        log_remove "$key"
        removed=$((removed + 1))
    fi
done

# Write to a temporary file in the same directory, then rename over .env, so an
# interrupted run never leaves a truncated .env: the original stays intact until
# the atomic rename swaps in the fully-written replacement.
tmp=$(mktemp "${ENV}.XXXXXX")
printf '%s' "$output" > "$tmp"
mv "$tmp" "$ENV"

if [ "$added" -eq 0 ] && [ "$removed" -eq 0 ]; then
    log_info "$ENV is already up to date"
else
    log_info "$ENV synced ($added added, $removed removed)"
fi
