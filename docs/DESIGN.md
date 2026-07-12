# Ichoi — Design

Ichoi is a music player, library manager, and jukebox server. The **core** ships as a
single static binary with no system library dependencies, plus a bundled static **ffmpeg**
sidecar for transcoding. Its native API is a CSIL transport; its only web-facing surface
serves browser UIs. It is a distributed jukebox — satellites on other hosts expose their
speakers as shared, controllable targets — and clients may span multiple servers and copy
content between them.

Status: **design**. Nothing here is implemented yet.

---

## 1. Goals and non-goals

### Goals

- **Single static binary, zero system-library dependencies at build or link time.** Audio
  output is loaded at *runtime* (§6.1), so the binary runs unchanged in a scratch
  container — it simply reports "no audio outputs." Cross-compiled for **amd64 and arm64**
  (Raspberry Pi satellites are first-class), plus darwin/windows for clients.
- **CSIL-native.** The real API is a CSIL service; everything Ichoi does is expressible
  over it.
- **Browser clients first-class**, served by the same binary over a WebSocket carrier.
  **Firefox is the first-class browser.**
- **Distributed jukebox.** Satellites register their outputs with a core as shared,
  controllable targets.
- **Bandwidth-saving transcoding**, negotiated per consumer. In v1.
- **Multi-server clients.** A client may connect to several servers, browse each, and —
  when permitted — **copy** content between them (never delete).
- **Identity via LinkKeys**, or login-less. Ichoi holds no user passwords.
- **Gapless, sample-accurate playback** with time-based seeking.
- **Themeable UI**, multiple UIs served concurrently, i18n and a11y from the first commit.
- **Apache-2.0**, no copyleft or patent-encumbered code inside the Ichoi binary.

### Non-goals

- **No Subsonic/Airsonic API compatibility** (§14). The domain model makes no concession.
- **No video.** Ever.
- **No Opus *encoding*.** We never transcode *to* Opus. Opus *source* files are supported
  for playback by transcoding them to AAC-LC (§5.6) — no edge decoder is added.
- **No synchronized multi-room playback** in v1; each target plays independently (§16).
- **No server-to-server transfer** in v1; cross-server copy is client-mediated (§7). Direct
  federation is a possible later addition, not on the roadmap.

### Supported formats

**Playback (client-decodable via Symphonia, on nodes natively and in browsers as WASM):**
MP3, AAC-LC, OGG Vorbis, FLAC, M4A (AAC-LC and ALAC), WAV.

**Transcode input (ffmpeg-decodable):** the above plus anything ffmpeg reads. A
non-client-decodable source is handled by transcoding it — notably **Opus** and **HE-AAC**
source files, which always play via transcode-to-AAC-LC, never direct (§5.6).

**Not supported:** WMA (dropped).

---

## 2. Stack decisions

| Concern | Choice | Notes |
|---|---|---|
| Language | **Rust** | Edition 2024 |
| HTTP | **axum** + `tower-http` | Native WebSocket; deviates from house Rocket 0.5 |
| CSIL | **csilgen** types + `csilgen-transport` | CSIL-Events |
| Database | **SQLite only**, `diesel` 2.2 + `diesel_migrations` | Single migration tree |
| Async | **tokio** | csilgen's "no async" binds csilgen, not us |
| Logging | **`log` + `env_logger`** | House convention |
| CLI | **clap 4**, derive | `serve` is one subcommand |
| TLS (CSIL) | **rustls**, 3 auto-rotating keypairs | Satellites pin fingerprints (§4.2); browser TLS is proxy-fronted |
| Audio demux/probe | **Symphonia** 0.6.x (MPL-2.0) | Server-side: demux + probe only |
| Audio decode (edge) | **Symphonia** | Nodes native, browsers WASM |
| Audio output | **`dlopen2` ALSA shim** (Linux); cpal on macOS/Windows | Host audio loaded at *runtime*, never linked on Linux (§6.1) |
| Transcode | **ffmpeg** subprocess | Bundled static sidecar; never linked (§5.3) |
| Tags | **lofty** | Broad coverage incl. cover art |
| IDs | **uuid** v7 | House convention |
| Errors | `thiserror`+`anyhow` (binary); hand-rolled (`libichoi`) | House convention |

### Deviations from house style

- **axum, not Rocket 0.5** — the browser CSIL carrier is a WebSocket; axum has native
  support. `ServeDir` serves UI assets.
- **SQLite only** — the database is files in the media directory.
- **tokio/async** — `csilgen-transport`'s synchronous `handle_envelope` wraps in
  `spawn_blocking`.

---

## 3. Roles and topology

Three roles, all the same binary.

| Role | Has | Does |
|---|---|---|
| **Core** | collection, database, ffmpeg, 3 TLS keypairs | Serves the API, indexes, transcodes, coordinates jukebox state, accepts client-mediated imports. May own local outputs. |
| **Satellite node** | node token, pinned core keys, local outputs | Connects **out** to one core, registers outputs as shared targets, plays media. **Dumb** — no library access. Typically a Pi. |
| **Client** | a LinkKeys identity | Browser or native app. Browses one or more servers, plays locally (private player), copies between servers if permitted, and may become a shared satellite of **one** server. |

```
                    ┌──────────────────────────────┐   ┌──────────────────────────────┐
                    │  CORE A (home)                │   │  CORE B (kids' house)        │
                    │  :4042 http   :4043 csil+tls  │   │  :4042 http   :4043 csil+tls │
                    └───▲───────────▲───────────▲───┘   └───────▲──────────────────────┘
                        │           │           │               │
              (out only)│  (out)    │        WebSocket           │  CSIL
              node token│  node     │           │                │  (2nd session)
              +pinnedkey│  token    │           │                │
                 ┌──────┴──┐ ┌──────┴────┐ ┌────┴──────────────────────┐
                 │SATELLITE│ │ SATELLITE │ │       CLIENT               │
                 │"Kitchen"│ │"Living Rm"│ │  browses A and B           │
                 │ AAC-LC  │ │  AAC-LC   │ │  copies A→B (mediated)     │
                 └─────────┘ └───────────┘ │  satellite of A ("Ann's …")│
                                           └────────────────────────────┘
```

Absolute invariant: **satellites connect to the core; the core never connects to a
satellite.** Control and media travel down the connection the satellite opened
(CSIL-Events is bidirectional over one socket). This also makes NAT'd satellites work.

Mechanically, a satellite is a headless client that others can control and that outputs to
physical devices — it **reuses the client media/playback code** (§5) and receives the
**same media payload a client does**.

---

## 4. Transports and transport security

### 4.1 Listeners

| Listener | Port | Carrier | Peers |
|---|---|---|---|
| CSIL | **4043** | TCP + TLS (rustls) | native clients, satellites |
| HTTP | **4042** | TCP (+TLS) | browser: SPA assets + `/ws` upgrade |

CSIL-Events is the transport for everyone. Persistent, reliable, ordered, bidirectional,
authenticated once at `$hello`; control and media frames multiplex on one connection. HTTP
does two things only: serve static UI files and accept the WebSocket upgrade. CSIL-RPC and
CSIL-Datagrams are unused.

### 4.2 Keys, pinning, rotation

The **CSIL/TCP** channel (`:4043`) uses TLS with **three server keypairs** — the LinkKeys
domain-key shape, but purely for transport confidentiality. There is **no domain
verification**; these keys exist only to secure core↔satellite/native comms.

- A satellite is provisioned with a **node token** *and* the core's **key fingerprints**.
  The token authenticates satellite→core; the pinned fingerprints authenticate
  core→satellite. **Neither direction is MitM-able.**
- **Rotation is automatic**, no admin action. The three keys carry **staggered validity —
  initially 1, 2, and 3 years** — and when the oldest lapses it is renewed at 3 years,
  keeping a rolling ~1/2/3-year offset so there are always three live keys at different
  ages. Satellites pin all three and pick up a renewed key over the still-trusted channel.
  (A satellite offline long enough to lose *all* pinned keys — beyond ~2 years — must be
  re-provisioned.)

**Browser TLS is out of scope.** The HTTP listener (`:4042`) is **plain HTTP by default**.
For HTTPS/WSS to browsers, front Ichoi with a reverse proxy (e.g. **Caddy**) — documented.
CSIL-over-HTTP could carry our own TLS later; skipped for now. Clients authenticate by user
identity (§8), not by node token.

---

## 5. The media pipeline

### 5.1 Two modes, one client decoder

The server sends codec packets plus a small description; the client decodes. Packets come
from one of:

**Direct mode** — Symphonia's `FormatReader` demuxes the original file into raw packets. No
re-encode; bandwidth equals the original. This is the **full-quality** path (e.g. a browser
on a LAN wanting lossless FLAC).

**Transcoded mode** — the core runs **ffmpeg** to decode the source and re-encode to a
lower-bitrate, client-decodable target, then streams it. Used when a consumer declares a
bitrate cap, is pinned to a target codec (satellites, §6), or the source codec is not
client-decodable (Opus/HE-AAC source).

In both modes the client runs the **same Symphonia decoder** — possible only because the
transcode target is Symphonia-decodable (§5.2).

Consumers **negotiate** in `$hello` / per-stream (acceptable codecs, max bitrate,
prefer-original). The core picks the mode. This is the capability negotiation Subsonic
structurally lacked. Seeking is a control message in both modes (reposition the demuxer, or
restart ffmpeg at `-ss <offset>` before `-i`) — never byte-range.

### 5.2 Transcode target: AAC-LC default, MP3 optional, satellites pinned AAC-LC

The target must be Symphonia-decodable, which excludes Opus in v1. That leaves MP3 and
AAC-LC, and the choice is **negotiated**:

- **AAC-LC** (ffmpeg native `aac`) — **the default**, because it is the higher-quality
  choice per bit for music. Symphonia-decodable; sets up a future native iOS app.
- **MP3** (ffmpeg `libmp3lame`) — available when a client prefers maximum compatibility.
  Client settings may select it.
- **Direct/original** — a client may request no transcode for full quality when bandwidth
  allows.

**Satellites always use AAC-LC.** They are dumb and reuse the client path with a single
pinned codec; no per-satellite negotiation. High-bitrate AAC-LC to a speaker is
transparent.

We only *encode* AAC-LC, via subprocessed ffmpeg. AAC-LC patents are considered expired
(~2017–18); the exposure is ffmpeg's, not ours.

### 5.3 ffmpeg: bundled, subprocessed, never linked

Transcoding shells out to an **ffmpeg binary**, never linked (keeps Ichoi's license clean —
subprocess = mere aggregation, §13 — and dodges static-musl cross-compile pain).

- **Distribution:** release artifacts and container images bundle a **static ffmpeg for
  amd64 and arm64**, beside the Ichoi binary and web UI files. Works out of the box.
- **Resolution order:** (1) bundled next to the binary or `ICHOI_FFMPEG`; (2) system
  `PATH`. If neither, transcoding is disabled and the core serves direct mode only, with a
  warning. Ichoi never fails to start for lack of ffmpeg.
- **Build:** an **LGPL** ffmpeg with only the audio encoders we need (`libmp3lame`, native
  `aac`, later `libopus`/`libvorbis`) — none requiring `--enable-gpl`. Obligation stays at
  LGPL level (ship ffmpeg's corresponding source + build recipe; Ichoi is a separate
  program). **Verify at build time no encoder pulls `--enable-gpl`.**
- **Security:** the invocation is built from validated parameters. **Never a user-editable
  command template** — that is remote code execution.

### 5.4 Gapless playback

No decoder we use trims encoder delay (WebCodecs doesn't; Symphonia doesn't expose it —
`w3c/webcodecs#626`: *"Chrome doesn't do anything today for this."*). Ichoi trims where the
metadata lives:

| Source | Trim metadata |
|---|---|
| AAC | ~2112-sample priming; end padding from MP4 `elst` / `iTunSMPB` |
| MP3 | LAME/Xing delay+padding |
| Opus (future) | `pre-skip` in `OpusHead` |

Extracted once at scan into `tracks.trim_start_samples` / `trim_end_samples`, emitted with
each stream, honored by the client. Transcoded streams also account for the target
encoder's own priming.

### 5.5 Browser decode: Symphonia → WASM

The browser loads `ichoi-decoder.wasm` (Symphonia → `wasm32-unknown-unknown`) and decodes
everything with it — **not** WebCodecs. WebCodecs has three disqualifying holes: **ALAC
unsupported everywhere** (no registered codec string), **AAC fails in Firefox on Linux**
(our primary environment), **no WebCodecs in Firefox for Android**. Since a Symphonia→WASM
build is the only route to ALAC in a browser anyway, a second native path buys only a
shifting matrix. Decode is cheap (~100× realtime FLAC); bundle size is irrelevant (cached).

```
CSIL-Events (WebSocket)
  → codec packets + description     (direct: original codec; transcoded: AAC-LC/MP3)
  → ichoi-decoder.wasm (Symphonia)
  → PCM via postMessage             (NOT SharedArrayBuffer)
  → AudioWorkletNode ring buffer → AudioContext
```

**postMessage, not SharedArrayBuffer:** the SAB pattern needs `crossOriginIsolated` (COOP
`same-origin` + COEP `require-corp`), which breaks cross-origin theme assets. Themeability
wins; the efficiency delta is immaterial at audio data rates. Backpressure via
`WebSocket.bufferedAmount` and decoder queue depth; a dropped packet desyncs timestamps, so
detect gaps and reset past a lag threshold.

### 5.6 Opus: read-only, via transcode

We **never encode to Opus** — the transcode targets are AAC-LC and MP3, full stop. But Opus
*source* files still play: Symphonia has no Opus decoder, so an Opus file is not
client-decodable and therefore always takes **transcoded mode**, ffmpeg decoding the Opus
and re-encoding to AAC-LC. No Opus decoder is added at the edge. HE-AAC source is handled
identically.

The only thing lost is *direct* (lossless-passthrough) playback of Opus files, which is
moot — Opus is already lossy. A future edge-side Opus decoder could enable direct Opus
playback, but nothing needs it to satisfy "read Opus files."

---

## 6. Jukebox: nodes, targets, and players

### 6.1 Audio output, loaded at runtime

The host audio library is **loaded at runtime**, never linked at build time. This is a
hard requirement, and it is *why we cannot use stock cpal on Linux*: cpal's `alsa-sys`
creates a `DT_NEEDED` dependency on `libasound`, which the dynamic loader resolves **before
`main`** — so a cpal-linked binary would fail to start in a scratch container, before any
"no audio" detection could run.

Instead, on **Linux** we `dlopen` `libasound.so.2` ourselves (via `dlopen2`) and call its
small PCM surface directly (`snd_pcm_open` / `hw_params` / `writei` / `drain` / `recover`,
`snd_device_name_hint` for enumeration). On **macOS/Windows** the system audio frameworks
(CoreAudio/WASAPI) always exist, so cpal (or a native backend) is fine there. Consequences:

- The binary starts anywhere, including a **scratch container** with no audio stack: the
  `dlopen` fails, we catch it, enumerate zero devices, and report `audio_outputs: none`.
- No build- or link-time system dependency; only **major-version ABI** (`libasound.so.2`)
  compatibility, resolved at load.
- "Single static binary, no system libraries" is literally true; audio output is an
  optional runtime capability.

Linux JACK/PipeWire backends can be added later behind the same runtime-load pattern.

### 6.2 Registration and naming

A satellite connects out and registers its **host** (mutable friendly name: `homepi3829` →
"Kitchen") and each **output device** (`hw:CARD=foo` → "Main Speakers"). The core may
register its own outputs. Raw OS identifiers are retained for stable re-identification;
renaming is an admin action.

### 6.3 Shared targets vs private players

A **player** has a queue and transport state. Two kinds:

- **Shared target** — an output on a satellite or the core. Its state is visible to, and
  (per RBAC, §6.6) controllable by, other users. "See what's playing in the Kitchen and
  skip it."
- **Private player** — a client's local playback (browser Web Audio, or native `cpal`).
  Plays locally, **cannot be pushed to by anyone else**; may report state so the owner sees
  "playing on my phone."

### 6.4 Client satellite mode (kiosks)

A **client can toggle satellite mode on the fly**, turning its local output into a shared
target named **"`<Handle>`'s `<suffix>`"**. The `"<Handle>'s "` prefix is fixed and
unchangeable; the suffix defaults to "Device" and is user-settable **as long as it is
unique per server**. This enables **permanent client kiosks** — a dedicated tablet or
mini-PC that anyone (permitted) can queue music to.

A client may be a **satellite of only one server at a time**, even though it may be
*connected* to many (§7). The shared target is owned by that user's identity.

### 6.5 Playback authority and control flow

The host with the physical device is the **audio clock and authority**. For a shared
target: a controller sends a command over CSIL-Events → the core updates the target's
queue/transport in the database and pushes intent **down the satellite's existing outbound
connection** → the satellite pulls AAC-LC media and plays, reporting authoritative position
→ the core fans state to subscribers. The core never opens a connection to the satellite.

### 6.6 RBAC — a small fixed role set

Three roles, coarse on purpose, revamped later only if needed:

- **admin** — full control: manage accounts, trust domains, register/rename nodes and
  devices, edit settings, **import** (write into the library, including as a cross-server
  copy destination), control any shared target, and **export**.
- **member** — the normal authenticated user: browse and search, play, create and manage
  their own playlists, control shared targets, run client satellite mode, and **export**
  (copy *from* this server, where the server permits it). Cannot import or administer.
- **guest** — browse, search, and play to their own private player only. No shared-target
  control, no playlist persistence, no export.

Defaults: **login-less connections are guest**; the bootstrap account is **admin**; new
LinkKeys accounts are **member**. **Import is admin-only by default** — writing files into a
collection is privileged — and can be granted to a member later. Per-target and per-library
grants can layer on without reshaping this; the model (§10) reserves the tables.

### 6.7 Node authentication

A satellite authenticates with a **pre-shared node token** (generated on the core, placed
in the satellite's config), distinct from user identity: a token authenticates a *host*, a
LinkKey a *person*. The core stores only the token's hash; it is revocable. The satellite
additionally pins the core's key fingerprints (§4.2).

---

## 7. Multi-server clients and cross-server copy

A client is not bound to one server.

- **Connect to many.** A client holds a session (§8) per server and can **browse and
  search** each. LinkKeys makes multi-server identity trivial — the same person
  authenticates to each server; it is then purely a permissioning question.
- **Copy, if permitted.** A client may **copy songs, albums, and playlists from one server
  to another** — never delete, never modify the source. Copy requires **read/export** on
  the source and **import** on the destination.
- **It is a file copy.** A copy transfers the **actual files plus any adjacent folder
  media** — album art, lyrics, and other sidecar files in the album folder — with their
  embedded metadata intact. The destination writes them into its library and picks them up
  with its normal scan. There is no database record-level reconciliation; it is files on
  disk. `content_hash` (§10) lets the destination skip files it already has.
- **Client-mediated only.** The transfer flows **through the client**: read files from the
  source, write them to the destination's permissioned **import** path. The **core never
  talks to another core.** Server-to-server direct transfer may come later; not on the
  roadmap.
- **Playlists are m3u.** A playlist is an **m3u file with server-root-relative paths**, so
  it ports directly. At playback, entries whose files are not found are **simply skipped**
  when building the in-memory queue — no error, no placeholder. Copy the referenced tracks
  alongside to fill the gaps.

Motivating use case: two households (home and the kids' house), each running a core,
sharing albums and playlists as a mutual backup — copy an album's folder over, share a
playlist m3u, fill missing tracks, all from one client logged into both.

**Deletion is never a client capability**, cross-server or otherwise; removing content is a
local administrative action.

---

## 8. Identity and authentication

**Ichoi has no user passwords** — no password column, hash, encryption key, or recovery. A
dividend of dropping Subsonic (§14).

- **LinkKeys RP.** Ichoi consumes the PKI via `linkkeys-rpc-client` (blocking, length-
  prefixed CBOR over mTLS, DNS-fingerprint-pinned) or an RP's internal API. Subject is
  **`UUID@domain.tld`** (domain is part of the key — Ichoi trusts other domains). The
  **`handle` claim** becomes the username, refreshed each login; a local persona may
  override the display name; the account ID never changes. Requested claims live in
  `ICHOI_RP_CLAIMS_CONFIG` (TOML).
- **Sessions.** LinkKeys authenticates; **Ichoi mints its own** high-entropy opaque token,
  storing only its SHA-256 (no Argon2/bcrypt — nothing low-entropy to attack). It rides
  `$hello.auth`. A multi-server client holds one such session per server.
- **Login-less default** — accepts any user connection, correct for a trusted LAN. Node
  tokens (§6.7) are a separate, always-required axis for satellites.
- **First-admin bootstrap.** `ICHOI_ADMIN_TOKEN` (or `--pre-shared-token`) unlocks account
  creation *only while zero accounts exist*; the operator authenticates via LinkKeys in
  that session and the resulting `UUID@domain.tld` becomes `admin`. Consumed on first use.

---

## 9. Configuration

Precedence: **env (`ICHOI_`-prefixed) → TOML config → defaults.**

| Setting | Env | Default |
|---|---|---|
| role | `ICHOI_ROLE` | `core` |
| music dir | `ICHOI_MUSIC_DIR` | *(required, core)* |
| audiobook dir | `ICHOI_AUDIOBOOK_DIR` | unset |
| database dir | `ICHOI_DB_DIR` | music dir |
| HTTP listen | `ICHOI_HTTP_ADDR` | `:4042` |
| CSIL listen | `ICHOI_CSIL_ADDR` | `:4043` |
| core address (satellite) | `ICHOI_CORE_ADDR` | *(required, satellite)* |
| core key fingerprints (satellite) | `ICHOI_CORE_KEYS` | *(required, satellite)* |
| node token (satellite) | `ICHOI_NODE_TOKEN` | *(required, satellite)* |
| admin bootstrap token | `ICHOI_ADMIN_TOKEN` | unset |
| ffmpeg override | `ICHOI_FFMPEG` | bundled → PATH |
| default transcode codec | `ICHOI_TRANSCODE_CODEC` | `aac` |
| log level | `ICHOI_LOG` | `warn` |

SQLite file (WAL) in the database directory.

---

## 10. Data model (sketch)

Illustrative, not final.

- `accounts` — `id` (`UUID@domain.tld`), `handle`, `display_name`, timestamps
- `sessions` — `token_sha256`, `account_id`, `expires_at`
- `roles`, `role_grants` — simple RBAC (§6.6); a grant may target a device
- `server_keys` — `id`, `public`, `private_enc`, `fingerprint`, `active`, `created_at`
  (three, rotatable, §4.2)
- `settings` — `key`, `value` (themes read here)
- `nodes` — `id`, `kind` (`core`|`satellite`|`client`), `hostname`, `friendly_name`,
  `token_sha256`, `platform`, `arch`, `last_seen`, `audio_outputs` (`none` when scratch)
- `output_devices` — `id`, `node_id`, `os_device_id`, `friendly_name`, `channels`,
  `sample_rates`, `is_default`
- `players` — `id`, `kind` (`shared`|`private`), `output_device_id?`, `owner_account_id?`,
  `name_suffix?` (client satellite mode)
- `player_queue_items` — `player_id`, `track_id`, `position`
- `player_state` — `player_id`, `status`, `current_track_id`, `position_ms`, `volume`
- `libraries` — `id`, `kind` (`music`|`audiobook`), `path`
- `tracks` — `id`, `library_id`, `root_relative_path` (playlist portability, §7), `codec`,
  `container`, `sample_rate`, `channels`, `bit_depth`, `duration_ms`, `bitrate`,
  `size_bytes`, `mtime`, `content_hash` (cross-server dedupe), `trim_start_samples`,
  `trim_end_samples`
- `artists`, `albums`, join tables
- `playlists` — an **index of m3u files** in the collection (the m3u is the source of
  truth; entries are root-relative, §7). `playlist_entries` caches parsed entries for
  querying/reordering; unresolved entries are skipped at playback.
- `listens`, `stars`, `ratings`

---

## 11. Web UI

- **SolidJS** SPA, modern, no iframes. Multiple UIs under `web/themes/<name>/`, served
  concurrently, optionally path-scoped, redirect at login. Theme settings live in the
  database so any UI honors them.
- **Web UI files are the only distributable not inside the binary** (ffmpeg is the other
  bundled file, §5.3).
- **i18n and a11y from the first commit**; the same primitives are exposed to third-party
  UIs. **Firefox first-class**; manual rendering verified with playwright-mcp on Firefox.

---

## 12. Testing, migrations, CI

- **DataUtils:** integration tests hit a **real SQLite DB** in a rolled-back transaction,
  via a copied `tests/common/` module (`create_test_pool` → `migrate_sqlite` →
  `begin_test_transaction`; drop = rollback). Factories take `DataMap` overrides and
  random-fill the rest. Handlers exercised as functions; no OS socket.
- **Migrations:** `diesel_migrations` + `embed_migrations!`, one SQLite tree at
  `migrations/`, run at `serve` startup under WAL. Schema DDL only; idempotent data
  backfills are separate **transforms** run every boot.
- **CI:** `.reactorcide/jobs/*.yaml` + a `tools.sh` mirror. No GitHub Actions. Conventional
  commits gate PRs; `semver-tags` releases on merge. **amd64 and arm64** are both release
  targets (binaries + container images). Enforcement by command: `cargo fmt --all`,
  `cargo clippy --workspace --all-targets -- -D warnings`.
- **Test media:** a few public-domain files, committed if small, **never bundled** in the
  binary or a container image.

---

## 13. Licensing policy

Apache-2.0. Root `LICENSE` only; no per-file SPDX headers. Nothing copyleft or patent-
encumbered inside the **Ichoi binary**.

| Dependency | License | Status |
|---|---|---|
| Symphonia | MPL-2.0 | **Clean.** Per-file copyleft; keep notices, link versioned source. |
| lofty | MIT/Apache | Clean. |
| cpal + host audio | Apache-2.0 crate; ALSA is LGPL | **`dlopen` at runtime, never linked** — no distribution obligation attaches to Ichoi. Output hosts only. |
| ffmpeg (bundled) | **LGPL** build, audio-only | **Separate program**, subprocessed. Ship its source + build recipe; Ichoi unaffected. |
| ffmpeg linked statically | GPL/LGPL | **Excluded.** |
| LAME/Shine as Rust crates | LGPL-2.0 | **Excluded** from the binary. (`libmp3lame` inside the ffmpeg subprocess is fine.) |
| Fraunhofer FDK AAC | bespoke | **Excluded.** No patent grant; not OSI; Debian non-free. We use ffmpeg's native AAC encoder. |

Preserved corrections: libFLAC the *library* is BSD-3 but the `flac`/`metaflac` *tools* are
GPL; **AAC-LC** ≈ patent-free since ~2017–18 while **HE-AAC/xHE-AAC** run to ~2030–31 (never
write "AAC is patent-free" without `-LC`); ASF Category A/B/X (all LGPL = X) does **not**
bind us (we're Apache-*licensed*, not an ASF project); never inline LGPL source (LGPL §3 →
GPLv2).

---

## 14. Why there is no Subsonic API

Dropped after starting as a goal; reasoning kept so it is not re-litigated.

Token auth `t = md5(password + salt)` (client-generated salt) forces the server to store a
**recoverable** secret — a one-way hash cannot verify it. Confirmed in every codebase:
gonic (plaintext), original Airsonic (plaintext, *"Airsonic unfortunately stores passwords
in plain-text"*), Navidrome (reversible AES-GCM, default key the public string
`"just for obfuscation"`), Airsonic-Advanced (bcrypt login + a separate encrypted app
credential). The API also has **no capability negotiation** (`format` is a hint; the client
discovers support by decoding bytes) and **transcoded-stream seeking is a hack**
(`timeOffset` is video-only; `estimateContentLength` guesses both break seeking and
truncate tracks). Dropping it deleted the recoverable-credential subsystem, the XML
envelope, the legacy surface, the negotiation hole, the seek pathology, and every
constraint on our transcode design. Cost — **no third-party mobile client until we write
one** — accepted. If ever added, it is an isolated feature-gated adapter crate.

---

## 15. Prior art

- **MoosicBox** (Rust, MPL-2.0) — no ffmpeg, Symphonia decode, per-codec encode crates.
- **Polaris** (Rust, MIT) — no transcoding; streams originals; ships in production.
- **Navidrome** (Go) — external ffmpeg for all transcoding; our model, except we bundle it.
- **Snapcast** — reference for synchronized multi-room (§16), the hard clock-sync problem
  we defer.
- **librespot** (Rust, MIT) — pure-Rust decode reference; moved off `lewton` to Symphonia.

---

## 16. Open questions

1. **`@wire-id` ordinals** — whether to pin CSIL compact-profile ordinals now (stable
   across the federation) or after the operation surface stabilizes. The verbose profile
   works without them.
2. **Cross-server import authorization detail** — how the destination scopes and audits an
   admin-mediated import; rate/size limits.

### Settled since last revision

- **Opus:** read-only via transcode-to-AAC-LC; never encoded; no edge decoder (§5.6).
- **RBAC:** three roles — admin / member / guest; import is admin-only by default (§6.6).
- **Audio output:** `dlopen` `libasound.so.2` on Linux (not stock cpal), cpal/native on
  macOS/Windows — honors the scratch-container guarantee (§6.1).
- **Copy mechanics:** file-level copy of tracks + folder sidecars; playlists are m3u;
  missing entries skipped at playback (§7).
- **Key rotation:** automatic, staggered 1/2/3-year validity renewed to 3 years; no admin
  flow. Browser TLS is proxy-fronted (Caddy), not ours (§4.2).
- **Client-satellite name uniqueness:** per server (§6.4).
