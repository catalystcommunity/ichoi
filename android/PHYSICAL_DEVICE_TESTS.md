# Physical device test record

These checks need a physical device or an emulator. They are external to this workspace.
Run them on Android 10 (API 29), each supported Android release up to the current release,
and one current 64-bit device.

1. Clear app data. Start the app. Confirm that the terms screen appears. Accept the terms,
   open both policy links, add an HTTPS server, and confirm that the library opens.
2. Add a public HTTP server. Confirm that the app rejects it. Add a private IPv4 and a
   private IPv6 HTTP server. Confirm that the warning appears and that Cancel does not save
   the profile. Confirm that I understand permits the connection.
3. Use a wrong certificate pin. Confirm that the connection fails. Return a redirect from
   the server. Confirm that the app reports a redirect error and does not follow it.
4. Sign in. Play a track. Lock the device and switch audio output. Confirm that the
   foreground media notification stays visible and that no media file appears in shared or
   app-private storage.
5. Open Settings. Confirm the server URL and privacy link. Sign out and confirm that a
   later request has no bearer token.
6. Start account deletion with an incorrect handle. Confirm that the session remains. Make
   the server return a timeout or an error. Confirm that the session remains and that the
   message names the selected server URL and its administrator. Delete with the exact handle
   and confirm that the local token is removed only after success.
7. As a signed-in user, report a playlist and an account. Test all four reasons, empty
   details, 2,000 Unicode scalar values, and 2,001 Unicode scalar values. Confirm that the
   last case is rejected before the request is sent.

Record the device model, Android version, app version, test date, and each result before a
store submission. AND-00 and AND-05 remain blocked until the permanent application ID and a
reviewed production demo URL exist.
