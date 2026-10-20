# Ichoi Android client

This is a native Kotlin client for an Ichoi server. It supports Android 10 (API 29) and
later. The app streams media in memory and does not make a persistent media cache.

The generated CSIL Kotlin client is an external source input at
`../server/generated/kotlin-client`. Do not edit that directory by hand. Regenerate it from
the CSIL schema when the schema changes.

The client uses the server CSIL-Events WebSocket. LinkKeys login returns through the
allow-listed `ichoi://linkkeys` callback. The app stores each session token through the
Android Keystore. HTTPS profiles require at least one TLS SHA-256 pin. Private HTTP
profiles require explicit consent and do not use a TLS pin.

## Checks

Run `./gradlew test`, `./gradlew lint`, `./gradlew assembleDebug`, and
`./gradlew androidCompliance`. The compliance task also checks the resolved dependency
graph. The release application ID is `com.catalystsquad.ichoi`. Debug builds use
`com.catalystsquad.ichoi.dev`. The `verifyStoreRelease` task rejects a release build with a
different ID.

Physical-device checks remain external: test on Android 10 and the current Android release,
including first run, HTTPS pinning, private HTTP consent, redirects, playback, sign-out,
account deletion failure retention, and report submission. No production demo URL is
selected in this project.

## Release signing

Google Play App Signing holds the app signing key. The project holds only the upload key.
The Reactorcide secret path `nsc/ichoi-android` holds the upload key. It has these keys:
`upload_keystore_b64`, `upload_keystore_password`, `upload_key_alias`, and
`upload_key_password`. The keystore is PKCS12, so the two passwords are the same. Do not
put the keystore in this repository.

Build the signed Play bundle with `scripts/release-bundle.sh X.Y.Z`. The script reads the
upload key from the local Reactorcide store and deletes its temporary copy on exit. A CI
job can set `ICHOI_UPLOAD_KEYSTORE_B64` and the `ICHOI_UPLOAD_*` password variables instead.
The version code is major × 10000 + minor × 100 + patch. `bundleRelease` fails without the
upload key.
