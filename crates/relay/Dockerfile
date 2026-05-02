# Multi-stage build: compile with full toolchain, ship with distroless runtime.
# Image size: ~35 MB.

FROM rust:1.85-slim AS builder
WORKDIR /build

# Pull workspace deps and build them once for layer caching.
RUN apt-get update && apt-get install -y --no-install-recommends \
        pkg-config libssl-dev ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY src-tauri ./src-tauri

# Build only the relay binary; skip Tauri / CLI to keep build minimal.
RUN cargo build --release --bin sec-relay -p secretariat-relay

# Runtime stage: distroless gives us glibc + ca-certs and nothing else.
FROM gcr.io/distroless/cc-debian12

COPY --from=builder /build/target/release/sec-relay /usr/local/bin/sec-relay

# Railway sets $PORT; our binary reads --bind. Expose 8443 as the convention.
EXPOSE 8443

# Default: bind 0.0.0.0:8443. Override with `--bind` arg or via railway.json.
ENTRYPOINT ["/usr/local/bin/sec-relay"]
CMD ["serve", "--bind", "0.0.0.0:8443"]
