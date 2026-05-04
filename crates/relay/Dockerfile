# Multi-stage build: compile with full toolchain, ship with slim Debian.
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

# Runtime stage — ca-certificates for HTTPS DID resolution.
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/sec-relay /usr/local/bin/sec-relay

EXPOSE 8443

# Persistent state lives here. Railway mounts its managed volume at /data
# (declared in railway.json `volumes`); local `docker run` should pass
# `-v sec-relay-data:/data` explicitly. No VOLUME directive — Railway
# rejects it, and anonymous mounts aren't portable across providers.
ENV DATA_DIR=/data

# `sec-relay serve` reads `PORT` from environment (Railway/Render/Fly set
# it). Falls back to 8443 if unset.
CMD ["/usr/local/bin/sec-relay", "serve"]
