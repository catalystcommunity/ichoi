#!/bin/sh
set -eu

# Run this script in an archive Release build. Only the permanent IOS-00 bundle ID
# can reach App Store Connect. A development bundle ID must never reach it.
permanent_bundle_id="com.catalystsquad.ichoi"
bundle_id="${PRODUCT_BUNDLE_IDENTIFIER:-}"
configuration="${CONFIGURATION:-Release}"
if [ "$configuration" = "Release" ] && [ "$bundle_id" != "$permanent_bundle_id" ]; then
  echo "ERROR: Release bundle ID '$bundle_id' is not the permanent ID $permanent_bundle_id." >&2
  exit 1
fi
echo "Bundle ID accepted for $configuration: $bundle_id"
