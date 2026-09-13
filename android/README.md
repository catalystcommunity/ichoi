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
graph. A release build is intentionally blocked while the
development application ID is active. AND-00 must select a permanent ID before store work.

Physical-device checks remain external: test on Android 10 and the current Android release,
including first run, HTTPS pinning, private HTTP consent, redirects, playback, sign-out,
account deletion failure retention, and report submission. No permanent application ID or
production demo URL is selected in this project.
