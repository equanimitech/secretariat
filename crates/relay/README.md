# `sec-relay` — Secretariat federation relay

Forwards encrypted envelopes between principals. Each principal hosts their
own (or uses one a peer they trust hosts for them). Not a central server.

See `AGENTS.md` invariants #1 and #4 for the architectural constraints, and
`docs/milestones/2026-05-02-v0-correspondence.md` for the v0 design.

## Run locally

```bash
cargo run -p secretariat-relay -- serve --bind 127.0.0.1:8443
curl http://127.0.0.1:8443/healthz
```

Allowlist mode (only listed DIDs may register):

```bash
cargo run -p secretariat-relay -- serve \
  --bind 127.0.0.1:8443 \
  --allowlist did:web:rafa.equanimi.tech,did:key:z6MkhaXgBZDvotDkL5257faiztiGiC2QtKLGpbnnEGta2doK
```

## Deploy on Railway (recommended)

Railway supports the included `Dockerfile` natively. Once this repo is
public, a "Deploy on Railway" button will live here that points at the
template URL. For now:

1. Create a new project in Railway → "Deploy from GitHub Repo"
2. Set the build to use `crates/relay/Dockerfile`
3. (optional) Set `RUST_LOG=info,sec_relay=debug` in service variables
4. Add a custom domain (e.g. `relay.your-domain.com`) — Railway provisions
   TLS automatically

The `railway.json` in this directory tells Railway:
- Build via the included Dockerfile
- Healthcheck `GET /healthz`
- Restart on failure

## Deploy elsewhere

Anywhere that runs a Linux binary works. The crate cross-compiles cleanly
with `cargo build --release --target x86_64-unknown-linux-musl` and the
resulting static binary needs nothing but a port to bind.

- **Render** — supports Dockerfile builds; free tier spins down on idle (a
  v0 hourly poll cadence still works, with a ~30s cold-start delay)
- **Hetzner / DigitalOcean VPS** — `docker run` the image, point a CNAME
- **Fly.io** — `fly launch` from the Dockerfile
- **Self-hosted on Mac via Tailscale** — for you-and-Marcelo private mode,
  zero hosting cost

## API surface (v0)

| Method | Path | Purpose |
|---|---|---|
| GET  | `/healthz` | Health probe (Railway, load balancers) |
| POST | `/v0/register` | Register a tenant DID + ed25519 pubkey |
| POST | `/v0/auth/challenge` | Request a nonce to authenticate as a tenant |
| POST | `/v0/auth/answer` | Sign the nonce, receive a bearer token (1h TTL) |
| POST | `/v0/inbox/{did}` | Queue an envelope for a registered DID (open — sender sig is verified by recipient downstream) |
| GET  | `/v0/inbox/{did}?after=<id>` | Pull queued envelopes (bearer auth, must match recipient DID) |

Wire details in the relay source — see `crates/relay/src/routes/`.
