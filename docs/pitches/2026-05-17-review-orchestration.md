# review orchestration — agent-driven, contract-aware, tree-shaped

Pitch — 2026-05-17. Revised against principal feedback:

> "Cadence_floor_minutes is misconfigured if I'm being honest. What
> matters: use Claude to fetch all channels with envelopes to review
> (unread OR since N days), fetch the contract.locals per channel,
> leverage the contracts to know what to present when. Start with
> overviews, allow iterative diving into channels."

The orchestration moves _to the agent_. The application layer's job
is indexing — return a tree of channels with their contracts attached
verbatim. The agent reads the contracts, decides presentation order,
narrates the overview, and dives on demand. No Rust-side ranking, no
filter VO, no `cadence_floor_minutes` moonlighting as a sort key.

## Boundaries

### Job to be done

As a principal reviewing a substrate of envelopes across nested
channels in multiple vaults, I want **Claude to orchestrate the
review session** — pull what's unread or since-N-days, read my
contracts to learn how I want each channel handled, give me a
tree-shaped overview, and let me dive into any channel iteratively
until I'm done — so the contracts I've already written stop being
inert documentation and start shaping the surface that uses them.

_When_: at the principal's review cadence (twice daily / weekly /
on-demand from the tray). Today: `review_org` opens Claude Code at
the vault root but with no agent prompt, no cursor, no overview
primitive. Claude either greps the whole substrate or asks the
principal to specify everything manually.

### Appetite

`medium`

Smaller than the earlier `big`-appetite draft once the orchestration
moves to the agent. Three MCP tools, one cursor file, one agent
template. No `ReviewFilter` VO, no tag domain model, no
contract-weight ranking code. Slices:

1. **Review cursor** (vault floor + per-channel overrides)
2. **`review_overview`** — tree response with contracts attached
3. **`review_channel`** **+** **`advance_review_cursor`**
4. **Agent scaffold** auto-installed on `sec orgs create` / `sec init`

Each ships independently. The order is the order of value: 1+2 alone
gives the agent enough to walk the principal through; 3 adds the
dive; 4 makes the agent automatic.

## Elements

Four primary elements. The cursor and the tools live in `core`; the
agent template lives in `crates/cli/templates/`.

### 1. Review cursor — vault floor + per-channel overrides

On-disk at `~/.secretariat/.review-cursor.json`:

```json
{
  "version": 1,
  "vaults": {
    "themia.pro": {
      "floor": "2026-05-17T18:00:00Z",
      "channels": {
        "channel:leggia": "2026-05-17T19:00:00Z",
        "channel:journals": "2026-05-16T08:00:00Z"
      }
    },
    "_self": {
      "floor": null,
      "channels": {}
    }
  }
}
```

Effective cursor for a `(vault, channel)` pair:

```rust
fn effective(vault_floor: Option<DateTime<Utc>>,
             channel_override: Option<DateTime<Utc>>)
             -> Option<DateTime<Utc>> {
    match (vault_floor, channel_override) {
        (None, c) => c,
        (Some(f), None) => Some(f),
        (Some(f), Some(c)) => Some(max(f, c)),
    }
}
```

So advancing the vault floor implicitly advances every channel that
was sitting below it. Advancing a channel only bumps that channel.

**Why both:** the principal can do an "I'm done with everything in
themia.pro for now" vault-level sweep (matches the tray's primary
verb) AND a per-channel iterative dive that advances only what was
actually reviewed. Single-level cursors force a binary I-saw-it-all
or I-saw-nothing — both lie under the iterative-dive UX.

### 2. `review_overview(since, vault?) -> ReviewTree`

```rust
pub enum Since {
    Unread,                    // use cursor
    DaysAgo(u32),
    AbsoluteUtc(DateTime<Utc>),
}

pub enum VaultSelector {
    All,
    Private,
    Org(OrgAlias),
}

pub struct ReviewTree {
    pub generated_at: DateTime<Utc>,
    pub since_used: Since,
    pub vaults: Vec<VaultNode>,
}

pub struct VaultNode {
    pub alias: String,                 // "_self" / "themia.pro"
    pub display_name: String,
    pub effective_cursor: Option<DateTime<Utc>>,
    pub channels: Vec<ChannelNode>,    // top-level handles
    pub total_count: usize,             // sum across the subtree
}

pub struct ChannelNode {
    pub handle: QueueHandle,            // channel:foo / channel:foo:bar
    pub count: usize,                   // entries in *this* node, not children
    pub total_count: usize,             // count + sum(children.total_count)
    pub newest_received_at: Option<DateTime<Utc>>,
    pub contract: Option<ContractAttachment>,
    pub children: Vec<ChannelNode>,
}

pub struct ContractAttachment {
    pub path: PathBuf,
    pub frontmatter_raw: String,        // YAML verbatim
    pub body: String,                   // prose verbatim
}
```

**No ranking applied.** Ordering: vaults in alias-sort order
(`_self` first), channels within a vault in `newest_received_at`
descending order (channels with no entries last). The agent reorders
according to contracts.

**Contracts attached verbatim.** The agent reads the YAML
frontmatter AND the prose body — both are signals. A channel whose
contract body says "I check this once a week, on Fridays" should be
treated differently from one whose body says "drop everything for
this." Machine-parsed fields are a bonus, not the whole story.

### 3. `review_channel(handle, since, vault?) -> ChannelView`

```rust
pub struct ChannelView {
    pub handle: QueueHandle,
    pub generated_at: DateTime<Utc>,
    pub effective_cursor: Option<DateTime<Utc>>,
    pub entries: Vec<ReviewEntry>,
}

pub struct ReviewEntry {
    pub file_path: PathBuf,
    pub from: Option<Did>,
    pub received_at: DateTime<Utc>,
    pub depth: Option<EnvelopeDepth>,
    pub urgency: Option<EnvelopeUrgency>,
    pub stamped: bool,
    pub encrypted: bool,
    pub body_preview: Option<String>,   // first ~280 chars, plaintext-only
}
```

Plus `advance_review_cursor(vault, channel?: QueueHandle)` — bumps
the vault floor when channel is `None`, otherwise bumps that
channel's per-channel override.

### 4. Agent scaffold

`<vault>/.claude/agents/review.md` autowritten by `sec orgs create`
and `sec init`. Initial prompt sketch:

```markdown
---
name: review
description: Walks the principal through the substrate's review queue.
tools:
  [
    mcp__secretariat__review_overview,
    mcp__secretariat__review_channel,
    mcp__secretariat__advance_review_cursor,
  ]
---

You are the review agent for this vault.

Open every session with `review_overview(since="unread")` (or accept
the principal's `since` override, e.g. "since 3 days").

Read every channel's `contract.frontmatter_raw` AND `contract.body`.
The body is where the principal told you how they want that channel
handled — let it govern your presentation order and depth, NOT the
raw counts. Channels with no contract get default treatment (one-
liner, no expansion).

Render the overview as a tree. For each channel: name + count + a
one-line gist drawn from the contract body. Group by vault.

Ask: "Dive into <channel>?" — wait for explicit yes.

On dive: `review_channel(handle, since)`, render entries ordered by
`received_at` desc with depth/urgency chips, decrypt-aware previews.
After the principal signals they're done with the channel
("reviewed", "next", "advance"), call
`advance_review_cursor(vault, channel)`.

On vault-level done ("I'm done with this vault"), call
`advance_review_cursor(vault)` without a channel — this advances
the floor and supersedes every per-channel override below it.

Anti-patterns to avoid:

- Don't summarize before the principal asks to dive.
- Don't auto-advance the cursor without confirmation.
- Don't read encrypted bodies — render "[encrypted]" and skip.
- Don't propose stamping — that's a separate ceremony.
```

## Risks

### 🐇 Rabbit holes

- **Contract bodies as free-form prose the agent has to interpret.**
  If the principal writes vague contracts, presentation degrades.
  Fine — that's the principal's surface to tune. The agent should
  _quote_ the relevant contract line back when justifying its
  ordering, so the principal sees the cause.

- **Tree depth.** Channels can nest arbitrarily. v1 walks the full
  tree; if a vault has 50+ leaf nodes, the overview blows past
  Claude's attention budget. Cap at depth 3 in v1; flatten deeper
  trees with `<parent>:…:<leaf>` rendering and add a "show full
  tree" follow-up tool later.

- **`received_at`** **recovery.** v1 reads frontmatter when present,
  falls back to the `YYYY/MM/DD` path date. If neither is reliable,
  the cursor's effectiveness drops. Spike before slice 2 ships.

- **Decrypted body previews leak ciphertext.** `body_preview = None`
  when `encrypted = true`. Agent renders "\[encrypted — read to
  decrypt]". Same rule as existing UI surfaces.

- **Cursor races with the daemon.** Background poll writes envelopes
  while the review session is open. The session is computed once
  at start; new arrivals show up next session. Freeze `generated_at`
  in the response so the agent can disclose the snapshot.

### 🏴 Off-sides called

- Tag domain model. Captures already carry free-form bodies; the
  agent reads them. No `tags: [..]` field added to envelope
  frontmatter in this slice. Revisit only if the agent can't
  triage without machine-readable tags.

- Filter VO. `since` is the only enum needed; `vault` is a thin
  selector. Everything else is the agent's call.

- Cross-vault rollups. The tree is rooted per-vault. "Review
  everything" calls `review_overview(since, vault=All)` and the
  agent fans across vaults itself; nothing in Rust merges them.

- Contract-weight ranking. Removed. `cadence_floor_minutes` keeps
  its existing job (delivery cadence) and doesn't moonlight.

### 🥩 Fat cut

- Auto-scaffolding the agent on `sec init`. Could ship slice 4 with
  a `sec orgs scaffold-agents` verb the principal runs manually
  first. Saves migration code for existing orgs.

- `body_preview`. v1 returns `None` always; the agent reads files
  by path when it needs the body. Less data in the overview, more
  tool calls during dive. Flip later if dives feel chatty.

- `DaysAgo(u32)` parse-side. Agent can compute the absolute UTC
  itself and pass `AbsoluteUtc`. Keeps the enum tighter.

### 🧪 Domain knowledge

- Confirm whether agent files at `<vault>/.claude/agents/review.md`
  are picked up by Claude Code when `cwd = <vault>` and `--agent
review` is passed. Per the upstream Claude Code docs the `agents/`
  tree walks up from cwd; the vault root _is_ the walk root in our
  setup, so this should work — verify before slice 4.

- Confirm `mcp__secretariat__*` is the namespace the agent's `tools: []` frontmatter wants. Match the existing MCP tool naming
  convention.

## Pitch

### Problem

The review surface ships a button. It doesn't ship a session. The
substrate already knows enough — every channel has a contract, every
envelope has a received-at — but nothing reads contracts to _shape_
the review. `cadence_floor_minutes` is the wrong proxy for "what
matters when"; the principal's contract bodies are the real signal,
and they're prose the agent should read.

Today the agent has no signal. Yesterday's plan tried to encode
ranking in Rust. That was wrong: the contracts ARE the ranking, the
agent IS the orchestrator, the application's job is to hand the
agent a tree and step back.

### The bet

Ship three MCP tools (`review_overview`, `review_channel`,
`advance_review_cursor`) plus a cursor file plus an agent template.
No `ReviewFilter` VO, no contract-weight machinery, no tag domain
model. The agent reads contract YAML AND prose verbatim and decides.

The bet pays off when the principal opens the tray, clicks Review,
and Claude says: _"You have 4 channels with new traffic._ _`channel:
leggia`_ _is the priority your contract calls out for client work; 6
envelopes._ _`channel:journals`_ _you marked weekly review, only 2 since
last Friday — bundle for later?_ _`channel:assemblee_generale`_ _you
marked stamp-required; 1 envelope waiting on your signature. The
inbox has 3 randoms."_ — then dives where the principal points.
Contracts go from inert docs to load-bearing surfaces.

### No-gos

- No machine-side weighting of contract fields. Contracts are
  presented; the agent decides. The principal can rewrite the
  contract body to change ordering without touching code.

- No central index of reviewable envelopes. The tools walk the
  substrate live (\[\[project_filesystem_authoritative]]).

- No background pre-rendering. Review is a session, not a feed.

- No stamp-during-review. Stamping stays its own ceremony with its
  own body-display contract per AGENTS.md rule #4.

## Reference

- v0.4.6 ship note (`review_org` button + OrgPicker UI)

- v0.4.5 ship note (per-channel cognition overrides — same
  `contract.local.md` substrate the review tools read)

- `docs/pitches/2026-05-13-launch-dispatch-root-path.md`

- AGENTS.md rule #4 (three-layer trust model)

- AGENTS.md rule #6 (every principal-facing primitive ships on the
  four surfaces — applies to all three tools here)
