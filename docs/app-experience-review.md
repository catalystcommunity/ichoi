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

## Next Actions

1. Test cached seeks on the physical Linux and Windows satellites. Measure the
   command-to-audio delay and check for underruns with the shorter output buffer.
2. Check long M4B playback, network loss, and system controls on physical phones.
3. Release matching server, web, mobile, and satellite builds. Rescan the library
   to apply the artwork changes from the preceding task.
4. Include a website version change in the release to publish its brand changes.
