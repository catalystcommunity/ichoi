# Ichoi web UI

The default theme uses Zest web components, Vite, and TypeScript. It has no
SolidJS dependency. The server serves the static files from
`themes/default/dist`. Clients use CSIL-Events over `/ws`.

## Build and check

Run these commands from `server/web/themes/default` with Node 26:

```sh
npm install
npm test
npm run build
npm run dev
```

The development server uses port 5173. It forwards `/ws`, `/api`, and `/media` to port 4042. Do not run
`npm ci` while a browser check uses the development server.

Open `http://localhost:5173/tests/browser/reliability.html` in a separate browser
profile. The fixture uses simulated replies. It restores local storage when it
ends. It checks request failures, retry, partial catalog results, queue edits,
search retention, and device selection.

## Components and state

`src/app.ts` owns the router, navigation, player, and queue panel. The queue stays
outside the browse view. Opening it does not replace search input or results.
`src/ui` contains the library, details, settings, device, and playback views.

Zest supplies the component lifecycle, router, controls, dialogs, notices,
virtual track list, and virtual range calculation. Ichoi supplies a responsive
cover grid with sparse pages. The published Zest versions match the sibling
`zest` repository at this change. Component requests are in that repository's
`component-bugs.md`.

The server and playback stores use explicit state cells from Zest's data service.
Subscriptions have cleanup functions. Network replies check the active request
before they change a view. The existing audio and queue engines remain in use.

The interface has album and artist browsing, audiobook progress, federated
search, playlists, account controls, reports, and administration. The persistent
player has a device selector and a queue button. Queue rows have Play, Move up,
Move down, and Remove controls. Undo restores queue edits. The More button gives
narrow screens access to volume, shuffle, repeat, and the previous track.

The interface is currently English. The domain message translator remains in
`src/lib/i18n.ts`. See `themes/default/ACCESSIBILITY.md` for access checks and gaps.

## Request failures

The connection handshake has a 15-second deadline. Each request has a 15-second
limit that includes the handshake. An expired request is not sent later. The
client does not automatically repeat a request that can change server data.

Failed reads show an error, technical details, and Retry. Settings panels load
separately. Reports do not wait for the account list. Account data is requested
when an administrator deletes a reported account. A failed page keeps navigation
and playback controls available.

Cover grids request pages near the visible area. List view loads all pages for
sorting. A page failure retains successful pages. Retry requests failed pages.

## Artwork

Visible covers request images of up to 256 pixels. Two artwork connections per
server keep these reads separate from library and playback requests. At most two
cover requests run at once per server. A browser memory cache retains up to 512
results. Missing covers remain cached until an artwork change. Other failures
can be retried. An artwork change clears that server's cover cache.

A dedicated server thread handles artwork reads and owns a thumbnail cache.
The cache defaults to 200 MB. Set `ICHOI_ARTWORK_CACHE_MB` or the TOML key
`artwork_cache_mb` to change it. Set zero to disable caching. Restart the server
to apply a change. The least recently used images are removed to meet the byte
limit. There is no fixed entry limit. See `docs/cover-art.md` for details. File keys
include path, picture index, size, modification time, image hash, and requested
thumbnail size. Uploaded image keys include the data hash and requested size.
No thumbnail files are written to disk.

## Alternative themes

A theme is a directory of static files. Use the handwritten files in `src/lib`
for examples of CBOR, the CSIL handshake, typed service calls, and audio playback.
The `public/ichoi-worklet.js` file supports browser audio. Local decode also needs
`ichoi-decoder.js` and `ichoi-decoder_bg.wasm`, or an injected decoder module.
Shared-device control does not need that decoder.

## Next Actions

Before release, run the checks above and the native app checks. After deployment,
check Settings, Audiobooks, cover loading, and shared playback on the target
instance. The working-tree changes do not update a running instance.
