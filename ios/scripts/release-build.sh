#!/bin/bash
# Test, archive, and export or upload the iOS app on macOS.
#
#   release-build.sh X.Y.Z export|upload < AuthKey.p8
#
# Read the App Store Connect API key on stdin. Do not pass it as a file argument.
# Set ASC_KEY_ID and ASC_ISSUER_ID. The build number is major * 10000 + minor * 100 + patch.
# "export" signs an App Store package in build/export. "upload" sends it to App Store Connect.
set -euo pipefail
set +x

version="${1:?usage: release-build.sh X.Y.Z export|upload}"
mode="${2:?usage: release-build.sh X.Y.Z export|upload}"
: "${ASC_KEY_ID:?set ASC_KEY_ID}" "${ASC_ISSUER_ID:?set ASC_ISSUER_ID}"
if [[ ! "$version" =~ ^([0-9]+)\.([0-9]{1,2})\.([0-9]{1,2})$ ]]; then
  echo "ERROR: version must be X.Y.Z with minor and patch below 100" >&2; exit 1
fi
build=$(( BASH_REMATCH[1] * 10000 + 10#${BASH_REMATCH[2]} * 100 + 10#${BASH_REMATCH[3]} ))
case "$mode" in export|upload) ;; *) echo "ERROR: mode must be export or upload" >&2; exit 1 ;; esac

root="$(cd "$(dirname "$0")/.." && pwd)"
umask 077
keydir="$(mktemp -d)"
trap 'rm -rf "$keydir"' EXIT
key="$keydir/AuthKey_$ASC_KEY_ID.p8"
cat > "$key"
test -s "$key" || { echo "ERROR: no API key on stdin" >&2; exit 1; }
auth=(-allowProvisioningUpdates
  -authenticationKeyPath "$key"
  -authenticationKeyID "$ASC_KEY_ID"
  -authenticationKeyIssuerID "$ASC_ISSUER_ID")

cd "$root"
sh scripts/check-compliance.sh
swift test
rm -rf build/Ichoi.xcarchive build/export

# Archive unsigned. The export signs with the cloud-managed distribution certificate
# through the API key. This needs no development profile, device, or unlocked keychain.
xcodebuild -project Ichoi.xcodeproj -scheme Ichoi -configuration Release \
  -destination generic/platform=iOS -archivePath build/Ichoi.xcarchive \
  MARKETING_VERSION="$version" CURRENT_PROJECT_VERSION="$build" \
  CODE_SIGNING_ALLOWED=NO archive

options="$keydir/ExportOptions.plist"
cp ExportOptions.plist "$options"
plutil -replace destination -string "$mode" "$options"
xcodebuild -exportArchive -archivePath build/Ichoi.xcarchive \
  -exportOptionsPlist "$options" -exportPath build/export "${auth[@]}"

echo "Version: $version ($build), mode: $mode"
