# Ichoi web UIs

This directory holds the **browser UIs** for Ichoi. Per DESIGN §11, the web UI
files are the only distributable that is *not* inside the Ichoi binary (the other
bundled file is ffmpeg, §5.3), so they live here as plain static assets the server
serves.

```
web/
  README.md            ← you are here
  themes/
    default/           ← the official theme (SolidJS SPA), documented below
    <your-theme>/      ← drop-in alternative UIs
```

## How the server serves a theme

The Ichoi core's HTTP listener (`:4042`) does exactly two things (DESIGN §4.1):

1. serve static UI files (`ServeDir` over a theme directory), and
2. accept the WebSocket upgrade at **`/ws`**, which carries **CSIL-Events** — the
   one API surface every client uses.

Multiple themes under `web/themes/<name>/` can be served concurrently, optionally
path-scoped, with a redirect at login (§11). Theme *settings* (e.g. default light
/ dark) live in the server database, so any theme honors them — this theme reads
them via `AdminService.get-settings` and applies `theme` if present.

A theme is served from its **build output** (`themes/<name>/dist/`), not its
source. The app is built with a **relative base** (`vite.config.ts` `base: "./"`),
so it works whether it is mounted at the site root or under a path like
`/themes/default/`. The WebSocket URL defaults to the serving origin's `/ws`, so a
browser served by the core connects straight back to it with no configuration.

## Build the default theme

Node 26 is the toolchain (`catalyst-tools` provides it):

```sh
cd web/themes/default
npm install
npm run build      # tsc --noEmit && vite build  →  dist/
npm run dev        # local dev server on :5173, proxies /ws → localhost:4042
```

`npm run build` emits `dist/` (hashed JS/CSS chunks, `index.html`, and the
`ichoi-worklet.js` audio worklet copied from `public/`). Point the server's static
handler at that `dist/` directory.

### The audio decoder (`ichoi-decoder`)

Local (private-player) playback decodes with a **Symphonia → WASM** module named
`ichoi-decoder` (DESIGN §5.5). That module is built separately and is **optional**
here: if it is absent, the UI degrades gracefully — browsing, search, playlists,
queueing, and **jukebox / shared-target control all keep working**; only local
decode-to-speakers is disabled, with a clear message. To enable it, drop the
wasm-bindgen glue `ichoi-decoder.js` + `ichoi-decoder_bg.wasm` next to the built
`index.html` (or inject `window.ichoiDecoder`); `src/lib/audio/decoder.ts` finds
it automatically.

## Bundle your own UI

A theme is just a directory of static files that talk CSIL-Events over `/ws`. To
build your own:

1. Create `web/themes/<your-name>/` and produce static assets (any framework, or
   none). Use a **relative asset base** so it can be path-mounted.
2. Speak the wire: open a WebSocket to `/ws`, complete the CSIL-Events
   `$hello` / `$hello-ack` handshake (verbose profile), then send/receive typed
   events. You can copy this theme's transport layer wholesale — it is
   self-contained and has no framework ties:
   - `src/lib/cbor.ts` — canonical CBOR (byte-exact with `csilgen-transport`),
   - `src/lib/csil.ts` — the CSIL-Events envelope, handshake, correlation, and
     channel dispatch (the wire seam is `encodeEnvelope` / `decodeEnvelope`),
   - `src/lib/services.ts` + `src/lib/schema.ts` — typed clients and record types
     mirroring `schema/*.csil`.
   - `public/ichoi-worklet.js` + `src/lib/audio/*` — the postMessage-fed playback
     pipeline.
3. Honor i18n and a11y from the start (§11) — see `ACCESSIBILITY.md`.

The transport layer is verified against csilgen's published conformance vectors,
so a UI built on it is on the same wire as native clients and satellites.

## What this theme implements

`themes/default/` is a complete SolidJS + Vite + TypeScript SPA:

- **Library** browse (albums / artists), **album** and **artist** detail, **search**.
- **Playlists** (m3u-backed, §7) with resolved-track playback.
- **Jukebox**: shared targets as live console channel strips with transport +
  volume, this client's private players, and "share this device" (§6.4).
- **Now Playing** with the local queue.
- **Settings**: appearance (theme / language), playback prefs (transcode codec,
  prefer-original, bitrate cap — the per-stream `StreamPref`, §5.1), and
  admin-gated server settings.
- **Multi-server** switcher (§7): one session per server, browse each, switch the
  active one; LinkKeys sign-in is stubbed with a clear TODO, login-less = guest.

See `themes/default/ACCESSIBILITY.md` for the a11y approach, and the header comment
in `themes/default/src/lib/csil.ts` for the wire-protocol assumptions the Rust side
must honor.
