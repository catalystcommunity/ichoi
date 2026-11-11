# Physical device test record

These checks need a physical device or an emulator. They are external to this workspace.
Run them on Android 10 (API 29), each supported Android release up to the current release,
and one current 64-bit device.

1. Clear app data. Start the app. Confirm that the terms screen appears. Accept the terms,
   open both policy links, add an HTTPS server, and confirm that the library opens.
2. Add a public HTTP server. Confirm that the app rejects it. Add a private IPv4 and a
   private IPv6 HTTP server. Confirm that the warning appears and that Cancel does not save
   the profile. Confirm that I understand permits the connection.
3. Tap **Connect to demo server**. Confirm that the library opens with no login. Use a
   server with an expired or self-signed certificate. Confirm that the connection fails.
   Return a redirect from the server. Confirm that the app reports a redirect error and does not follow it.
4. Sign in with LinkKeys. Confirm that the library opens at the top of the screen.
   Confirm that **Blocked accounts** appears only in Settings. Repeat with a slow
   server: open Settings, tap **Back to library**, and wait for the account list
   response. Confirm that the response does not change the library screen.
   Cancel a sign-in and confirm that the app does not show a successful sign-in.
   Play a track. Lock the device and switch audio output. Confirm that the
   foreground media notification stays visible and that no media file appears in shared or
   app-private storage.
5. Open Settings. Confirm the server URL and privacy link. Sign out and confirm that a
   later request has no bearer token.
6. Start account deletion with an incorrect handle. Confirm that the session remains. Make
   the server return a timeout or an error. Confirm that the session remains and that the
   message names the selected server URL and its administrator. Delete with the exact handle
   and confirm that the local token is removed only after success. Confirm that the server
   removes each public and private playlist that the account owns.
7. As a signed-in user, report a playlist and an account. Test all four reasons, empty
   details, 2,000 Unicode scalar values, and 2,001 Unicode scalar values. Confirm that the
   last case is rejected before the request is sent.
8. Block a playlist owner. Confirm that the playlist list no longer shows that account's
   public playlists. Open Settings and unblock the account. Confirm that its public
   playlists appear again. Repeat the check with two server profiles. Confirm that the
   block affects only the selected server.

Record the device model, Android version, app version, test date, and each result before a
store submission.

## Playback controls and messages

1. Play a track. Confirm that its title, server, and progress remain visible when
   you open the library or Settings. Pause, resume, seek, and stop the track.
2. Use Pause, Resume, and Stop in the notification. Repeat from the lock screen.
   Confirm that Stop removes the playback notification and releases the player.
3. Use a slow connection. Tap Stop while the track loads. Confirm that audio does
   not start later. Start a second track during loading and confirm that only the
   second track plays.
4. Let a track finish. Confirm that the controls and notification disappear.
5. Make a media request fail. Confirm that playback stops and an error message
   appears. Confirm that Dismiss closes it immediately. Repeat without Dismiss
   and confirm that the message closes after 10 seconds.
6. In Settings, change Message duration to 30 seconds. Repeat the failure and
   confirm that the message stays visible for 30 seconds. Restart the app and
   confirm that the setting is saved. Restore 10 seconds after the check.
7. Cause a second error before the first message closes. Confirm that the new
   message gets the full selected duration. Open another screen and confirm that
   navigation does not restart the timer.
8. Make playback fail while the app is in the background. Return to the app.
   Confirm that the error appears for the selected duration.
