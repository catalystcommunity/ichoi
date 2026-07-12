# Ichoi — Implementation Status (pre-alpha)

A snapshot of what is built against `docs/DESIGN.md`, what is verified, and what is stubbed.
**This runs as a whole system** — a browser loads the SPA, connects over CSIL-Events,
browses the real library, and plays audio from the server. Section numbers reference
DESIGN.md.

## Verified working (driven end-to-end, not just compiled)

- **Full stack in a real browser.** Loaded the built SolidJS SPA served by `ichoi serve`,
  which connected over the **real CSIL-Events WebSocket wire** (`$hello`→`$hello-ack`),
  listed albums with real scanned data, opened an album showing per-track FLAC metadata
  (codec / 44.1 kHz / 16-bit / duration), and on **Play** fired `GET /media/<id>` returning
  `206` audio to a native `<audio>` element. (Actual sound needs a real audio device + user
  gesture; headless autoplay is blocked — that's the only step not machine-verified here.)
- **CSIL-Events transport** (`transport.rs`): verbose-profile envelope
  `{event, payload:24(bstr), service?, id?}`, the `$hello`/`$hello-ack` handshake with auth
  → identity resolution, request/response correlation by `id`, errors carried as a
  `{code,message}` map — matching the web UI's vector-conformant codec. Covered by
  integration tests that build the exact browser frames.
- **HTTP media** (`media_http.rs`): direct byte-range serving (verified `206` with correct
  `Content-Range`) and ffmpeg transcode piping — verified producing real **AAC** (`?bitrate=96`)
  and **MP3** (`?format=mp3&bitrate=64`) below source bitrate, and serving the original when
  the cap is above source.
- **22 tests pass**, incl. DataUtils integration tests (real SQLite, rolled-back
  transactions, `DataMap` factories) and the CSIL-Events wire tests. `cargo clippy
  --workspace` is clean.
- **`ichoi serve`**: migrations → transforms → background scan → HTTP `:4042` + CSIL/TCP
  `:4043`, Ctrl-C shutdown. **Scanner** indexed real FLAC/MP3 (tags + technical properties).
  **Audio detection** via runtime `dlopen` of `libasound.so.2` (reported `"some"`; scratch
  container → `"none"`, still boots).

## DB-backed handlers (§ handlers)

All request/response operations of SessionService, LibraryService, PlayerService (+ share
enable/disable with the `"<Handle>'s <suffix>"` naming), AdminService, and
NodeService.register are wired to the store and reachable over both the WS and TCP paths.
`player.Subscribe` pushes a one-time state snapshot.

## Stubbed / TODO (honest gaps)

- **Symphonia→WASM decoder** (`ichoi-decoder.wasm`) does not exist (no wasm toolchain here).
  The design's pure media path (CSIL-Events chunks → WASM decode → AudioWorklet) is therefore
  inert; **the HTTP `/media` + `<audio>` path is a testing bridge** until it lands
  (`server/media_http.rs`, `stores/playback.tsx`). This is the main deviation from §5.
- **Live jukebox push**: `player.Subscribe` sends one snapshot, not a live stream; controlling
  a shared target doesn't yet fan updates to other subscribers (§6.5). `media.Stream` /
  `node.Session` are accepted but push nothing.
- **LinkKeys auth**: first-admin bootstrap + session minting work; the LinkKeys assertion path
  is a placeholder (no `linkkeys-rpc-client` verification yet, §7.1).
- **TLS + key rotation** (§4.2): the CSIL/TCP surface is plain; the three auto-rotating pinned
  keys are not implemented. Browser TLS is expected via a reverse proxy (deploy/Caddyfile).
- **Satellite role**: outbound node client, dlopen PCM playback, and the directive/report loop
  are not built; `--role satellite` warns and runs core surfaces.

## Known upstream issue

Resolved: the csilgen `rust-*` clean-build request is `Status: done`; regenerated output is
fmt/clippy clean. We still relax lints at the include site defensively.

## Run it

```sh
# 1. Build the server
cargo build --release          # → target/release/ichoi

# 2. Build the web UI (needs Node 20+)
cd web/themes/default && npm install && npm run build && cd -

# 3. Serve (point at a music folder; the DB is created under it)
export ICHOI_MUSIC_DIR=/path/to/music
export ICHOI_WEB_DIR=web/themes/default/dist
./target/release/ichoi scan    # index the library (optional; serve also scans)
./target/release/ichoi serve    # http :4042 (SPA + /ws + /media), csil :4043

# 4. Open http://localhost:4042  → browse and play.
```

Container: the multi-arch `Dockerfile` bundles the binary, a static LGPL audio-only ffmpeg,
and the built web UI; `deploy/docker-compose.yml` adds Caddy for browser TLS. Config is all
`ICHOI_`-prefixed env (§9).
