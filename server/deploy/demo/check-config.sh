#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if rg -n 'demo\.ichoi\.invalid|linkkeys\.ichoi\.invalid|example\.invalid|REPLACE-ME|NOT APPROVED' \
    "$script_dir/Caddyfile" \
    "$script_dir/docker-compose.yml" \
    "$script_dir/LICENSES.md"; then
    echo "demo deployment is blocked by placeholder or unapproved values" >&2
    exit 1
fi

echo "demo configuration has no known placeholder values"
