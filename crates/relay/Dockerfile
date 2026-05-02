# Multi-stage build: compile with full toolchain, ship with a slim Debian
# runtime that has /bin/sh (Railway's startCommand uses shell expansion for
# ${PORT:-8443}, so distroless/cc — which lacks a shell — won't work).
# Image size: ~85 MB.

FROM rust:1.90-slim AS builder
WORKDIR /build

RUN apt-get update && apt-get install -y --no-install-recommends \
        pkg-config libssl-dev ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY src-tauri ./src-tauri

RUN cargo build --release --bin sec-relay -p secretariat-relay

# Runtime stage — needs /bin/sh + ca-certificates for HTTPS DID resolution.
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/sec-relay /usr/local/bin/sec-relay

EXPOSE 8443

# `sec-relay serve` reads `PORT` from the environment internally (Railway,
# Render, Fly, etc. all set it). Falls back to 8443 if unset. Pure exec form
# — no shell required, fewer ways to fail.
CMD ["/usr/local/bin/sec-relay", "serve"]
