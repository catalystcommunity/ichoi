# Build from the source tree, then serve only the generated static files.
# The builder runs on the build platform: the static output is the same for every
# architecture, so a multi-architecture build needs no emulation.
FROM --platform=$BUILDPLATFORM python:slim AS builder
RUN apt-get update \
    && apt-get install -y --no-install-recommends git ca-certificates \
    && rm -rf /var/lib/apt/lists/*
RUN pip install --no-cache-dir uv
WORKDIR /build
COPY . .
RUN ./tools.sh build && ./tools.sh check-deploy

# No RUN step in this stage, so each target architecture builds without emulation.
FROM caddy:2.9.1-alpine
WORKDIR /site/logs
WORKDIR /site
COPY Caddyfile /etc/caddy/Caddyfile
COPY --from=builder /build/site ./root
