# Ichoi

Ichoi is a music player, library manager, and **distributed jukebox** server. One
static binary runs your library, streams to browsers and native clients, and turns
speakers on other machines — a Raspberry Pi in the kitchen, a tablet by the door —
into shared, controllable playback targets that anyone you permit can queue music to.

It is deliberately not a Subsonic clone. Ichoi has **no passwords** (identity is
[LinkKeys](https://github.com/catalystcommunity/linkkeys), or login-less on a trusted
LAN), a real **capability-negotiated** media pipeline, and a **CSIL-native** API — the
web UI, native apps, and satellites all speak the same transport. See
[`docs/DESIGN.md`](docs/DESIGN.md) for the full architecture and the reasoning behind
every decision, and [`CONTRIBUTING.md`](CONTRIBUTING.md) for working conventions.

Status: **design**. The architecture is settled; implementation is in progress.

## What it does

- **Streams full-quality or transcodes on demand.** The client always runs the same
  Symphonia decoder; the core sends original packets (direct mode) or re-encodes to
  AAC-LC / MP3 (transcoded mode) based on what each consumer negotiates — bitrate cap,
  codec, or an undecodable source (Opus, HE-AAC). Transcoding shells out to a bundled
  static **ffmpeg**; it is never linked.
- **Distributed jukebox.** Satellites connect *out* to a core and register their outputs
  as shared targets. Control and audio travel back down the connection the satellite
  opened, so NAT'd Pis just work. The core never connects to a satellite.
- **Multi-server clients.** One client can browse several servers at once and, when
  permitted, **copy** albums and playlists between them (never delete).
- **Browser-first**, served by the same binary over a WebSocket carrier, with a
  Symphonia→WASM decoder. Firefox is first-class. Themeable, i18n + a11y from day one.
- **Gapless, sample-accurate** playback with time-based seeking.

## The three roles

All three are the same binary; the role is configuration.

| Role | What it is |
|---|---|
| **core** | Owns the library, database, ffmpeg, and TLS keys. Serves the API, indexes and transcodes, coordinates jukebox state, accepts client-mediated imports. May own local outputs. |
| **satellite** | A dumb output node (typically a Pi). Connects out to one core, registers its speakers as shared targets, plays AAC-LC. No library access. |
| **client** | A browser or native app carrying a LinkKeys identity. Browses one or more servers, plays locally, copies between servers, and may become a shared satellite of one server. |

## Quickstart

```sh
# Run a core over your music library.
ICHOI_MUSIC_DIR=/path/to/music ichoi serve
```

That serves the browser UI on `http://localhost:4042` and the CSIL+TLS listener on
`:4043`. On a trusted LAN this is login-less (guest); the first LinkKeys login promoted
with `ICHOI_ADMIN_TOKEN` becomes the admin.

Browsers should reach Ichoi over HTTPS via a reverse proxy — the HTTP listener is plain
HTTP by design. See [`deploy/`](deploy/) for a ready-to-edit **Caddy** example and a
docker-compose that runs Ichoi behind it.

## Configuration

Precedence is **environment (`ICHOI_`-prefixed) → TOML config → defaults**. The common
settings:

| Setting | Env | Default |
|---|---|---|
| Role | `ICHOI_ROLE` | `core` |
| Music directory | `ICHOI_MUSIC_DIR` | *(required for a core)* |
| Audiobook directory | `ICHOI_AUDIOBOOK_DIR` | unset |
| Database directory | `ICHOI_DB_DIR` | music dir |
| HTTP listen | `ICHOI_HTTP_ADDR` | `:4042` |
| CSIL listen | `ICHOI_CSIL_ADDR` | `:4043` |
| Core address (satellite) | `ICHOI_CORE_ADDR` | *(required for a satellite)* |
| Core key fingerprints (satellite) | `ICHOI_CORE_KEYS` | *(required for a satellite)* |
| Node token (satellite) | `ICHOI_NODE_TOKEN` | *(required for a satellite)* |
| Admin bootstrap token | `ICHOI_ADMIN_TOKEN` | unset |
| ffmpeg override | `ICHOI_FFMPEG` | bundled → `PATH` |
| Default transcode codec | `ICHOI_TRANSCODE_CODEC` | `aac` |
| Log level | `ICHOI_LOG` | `warn` |

The database is a single SQLite file (WAL) in the database directory. ffmpeg is resolved
bundled-next-to-the-binary first, then `PATH`; if neither is found, transcoding is
disabled and the core serves direct mode only — Ichoi never fails to start for lack of
ffmpeg.

## Repository layout

Each independently versioned target lives in its own top-level directory (the
multiple-directory release strategy — a future mobile app would be a sibling `mobile/`).
The server target is under [`server/`](server/); the repo root holds only shared CI
([`.reactorcide/`](.reactorcide/)), project docs, and the [`./tools.sh`](tools.sh) helper.

## Building

Ichoi is a single static binary with zero system-library link dependencies. SQLite is
compiled in; ffmpeg and host audio are never linked. Direct `cargo` runs from the server
target dir; use the musl targets for a fully static build:

```sh
cd server
cargo build --release --target x86_64-unknown-linux-musl   # amd64
cargo build --release --target aarch64-unknown-linux-musl  # arm64 (Pi satellites)
```

Day-to-day, use [`./tools.sh`](tools.sh) from the repo root (it drives the server target
for you), which mirrors what CI runs:

```sh
./tools.sh build     # cargo build --workspace
./tools.sh test      # TEST_DATABASE_BACKEND=sqlite cargo test --workspace
./tools.sh fmt       # cargo fmt --all
./tools.sh clippy    # cargo clippy --workspace --all-targets -- -D warnings
./tools.sh check     # csil-validate + fmt + clippy + test (the full local gate)
./tools.sh gen       # regenerate CSIL bindings (server + all client languages)
```

## Docker

The [`server/Dockerfile`](server/Dockerfile) produces a minimal **scratch** image (multi-arch,
`linux/amd64` + `linux/arm64`) containing the `ichoi` binary, a bundled static **LGPL**
ffmpeg, and the web UI. Scratch is safe because host audio is loaded at runtime — a
server needs no audio output, so the image just reports `audio_outputs: none`.

```sh
docker buildx build --platform linux/amd64,linux/arm64 -t ichoi:dev server
docker run --rm -e ICHOI_MUSIC_DIR=/music -v /path/to/music:/music -p 4043:4043 ichoi:dev
```

Published images live at `containers.catalystsquad.com/public/catalystcommunity/ichoi`.
For a full deployment behind TLS, see [`server/deploy/docker-compose.yml`](server/deploy/docker-compose.yml).

## Testing & CI

Tests hit a **real SQLite database** inside a transaction that rolls back — no mocks for
the data layer, every test isolated in its own transaction. CI runs on
[Reactorcide](https://github.com/catalystcommunity/reactorcide) (not GitHub Actions):
conventional commits gate PRs and fan out to build (amd64 + arm64), the SQLite suite, and
CSIL validation; `semver-tags` cuts per-target releases (`server/vX.Y.Z`) on merge to
`main`. `./tools.sh check` reproduces the gate locally. Definitions live in
[`.reactorcide/jobs/`](.reactorcide/jobs/).

## License

Apache-2.0. See [`LICENSE`](LICENSE). Nothing copyleft or patent-encumbered lives inside
the Ichoi binary; the bundled ffmpeg is a separate LGPL program, subprocessed (see
[`docs/ffmpeg.md`](docs/ffmpeg.md)).
