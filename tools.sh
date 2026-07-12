#!/usr/bin/env bash
# Ichoi dev tooling. Mirrors what CI runs so it can be exercised locally.
#
#   ./tools.sh <command>
#
# CSIL:
#   csil-validate   validate the schema (whole graph via the entry point)
#   csil-lint       lint the schema
#   csil-fmt        format the schema in place
#   csil-breaking   check the working schema against a baseline (BASELINE=<dir|file>)
#   gen             validate, then regenerate the Rust server + all client bindings
#   gen-server      regenerate only the Rust server bindings
#   gen-clients     regenerate all client-language bindings
#
# Rust:
#   build | test | fmt | clippy | check
#
# Local dev:
#   dev-run-local   build + serve against a local data dir (default ~/data/ichoi, laid out
#                   as <data>/{music,audiobooks,database}). Override ICHOI_DATA_DIR; force a
#                   fresh web build with WEB_BUILD=1. Ctrl-C to stop.
#
# csilgen resolution: uses `csilgen` on PATH if present, else `cargo run` from a sibling
# csilgen checkout (override with CSILGEN_REPO=/path/to/csilgen). The WASM generators must
# be installed once in that checkout:  cargo run -p xtask install-wasm

set -euo pipefail

# Repo-level tooling. The server target's source lives under server/ (the multiple-directory
# release layout); this script drives it from the repo root.
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SERVER="$ROOT/server"
SCHEMA="$SERVER/schema"
ENTRY="$SCHEMA/ichoi.csil"
GEN_ROOT="$SERVER/generated"
CSILGEN_REPO="${CSILGEN_REPO:-$ROOT/../csilgen}"

# The Rust server binding we implement against.
SERVER_TARGET="rust-server"

# Every client language we publish bindings for. csilgen generates one target per run.
CLIENT_TARGETS=(
  rust-client
  typescript-client
  go-client
  python-client
  java-client
  kotlin-client
  swift-client
  dart-client
  csharp-client
  elixir-client
  ocaml-client
  ruby-client
  c-client
  zig-client
)

# Resolve the csilgen invocation into an array command prefix.
if command -v csilgen >/dev/null 2>&1; then
  CSILGEN=(csilgen)
elif [ -f "$CSILGEN_REPO/Cargo.toml" ]; then
  CSILGEN=(cargo run --quiet --manifest-path "$CSILGEN_REPO/Cargo.toml" -p csilgen --)
else
  echo "error: no 'csilgen' on PATH and no csilgen checkout at CSILGEN_REPO=$CSILGEN_REPO" >&2
  exit 1
fi

csil_validate() { "${CSILGEN[@]}" validate --input "$ENTRY"; }
csil_lint()     { "${CSILGEN[@]}" lint "$SCHEMA"; }
csil_fmt()      { "${CSILGEN[@]}" format "$SCHEMA"; }
csil_breaking() { "${CSILGEN[@]}" breaking --current "${BASELINE:?set BASELINE=<dir|file>}" --new "$ENTRY"; }

gen_one() {
  local target="$1"
  local out="$GEN_ROOT/$target"
  mkdir -p "$out"
  echo ">> generate $target"
  "${CSILGEN[@]}" generate --input "$ENTRY" --target "$target" --output "$out"
}

gen_server()  { gen_one "$SERVER_TARGET"; }
gen_clients() { local t; for t in "${CLIENT_TARGETS[@]}"; do gen_one "$t"; done; }
gen_all()     { csil_validate; gen_server; gen_clients; }

rust_build()  { ( cd "$SERVER" && cargo build --workspace ); }
rust_test()   { ( cd "$SERVER" && TEST_DATABASE_BACKEND=sqlite cargo test --workspace ); }
rust_fmt()    { ( cd "$SERVER" && cargo fmt --all ); }
rust_clippy() { ( cd "$SERVER" && cargo clippy --workspace --all-targets -- -D warnings ); }

# Build the web UI (only if it hasn't been built yet). Override with WEB_BUILD=1 to force.
web_build_if_needed() {
  local dist="$SERVER/web/themes/default/dist/index.html"
  if [ -f "$dist" ] && [ "${WEB_BUILD:-0}" != "1" ]; then
    return 0
  fi
  echo ">> building web UI (server/web/themes/default)"
  # shellcheck disable=SC1090
  source "${CATALYST_TOOLS:-$HOME/.local/catalyst-tools}/env.sh" 2>/dev/null || true
  ( cd "$SERVER/web/themes/default" && npm install && npm run build )
}

# Build ichoi (debug) and serve it against a local data directory laid out as
# <data>/{music,audiobooks,database}. Defaults to ~/data/ichoi; override with
# ICHOI_DATA_DIR. Ctrl-C stops it. Forces a fresh web build with WEB_BUILD=1.
dev_run_local() {
  local data="${ICHOI_DATA_DIR:-$HOME/data/ichoi}"
  if [ ! -d "$data/music" ]; then
    echo "error: $data/music not found (set ICHOI_DATA_DIR to your data root)" >&2
    exit 1
  fi
  web_build_if_needed
  echo ">> building ichoi (debug)"
  ( cd "$SERVER" && cargo build -p ichoi )
  echo ">> serving $data on http://localhost:4042  (csil :4043)"
  ICHOI_MUSIC_DIR="$data/music" \
  ICHOI_AUDIOBOOK_DIR="$data/audiobooks" \
  ICHOI_DB_DIR="$data/database" \
  ICHOI_WEB_DIR="$SERVER/web/themes/default/dist" \
  ICHOI_LOG="${ICHOI_LOG:-info}" \
    exec "$SERVER/target/debug/ichoi" serve
}

case "${1:-}" in
  csil-validate) csil_validate ;;
  csil-lint)     csil_lint ;;
  csil-fmt)      csil_fmt ;;
  csil-breaking) csil_breaking ;;
  gen|regen)     gen_all ;;
  gen-server)    gen_server ;;
  gen-clients)   gen_clients ;;
  build)         rust_build ;;
  test)          rust_test ;;
  fmt)           rust_fmt ;;
  clippy)        rust_clippy ;;
  check)         csil_validate; rust_fmt; rust_clippy; rust_test ;;
  dev-run-local) dev_run_local ;;
  *)
    echo "usage: ./tools.sh {csil-validate|csil-lint|csil-fmt|csil-breaking|gen|gen-server|gen-clients|build|test|fmt|clippy|check|dev-run-local}" >&2
    exit 1 ;;
esac
