# Build from the source tree, then serve only the generated static files.
FROM python:slim AS builder
RUN apt-get update \
    && apt-get install -y --no-install-recommends git ca-certificates \
    && rm -rf /var/lib/apt/lists/*
RUN pip install --no-cache-dir uv
WORKDIR /build
COPY . .
RUN ./tools.sh build

FROM caddy:2.9.1-alpine
WORKDIR /site
RUN mkdir /site/logs
COPY Caddyfile /etc/caddy/Caddyfile
COPY --from=builder /build/site ./root
