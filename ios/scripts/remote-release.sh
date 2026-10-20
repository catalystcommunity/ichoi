#!/usr/bin/env bash
# Run release-build.sh on the macOS build host from Linux.
#
#   ios/scripts/remote-release.sh X.Y.Z export|upload
#
# Copy the working tree (tracked and unignored files) to the host. Pipe the App Store
# Connect API key from the Reactorcide secret path nsc/apple over ssh. The key is never
# written to the host disk outside a temporary directory that the build deletes.
set -euo pipefail
set +x

version="${1:?usage: remote-release.sh X.Y.Z export|upload}"
mode="${2:?usage: remote-release.sh X.Y.Z export|upload}"
host="${ICHOI_MAC_HOST:-todhansmann@10.19.81.206}"
remote_dir="${ICHOI_MAC_DIR:-build/ichoi}"
repo="$(cd "$(dirname "$0")/../.." && pwd)"

# The iOS build needs only the app and the generated Swift client.
cd "$repo"
ssh "$host" "mkdir -p '$remote_dir'"
git ls-files -co --exclude-standard -- ios server/generated/swift-client \
  | rsync -a --files-from=- ./ "$host:$remote_dir/"

unset REACTORCIDE_API_URL REACTORCIDE_API_TOKEN
export REACTORCIDE_SECRETS_PASSWORD="${REACTORCIDE_SECRETS_PASSWORD:-$(cat ~/.reactorcide-pass)}"
key_id="$(reactorcide secrets get nsc/apple asc_key_id)"
issuer_id="$(reactorcide secrets get nsc/apple asc_issuer_id)"

reactorcide secrets get nsc/apple asc_api_key_p8 | ssh "$host" \
  "ASC_KEY_ID='$key_id' ASC_ISSUER_ID='$issuer_id' bash '$remote_dir/ios/scripts/release-build.sh' '$version' '$mode'"
