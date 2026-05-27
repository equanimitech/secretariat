---
$attestation:
  $type: tech.equanimi.secretariat.stamp
  signer: did:key:z6MkjB8PQaN1vuUzdtnJsxyXR2f8d3tckGHkUYZMDytQsfak
  act: attest
  docHash: sha256:9d8b13e3ac1e84b22a2e7bf4c0fbbfe68117ef3df049369459b0f83ea4aedf2e
  docFilename: 2026-05-27-signet-protocol.md
  stampedAt: 2026-05-27T15:53:09.088166Z
  signature: ed25519:G/BEMbnG7wTbyB9QZ5nEny8vlzrlya0Isa9nZpLwcmrN4LZINhyFM3hsnLjTJHP6X/0SUxm70bKk88L3IkpKBg==
---
# Signet — the stamping protocol as a separate project

Status: proposal. Founding architecture note.
Author: Rafa (with Claude as scribe)
Date: 2026-05-27
Supersedes: implicit "protocol embedded in Secretariat-the-app" model.

## Premise

Secretariat today bundles three things into one codebase:

1. **A stamping protocol** — DIDs, envelopes, signatures, attestation ceremony, verify.
2. **A correspondence/journal app** — channels, captures, editor, MCP tools, daemon.
3. **A philosophical thesis** — "the signet returns; humans seal what AI drafts" (Torchbearer essay).

The protocol is the only piece with adoption potential outside Rafa's machine. The app is anchor-user-shaped (Rafa + Christophe). The thesis is paradigm work.

Pulling the protocol out as **Signet** lets each piece be what it is. Signet aims at adoption. Secretariat (or whatever the app becomes) aims at daily use. The thesis describes Signet.

## Is Signet even necessary?

Honest question raised mid-design. Working through it:

Most of what Signet would do already exists somewhere — ed25519 signing (everywhere), DIDs (W3C specs), biometric gates (OS APIs), lexicons-as-record-types (AT Protocol), markdown frontmatter parsing (every static site generator). **The crypto is boring (correctly so); the value isn't crypto novelty.**

Closest existing alternatives:

| Tool                         | Could it replace Signet?                                        | Gap                                                                                              |
| ---------------------------- | --------------------------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| git signed commits (GPG/SSH) | ✅ for repo-resident files; Touch ID via SSH agent already works | per-file signatures, portable-out-of-repo docs, in-document seal, DID identity                   |
| sigstore / cosign            | ✅ cross-platform, mature, signs blobs                           | OIDC identity (not DID); detached sig file (not in-document); built for CI/CD not correspondence |
| minisign / signify           | ✅ tiny ed25519 sign                                             | no identity model, no envelope convention, no biometric gate                                     |
| JWS in frontmatter           | ✅ standard, libraries everywhere                                | DID + biometric + convention work still required                                                 |
| AT Protocol records          | ⚠️ same lexicon shape, same DID model                           | federated/public; correspondence is private                                                      |
| Nostr events                 | ⚠️ DID-equivalent (npub), signed                                | public-by-default broadcast                                                                      |
| W3C Verifiable Credentials   | ✅ DID + signed claims                                           | heavy spec, JSON-LD, academic                                                                    |

Signet's actual distinctive niche, narrowed:

> **Portable, in-document signatures on individual markdown files, identified by DID, callable by agents via MCP.**

Three things distinguish that niche from existing tools:

1. **Signature embedded** ***in*** **the markdown frontmatter** — travels with the file. Git signs commits (not portable). Cosign produces detached sigs (not embedded). Signet's seal rides inside the envelope. This matches the article's "seal at the top" aesthetic and is functionally important for documents that leave a repo.
2. **DID-based identity** instead of GPG/OIDC. No CA, no key servers, no email-as-identity. Light, sovereign, peer-to-peer.
3. **MCP-first integration surface** — no existing signing tool is designed primarily for agent consumption. As agent tooling becomes the dominant integration pattern, this matters.

Honest acknowledgment: **Signet is mostly conventions + glue on top of existing primitives.** Same shape as Sigstore (conventions on ed25519 + OIDC + transparency logs) or Nostr (conventions on secp256k1 + JSON). That's not disqualifying — it's the typical pattern for an interop-shaped protocol — but it sets expectations: the value is the spec + the ergonomics + the article, not crypto novelty.

For repo-bound daily use (the journaling use case), **git signed commits would work today** with Touch ID via SSH agent. Signet's value emerges specifically when documents need to leave the repo, travel to recipients, and carry their seal with them. That use case has zero users today; whether it materializes is the article's bet.

Decision: ship Signet as planned. Make peace with the reinvention cost. Focus delivery on (a) clear spec, (b) MCP-first ergonomics, (c) the article as cultural anchor. Don't pretend the crypto is the moat.

## Why a new project — the adoption constraints make the case

Last conversational pass questioned whether a separate project is justified when there's only one consumer. Two design constraints flip that answer:

1. **Cross-platform from day one (Mac / Windows / Linux).** Secretariat is Mac-only by stated scope; carrying that constraint into the protocol kills adoption before it starts. Cross-platform isn't a backlog item — it's a founding requirement.
2. **MCP-by-default.** The primary integration point is an MCP server, not a CLI or library. Any agent on any platform plugs in via `signet-mcp` with zero ceremony. CLI is a thin secondary surface for power users / scripting.

These constraints don't fit Secretariat's design center (Rafa-first, Mac-first, Tauri shell as primary surface). They define Signet's.

## What Signet is

A focused primitive in the family of cosign / age / JWT: a small protocol with a reference implementation, designed to be embedded in other tools.

| Component                      | Purpose                                                                                                                                         |
| ------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| **Protocol spec**              | Canonical schemas + behavior. Lexicon files + a written RFC-style doc.                                                                          |
| **Reference Rust impl**        | `signet-core` crate — domain + ports, no IO.                                                                                                    |
| **CLI binary**                 | `signet` — 6 verbs: `init / agent / compose / stamp / verify / read`.                                                                           |
| **MCP server binary**          | `signet-mcp` — same 6 verbs as MCP tools, JSON Schema generated from Rust types.                                                                |
| **Keychain abstraction**       | `signet-keychain` — port + 3 adapters (macOS Keychain, Windows Credential Manager, libsecret).                                                  |
| **Biometric gate abstraction** | `signet-biometric` — port + adapters (Touch ID on macOS, Windows Hello on Windows, WebAuthn/Passkey as portable fallback for Linux + headless). |
| **Test vectors**               | Canonical envelopes per record type — interop fixtures.                                                                                         |

## What Signet is NOT

| Out                                                         | Where it lives instead                          |
| ----------------------------------------------------------- | ----------------------------------------------- |
| Channels / handle-paths / multi-party logs                  | App layer (Secretariat-the-app, Penceive, etc.) |
| Workspaces / activation surfaces / `.claude/` inheritance   | App layer                                       |
| Federation / relay / outbox / inbox                         | App layer (or out of scope entirely)            |
| Captures / journal / cron / autonomous cadence              | App layer                                       |
| Editor / UI / Tauri shell                                   | App layer                                       |
| Storage layout decisions (folders, content-addressed, etc.) | App layer                                       |
| Invite / bilateral correspondence ceremony                  | App layer                                       |

Signet does not have an opinion on where stamps live on disk, how they're organized, or how the user composes/reads them. It only knows: how to sign, how to stamp (with biometric gate), how to verify, how to identify principals via DID.

## Repo structure

```
~/Developer/equanimitech/signet/
├── crates/
│   ├── signet-core/          # protocol domain + ports (no IO)
│   ├── signet/               # CLI binary
│   ├── signet-mcp/           # MCP server binary
│   ├── signet-keychain/      # keychain abstraction + 3 adapters
│   └── signet-biometric/     # biometric gate abstraction + adapters
├── lexicons/                 # record-shape schemas (the wire contract)
├── spec/                     # protocol RFC document
├── examples/                 # canonical envelopes, test vectors
├── .github/workflows/        # CI matrix: macOS + Windows + Linux
└── README.md                 # what it is, how to install, link to spec
```

## Cross-platform implications

Each platform-bound concern in current Secretariat needs a portable abstraction:

| Concern                            | macOS today                                        | Windows                                    | Linux                                                            |
| ---------------------------------- | -------------------------------------------------- | ------------------------------------------ | ---------------------------------------------------------------- |
| Keychain                           | Security framework / `security-framework` crate    | Credential Manager / `windows-credentials` | Secret Service via `libsecret` / `secret-service-rs`             |
| Biometric gate                     | LocalAuthentication (Touch ID, Apple Watch unlock) | Windows Hello via `windows-rs`             | CTAP2 direct (Yubikey / hardware key) + `fprintd` D-Bus fallback |
| Headline-in-dialog (anti-phishing) | LAContext reason string                            | Windows Hello prompt text                  | CTAP2 message / fprintd prompt                                   |
| Filesystem paths                   | `~/.secretariat/` etc.                             | `%APPDATA%\Signet\`                        | `$XDG_DATA_HOME/signet/`                                         |
| MCP install                        | `claude mcp add` works today                       | Same                                       | Same                                                             |
| Daemon (if any)                    | launchd                                            | Service Manager                            | systemd user units                                               |

**Biometric gate — no server required.** Earlier drafts of this note framed WebAuthn as the portable abstraction, which implied (incorrectly) a localhost HTTPS server. Corrected design:

* **macOS**: LocalAuthentication (already partially in `native_biometric.rs`). Native, no server, no webview.

* **Windows**: Windows Hello API via `windows-rs`. Native, no server, no webview.

* **Linux**: CTAP2 direct to a hardware key (`authenticator-rs` crate) as primary; `fprintd` D-Bus fallback where available.

* **Headless / dev / CI**: CTAP2 with a hardware key.

`BiometricGate` is a port. Three native adapters + one CTAP2 fallback. Crates of interest: `security-framework` (macOS), `windows-rs` (Windows Hello), `authenticator-rs` (Mozilla's CTAP2 client, used in Firefox), `passkey-rs` (library-shaped WebAuthn impl, no HTTP). Signet never runs a server.

WebAuthn-via-browser-API (with localhost HTTPS) is not the path. The "Relying Party" terminology in WebAuthn does not require a remote endpoint — it can be the local process — but the browser-mediated flow does require some HTTPS host. CTAP2 + native APIs bypass that entirely.

## MCP-by-default — what that demands

Today Secretariat ships an MCP server as a sidecar. For Signet, MCP is the **primary surface**, not a sidecar.

| Thing      | Today (Secretariat)                                                            | Signet                                                                                                    |
| ---------- | ------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------- |
| Install    | Tauri shell bundles `sec-mcp`; `sec mcp install` wires it                      | `cargo install signet` + `claude mcp add signet` (or equivalent)                                          |
| Tool names | `compose / stamp / verify / read / list_channels / ...` (app + protocol mixed) | `compose / stamp / verify / read / agent_add / agent_list` (protocol only)                                |
| Schema     | Hand-written in `crates/mcp/server.rs`                                         | Generated from Rust types via `rmcp` / `schemars`                                                         |
| Daemon dep | Currently MCP can run standalone                                               | Standalone by design; no daemon required                                                                  |
| Storage    | Bound to `~/.secretariat/`                                                     | Configurable storage path; Signet's "store" is just a directory of stamped markdowns. App decides layout. |

Goal: a developer adopting Signet should be able to:

```bash
cargo install signet
signet init
claude mcp add signet
# Done. Their agent can now stamp markdown.
```

Three commands. No project structure required. The agent gets stamping as a capability.

## Relationship to Secretariat

Three viable transitional paths. Each ends at the same shape (Signet protocol + at least one consumer).

| Path                      | Move                                                                                                                                                               | Pro                                                      | Con                                                                               |
| ------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------- | --------------------------------------------------------------------------------- |
| **Clean room**            | Build Signet from scratch in a new repo; Secretariat continues until Signet is mature enough to replace its protocol layer.                                        | No legacy baggage; cross-platform discipline from line 1 | Duplicates work; two protocol implementations until reconverge                    |
| **Extract then refactor** | Copy current `crates/core` + relevant pieces into Signet repo; refactor for cross-platform; Secretariat depends on Signet via Cargo as soon as Signet is buildable | Reuses thought; faster initial Signet                    | Carries Mac assumptions until refactored; risk of "extraction was wrong"          |
| **Internal layer first**  | Discipline the Signet-shaped layer inside Secretariat (already mostly there in `crates/core`). Promote to separate repo only when cross-platform need bites.       | No new repo overhead; lowest cost                        | Doesn't satisfy the "cross-platform from day one" constraint; same project we had |

The "internal layer first" path was my prior recommendation. It does not satisfy the new design constraints. If Signet's reason for existing is cross-platform + MCP-first adoption, it has to be designed under those constraints from the start. That implies path 1 or 2.

**Recommended: clean room.** Build Signet in a new repo. Don't try to refactor Secretariat's Mac-flavored code into something portable; start fresh with portability as a constraint. Secretariat continues as-is until Signet v0.1 is buildable, then Secretariat is migrated as a consumer.

The clean-room approach also forces the protocol spec to be *written* (not just *implemented*). That spec is what makes Signet adoptable.

## First milestone — Signet v0.1

Scope:

* [ ] `signet-core` crate: DID (`did:key` + `did:web`), envelope frontmatter, ed25519 signature, stamp record shape, verify algorithm

* [ ] `signet` CLI: `init / agent / compose / stamp / verify / read` working on all three platforms

* [ ] `signet-mcp` MCP server: same 6 verbs exposed; schema auto-generated

* [ ] Keychain abstraction: macOS + Windows + Linux adapters; identity-record persistence

* [ ] Biometric gate: WebAuthn-based portable adapter; headline + hash-prefix in challenge

* [ ] Lexicons: `envelope`, `signature`, `stamp` record shapes documented

* [ ] Test vectors: one canonical envelope per record type; round-trip sign + verify on each platform

* [ ] CI: GitHub Actions matrix passing on macOS / Windows / Linux

* [ ] README + install instructions

* [ ] Minimal spec doc (`spec/protocol.md`) — at least: identity model, envelope structure, signature/stamp/verify algorithms, security considerations

Out of scope for v0.1:

* Counter-stamp ceremony (lexicon entry only; no implementation)

* Stamp chain / Merkle batching

* Cross-platform daemon (Signet is library + CLI + MCP, no daemon)

* Encryption of envelope bodies (signed cleartext only; encryption is an app-layer concern for v0.1)

* Storage layout opinions

* GUI of any kind

## Implication for Secretariat-the-current-repo

While Signet incubates:

* Secretariat continues to ship to Rafa. No urgent change.

* The `crates/core` protocol code stays where it is — it's Secretariat's protocol layer until Signet replaces it.

* The "Stamped by Humans" article should be re-read with Signet in mind; the philosophical referent is Signet, not Secretariat. Minor edits where the essay names the system.

* New protocol-shaped work goes into Signet, not Secretariat. App-shaped work continues in Secretariat.

When Signet hits v0.1:

* Secretariat adds Signet as a Cargo dependency

* Secretariat's protocol crates are gradually replaced by Signet calls

* Secretariat keeps its app concerns (editor, MCP tools beyond the protocol, captures, journal, workspaces)

* Migration is incremental — no flag day

## Implication for the AG-adaptive renderer pitch

The AG renderer is app-layer, not protocol-layer. It stays in Secretariat's pitch backlog. Signet has no opinion on rendering.

## Implication for the article

"Stamped by Humans" describes the act of sealing. That act IS the Signet protocol. The article should probably:

* Introduce the protocol by name once (§5 or §7, when the envelope format is shown)

* Frame Signet as the substrate; "Secretariat" or "Penceive" or future apps as consumers

* The §5 envelope example is *literally* a Signet envelope

This is a small edit. Defer until Signet v0.1 exists so the article isn't promising vapor.

## What this changes about the bounded context

AGENTS.md's bounded-context section names two anchor flows: Rafa↔Marcelo (book) and Rafa↔Christophe (Themia briefs). With Marcelo dropped as an anchor (per conversation 2026-05-27), the remaining anchor is Themia. That's a Secretariat-the-app concern, not a Signet concern.

Signet's anchor users are **any developer who wants to add stamping to their agent or tool**. The first concrete consumer is Secretariat itself. The second could be Penceive. The third — unknown today, but the cross-platform + MCP-first design is what makes the third user possible.

## Open questions

1. **Lexicon namespace.** Today: `tech.equanimi.secretariat.*`. Options for Signet: keep as-is (URI is just identity), rename to `tech.equanimi.signet.*`, or go vendor-neutral (`org.signet.*` or similar). Lean keep-as-is for v0.1; coordinate rename to `tech.equanimi.signet.*` at v1.0.
2. **CLI binary name.** `signet` is descriptive but 6 chars. `sig` is shorter but collides with signal-processing. Lean `signet`.
3. **Crate publication.** Publish to crates.io from v0.1, or wait for stability? Publishing pressures the API to settle but lock-in is real. Lean: publish from v0.1 with explicit `0.x = no stability promise` notice.
4. **License.** Apache-2.0 / MIT dual is conventional for protocol crates. Confirm with Rafa.
5. **Repo location.** `~/Developer/equanimitech/signet/` follows the existing namespacing under `equanimitech/`. GitHub: `equanimitech/signet` or its own org? Lean equanimitech namespace.
6. **WebAuthn-as-portable-biometric — is the UX acceptable on macOS?** Touch ID via WebAuthn works but goes through the browser/Passkey flow. Native Touch ID via LocalAuthentication is smoother. Decision: ship WebAuthn-portable in v0.1; add native Touch ID adapter in v0.2 if UX warrants.

## Next moves

1. Capture this doc as the founding architecture note. ✅ (this file)
2. Decide on the open questions above (Rafa).
3. Create the Signet repo skeleton (Cargo workspace, CI matrix, README stub).
4. Port `signet-core` from current `crates/core`, refactoring for cross-platform.
5. Build the keychain + biometric portable abstractions.
6. Wire `signet-mcp` and `signet` CLI on top.
7. Write the spec doc.
8. Tag v0.1 when CI passes on all three platforms with a canonical envelope round-trip.

Appetite: an exploratory cycle, not a 2-week bet — protocol work surfaces unknowns. Re-shape into a proper pitch once the repo skeleton exists and the unknowns are scoped.
