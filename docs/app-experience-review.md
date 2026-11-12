# App experience review

Reviewed on 2026-11-10. These changes are in the working tree. The live server and
satellites were not changed.

## Brand and layout

The iOS and Android launcher icons use the approved exports in `assets/brand`.
The web app uses the approved navigation mark, favicon, touch icon, and regular
and maskable PWA icons. Run `python3 scripts/install-brand-assets.py` after a
brand export. Two runs produce the same files.

The web transport now uses a grid row with automatic height. Content stays above
the controls. The row includes the bottom safe area. Narrow screens put track
information, output selection, volume, and playback actions in separate rows.
Checks at widths of 390, 768, and 1280 pixels found no overlap or page overflow.

Both mobile apps use the web palette: warm paper or dark brown surfaces, amber
actions, and cyan playback accents. Android uses system light and dark modes,
48 dp action targets, and persistent Library, Search, and Queue controls. iOS
uses system appearance, named color assets, and a direct queue toolbar action.
The iPhone review found weak subtitle contrast. Explicit text and muted colors
correct that issue. The search prompt was shortened to fit the field.

## Navigation

Queue state is separate from browsing state. Native queues support add, play,
remove, reorder, and undo. Playback advances to the next queued track.

Android keeps the previous view and its scroll position while the queue is open.
iOS presents the queue in a sheet above the existing library or search. The web
stores search text, library view options, the return route, and scroll positions
per server. It also restores the separate album and audiobook grid scroll areas.
Clear the search field or select Library or Audiobooks to start a new browse.

The Android emulator returned from the queue to the same search text and the
same visible row coordinates. A regression test also checks view identity.
The web returned to a search at 220 pixels and an audiobook grid at 2400 pixels.

## Server entry

A bare domain, including a port, defaults to HTTPS on Android and iOS. Existing
checks still reject credentials, paths, queries, fragments, and public HTTP.
Private HTTP still needs explicit consent.

The Android emulator connected after entry of `ichoi.catalystsquad.com` without
a scheme. It loaded the public demo library and search results. An isolated
iPhone simulator also loaded the demo library and its available covers.

## Playback buffer

Native satellites decode ahead into a ten-minute PCM buffer. A seek within this
buffer changes the playback cursor and playback identity without another media
open request. Pause keeps the buffer. The decoder and output run separately.
The core supports stream pause and resume for flow control. Its media output
channel holds at most eight frames. Stop still cancels the producer.

The PCM buffer has a 512 MiB safety limit. At extreme sample rates or channel
counts, this limit can shorten the ten-minute window. The hardware output buffer
is 50 ms. The full warm buffer is not a startup requirement.

Android and same-origin web playback use transient byte-range caches with a
128 MiB limit. They estimate ten minutes from file size and track duration.
Variable bitrate can change the exact time covered. Playback reads use exact
byte ranges. No media cache is written to disk. Web buffering requires an active
service worker and a server that supports ranges. Cross-origin or converted
responses retain native browser behavior. iOS requests a 600-second forward
buffer from AVPlayer; the platform controls the actual amount retained. See
[Apple’s forward buffer documentation](https://developer.apple.com/documentation/avfoundation/avplayeritem/preferredforwardbufferduration).

Cached seeks avoid another network read. Seeks outside the retained window still
need network access or decoding. No physical satellite latency claim was measured
in this review. Release the core and native satellites together for flow control.

## Validation

- Rust: the workspace suite passed. Added checks cover ten-minute cache seeks,
  pause retention, safe eviction, identity changes, reuse of the existing stream,
  and authenticated pause/resume dispatch. Clippy passes with warnings denied.
- Web: 43 tests pass. The build passes. Browser layout and scroll checks pass.
- Android: 41 tests pass. Build, lint, and compliance checks pass. Demo connection,
  bare-domain entry, search, and queue return were checked on an API 29 emulator.
  A demo track advanced through playback. A seek moved it to 2:57, and Pause
  changed the media session to paused. Stop removed the media session. The
  emulator had audio output disabled.
- iOS: 14 core tests and the unsigned simulator build pass. Static checks pass.
  An iPhone 17 simulator on iOS 26.5 was used for the visual review.

The Linux host had no KVM device. The Android emulator used software emulation.
See the [Android emulator command-line documentation](https://developer.android.com/studio/run/emulator-commandline)
for that mode. These checks do not replace physical audio and interruption tests.

## Website brand review (2026-11-11)

The website used an old letter mark and a separate teal and yellow palette.
Its dark-mode primary button used white text on a pale background. It had no
touch icon or social preview image.

The website now uses the approved light and dark marks, favicon files, touch
icon, and 1200 by 630 social image. Its surfaces and text use the app palette.
The light-mode links use a darker amber for text contrast. Primary
buttons use separate text colors for light and dark modes. Blog index templates
use the same header and metadata as the main pages.

The repository build copies the assets from the brand kit. The standalone Docker
context uses the checked-in copies. Both build paths produced identical output.
Two consecutive repository builds also produced identical output. All local
page and asset links resolve. The deployment placeholder check passes.

Browser checks covered the home, product, setup, privacy, Android, iOS, and
server pages at widths of 390, 768, and 1280 pixels in light and dark modes.
There were no broken images or horizontal overflow at those widths. The header
selected the correct mark in each mode. Normal text color pairs have a contrast
ratio of at least 4.5:1. The website changes are not deployed.

## Zest and native app changes (2026-11-11)

The web UI now uses Zest components and explicit state subscriptions. SolidJS
and its router, primitives, JSX source, and build plugin have been removed.
The library has cover grids, search, and list sorting. The persistent player
has a device selector and a queue panel with direct edit controls. Queue edits
keep the search input and results mounted. Settings requests fail separately.

Android and iOS now show cover grids and persistent device and queue controls.
Both can select shared players and edit the selected player's queue. Commands
run in order. Poll replies from an old server or device cannot replace current
state. The published builds have not been replaced.

Native transports check ServiceError before they decode a successful response.
A 401 reply now opens sign-in controls instead of reporting a missing libraries
field. Requests have a 15-second deadline, including the handshake. Existing
bare-domain normalization remains in use.

Cover requests use smaller visible images on the web, a bounded browser cache,
and separate artwork connections. The server caches thumbnails in memory.
Three local 256-pixel requests took 99, 110, and 137 ms on first access, then
2, 2, and 1 ms on repeat access. These are local measurements with a copied
catalog and local media. They do not measure the home server or its network.

Validation for this change: 20 browser reliability checks, the web unit suite
and build, 14 Rust artwork tests, and Rust Clippy pass. Android has 46 passing
unit tests, a passing lint check, and a debug build. The unsigned iOS simulator
build and 14 core tests pass. The keyboard check confirmed that Space activates
a queue Remove button. Removing a selected idle track keeps playback idle.
The browser also started a local track through the development proxy. The
Android emulator reached the new demo library. System UI errors and a later
emulator exit limited its cover-loading review. The iPhone simulator loaded
the demo grid and covers. Physical audio checks remain necessary.

The Zest component request file records a nested-button keyboard defect and a
request for a responsive virtual cover grid. The app has a local workaround for
the keyboard defect. No Zest component source was changed.

## Sidebar and native control revision (2026-11-11)

The web uses a left sidebar again. At widths up to 900 pixels, Menu opens a
modal drawer. Close, Escape, outside taps, and navigation close the drawer.
The drawer scrolls on short screens. Its heading stays visible. The queue
opens from the right and keeps the browse view mounted. Its own playback and
device controls remain accessible when the mobile dialog is open. Track titles
and editing buttons use separate rows in narrow lists.

Android has a visible Menu button on each screen. The library shows covers
before secondary shortcuts. Header Search replaces the duplicate search row.
Buttons have borders, clear
spacing, and touch targets of at least 48 dp. The primary playback action uses
the accent color. Device selection has a separate row. Queue and Stop are
separate buttons. The iOS source has the same control structure, a labeled Menu,
and buttons with at least 44-point targets. After source-transfer approval, the
iOS simulator build, 14 core tests, and static compliance checks passed. The
simulator review corrected the primary button color and the visible Menu label.

The web fixture passes 28 checks on mobile, including navigation, focus return,
queue edits, search retention, and an invalid artwork fallback. Chromium checks
cover 320, 390, and 768-pixel widths. Desktop and short landscape layouts were
also inspected. Firefox was not checked because its test browser is absent.
Android passes 51 tests, lint, and compliance checks. Google Play update tests
cover consent, background download, explicit restart, cancellation, and retry.
The signed Android bundle is version 0.1.3 (103). Nothing was uploaded.

See [mobile updates](mobile-updates.md) for the release procedure. The reported
Play installation failure reported only "update failed". Compare installation
version and signing details if the failure occurs again.

## Native music player revision (2026-11-11)

This revision replaces the native control layout described above. Android and
iOS use the web theme colors. Library, Search, and Queue are bottom navigation
items. The small player stays above navigation. Tap its title or cover to open
Now playing. Device selection remains available in both player views.

The library uses cover tiles. Search uses album rows and compact song rows.
Each song has separate Add and More controls. Album details show the cover,
title, artist, and a Play control. The queue has an Edit mode, reorder controls,
Remove, and Undo. Android also supports a drag from the row handle. iOS uses
native move and delete actions. Queue sheets retain playback controls.

Android keeps the search view when the user changes tabs. A library request can
finish while another tab is open. Search shows loading, empty, and retry states.
A decoder error caused valid Android covers to stay as placeholders. The sample
size now starts at one. A regression test decodes a real PNG. Repeated Android
cover requests share a bounded memory cache and two background workers.

The Android debug build, 56 tests, lint, and compliance checks pass. These tests
cover search retention, background library loading, queue move/remove/undo,
and the small player height with native font rendering. The signed Android
bundle is version 0.1.4 (104). Both debug and release variants pass all 56 tests. This bundle replaces the earlier 0.1.3 artifact.
Nothing was uploaded. The unsigned iOS simulator build, 14 core tests, and
static compliance checks pass. The iPhone simulator shows the library and
loaded covers. Android demo playback advanced in the muted emulator. The full
player showed the position, duration, and Pause control. Physical device checks
remain necessary for audio, accessibility, and store updates.

## Interactive playback progress (2026-11-11)

The small player and Now playing view now have an interactive progress line on
Android and iOS. Tap a position or drag the thumb. The time label shows the
selected position during the drag. Release sends one seek immediately. Playback
updates do not reset the drag position. A pending position returns to the
reported position after three seconds if the player does not confirm it.

A cancelled drag does not seek. A track change during a drag prevents the seek
from changing the new track. Android accessibility adjustments use the same
seek action. Playback with no known duration disables the control. Each small
player has elapsed and total time labels and a 48 dp Android or 44 point iOS
seek area. The iOS background prevents library text from showing below the line.

The signed Android bundle was rebuilt as 0.1.4 (104) with user approval because
the previous bundle was not uploaded. Both build variants pass 60 tests. Lint
and compliance checks pass. The iOS simulator build and 14 core tests pass.
The existing range-cache test confirms that a cached read needs no new request.
A real Android emulator check kept one track paused. A tap moved playback to
51 seconds, and a drag moved it to 306 seconds. Media-session reports confirmed
both positions and the paused state. The screenshot is
`/tmp/ichoi-android-progress.png`. A seek outside cached data can still require
a network read. Physical audio latency has not been measured.

## Next Actions

1. Check a Play update from the previous release on a physical phone.
2. Test cached seeks on the physical Linux and Windows satellites. Measure the
   command-to-audio delay and check for underruns with the shorter output buffer.
3. Check long M4B playback, network loss, and system controls on physical phones.
4. Release matching server, web, mobile, and satellite builds. Rescan the library
   to apply the artwork changes from the preceding task.
5. Include a website version change in the release to publish its brand changes.
