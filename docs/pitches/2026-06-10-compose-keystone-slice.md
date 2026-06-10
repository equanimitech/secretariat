---
$attestation:
  $type: tech.equanimi.secretariat.stamp
  signer: did:key:z6MkjB8PQaN1vuUzdtnJsxyXR2f8d3tckGHkUYZMDytQsfak
  act: attest
  docHash: sha256:823f4322a64365cf77e53367d2930ed14597174e71b5b0128e16f24f61b070a6
  docFilename: 2026-06-10-compose-keystone-slice.md
  stampedAt: 2026-06-10T11:37:45.304330Z
  signature: ed25519:zpcJNNqeCag0IcOxfKweTWfVH2W4LCpe/KRS9CnhlUj7no407EDEY4rkgXOq+eE02Yq+AGqpc4e5RWMUCNvAAQ==
---

# Pitch — `compose`: every doc written through Secretariat, signed at birth

**Bet:** Ship `sec compose` + an MCP `compose` tool — the scribe writes docs
into a registered repo with owned conventions and a `$signature` from its
agent key, in one act.

**Why it matters:** Hard rule #4 says every authored body carries its author's
signature; today scribe-written docs land unsigned via generic file writes.
Compose closes the write-side of the trust model and becomes the funnel that
search and review build on.

---

## Boundaries

**JBTD:** As the scribe, when I capture or draft any doc (idea, pain,
decision, pitch, note), I want one verb that places it by convention and signs
it with my agent key, so every doc enters the substrate verifiable. Baseline:
generic `Write` — unsigned, conventions re-derived per session.

**Out:**
- No worktree placement — docs land in the repo's `docs/` on the current
  branch (the docs-pipeline spec is its own slice; the commit step moves with
  placement when it lands).
- No push — compose commits locally; outward flow (draft PR) is the
  docs-pipeline slice.
- No edit/re-sign flow — create-only; existing path errors, never overwrites.
- No enforcement (hooks blocking generic writes) — compose is the positive
  path; policing comes later if needed.
- No stamp — composing signs; stamping stays the principal's separate act.

## Elements

- **`compose_ops` use case** (`crates/core/src/application/compose_ops.rs`,
  new). Resolve repo via the `[[repos]]` registry (`repo_registry.rs:43`);
  map type → bucket (`idea`→`docs/ideas/`, `pain`→`docs/pain/`,
  `decision`→`docs/decisions/`, `pitch`→`docs/pitches/`, `note`→`docs/`);
  name `<date>-<kebab-slug>.md`; reject existing paths.
- **Sign + commit at birth.** Load the scribe's key
  (`agent_signing_key_path`) + DID from `authorized_agents`;
  `EnvelopeSignature::sign_body` (`signature.rs:114`, `SignerRole::Agent`);
  embed via the `$signature` machinery (`markdown.rs:96`). Then commit
  pathspec-scoped only — `git add -- <path>` + commit that path, message
  `docs(<type>): <title>`; never `-A`, co-mingled state untouched. Skip
  commit with a warning (file still written + signed) when mid-rebase/merge
  or detached HEAD. Single-scribe assumption: error if zero or >1 scribes.
  The signed commit is the "dispatch = signature" tier.
- **Parallel surfaces** — `sec compose --repo <path> --type <t> --title <s>`
  (body from stdin or `--body-file`) in `crates/cli/src/commands/compose.rs`
  + MCP `compose` tool (`crates/mcp/src/server.rs`) taking
  `{repo, doc_type, title, body}`, returning the written path.
- **Tests** — compose→verify round-trip (signature valid against scribe key);
  bucket + slug naming; collision error; no-scribe error. Gates:
  `cargo test --workspace`, `cargo clippy -- -D warnings`.

## Risks

**🐇 Rabbit holes:**
- Type taxonomy debates — ship the five buckets above; new types are a
  one-line map edit later.
- Body templating per type (frontmatter scaffolds) — keep to `type` + caller
  body; templates are editorial, not protocol.
- Multi-agent selection (`--as <agent>`) — cardinality is 1 today; add the
  parameter when a second scribe exists.

**🏴 Off-sides:** Worktree placement, edit/re-sign, Slack ingest as a compose
caller, hook enforcement — all adjacent, all later.

**🥩 Fat cut:** Auto-registering an unregistered repo. Compose errors with
"run `sec repo add`" instead.

**🧪 Domain knowledge:**
- `$signature` covers the canonical **body** hash only — editorial
  frontmatter edits (status flips, tags) don't invalidate it. Verify
  `verify_document_layered` reports the agent signature without requiring an
  agent manifest on disk.
- Compose on a feature branch commits the doc to that branch — the floater
  problem, committed edition. Accepted for the keystone; the worktree slice
  changes placement, not the verb.
- No new record shape: `tech.equanimi.secretariat.signature` lexicon already
  covers the embedded block — no lexicon diff needed (hard rule #3 satisfied
  by absence).

## Acceptance

1. `sec compose --repo . --type idea --title "test"` with stdin body writes
   `docs/ideas/<today>-test.md` carrying editorial frontmatter + `$signature`
   (signer = scribe DID, `signer_role: agent`).
2. The doc is committed (`git log -1 -- <path>` shows `docs(idea): test`);
   the tree is otherwise as dirty or clean as before — no other path staged
   or committed.
3. `sec verify` on the composed doc: signature **valid**, stamp **absent** —
   informational, not authoritative.
4. MCP `compose` produces the identical doc and returns its path.
5. Composing to an existing path errors; mid-rebase compose writes + signs
   but skips the commit with a warning.
6. `cargo test --workspace` + `cargo clippy -- -D warnings` green.

---

_Drafted by Claude (scribe)._
