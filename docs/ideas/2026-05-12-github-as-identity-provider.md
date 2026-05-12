# GitHub as identity provider

**Date:** 2026-05-12
**Tags:** `equanimitech/secretariat`, identity, did:web, onboarding
**Status:** idea — captured during slice 1 shaping
**Related:**
- `docs/ideas/2026-05-12-workspace-registry-and-repo-local-substrate.md`
- `docs/decisions/2026-05-12-substrate-layout-v03.md`

---

## The pitch in one paragraph

Use GitHub Pages (free, every developer already has it) as the hosting
target for `did:web` identity documents. A user's identity becomes
`did:web:<username>.github.io`, anchored in a `.well-known/did.json`
they push to their own `<username>.github.io` repo. No new DID method
needed — it's the existing `did:web` flow with a hosting answer every
developer can use today. This composes with the workspace-registry idea
(channels as repos) into a single coherent stack: GitHub hosts identity
+ git transports channel updates + no central authority appears anywhere
in the chain except the one GitHub already is in the user's life.

---

## Why this matters

- **Zero new infrastructure for the user.** Anyone with a GitHub
  account can host a `did:web` identity in 5 minutes. No domain
  purchase, no DNS configuration, no server.
- **Leverages a muscle developers already have.** `git push` is
  familiar. Hosting `did.json` becomes "commit this file, push, enable
  Pages."
- **Composes with workspaces (idea B).** If channels live in
  `.secretariat/`-marked repos and identities anchor at GitHub Pages,
  git becomes the primary distribution medium for everything. The
  whole substrate runs on existing developer infrastructure, no SaaS
  required.
- **Encourages git literacy.** For the autonomous-enterprise framing,
  agents and humans collaborating via git is itself a strong default
  — version-control, audit trail, branching. Anchoring identity in
  the same place reinforces the muscle.
- **Discoverable.** "What's your GitHub?" → `did:web:<username>.github.io`
  deterministically. Invite flows can be username-based.

---

## How it works

GitHub Pages lets a user host static files at `<username>.github.io`
by creating a repo of that exact name and pushing to it. `.well-known/`
paths are served as-is. So:

1. User creates `<username>.github.io` repo.
2. Enables Pages on `main` branch, `/` root.
3. Adds `.well-known/did.json` containing:

   ```json
   {
     "@context": ["https://www.w3.org/ns/did/v1"],
     "id": "did:web:<username>.github.io",
     "verificationMethod": [{
       "id": "did:web:<username>.github.io#stamp-key-1",
       "type": "Ed25519VerificationKey2020",
       "controller": "did:web:<username>.github.io",
       "publicKeyMultibase": "<their-pubkey-multibase>"
     }],
     "assertionMethod": ["did:web:<username>.github.io#stamp-key-1"]
   }
   ```

4. Pushes. GitHub serves it at
   `https://<username>.github.io/.well-known/did.json`.

5. Anyone verifying an envelope signed by this DID fetches the URL,
   reads the verification method, validates the signature against the
   advertised key. Same as any `did:web`.

The private key never leaves the user's machine; GitHub only hosts the
public document. Sovereignty preserved.

---

## Onboarding flow (MCP-first)

```
[in Claude Code, fresh install]

User: "Set me up using my GitHub account."

Claude → setup_identity({did_method: "web", host: "github-pages"})
  ← {needs_input: ["github_username"]}

Claude: "What's your GitHub username?"

User: "rafaba"

Claude → setup_identity({
  did_method: "web",
  host: "github-pages",
  github_username: "rafaba",
  display_name: ...,
  full_name: ...
})
  ← {
      stage: "publish_required",
      did: "did:web:rafaba.github.io",
      did_json_path: "~/.secretariat/.staging/did.json",
      instructions: "..."
    }

Claude: "I've generated your keypair and a did.json document. To
finish, you need to publish it on GitHub Pages:

1. Create a public repo named **`rafaba.github.io`** (must match
   exactly).
2. Enable Pages: Settings → Pages → Source: `main` branch, `/` root.
3. Add the file at `.well-known/did.json` with this content:

   ```json
   <contents shown>
   ```

4. Commit and push.
5. Wait ~30 seconds for Pages to deploy.

When that's done, tell me and I'll verify."

User: "Done."

Claude → setup_identity({stage: "verify", did: "did:web:rafaba.github.io"})
  ← fetches https://rafaba.github.io/.well-known/did.json
  ← validates the pubkey matches the local key
  ← {state: "ready", identity_root: "~/.secretariat/rafaba.github.io/"}

Claude: "Verified. Your identity is `did:web:rafaba.github.io`,
anchored in your GitHub. Anyone who knows your GitHub username can
verify your envelopes by fetching that URL. No Secretariat server, no
central registry — your domain, your repo, your control. What's next?"
```

For users without GitHub or who don't want to expose a username, the
fallback is `did:key` (device-anchored, no network dependency).

---

## Risks + mitigations

| Risk | Mitigation |
|---|---|
| GitHub kicks the user out → DID unverifiable | `also_known_as` field listing fallback DID (e.g. did:key); rotation envelopes signed by both keys |
| GitHub Pages outage → temporary unverifiability | Cached `did.json` under `peers/` per substrate-global cache rule; staleness window acceptable for verification grace |
| User accidentally deletes `.well-known/did.json` | Same as above — cached copy + clear error message guiding re-publish |
| Username squatting (someone takes `<username>` and hosts a different key) | The key in the repo IS the identity — if the user lost the GitHub repo, the private key on their disk still doesn't match the new repo's published key, so it would just be a different identity. Identity is anchored in the keypair, not the username. The risk is that the *historical* DID (which others have cached) now resolves to someone else's key — fixed by key rotation + DID change. |

---

## What this composes with (forward-looking)

- **Workspaces (idea B):** channels live in repos with `.secretariat/`
  markers. Combined with GitHub-hosted identity: every aspect of
  Secretariat distribution rides git.
- **End-state monoslice (idea C):** if monoslice ever becomes the
  shipping path, GitHub identity would be one of the
  `setup_identity` paths from day one.
- **Workspace marketplace:** `cd <foreign-repo> && sec workspace
  register .` lets users discover channels by browsing GitHub repos.
  Discovery via search, not via central directory. Subscription via
  invite, never via "join button."

---

## No-gos

- No GitHub OAuth flow. That'd require Secretariat to be a registered
  OAuth client (central authority) and the user to grant tokens. We're
  using GitHub as dumb static hosting, not as auth.
- No GitHub API calls beyond fetching the public `did.json`. No
  reading of the user's repos, no listing of their orgs, no scraping.
- No mirror to GitLab/Bitbucket as a built-in feature — but the same
  rule trivially extends to any `did:web` host (GitLab Pages,
  Cloudflare Pages, Vercel, a static S3 bucket, your own server). The
  flow is the same; only the host instruction differs.

---

## Slice positioning

Goes into the identity-setup slice (whenever that lands). For today's
channels-first slice, no impact — captures don't care about identity
host.
