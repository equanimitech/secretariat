# slice 3 — identity consolidation

Pitch — 2026-05-18. Next slice after the namespace collapse (v0.6.0
shipped today). Same direction of travel: every principal-owned file
ends up under `_self/` so the layout invariant "principal-as-queue-root
mirrors org-as-queue-root" actually holds.

Today the principal's identity is scattered at the vault root:

```
~/.secretariat/
├── did                      ← DID string, one line
├── key                      ← ed25519 PKCS#8 PEM, 0600
├── did.json                 ← DID document scaffold (did:web hosts upload this)
├── profile.json             ← display_name + full_name
├── _self/                   ← (added in v0.6.0; otherwise empty until you create a channel)
│   └── channels/...
├── orgs/<alias>/
│   ├── .org                 ← org metadata
│   └── channels/...
```

The shape `_self/` and `orgs/<alias>/` are _supposed_ to be the same
primitive — both queue-roots, one for the principal, one for each org
the principal subscribes to. But `_self/` is missing its identity
file, its profile, and its DID document; those still live at the vault
root. Meanwhile `orgs/<alias>/.org` carries an org's metadata in the
right place. Asymmetry leaks.

## Boundaries

### Job to be done

As a principal opening `~/.secretariat/`, I want **my own identity to
live inside `_self/`** — same shape as an org's identity file inside
its own dir. So that backup is `tar -czf principal.tgz _self/`,
restore is the reverse, and the file layout teaches the rule "every
queue-root knows itself."

_When_: every time I write onboarding docs and have to explain "your
key lives at `~/.secretariat/key`, your DID lives at `~/.secretariat/did`,
your profile at `~/.secretariat/profile.json` — wait but your _channels_
live under `~/.secretariat/_self/channels/`." The split-brain at the
filesystem root makes the substrate look unfinished. It is unfinished.
Finish it.

### Appetite

`small`. One coherent move. Estimate: ~one focused day.

This is much narrower than slice 2 (which touched every load/save
site). Slice 3 touches:

- `KeyPaths` paths: `signing_key`, `did_document`, `profile` → repoint
  to `_self/identity/*`.
- `identity.md` frontmatter shape — new file, sole writer is
  `sec init` (CLI) / Tauri `init_identity`.
- One migration step bolted onto the existing v0.6.0 hand-script (or a
  fresh `migrate-vault-v0.6.1.sh` if v0.6.0 already ran).
- Lexicon edit for the (new) `identity` record type.
- Onboarding docs.

No domain logic moves. No envelope plumbing changes. Risk surface is
narrow but very deep at one point: the **key file move**.

### What's in scope

**New layout:**

```
~/.secretariat/
├── _self/
│   ├── identity.md              ← frontmatter: did, display_name, full_name, key_path, did_method, created_at, rotations[]
│   ├── identity/
│   │   ├── key                  ← raw ed25519 PKCS#8 bytes, 0600 (unchanged content; moved location)
│   │   └── did.json             ← DID document scaffold (did:web hosts upload this)
│   ├── contract-stub.md         ← (shipped in v0.6.0)
│   └── channels/...             ← (shipped in v0.6.0)
├── orgs/<alias>/...
├── preferences.toml             ← stays at root (app-level machine config; see Elements §6)
├── relay-state.json             ← stays at root (daemon runtime state)
├── contacts.json                ← stays at root in slice 3; slice 4 moves to _self/contacts.md
├── template.md                  ← stays at root in slice 3; per-channel override is its own slice
└── ...
```

- Move `did` + `key` + `did.json` + `profile.json` data into
  `_self/identity.md` + `_self/identity/key` + `_self/identity/did.json`.
- `KeyPaths.signing_key`, `KeyPaths.did_document`, `KeyPaths.profile`
  repoint to the new paths. `KeyPaths` gains `identity_md`.
- `sec init` (CLI) and Tauri `init_identity` write the new layout from
  day one — fresh installs never see the old shape.
- Migration: hand-script step that reads `did` + `key` + `did.json` +
  `profile.json`, writes `_self/identity.md` (frontmatter from
  profile + did + key metadata) + `mv`'s `key` and `did.json` into
  `_self/identity/`, deletes the old `did` text file.
- Lexicon entry for `tech.equanimi.secretariat.identity` (new record
  type) shipped in the same commit per AGENTS.md rule #3.
- Onboarding docs (`docs/audits/2026-05-04-onboarding-ux.md`,
  `docs/milestones/2026-04-30-first-signed-message.md`) updated to
  show the new paths.

### What's out of scope

- `contacts.json` → `_self/contacts.md` (slice 4 — different file
  shape conversation, deserves its own pitch).
- `cognition.json` → `_self/cognition.md` (slice 4).
- `orgs/<alias>/.org` → `orgs/<alias>/org.md` (slice 4 — same JSON →
  markdown design conversation).
- `relay-state.json` move — daemon-managed runtime state, not
  principal-editable; lives at root happily. Future could move to
  `.runtime/relay-state.json` but that's cosmetic.
- `preferences.toml` move — app-level machine config (window state,
  cognition launcher, dev flags). Tauri settings pane writes it; not
  principal-prose. Stays at root. See Elements §6.
- HSM / encrypted-bundle key wrapping. Keys stay loose binary at
  `_self/identity/key` as before — just at a new path. Rotation /
  migration UX is a separate wedge.
- Multi-device same-principal sync. Out-of-scope; future v0.7+ wedge.
- Per-channel `template.md` override — separate slice (needs the
  queue_dir resolver alignment first; flagged in v0.6.0 release notes).

## Elements

### 1. `identity.md` shape

```markdown
---
$type: tech.equanimi.secretariat.identity
did: did:web:rafa.equanimi.tech
did_method: did:web
display_name: Rafa
full_name: Rafael T. Ballestiero
key_path: identity/key # relative to this file
key_type: ed25519
key_created_at: 2026-05-12T05:55:00Z
key_rotations: [] # append on rotation; old entries point at archived key paths
created_at: 2026-05-12T05:55:00Z
---

# Identity

The principal's identity record. The DID is the canonical identifier;
the file referenced by `key_path` is THE proof. Backup this directory,
not just the JSON files that used to be here.

This body is principal-editable prose. Free-form notes about the
identity — preferred pronouns, signature line, hand-written context
for anyone restoring this vault years from now.
```

Same pattern as `contract.local.md` from v0.6.0: typed YAML
frontmatter for the machine-enforced fields (`did`, `key_path`,
`key_type`), markdown body for principal-editable prose.

The `key_rotations` array starts empty. When key rotation ships as a
separate wedge, each rotation appends one entry (old key archived to
`identity/key.<rotation-ts>`) with metadata; the active key always
lives at `identity/key`.

### 2. New `KeyPaths` shape

```rust
pub struct KeyPaths {
    pub root: PathBuf,
    pub self_root: PathBuf,

    // -- new (slice 3) --
    pub identity_md: PathBuf,         // <self_root>/identity.md
    pub identity_dir: PathBuf,        // <self_root>/identity/

    // -- repointed (slice 3) --
    pub signing_key: PathBuf,         // <self_root>/identity/key  (was root.join("key"))
    pub did_document: PathBuf,        // <self_root>/identity/did.json  (was root.join("did.json"))
    pub profile: PathBuf,             // <self_root>/identity.md  (was root.join("profile.json"))

    // -- unchanged --
    pub contacts: PathBuf,            // root.join("contacts.json")  ← slice 4 moves this
    pub relay_state: PathBuf,         // root.join("relay-state.json")  ← stays
    pub preferences: PathBuf,         // root.join("preferences.toml")  ← stays
    pub orgs_root: PathBuf,
    pub contract_stub: PathBuf,
    pub contextification_log: PathBuf,
    pub template: PathBuf,            // root.join("template.md")  ← stays in slice 3
    pub bin: PathBuf,
    pub peers_cache: PathBuf,
    pub legacy_cognition_config: PathBuf,
    pub legacy_cadence: PathBuf,
}
```

`KeyPaths::profile` repoints to the same file as `KeyPaths::identity_md`
in slice 3 — they're aliases pointing at one file. Earlier-shipped
`load_profile` / `save_profile` (in `profile_store.rs`) continue
working against that path; the profile_store learns to read +
write the new `identity.md` shape (frontmatter parser + markdown body
preservation).

### 3. `sec init` / Tauri `init_identity` updates

Fresh installs write the new layout directly:

```rust
pub fn init_identity(paths: &KeyPaths) -> Result<IdentityState, ...> {
    // 1. fs::create_dir_all(&paths.identity_dir)
    // 2. Generate keypair, derive did:key (or accept --did did:web:...)
    // 3. save_signing_key(&paths.signing_key, &key)   // writes to <self_root>/identity/key
    // 4. write_did_document(&paths.did_document, &did, &pubkey)  // writes <self_root>/identity/did.json
    // 5. write_identity_md(&paths.identity_md, &did, &profile, &key_metadata)
    // 6. (no more separate `did` text file at root — DID lives in identity.md frontmatter)
}
```

The old `did` text file at root is gone. Callers that need the DID
string read it from `identity.md` frontmatter via a small
`load_principal_did(&paths)` helper. The Tauri front-end's
`init_identity` mirrors.

### 4. Migration step

Append to `scripts/migrate-vault-v0.5.0.sh` (or rename to
`migrate-vault-v0.6.x.sh` since v0.6.0 already ran). New steps after
the existing namespace-collapse moves:

```bash
# ---- slice 3 — identity consolidation ----
if [[ -f "$VAULT/key" && ! -f "$SELF_ROOT/identity/key" ]]; then
  mkdir -p "$SELF_ROOT/identity"

  # Read existing did + profile.
  did="$(cat "$VAULT/did" 2>/dev/null || echo '')"
  display_name=""; full_name=""
  if [[ -f "$VAULT/profile.json" ]]; then
    display_name="$(jq -r '.display_name // ""' "$VAULT/profile.json")"
    full_name="$(jq -r '.full_name // ""' "$VAULT/profile.json")"
  fi

  # Build identity.md frontmatter (unknown profile keys preserved verbatim — see Risks).
  # ... HEREDOC emitting the new file ...

  # Move key + did.json (NEVER rm; never cp).
  mv "$VAULT/key" "$SELF_ROOT/identity/key"
  [[ -f "$VAULT/did.json" ]] && mv "$VAULT/did.json" "$SELF_ROOT/identity/did.json"

  # Sanity-check: signature roundtrip against the new key path.
  # (Optional but recommended — see Risks.)

  # Old did text file: DID now lives in identity.md frontmatter; remove the text file
  # ONLY after identity.md is verified readable.
  rm -f "$VAULT/did"
  rm -f "$VAULT/profile.json"
fi
```

Pre-flight `tar` snapshot already covers the rollback path (existing
script behavior). Add a post-move check: `load_principal_did(paths)`
must return the same DID as before; mismatch = abort + restore.

### 5. Lexicon

New record type `tech.equanimi.secretariat.identity` in
`lexicons/tech.equanimi.secretariat.identity.json`. Shape mirrors the
frontmatter:

- `did` (required) — DID string
- `did_method` (required, knownValues: `did:key`, `did:web`)
- `display_name` (required) — UI / informal context
- `full_name` (optional) — formal contexts (envelope signatures,
  legal artifacts)
- `key_path` (required) — relative to identity.md, conventionally
  `identity/key`
- `key_type` (required, knownValues: `ed25519`)
- `key_created_at` (required, format: datetime)
- `key_rotations` (optional, array; empty by default)
- `created_at` (required, format: datetime)

Ships in the same commit as the Rust code per AGENTS.md hard rule #3.

### 6. Why `preferences.toml` doesn't move

App-level machine config (window position, terminal picker, cognition
launcher selection, dev-mode flags). The Tauri settings pane is the
authoritative writer. Not principal-prose; not part of identity. TOML
has the right ergonomics for that surface (typed, no parser
surprises).

The markdown-everywhere rule applies to **principal-authored,
agent-read context**: identity, contracts, templates, contacts,
cognition prompts. Not machine config.

Same logic for `relay-state.json`: pure daemon runtime state,
regenerable, never principal-edited.

## Risks

### 🐇 Rabbit holes

- **Key move atomicity.** `mv` is atomic on the same filesystem (same
  inode), so the existing-script approach is safe on local-disk
  vaults. On a vault that crosses filesystems (rare — vaults live
  under `$HOME`), `mv` falls back to copy+unlink and the unlink could
  fail mid-flight. Mitigation: the pre-flight `tar` snapshot already
  covers it; add the same `cp` → unlink check that the namespace
  collapse migration uses.

- **`profile.json` → `identity.md` frontmatter loss.** Round-trip via
  serde → typed struct → serde is lossy if the principal added
  unknown keys to `profile.json`. Mitigation: migration script does
  NOT round-trip — reads raw JSON, emits raw YAML, preserves unknown
  keys verbatim under an `extra:` block. Same pattern slice 2's
  `.channelDef` → `channel.md` migration used.

- **Tauri app daemon recreates old paths.** When v0.6.x app is
  running with v0.6.0 sidecars and the migration runs, `ensure_dirs()`
  might recreate `did` / `key` at root because old `KeyPaths` knows
  those paths. Mitigation: same as v0.6.0 cutover — quit the app
  before running the migration, install the new release, restart.

- **`load_principal_did` callers.** Right now ~12 call sites read
  the `did` text file directly. Slice 3 routes them through a
  single helper that reads `identity.md` frontmatter. Each call
  site's a one-liner change but the audit is what takes time.
  Mitigation: grep + bulk replace; runs in an hour, not a day.

- **MCP tool descriptions reference paths.** `secretariat://contacts`
  - `secretariat://orgs` resource render functions don't reference
    identity files, but onboarding prompts (`onboard.md`) might. Audit
  - update.

### 🏴 Off-sides called

- HSM / encrypted-bundle key wrapping. Loose binary at known path
  remains; rotation ergonomics is a separate wedge.
- `did.json` upload automation for `did:web` principals. Manual upload
  to their domain's `.well-known/did.json` stays the principal's job.
- Multi-device sync. The new `_self/identity/` directory is portable
  via `tar` / `rsync` / `git` (per portability invariants); turning
  that into a sync product is its own thing.

### 🥩 Fat cut

- Could ship the move WITHOUT consolidating `did` text file +
  `profile.json` into `identity.md` frontmatter. Just `mv key` and
  `mv did.json` into `_self/identity/`; keep `did` + `profile.json`
  as separate root files. Buys ~3 hours. Cost: the asymmetry stays
  visible at the root (some files moved, others didn't). Recommend
  against the cut — the full move is small enough to do once.

### 🧪 Domain knowledge

- Confirm: do any non-Rafa principals have `did:web` setups today?
  If yes, `did.json` move could break their hosting flow (the file
  they upload is at a different on-disk path). Marcelo + Christophe
  are `did:key`, so no `did.json` files exist for them — no upload
  flow to break. For Rafa: `did:web:rafa.equanimi.tech`'s
  `.well-known/did.json` is hosted statically; just need to update
  the local source-of-truth path.

## Pitch

### Problem

The v0.6.0 layout makes `_self/` the principal's queue-root, mirroring
`orgs/<alias>/` for org subscriptions. But the principal's _identity_
— DID, keypair, profile, DID document — still sits at the vault
root. Asymmetry leaks into every onboarding doc, every backup
instruction, every "where does my key live?" question.

This is sediment from v0.2 (when there was no `_self/` and the vault
root WAS the principal's workspace). v0.6.0 collapsed the rest of the
namespace into a single primitive; the identity files are the
remaining hold-out.

### The bet

Move principal identity into `_self/identity.md` + `_self/identity/`.
Keep `preferences.toml` and `relay-state.json` at root (app config +
daemon state, not principal context). Consolidate `did` + `profile.json`
data into one principal-editable `identity.md` with YAML frontmatter

- free-form markdown body. Same pattern as v0.6.0's
  `contract.local.md`: typed fields the machine enforces, prose the
  agent respects.

Ship as v0.6.1 (or v0.7.0 if we want to signal the layout is now
truly self-symmetric). Hand-script migration that `mv`s the key file
and consolidates the rest. Pre-flight tar snapshot + post-move DID
roundtrip gate, same discipline as v0.6.0.

The bet pays off when:

- "Backup my Secretariat" = `tar -czf backup.tgz ~/.secretariat/_self/
~/.secretariat/orgs/`. Two paths, both shaped the same way.
- "Where does my key live?" has one answer: `_self/identity/key`.
- Onboarding docs stop saying "your key is at `~/.secretariat/key`
  AND your channels are under `_self/channels/`" with a confused
  conjunction.
- Future slice 4 (`contacts.md`, `cognition.md`, `org.md`) lands
  against a coherent identity primitive instead of working around
  scattered root files.

### No-gos

- **No key destruction.** `mv` only on the key file; pre-flight `tar`
  snapshot is the rollback. Same envelopes-never-destroyed discipline
  applies to keys (memory: `feedback_envelopes_never_destroyed` —
  the same principle generalizes). Loss of the key is loss of every
  signature the principal can verify going forward; treat with the
  same care as envelope loss.
- **No format proliferation.** Markdown + frontmatter for principal
  context (identity.md); binary for keys; JSON for daemon runtime
  state (relay-state.json); TOML for app config (preferences.toml).
  Four formats, each with a clear reason. Slice 3 doesn't introduce
  a fifth.
- **No sync surface added.** Slice 3 is layout only — multi-device
  sync, key rotation UX, HSM wrapping all remain out of scope.

## Reference

- AGENTS.md rule #3 (lexicons-as-SoT-by-practice), rule #6
  (four-surface parallel)
- v0.6.0 release: 2213f6d (merge), d1237a3 (release commit)
- Slice 2 pitch (predecessor): `docs/pitches/2026-05-17-collapse-namespaces.md`
- Migration discipline memory: `memory/feedback_envelopes_never_destroyed.md`
  (key files are sovereign sediment too; same `mv`-only rule applies)
- `crates/core/src/infrastructure/keys.rs` — current `KeyPaths`
  shape; slice 3's primary edit target
- `crates/core/src/infrastructure/profile_store.rs` — current JSON
  reader; slice 3 teaches it to read frontmatter + body
- `crates/cli/src/commands/init.rs` + `src-tauri/src/commands/secretariat.rs::init_identity`
  — both need updates to write the new layout
- `scripts/migrate-vault-v0.5.0.sh` — append slice 3 migration steps
  OR rename to `migrate-vault-v0.6.x.sh`
- Lexicons: new `lexicons/tech.equanimi.secretariat.identity.json`
