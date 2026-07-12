# Bundled ffmpeg — how it's built and the LGPL source offer

Ichoi transcodes by shelling out to an **ffmpeg binary**. It is bundled beside the Ichoi
binary in release artifacts and container images, but it is **never linked** into Ichoi
(DESIGN §5.3, §13). This keeps Ichoi's own license clean (subprocess = mere aggregation)
and dodges static-musl cross-compile pain.

## What we ship

An **LGPL** ffmpeg with only the audio encoders we need — nothing that requires
`--enable-gpl` and nothing `nonfree`:

- **Encoders:** native `aac` (the default transcode target) and `libmp3lame` (MP3, LGPL).
  We never encode to Opus (DESIGN §5.2, §5.6).
- **Decoders:** the formats we read, including Opus and HE-AAC sources (played via
  transcode-to-AAC-LC), plus ALAC, FLAC, Vorbis, MP3, AAC, and PCM.
- **No** `--enable-gpl`, **no** `--enable-nonfree`, **no** Fraunhofer FDK AAC (DESIGN §13).

We use ffmpeg's **native AAC encoder**, not `libfdk_aac` — FDK has no patent grant, isn't
OSI, and is Debian non-free. AAC-LC patents are considered expired (~2017–18); the
exposure is ffmpeg's, not ours.

## How it's built (the recipe)

The exact, reproducible recipe is the ffmpeg stage of the repo-root
[`Dockerfile`](../server/Dockerfile). In short:

1. Build `libmp3lame` statically from source (LGPL-2.0), linked only into the ffmpeg
   subprocess.
2. Configure ffmpeg with `--disable-everything` + an explicit audio allowlist,
   `--enable-static --disable-shared`, `--enable-libmp3lame`, the native `aac` encoder,
   and **no** `--enable-gpl` / `--enable-nonfree`.
3. **Build-time license gate:** the image assertion greps `ffmpeg -buildconf` and *fails
   the build* if `--enable-gpl` or `--enable-nonfree` ever appears (DESIGN §5.3: "verify
   at build time no encoder pulls `--enable-gpl`").

The result is a single static ffmpeg binary for each release architecture (amd64, arm64).

## Runtime resolution

Ichoi finds ffmpeg in this order (DESIGN §5.3):

1. Bundled next to the Ichoi binary, or the path in `ICHOI_FFMPEG`.
2. System `PATH`.

If neither is found, transcoding is disabled and the core serves direct mode only, with a
warning. Ichoi never fails to start for lack of ffmpeg.

## LGPL obligation

The bundled ffmpeg and libmp3lame are LGPL. Ichoi is a **separate program** that invokes
ffmpeg as a subprocess, so no copyleft obligation attaches to Ichoi itself (DESIGN §13).
The LGPL obligation we *do* carry is to offer ffmpeg's **corresponding source** and the
build recipe:

- **Build recipe:** this file plus the ffmpeg stage of the [`Dockerfile`](../server/Dockerfile),
  which pins the exact ffmpeg and LAME versions and every configure flag.
- **Source:** the pinned upstream releases —
  `https://ffmpeg.org/releases/ffmpeg-<version>.tar.xz` and
  `https://downloads.sourceforge.net/project/lame/lame/<version>/lame-<version>.tar.gz`
  (versions are the `FFMPEG_VERSION` / `LAME_VERSION` build args in the Dockerfile).

We never inline LGPL source into Ichoi (LGPL §3 would convert it to GPLv2). ffmpeg stays a
separate, subprocessed binary.
