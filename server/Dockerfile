# Ichoi server image — minimal, multi-arch (linux/amd64 + linux/arm64).
#
# Produces a scratch image containing exactly three things (DESIGN Sec.5.3, 11):
#   1. the `ichoi` static binary          (/usr/local/bin/ichoi)
#   2. a bundled static LGPL ffmpeg       (/usr/local/bin/ffmpeg, "next to" ichoi)
#   3. the web UI themes                  (/app/web/themes/)
#
# The final stage is `scratch`. That is safe and intentional: Ichoi never links
# system libraries. SQLite is compiled in (diesel bundled), ffmpeg is a
# subprocess (never linked, DESIGN Sec.5.3), and host audio output is loaded at
# *runtime* via dlopen of libasound (DESIGN Sec.6.1) — so on a scratch image with
# no audio stack the dlopen simply fails, is caught, and the server reports
# `audio_outputs: none`. A core/server needs no audio output, so scratch is the
# right base.
#
# Build (multi-arch, requires buildx / buildkit):
#   docker buildx build --platform linux/amd64,linux/arm64 -t ichoi:dev .
#
# The Rust builder and web UI stages CROSS-compile on the native build host (no QEMU):
# ring's aarch64 assembly SIGSEGVs under QEMU emulation, so the arm64 binary is built
# with the gnu cross toolchain + arm64 target headers instead. Only the ffmpeg stage
# still builds per-target (emulated when the target is arm64).

# -----------------------------------------------------------------------------
# Stage 1: build a static, audio-only, LGPL ffmpeg from source.
#
# DESIGN Sec.5.3 / Sec.13: the bundled ffmpeg must be an LGPL build with only the
# audio encoders we need — nothing that requires `--enable-gpl`. We compile
# libmp3lame (for MP3 encode) statically and configure ffmpeg with `--disable-
# everything` + an explicit audio allowlist, native `aac` encoder, and NO
# `--enable-gpl` / `--enable-nonfree`. A build-time assertion (below) fails the
# image if the resulting buildconf ever contains those flags.
#
# LGPL obligation (DESIGN Sec.13): we ship ffmpeg's corresponding source + this
# recipe; ffmpeg stays a separate, subprocessed program, so Ichoi is unaffected.
# See docs/ffmpeg.md for the source-offer details.
# -----------------------------------------------------------------------------
FROM debian:bookworm-slim AS ffmpeg
ARG FFMPEG_VERSION=7.1
ARG LAME_VERSION=3.100
RUN apt-get update && apt-get install -y --no-install-recommends \
      build-essential nasm yasm pkg-config curl xz-utils ca-certificates && \
    rm -rf /var/lib/apt/lists/*

# Static libmp3lame (LGPL-2.0). Linked only into the ffmpeg subprocess, never
# into Ichoi (DESIGN Sec.13).
WORKDIR /src
RUN curl -fsSL "https://downloads.sourceforge.net/project/lame/lame/${LAME_VERSION}/lame-${LAME_VERSION}.tar.gz" -o lame.tar.gz && \
    tar -xzf lame.tar.gz && cd "lame-${LAME_VERSION}" && \
    ./configure --prefix=/opt/lame --disable-shared --enable-static --disable-frontend && \
    make -j"$(nproc)" && make install

# ffmpeg (LGPL config, audio only, fully static binary).
RUN curl -fsSL "https://ffmpeg.org/releases/ffmpeg-${FFMPEG_VERSION}.tar.xz" -o ffmpeg.tar.xz && \
    tar -xf ffmpeg.tar.xz && cd "ffmpeg-${FFMPEG_VERSION}" && \
    PKG_CONFIG_PATH=/opt/lame/lib/pkgconfig ./configure \
      --prefix=/opt/ffmpeg \
      --pkg-config-flags=--static \
      --extra-cflags="-I/opt/lame/include" \
      --extra-ldflags="-L/opt/lame/lib -static" \
      --disable-shared --enable-static \
      --disable-doc --disable-htmlpages --disable-manpages --disable-podpages --disable-txtpages \
      --disable-debug \
      --disable-network \
      --disable-autodetect \
      --disable-everything \
      --enable-small \
      --enable-swresample \
      --enable-libmp3lame \
      --enable-protocol=file,pipe \
      --enable-demuxer=mov,mp3,flac,ogg,matroska,wav,aac,aiff,w64,asf \
      --enable-parser=aac,aac_latm,flac,vorbis,opus,mpegaudio,mpeg4audio \
      --enable-decoder=aac,aac_latm,mp3,mp3float,flac,vorbis,opus,alac,pcm_s16le,pcm_s24le,pcm_s32le,pcm_f32le \
      --enable-encoder=aac,libmp3lame \
      --enable-muxer=adts,mp3,ipod,mp4 \
      --enable-filter=aresample,aformat,anull,atrim && \
    make -j"$(nproc)" && make install

# Build-time license gate (DESIGN Sec.5.3): fail the image if any component
# pulled in GPL or nonfree. LGPL only.
RUN /opt/ffmpeg/bin/ffmpeg -hide_banner -buildconf | tee /tmp/buildconf && \
    if grep -q -- '--enable-gpl' /tmp/buildconf; then echo "FATAL: ffmpeg is GPL, not LGPL"; exit 1; fi && \
    if grep -q -- '--enable-nonfree' /tmp/buildconf; then echo "FATAL: ffmpeg is nonfree"; exit 1; fi && \
    echo "OK: ffmpeg is a clean LGPL build"

# -----------------------------------------------------------------------------
# Stage 2: build the ichoi binary as a fully static musl executable.
#
# musl gives a static binary with zero system-library link deps (DESIGN Sec.1),
# so it runs in scratch. SQLite is compiled in via diesel's bundled
# libsqlite3-sys; no libpq (SQLite-only, DESIGN Sec.2).
# -----------------------------------------------------------------------------
FROM --platform=$BUILDPLATFORM rust:1-bookworm AS builder
ARG TARGETARCH
# Cross-compile on the native build host (no QEMU). musl-tools links the amd64 musl
# target; the aarch64 gnu cross gcc + arm64 target headers (libc6-dev-arm64-cross) compile
# ring/libsqlite3-sys's C and assemble ring's aarch64 asm for the arm64 target.
RUN apt-get update && apt-get install -y --no-install-recommends \
      pkg-config musl-tools gcc-aarch64-linux-gnu libc6-dev-arm64-cross && \
    rm -rf /var/lib/apt/lists/*
ENV CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=aarch64-linux-gnu-gcc \
    CC_aarch64_unknown_linux_musl=aarch64-linux-gnu-gcc
RUN case "$TARGETARCH" in \
      amd64) echo x86_64-unknown-linux-musl  > /tmp/triple ;; \
      arm64) echo aarch64-unknown-linux-musl > /tmp/triple ;; \
      *) echo "unsupported TARGETARCH: $TARGETARCH" >&2; exit 1 ;; \
    esac && \
    rustup target add "$(cat /tmp/triple)"

WORKDIR /build
# Build context is server/. COPY only what the build needs (the .dockerignore
# excludes everything else).
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY schema/ schema/
COPY generated/ generated/
COPY migrations/ migrations/
RUN cargo build --release --target "$(cat /tmp/triple)" --bin ichoi && \
    cp "target/$(cat /tmp/triple)/release/ichoi" /ichoi

# -----------------------------------------------------------------------------
# Stage 2b: build the web UI (SolidJS SPA). The output is arch-independent static
# assets, built once on the native host and served from /app in the final image.
# -----------------------------------------------------------------------------
FROM --platform=$BUILDPLATFORM node:22-bookworm-slim AS web
WORKDIR /web
COPY web/ ./
RUN cd themes/default && npm ci && npm run build

# -----------------------------------------------------------------------------
# Stage 3: the minimal runtime image.
# -----------------------------------------------------------------------------
FROM scratch
# The ichoi binary and its bundled ffmpeg live in the same directory, so Ichoi's
# ffmpeg resolution (bundled-next-to-binary, DESIGN Sec.5.3) finds it with no
# configuration.
COPY --from=builder /ichoi /usr/local/bin/ichoi
COPY --from=ffmpeg /opt/ffmpeg/bin/ffmpeg /usr/local/bin/ffmpeg
# CA roots for the LinkKeys RP client / outbound TLS.
COPY --from=ffmpeg /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
# The web UI (DESIGN Sec.11) — the only distributable served from disk rather than
# baked into the binary. The server serves web_dir directly (default
# web/themes/default, relative to WORKDIR /app), so the BUILT SPA contents go there.
COPY --from=web /web/themes/default/dist/ /app/web/themes/default/

WORKDIR /app
# HTTP (browser SPA + /ws) and CSIL+TLS listeners (DESIGN Sec.4.1). Both are
# configurable via ICHOI_HTTP_ADDR / ICHOI_CSIL_ADDR (DESIGN Sec.9). EXPOSE is
# informational only.
EXPOSE 4042 4043
ENTRYPOINT ["/usr/local/bin/ichoi"]
CMD ["serve"]
