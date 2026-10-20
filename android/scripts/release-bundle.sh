#!/usr/bin/env bash
# Build the signed Play bundle for one release version.
#
#   android/scripts/release-bundle.sh 0.1.0
#
# The version code is major * 10000 + minor * 100 + patch, so it always increases.
# The upload key comes from the Reactorcide secret path nsc/ichoi-android. A CI job can
# instead set ICHOI_UPLOAD_KEYSTORE_B64 and the three ICHOI_UPLOAD_* password variables.
# The script prints no secret value. It deletes the temporary keystore on exit.
set -euo pipefail
set +x

version="${1:?usage: release-bundle.sh X.Y.Z}"
if [[ ! "$version" =~ ^([0-9]+)\.([0-9]{1,2})\.([0-9]{1,2})$ ]]; then
  echo "ERROR: version must be X.Y.Z with minor and patch below 100" >&2; exit 1
fi
code=$(( BASH_REMATCH[1] * 10000 + 10#${BASH_REMATCH[2]} * 100 + 10#${BASH_REMATCH[3]} ))

root="$(cd "$(dirname "$0")/.." && pwd)"
umask 077
keystore="$(mktemp --suffix=.jks)"
trap 'rm -f "$keystore"' EXIT

if [ -z "${ICHOI_UPLOAD_KEYSTORE_B64:-}" ]; then
  unset REACTORCIDE_API_URL REACTORCIDE_API_TOKEN
  export REACTORCIDE_SECRETS_PASSWORD="${REACTORCIDE_SECRETS_PASSWORD:-$(cat ~/.reactorcide-pass)}"
  get() { reactorcide secrets get nsc/ichoi-android "$1"; }
  get upload_keystore_b64 | base64 -d > "$keystore"
  export ICHOI_UPLOAD_KEYSTORE_PASSWORD="$(get upload_keystore_password)"
  export ICHOI_UPLOAD_KEY_ALIAS="$(get upload_key_alias)"
  export ICHOI_UPLOAD_KEY_PASSWORD="$(get upload_key_password)"
  unset REACTORCIDE_SECRETS_PASSWORD
else
  printf '%s' "$ICHOI_UPLOAD_KEYSTORE_B64" | base64 -d > "$keystore"
  unset ICHOI_UPLOAD_KEYSTORE_B64
fi
export ICHOI_UPLOAD_KEYSTORE="$keystore"

cd "$root"
./gradlew --no-daemon -q clean check bundleRelease \
  -PichoiVersionName="$version" -PichoiVersionCode="$code"

aab="$root/app/build/outputs/bundle/release/app-release.aab"
jarsigner -verify "$aab" >/dev/null
echo "Signed bundle: $aab"
echo "Version: $version ($code)"
keytool -printcert -jarfile "$aab" | grep -m1 "SHA256:" | sed 's/^[[:space:]]*/Upload certificate /'
