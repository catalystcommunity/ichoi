#!/bin/sh
set -eu

root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
plist="$root/Ichoi/Info.plist"
privacy_manifest="$root/Ichoi/PrivacyInfo.xcprivacy"
source="$root/Ichoi"

# This check does not inspect generated Swift. Generated files are inputs to Xcode.
for forbidden in exportManifest exportChunk beginImport importChunk finishImport importTrack cancelImport; do
  if grep -R -E "[.]${forbidden}[(]" "$source" >/dev/null 2>&1; then
    echo "ERROR: prohibited generated operation used: $forbidden" >&2
    exit 1
  fi
done
if grep -Eq 'NSPhotoLibraryUsageDescription|NSCameraUsageDescription|NSContactsUsageDescription|NSBluetooth|UIFileSharingEnabled|LSSupportsOpeningDocumentsInPlace' "$plist"; then
  echo "ERROR: storage, Bluetooth, photo, or contacts permission found" >&2
  exit 1
fi
grep -q '<string>audio</string>' "$plist"
grep -q 'NSAllowsLocalNetworking' "$plist"
if grep -q 'NSAllowsArbitraryLoads' "$plist"; then
  echo "ERROR: broad App Transport Security exception found" >&2
  exit 1
fi
grep -q 'https://catalystichoi.com/privacy.html' "$source/Config.swift"
grep -q 'PRODUCT_BUNDLE_IDENTIFIER' "$root/Config/Debug.xcconfig"
grep -q 'verify-release.sh' "$root/Ichoi.xcodeproj/project.pbxproj"
grep -q 'GeneratedClientCompatibility.swift' "$root/Ichoi.xcodeproj/project.pbxproj"
grep -q 'IchoiCore' "$root/Ichoi.xcodeproj/project.pbxproj"
grep -q 'PrivacyInfo.xcprivacy' "$root/Ichoi.xcodeproj/project.pbxproj"
grep -q 'NSPrivacyAccessedAPICategoryUserDefaults' "$privacy_manifest"
grep -q '<string>CA92.1</string>' "$privacy_manifest"
grep -q '<key>NSPrivacyTracking</key>' "$privacy_manifest"
grep -q '<false/>' "$privacy_manifest"
grep -q 'appendingPathComponent("media")' "$source/ContentView.swift"
grep -q 'forHTTPHeaderField: "Authorization"' "$source/PlaybackManager.swift"
if grep -R -F '/media/stream' "$source" >/dev/null 2>&1; then
  echo "ERROR: obsolete media endpoint found" >&2
  exit 1
fi
echo "iOS static compliance checks passed"
