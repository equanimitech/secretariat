# slice 4 — principal-context to markdown

Pitch — 2026-05-18. Filed alongside slice 3 (identity consolidation).
Same direction of travel: every principal-authored, agent-read context
file lives as markdown + frontmatter under the queue-root it belongs
to. After slice 3 + slice 4, the entire principal-context surface is
markdown — one mental model, one editing affordance, one parser
family.

Two JSON files remain after slice 3:

```
~/.secretariat/contacts.json         ← who I correspond with
~/.secretariat/orgs/<alias>/.org     ← per-org metadata (alias, did, name, description)
```

Convert both to markdown + frontmatter. Same pattern slice 2 used for
`channel.md` and `contract.local.md`, same pattern slice 3 uses for
`identity.md`.

Scope-bounded: NOT every JSON/TOML file in the vault. Only the ones
the **principal authors and the agent reads as context**.
`preferences.toml` stays TOML (app-level machine config). `relay-state.json`
stays JSON (daemon-managed runtime state). See Elements §5.

## Boundaries

### Job to be done

As a principal opening `~/.secretariat/`, I want **one mental model
for principal-context files** — frontmatter for the typed fields,
markdown body for free-form prose the agent reads. Same as `channel.md`,
`contract.local.md`, `identity.md`. No JSON files I'm expected to
hand-edit.

*When*: every time I want to add a note to a contact ("Marcelo
prefers vouvoiement", "Christophe corresponds in French only") and
have to either invent a side-channel for it or stuff it into a free-text
JSON field, the JSON shape is in the way. Same for org metadata —
"why we set this org up", "who's the operational lead" — those are
prose the agent should respect, not enums.

### Appetite

`small`. ~one focused day. Mostly mechanical: copy slice-3's pattern
twice. Tests follow channel.md / contract.local.md / identity.md
templates already in place.

Touches:
- `contact_store.rs` — JSON reader/writer → markdown reader/writer
- `org_store.rs` — JSON reader/writer → markdown reader/writer
- `KeyPaths.contacts` repoint to `_self/contacts.md`
- Existing migration script gains two more move steps (slice-3 +
  slice-4 ship together as a v0.7.0 cutover OR slice-4 ships
  separately as v0.7.1)
- Lexicons: `tech.equanimi.secretariat.orgDoc` may need a tightening
  pass (the lexicon already exists for the signed-envelope variant
  of org metadata; the local `org.md` shape should align)
- Two new tests for the body-preserving roundtrip (read + edit +
  write must not lose prose)

### What's in scope

**New layout (after slice 3 + slice 4):**

```
~/.secretariat/
├── _self/
│   ├── identity.md                ← (slice 3) frontmatter: did, profile, key metadata
│   ├── identity/key, did.json     ← (slice 3) raw + DID document
│   ├── contracts.md? NO.          ← contract.local.md lives per-channel, not at _self root
│   ├── contacts.md                ← (slice 4) one ## per contact, frontmatter under each
│   ├── contract-stub.md           ← (v0.6.0) editable template for new channels
│   └── channels/...
├── orgs/<alias>/
│   ├── org.md                     ← (slice 4) frontmatter: alias, did, name, description
│   ├── contract.local.md          ← (v0.6.0) consumption contract at org root
│   └── channels/...
├── preferences.toml               ← stays — app config
├── relay-state.json               ← stays — daemon state
├── template.md                    ← stays at root until queue_dir alignment slice
```

**Conversions:**

- `contacts.json` → `_self/contacts.md` (Elements §1)
- `orgs/<alias>/.org` → `orgs/<alias>/org.md` (Elements §2)
- Migration extends the existing hand-script (Elements §3)
- Lexicons reviewed for alignment (Elements §4)

### What's out of scope

- `preferences.toml` move or format change. App-level machine config;
  Tauri settings pane owns it. Stays TOML, stays at root. See Elements §5.
- `relay-state.json` move or format change. Daemon-managed runtime
  state, regenerable, never principal-edited. Stays JSON, stays at root.
- `cognition.json` conversion. The file is already retired — cognition
  config moved into `preferences.toml`'s `[cognition]` block. `KeyPaths.legacy_cognition_config`
  exists only for one-time migration reads from pre-v0.5 installs.
- Multi-contact-per-DID, contact merging, contact-rename UX. Schema
  changes; out of scope for slice 4 which is format-only.
- Per-contact private channels for DM (covered by the queue_dir
  alignment slice).
- Encrypted contact book. The current contacts file lives at `0600`;
  encryption-at-rest is a separate wedge.

## Elements

### 1. `_self/contacts.md` shape

One markdown section per contact. Frontmatter under each header
carries the typed fields; the body is free-form prose about the
contact.

```markdown
# Contacts

## Marcelo

---
did: did:key:z6Mk4...
display_name: Marcelo
full_name: Marcelo Ballestiero
relay_endpoint: wss://relay.rafa.equanimi.tech
added_at: 2026-04-30T12:00:00Z
---

Co-author on the Autonomous Enterprise book. Prefers concrete examples
to abstractions. Reads in Portuguese when I write in Portuguese;
otherwise English.

## Christophe

---
did: did:web:christophe-marchand.com
display_name: Christophe
full_name: Christophe Marchand
added_at: 2026-05-02T09:00:00Z
---

Avocat, dommage corporel. Vouvoiement always. French only — never
switch to English even if the topic is technical. Lives on TestFlight
Mac side; Windows when Themia ships there.
```

**Parsing rule.** Sections delimited by `^## `. Each section's
frontmatter is the first `---\n...\n---\n` block after the header;
everything after the closing `---` (until the next `## ` or EOF) is
the body prose. Per-contact YAML is required for the typed fields;
the body is optional.

**Why one file with sections, not one file per contact?** Contacts
are a single principal-curated list, not a queue. The principal
reads/edits them as a unit ("who do I correspond with?"). One file
is the right granularity — like the address book in a single
markdown table, but with optional prose under each entry.

**Why frontmatter under each `##` instead of one top frontmatter
with an array?** Putting the body under each entry as free prose is
the whole point. A top-level array can't carry per-entry markdown
prose. The per-section frontmatter pattern is the same trick `obsidian`
templates use; the YAML parser sees three small docs, not one big one.

**Backward-compat reads** for the legacy `contacts.json`: the loader
checks for `_self/contacts.md` first, falls back to root
`contacts.json` if absent (transparent during the slice-3+4
migration window). Tests cover both paths.

### 2. `orgs/<alias>/org.md` shape

```markdown
---
$type: tech.equanimi.secretariat.org
alias: themia.pro
did: did:web:themia.pro
name: Themia
description: Legal-tech jurimetry platform
created_at: 2026-05-12T03:00:00Z
---

# Themia

Org-level prose: why this org exists in my vault, who the operational
contact is, what kind of traffic I expect, signature line for org
correspondence. The agent reads this when surfacing org-related
context (review sessions, compose).

Free-form. Principal-editable. Survives every future migration.
```

Same pattern as `channel.md`. Frontmatter carries the machine-enforced
fields; body is principal prose.

**Why this isn't called `org.local.md`?** Because the metadata is
the org's identity, not a per-subscriber preference. `contract.local.md`
gets the `.local` suffix to signal "private to this subscriber, never
shared". `org.md` is closer to `channel.md` — the org-side identity
record. (Note: the local-vs-published distinction will harden when
relay sync ships and signed `orgDoc` envelopes carry the wire-shaped
metadata. The local `org.md` stays the principal's editable view; a
future `org.signed.md` could carry the relay-fetched authoritative
version. Out of scope for slice 4.)

### 3. Migration

Append to `scripts/migrate-vault-v0.5.0.sh` (or rename to a v0.7.0
script if slice 3 + 4 ship together):

```bash
# ---- slice 4 — contacts ----
if [[ -f "$VAULT/contacts.json" && ! -f "$SELF_ROOT/contacts.md" ]]; then
  echo "[migrate] converting contacts.json → _self/contacts.md"

  # Build contacts.md from JSON. Each contact becomes a ## section with
  # frontmatter. Unknown JSON keys preserved verbatim under each
  # frontmatter (no round-trip through typed struct).
  python3 -c "
import json, sys
data = json.load(open('$VAULT/contacts.json'))
out = ['# Contacts\n']
for c in data.get('contacts', []):
    name = c.get('display_name', c.get('did', 'unknown'))
    out.append(f'\n## {name}\n')
    out.append('\n---')
    for k, v in c.items():
        out.append(f'\n{k}: {v}')
    out.append('\n---\n')
print(''.join(out))
" > "$SELF_ROOT/contacts.md"

  # Don't delete contacts.json yet — keep until backward-compat read
  # path retires (separate slice). Move to .archive instead.
  mkdir -p "$VAULT/.archive"
  mv "$VAULT/contacts.json" "$VAULT/.archive/contacts.json"
fi

# ---- slice 4 — orgs ----
for org_dir in "$VAULT/orgs"/*/; do
  [[ -d "$org_dir" ]] || continue
  if [[ -f "$org_dir/.org" && ! -f "$org_dir/org.md" ]]; then
    echo "[migrate] converting $org_dir.org → org.md"
    python3 -c "
import json, sys
data = json.load(open('$org_dir/.org'))
alias = data.get('alias', '')
name = data.get('name', alias)
print('---')
print('\$type: tech.equanimi.secretariat.org')
for k, v in data.items():
    if v is not None and v != '':
        print(f'{k}: {v}')
print('---')
print(f'\n# {name}\n')
" > "$org_dir/org.md"
    mkdir -p "$VAULT/.archive/orgs/$(basename $org_dir)"
    mv "$org_dir/.org" "$VAULT/.archive/orgs/$(basename $org_dir)/.org"
  fi
done
```

**Discipline preserved:** legacy JSON files `mv`'d to `.archive/`,
never `rm`'d. Same envelopes-never-destroyed rule extends to
principal-curated data: keys and contacts both fall in the "sovereign
sediment" class.

Pre-flight `tar` snapshot already covers the rollback (existing
script behavior).

Post-move test: load `contacts.md` and confirm contact count matches
pre-migration JSON; load each `org.md` and confirm alias matches the
directory name.

### 4. Lexicons

Two records to align:

- **`tech.equanimi.secretariat.contact`** (new). Per-contact record
  shape. Mirrors the per-section frontmatter in `contacts.md`. Required
  fields: `did`, `display_name`. Optional: `full_name`, `relay_endpoint`,
  `added_at`, free-form extras.
- **`tech.equanimi.secretariat.org`** (new). Org metadata. Mirrors
  `org.md` frontmatter. Distinct from the future signed `orgDoc`
  envelope (relay-published; out of scope).

Both ship in the same commit as the Rust code per AGENTS.md rule #3.

### 5. Why `preferences.toml` and `relay-state.json` stay

Two rules clarify the line:

**Markdown for principal context.** Files the principal authors and
the agent reads as context: identity, contacts, contracts, channels,
orgs. These get YAML frontmatter for typed fields + markdown body
for prose. One model, one parser family.

**Non-markdown for machine state.** Files only the machine writes
and reads: app configuration (window state, terminal picker, dev
flags), daemon runtime state (relay cursors, queued envelope offsets).
These stay in their native format (TOML for human-debuggable config,
JSON for machine-emitted state).

The test: would the principal want to hand-edit this in vim with
markdown prose around it? Yes → markdown. No → leave it.

Applies cleanly:
- `preferences.toml` — Tauri settings pane writes it; principal
  occasionally hand-edits; structured config; **stays TOML**.
- `relay-state.json` — daemon emits relay cursor state; principal
  never edits; would only ever be `rm`'d in disaster recovery;
  **stays JSON**.
- `contacts.md` — principal curates the list, wants prose under
  each contact; **markdown**.
- `org.md` — principal authored the org, wants prose about it;
  **markdown**.

## Risks

### 🐇 Rabbit holes

- **Per-section frontmatter parser.** Three small YAML docs per
  file, delimited by `## ` headers, is a parser I haven't written
  yet. Mitigation: ~30 lines of split-and-trim code; tested against
  same harness as `contract_store::split_frontmatter`. Don't
  over-engineer with a markdown AST parser.

- **Display-name conflicts after edit.** Today `ContactBook` enforces
  unique `display_name.slug()`. If the principal hand-edits
  `contacts.md` and creates a duplicate slug, the loader must error
  clearly (which contact wins?). Mitigation: loader emits a
  `DuplicateSlug` error pointing at line numbers; principal fixes.
  Same invariant the current ContactBook enforces — surface earlier.

- **`orgs/<alias>/.org` is currently used by tests.** Many tests
  write `.org` directly. They need to either keep working via the
  backward-compat read path OR get bulk-rewritten. Mitigation:
  loader supports both (`.org` JSON for back-compat, `org.md` for
  new). Tests opt into either depending on what they're exercising.
  No forced rewrite.

- **The lexicons are still mutable (no codegen yet).** Per AGENTS.md
  out-of-scope list. Adding `contact` + `org` records to lexicons is
  in-scope work for this slice; no external consumers can be broken
  yet.

- **Contact book encryption.** `contacts.json` already at `0600`;
  the new `contacts.md` inherits that mode (`save_contract`-style
  atomic write with mode). Don't accidentally regress to `0644`.

### 🏴 Off-sides called

- Encrypted-at-rest contact book. Separate wedge.
- Multi-account-per-DID (contact aliases, contact merging). Schema
  change; future slice.
- Signed-envelope orgDoc relay-published variant. Lands with federation,
  out of scope.

### 🥩 Fat cut

- Could ship `contacts.md` only, defer `org.md` to a later slice.
  Buys ~3 hours. Cost: asymmetry stays — orgs still carry `.org`
  JSON while `_self/` is fully markdown. Recommend against the cut;
  slice is small enough.

- Could ship `org.md` only, defer `contacts.md`. Same asymmetry
  argument. Same recommendation: don't cut.

### 🧪 Domain knowledge

- Confirm Marcelo + Christophe have empty `contacts.json` (no
  contacts curated yet). If yes, their migration is a no-op for
  contacts and the slice is single-principal-scope risk for me
  alone.
- Confirm no tooling (CI scripts, deployment) reads `contacts.json`
  by path. Grep + manual sweep.

## Pitch

### Problem

After slice 2 (namespace collapse) + slice 3 (identity consolidation),
the only JSON files left in `~/.secretariat/` that the principal
might want to hand-edit are `contacts.json` and the per-org `.org`
files. Every other principal-context file is markdown + frontmatter:
`channel.md`, `contract.local.md`, `identity.md`, `contract-stub.md`,
`template.md`. The remaining JSON is dissonance — the principal can't
add prose, the editor can't render it as text, the agent can't read
notes the principal scribbles next to a contact.

This is residue from before the frontmatter-first pattern landed. v0.5
proved the pattern on `channel.md`; v0.6.0 extended it to
`contract.local.md`; slice 3 takes it to `identity.md`. Finishing the
sweep is a one-day slice.

### The bet

Convert `contacts.json` → `_self/contacts.md` (one `##` section per
contact, frontmatter + body) and `orgs/<alias>/.org` →
`orgs/<alias>/org.md` (frontmatter + body, same shape as `channel.md`).
Keep `preferences.toml` and `relay-state.json` as-is — they're
machine config, not principal context.

Ship slice 3 + slice 4 together as v0.7.0 "layout-complete" (every
principal-context file is markdown; the entire vault layout is
self-symmetric). Migration script extends the v0.6.0 hand-script with
two more conversion steps. Legacy JSON files `mv`'d to `.archive/`,
never deleted, per the sovereign-sediment rule.

The bet pays off when:
- "Can I add a note to a contact?" → yes, write prose under their `##`.
- "Why is this org here?" → check `orgs/<alias>/org.md` body.
- The principal-context surface has one parser family across the
  whole vault.
- Slice 4 is the last format-cleanup slice; whatever comes after
  (queue_dir alignment, peer/contact collapse, federation) lands
  against a stable markdown foundation.

### No-gos

- **No format proliferation.** Markdown + frontmatter for principal
  context. JSON for daemon runtime state. TOML for app config.
  Binary for keys. Four formats, each with a clear reason. Slice 4
  doesn't add a fifth.
- **No data destruction.** Legacy JSON files `mv`'d to `.archive/`,
  never `rm`'d. Pre-flight tar snapshot is the rollback. Sovereign
  sediment rule applies to keys, envelopes, and curated lists
  (contacts) alike.
- **No backward-compat shim that lives forever.** Loader supports
  both old + new for the migration window only. Once Rafa, Marcelo,
  Christophe all run v0.7.0, drop the JSON read path in v0.8.0.

## Reference

- v0.6.0 release: 2213f6d (merge), d1237a3 (release commit)
- Slice 2 pitch: `docs/pitches/2026-05-17-collapse-namespaces.md`
- Slice 3 pitch: `docs/pitches/2026-05-18-slice-3-identity-consolidation.md`
- `crates/core/src/infrastructure/contact_store.rs` — current JSON
  reader; slice 4's edit target
- `crates/core/src/infrastructure/org_store.rs` — current JSON
  reader; slice 4's other edit target
- `crates/core/src/infrastructure/contract_store.rs` — shape to mirror
  for markdown + frontmatter parsing
- Lexicons: new `lexicons/tech.equanimi.secretariat.contact.json` +
  `lexicons/tech.equanimi.secretariat.org.json`
- Memory: `feedback_envelopes_never_destroyed` (the rule extends to
  contact lists — sovereign sediment)
