# Mobile playback controls and messages

Android and iOS show the track title, selected playback server, progress, and
Pause or Resume and Stop controls. The controls stay available during navigation.
The progress slider changes the position in the track. Stop also cancels loading.

Android exposes playback actions in its media notification and media session.
iOS exposes them through the system media commands. Completion and playback
failure clear the current track. Android also removes its foreground notification.

Errors appear in a banner with a Dismiss button. The default duration is 10
seconds. Settings accepts a duration from 1 to 60 seconds and saves it on the
device. The new value applies to the next message. A new error replaces the old
message and starts a new timer. Errors received in the background remain
available until the app shows them.

Android also has App settings on the welcome screen. iOS has the duration setting
on the server connection screen and in Settings.

## Automated checks

- Android: run `test`, `lint`, `assembleDebug`, and `androidCompliance` with
  Gradle and Java 17. Playback tests use a simulated player and media-session
  interface. They do not confirm device audio output or lock-screen behavior.
- iOS: run `swift test` and `scripts/check-compliance.sh` from `ios/`, then build
  the Ichoi Xcode scheme with code signing disabled. The core tests check message
  timing, dismissal, replacement, and saved settings.

## Device checks

Run the playback and message checks in `android/PHYSICAL_DEVICE_TESTS.md` on
Android. Repeat them on iPhone. On iPhone, also check the controls and error
messages while Settings or the report sheet is open. Check system media controls
in Control Center and on the lock screen.

Record the device, OS version, app version, date, and results before release.

## M4B playback

Direct iOS playback now uses authenticated byte ranges, with at most 512 KiB per
request. The player can start and seek without a full audiobook download. Stop
cancels pending requests. Server-converted audio retains the buffered path, with
a 64 MiB limit. See `cover-art.md` for codec details and current limits.

On macOS, run `ios/scripts/check-m4b-playback.sh`. It checks the real resource
loader with a synthetic two-chapter M4B file and a local server. It verifies
PCM decoding from the one-second position, authenticated range requests, and
cancellation during preparation. This check does not replace phone testing.

On each physical phone, play a long M4B file. Check the initial start, a seek near
the end, pause, resume, stop during loading, and loss of connection.


## Search and queue return

Open a search with enough results to scroll. Add a track to the queue. Open Now
Playing, change the queue, and go back. Check that the query and visible rows
stay the same. Repeat with music albums and audiobooks. Clear the query or select
the library entry to start a new browse. See `app-experience-review.md` for results
and buffer limits.
