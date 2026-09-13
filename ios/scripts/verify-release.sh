#!/bin/sh
set -eu

# Run this script in an archive Release build. It is intentionally strict while
# IOS-00 is blocked: a development bundle ID must never reach App Store Connect.
bundle_id="${PRODUCT_BUNDLE_IDENTIFIER:-}"
configuration="${CONFIGURATION:-Release}"
if [ "$configuration" = "Release" ] && {
  [ -z "$bundle_id" ] ||
  [ "$bundle_id" = "com.catalystcommunity.ichoi.dev" ] ||
  [ "$bundle_id" = '$(ICHOI_PERMANENT_BUNDLE_ID)' ];
}; then
  echo "ERROR: IOS-00 is blocked. Select and set the permanent bundle ID before release." >&2
  exit 1
fi

if [ "$configuration" = "Release" ]; then
  case "$bundle_id" in
    *ichoi.invalid*|*dev*) echo "ERROR: development or placeholder bundle ID cannot be archived." >&2; exit 1 ;;
  esac
fi
echo "Bundle ID accepted for $configuration: $bundle_id"
