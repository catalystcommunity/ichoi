#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
manifest="$root/app/src/main/AndroidManifest.xml"
source="$root/app/src/main/java"

allowed='android.permission.INTERNET|android.permission.ACCESS_NETWORK_STATE|android.permission.FOREGROUND_SERVICE|android.permission.FOREGROUND_SERVICE_MEDIA_PLAYBACK'
bad_permissions=$(sed -n 's/.*uses-permission android:name="\([^"]*\)".*/\1/p' "$manifest" | grep -Ev "^($allowed)$" || true)
test -z "$bad_permissions" || { echo "Forbidden Android permission: $bad_permissions" >&2; exit 1; }
test "$(grep -c 'uses-permission' "$manifest")" -eq 4 || { echo "Manifest permission set is not exact" >&2; exit 1; }
grep -q 'foregroundServiceType="mediaPlayback"' "$manifest"

# Generated CSIL sources are an input and are intentionally not audited as app source.
if rg -n -i 'analytics|advertis|billing|in-app.purchase|crashlytics|sentry|firebase|updater|remote.?code' "$source"; then
  echo "Prohibited SDK or monetization reference" >&2; exit 1
fi
if rg -n 'export-manifest|export-chunk|begin-import|import-chunk|finish-import|import-track|cancel-import' "$source"; then
  echo "Prohibited server operation referenced by handwritten app source" >&2; exit 1
fi
if rg -n 'FileOutputStream|FileInputStream|openFileOutput|MediaStore|DownloadManager|Room\.databaseBuilder|\.download|copyTo\(' "$source"; then
  echo "Persistent media or download operation found" >&2; exit 1
fi
grep -q 'applicationId = "community.catalyst.ichoi.dev"' "$root/app/build.gradle.kts"
grep -q 'STORE_RELEASE_ENABLED.*false' "$root/app/build.gradle.kts"
echo "Android compliance checks passed"
