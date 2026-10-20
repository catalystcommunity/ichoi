#!/usr/bin/env bash
# Export all brand assets. Requires Python 3, rsvg-convert, and ImageMagick.
set -euo pipefail
BRAND_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec python3 "$BRAND_DIR/build.py" "$@"
