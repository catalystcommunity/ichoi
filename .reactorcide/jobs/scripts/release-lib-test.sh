#!/bin/sh
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
# shellcheck source=release-lib.sh
. "${SCRIPT_DIR}/release-lib.sh"

TEST_DIR=$(mktemp -d)
trap 'rm -rf "${TEST_DIR}"' EXIT

mkdir -p "${TEST_DIR}/bin" "${TEST_DIR}/repo/server/version"
cat > "${TEST_DIR}/bin/gh" <<'EOF'
#!/bin/sh
case "${GH_RELEASES:-}" in
  *"|$3|"*) exit 0 ;;
  *) exit 1 ;;
esac
EOF
chmod +x "${TEST_DIR}/bin/gh"
PATH="${TEST_DIR}/bin:${PATH}"
export PATH

cd "${TEST_DIR}/repo"
git init -q
git config user.name test
git config user.email test@example.invalid
echo 0.3.1 > server/version/VERSION.txt
git add server/version/VERSION.txt
git commit -qm 'initial release'
git tag server/v0.4.0

published="${TEST_DIR}/published.txt"
: > "${published}"
GITHUB_PAT=test-token
export GITHUB_PAT

# A tag without a GitHub Release is recovered.
GH_RELEASES=''
export GH_RELEASES
recover_unreleased_targets "server" "${published}" example/ichoi
test "$(cat "${published}")" = "server 0.4.0 server/v0.4.0"

# An already-selected target is not duplicated.
recover_unreleased_targets "server" "${published}" example/ichoi
test "$(wc -l < "${published}" | tr -d ' ')" = 1

# A tag with an existing GitHub Release needs no recovery.
: > "${published}"
GH_RELEASES='|server/v0.4.0|'
export GH_RELEASES
recover_unreleased_targets "server" "${published}" example/ichoi
test ! -s "${published}"

echo "release-lib tests passed"
