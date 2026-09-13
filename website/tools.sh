#!/usr/bin/env bash
# Build and validate the Ichoi project website.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# The shared build host can expose a read-only user cache. Keep the default
# cache task-local, while allowing CI to provide its own cache path.
export UV_CACHE_DIR="${UV_CACHE_DIR:-/tmp/ichoi-uv-cache}"
export UV_TOOL_DIR="${UV_TOOL_DIR:-/tmp/ichoi-uv-tools}"
PYSOCHA_REF="18b2d6e704ba63e80f82d287b5831c1151c388a4"
PYSOCHA_SOURCE="git+https://github.com/catalystcommunity/pysocha.git@${PYSOCHA_REF}"

pysocha() {
    command -v uv >/dev/null 2>&1 || { echo "uv is required" >&2; exit 1; }
    cd "$SCRIPT_DIR/site-src"
    uv tool run --from "$PYSOCHA_SOURCE" pysocha "$@" --config-file config.yaml
}

check_deploy() {
    local found=0
    while IFS= read -r value; do
        echo "deployment placeholder found: ${value}" >&2
        found=1
    done < <(rg -n -o 'ichoi\.invalid|REPLACE-ME|privacy@ichoi\.invalid|support@ichoi\.invalid|demo\.ichoi\.invalid' \
        "$SCRIPT_DIR/values.yaml" "$SCRIPT_DIR/site-src" || true)
    if [[ "$found" -ne 0 ]]; then
        echo "replace all domain and contact placeholders before deployment" >&2
        return 1
    fi
}

case "${1:-}" in
    build)
        pysocha build
        ;;
    preview)
        pysocha preview
        ;;
    check-deploy)
        check_deploy
        ;;
    help|--help|-h|"")
        echo "Usage: ./tools.sh {build|preview|check-deploy}"
        ;;
    *)
        echo "unknown command: $1" >&2
        exit 1
        ;;
esac
