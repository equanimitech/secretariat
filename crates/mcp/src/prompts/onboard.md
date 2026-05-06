# /onboard — bring the principal into Secretariat

You are about to walk the principal through Secretariat onboarding: identity setup + first stampable correspondence relationship.

The end state: the principal has an ed25519 keypair, a DID, the substrate scaffolding under `~/.secretariat/`, and one peer in their contact book they can compose stamped envelopes to.

## Recipe

### 1. Detect current state

Check whether the principal already has an identity. The `init` tool is idempotent-safe (refuses if a key exists), so you can either:

- Try `daemon_status` — if the daemon is installed/loaded, the principal almost certainly already has an identity. Skip to step 3.
- Or just attempt `init` and read the response. If it errors with a "key already exists" message, skip to step 3.

If the principal has no identity yet, continue to step 2.

### 2. Initialize identity

Ask the principal one question first: *"Do you own a domain you can host a DID document at (e.g. `rafa.equanimi.tech`)?"*

- **No / not sure** → call `init` with no `did` argument. The substrate generates an ed25519 key and derives a `did:key` (zero hosting required).
- **Yes** → call `init` with `did: "did:web:<their-domain>"`. After init, surface the published `did.json` content (read it from `~/.secretariat/did.json`) and tell the principal they need to host it at `https://<domain>/.well-known/did.json` for peers to resolve them.

Either way, after `init` succeeds, render the DID to the principal in plaintext. They will need to share it with peers (out of band) before the first invite/claim.

Then offer to install the daemon: *"Install the background daemon now? It runs on login and survives reboot."* — if yes, call `daemon_install`.

### 3. Establish the first correspondence relationship

Per the principal's invite-is-correspondence model: the bidirectional contact-add IS the relationship.

Ask the principal: *"Do you want to invite someone, or claim an invite someone sent you?"*

#### Inviter path

1. Confirm they're a registered tenant of a relay (the `invite_create` tool requires this; if it errors, surface the error and tell the principal they need to register first — this is a v0.x rough edge).
2. Call `invite_create` with optional `purpose` (e.g. `"first-contact"`) and default TTL.
3. Render the `claim_url` to the principal. Tell them: *"Send this URL to the peer through whatever transport you trust — email, Signal, paper. They run `/onboard` on their side and choose 'claim'."*
4. After the peer claims, the relay will route the peer's first envelope through; the inviter will see it appear in their inbox at the next review session. Mention this; do not promise a notification.

#### Claimant path

1. Ask the principal for the claim URL the peer sent.
2. Ask for a display name to give the inviter in the local contact book (e.g. *"Marcelo"* — defaults to the host portion of the DID).
3. Call `invite_claim` with the URL and name.
4. The tool auto-registers the principal's DID with the relay if needed AND adds the inviter to the contact book (this is the bidirectional contact-add). On success, render: *"You are now connected to <inviter_did>. They are in your contacts as <name>."*

### 4. End naturally

After the relationship is established, surface a single line: *"You can now compose envelopes to <peer> with `/compose <peer>`."* Then stop.

Do NOT proactively walk them through composing their first envelope — that's a separate cadence decision the principal makes.

## Rules

- **Render the DID in plaintext after init.** The principal needs to see and possibly copy it. Do not hide it behind a tool result they have to ask for.
- **Never call `init` without confirming the DID method choice.** `did:key` is irreversible per identity (you cannot upgrade to `did:web` without rotating keys); `did:web` requires hosting infrastructure.
- **Do not promise notifications, push, or read receipts.** Per the project's no-read-receipts invariant: the inviter sees claimed peers via inbox arrivals at review time, not via push.
- **The claim URL is sensitive.** Treat it like a one-shot bearer token. Don't quote it in chat history beyond the principal's confirmation; don't log it.
- **Do not auto-install the daemon without asking.** Some principals run Secretariat as a foreground tool; the LaunchAgent should be opt-in.
