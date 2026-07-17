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
  `206` audio to a native `<audio>` element.
- **CSIL-Events transport** (`transport.rs`): verbose-profile envelope
  `{event, payload:24(bstr), service?, id?}`, the `$hello`/`$hello-ack` handshake with auth
  → identity resolution, request/response correlation by `id`, errors carried as a
  `{code,message}` map — matching the web UI's vector-conformant codec. Covered by
  integration tests that build the exact browser frames.
- **Browser HTTP media** (`media_http.rs`): direct byte-range serving (verified `206` with correct
  `Content-Range`) and ffmpeg transcode piping — verified producing real **AAC** (`?bitrate=96`)
  and **MP3** (`?format=mp3&bitrate=64`) below source bitrate, and serving the original when
  the cap is above source.
- **Satellite CSIL media**: satellite mode is outbound-only. It authenticates to the core
  CSIL/TCP listener with a pre-shared node token, registers output devices, receives
  `NodeService.Session` directives, requests `MediaService.Stream`, receives `MediaEvent`
  chunks, and sends `NodeReport` progress on the same duplex CSIL/TCP connection. Frame reads
  are handled by dedicated reader tasks so outbound writes cannot cancel a partial length-
  prefixed read. It starts decoding as soon as `MediaHeader` + chunks arrive, decodes locally
  with Symphonia, and writes PCM to ALSA via runtime `dlopen` on Linux. The satellite path
  does not expose or call HTTP. Verified on a local Linux/PipeWire/ALSA host by selecting the
  satellite target in the guest UI and playing a real MP3 with audible output and advancing UI
  progress.
- **28 tests pass**, incl. DataUtils integration tests (real SQLite, rolled-back
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
`player.Subscribe` gets an initial snapshot and live pushes when player state changes.

## Stubbed / TODO (honest gaps)

- **Browser playback is still native HTTP audio.** Satellites no longer need a WASM decoder:
  they use native Symphonia on the node. The only reason to revive
  `ichoi-decoder.wasm` would be a future browser-only CSIL media path
  (`CSIL-Events chunks -> AudioWorklet`) instead of the current `/media` + `<audio>` path.
- **Live jukebox polish**: player state fan-out is in place for control/report changes. The
  remaining work is persistence/replay for subscribers that reconnect mid-transition.
- **Satellite stream seeking is functional but simple.** A seeked load starts the CSIL media
  stream from the beginning and discards decoded PCM until the requested position. That keeps
  the protocol working without HTTP, but large seeks should eventually become range-aware.
- **LinkKeys auth**: DNS-less local-RP login is implemented as an exact opt-in mode with a
  database-backed SDK identity, domain/handle admission rules, single-use browser attempts,
  and ordinary Ichoi session minting. The regular full-RP assertion path remains a placeholder.
- **CSIL/TLS** (§4.2): native TCP and satellite traffic is encrypted with rustls. The core
  generates a persistent keypair, satellites pin its SHA-256 SPKI fingerprint, and node
  tokens are sent only inside the established TLS channel. Multiple accepted pins make
  staged rotation possible; automatic rolling rotation and in-band pin updates remain.
- **Native satellite installation**: `ichoi install satellite` plans and installs systemd
  units on Linux, a per-user LaunchAgent on macOS, and either an `ONLOGON` task or native
  Windows Service. Its configuration contains the core address, pins, and node token with
  private file permissions. `--dry-run` is serviced by the same deterministic planner used
  for real installation.
- **Native satellite audio**: Linux retains the runtime-loaded ALSA backend. macOS and
  Windows compile target-gated CPAL backends over CoreAudio and WASAPI, respectively.
  Browser TLS remains expected via a reverse proxy (deploy/Caddyfile).

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
