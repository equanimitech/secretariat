# /onboard — establish the first stampable correspondence relationship

You are about to walk the principal through Secretariat onboarding.

The end state: the principal has an ed25519 keypair, a DID, the substrate scaffolding under `~/.secretariat/`, and one peer in their contact book they can compose stamped envelopes to.

## Recipe

### 1. Detect identity

Check whether the principal has already initialized their identity. The cheapest signal: try to fetch the `secretariat://template` resource — it only exists once `init` has run and seeded `~/.secretariat/`.

- **Resource fetches successfully** → identity exists, skip to step 3.
- **Resource missing / read fails** → identity not yet set up, go to step 2.

### 2. Identity setup happens outside MCP

Tauri owns identity creation — there's a tray-anchored onboarding popover for the one-shot first-launch ritual that handles keypair generation, DID derivation (default `did:key`), and substrate scaffolding. The MCP is intentionally not a third path here.

Tell the principal:

> _Open Secretariat.app from the menu bar tray. The first-launch popover will set up your identity and key. Once that's done, come back and I'll continue._

If they prefer the CLI: `sec init` (no flags for `did:key`, or `--did did:web:<their-domain>` if they own a domain to host a DID document at). After CLI init, they should also launch Secretariat.app once so the silent-wire installs the daemon.

Wait for the principal to confirm identity is set up, then re-fetch `secretariat://template`. Once it returns, proceed to step 3.

### 3. Establish the first correspondence relationship

Per the principal's invite-is-correspondence model: the bidirectional contact-add IS the relationship.

Ask the principal:

> _Do you want to invite someone, or claim an invite someone sent you?_

#### Inviter path

1. Call `invite` with optional `purpose` (e.g. `"first-contact"`) and default TTL.
2. If the call errors with a relay-tenant complaint, surface that error verbatim — the principal needs a registered relay tenancy, which is currently a v0.x rough edge that the Tauri Settings pane handles.
3. On success, render the `claim_url` to the principal. Tell them:

   > _Send this URL to the peer through whatever transport you trust — email, Signal, paper. They run `/onboard` on their side and choose 'claim'. The relay will route their first envelope to your inbox at the next review session._

   Do not promise a notification — per the no-read-receipts invariant, peer arrivals surface only at review time.

#### Claimant path

1. Ask the principal for the claim URL the peer sent.
2. Ask for a display name to give the inviter in the local contact book (e.g. _"Marcelo"_) — defaults to the host portion of their DID if omitted.
3. Call `accept_invite` with the URL and name.
4. The tool auto-registers the principal's DID with the relay if needed AND adds the inviter to the contact book (this is the bidirectional contact-add).
5. On success, render: _"You are now connected to <inviter_did>. They are in your contacts as <name>."_

### 4. End naturally

After the relationship is established, surface a single line: _"You can now compose envelopes to <peer> with `/compose <peer>`."_ Then stop.

Do NOT proactively walk them through composing their first envelope — that's a separate cadence decision the principal makes.

## Rules

- **Do not call `init` from MCP.** It's not a tool here. Identity creation goes through the Tauri popover or the CLI, never through Claude.
- **The claim URL is sensitive.** Treat it like a one-shot bearer token. Don't quote it in chat history beyond the principal's confirmation; don't log it.
- **Do not promise notifications, push, or read receipts.** Per the no-read-receipts invariant: the inviter sees claimed peers via inbox arrivals at review time, not via push.
- **One peer at a time.** If the principal wants to invite multiple peers, run `invite` once per peer; don't try to bulk-create.
