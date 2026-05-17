# channel emojis — visual rhyme across launcher, picker, tray

Pitch — 2026-05-17. Source:
`~/.secretariat/queues/inbox/triage/20260517T183653Z-7qnlbs.md`
("channels can have emojis").

Channels today are strings: `channel:leggia`, `channel:journals`,
`channel:dommage-corporel`. The quick-pane Launch rows show them
verbatim. The OrgPicker buttons show the org name. The tray popover
(when it ships) will show vault names + verbs. Adding an emoji per
channel gives the principal a peripheral-visual anchor — they recognize
🪶 before they read `channel:journals`.

## Boundaries

### Job to be done

As a principal switching between channels many times a day via the
quick-pane and the OrgPicker, I want a single-character visual
identifier on each channel so I can scan a list of 10 channels in 200ms
without reading the handle string, and so the channel's identity
"travels" across every surface that lists it.

*When*: every time the quick-pane dropdown renders, every time the
OrgPicker stack shows, every future surface that lists channels.
Baseline today: handles are strings; visual scanning is
character-by-character; channel:dev:secretariat and
channel:dev:zenborg need read attention to disambiguate.

### Appetite

`small`

Small VO addition, one new optional field on `.channelDef`, light
plumbing through the existing Tauri commands and the React surfaces.
No new domain primitives.

## Elements

Three primary elements.

### 1. `emoji` field on `.channelDef`

Optional `String` with the constraint "exactly one grapheme cluster
that contains an extended-pictographic codepoint." (Validated at
`ChannelDef::new`.) Empty `None` is fine — channels created before
this slice ship with no emoji and render with a fallback bullet.

```rust
pub struct ChannelDef {
    pub handle: QueueHandle,
    pub name: String,
    pub description: String,
    pub emoji: Option<String>,      // NEW
    pub created_at: DateTime<Utc>,
}
```

Stored in `.channelDef` frontmatter:

```yaml
$type: tech.equanimi.secretariat.channelDef
handle: channel:journals
name: Journals
description: Private journaling — LM Studio routing
emoji: 🪶
created_at: 2026-05-17T17:35:00Z
```

### 2. `--emoji` flag on `sec channels create`

```bash
sec channels create channel:leggia --org themia.pro --name "Leggia" --emoji 🧪
```

Interactive prompt when flag absent — `sec channels create` already
asks for `--name` interactively; we ask for an emoji on the next
line. The prompt suggests one from the handle's slug using a tiny
heuristic table:

| Slug contains | Suggested |
|---------------|-----------|
| `journal` / `diary` | 🪶 |
| `inbox` / `triage` | 📥 |
| `dev` / `code` | 💻 |
| `client` / `customer` | 👥 |
| `dommage` / `legal` / `law` | ⚖️ |
| `book` / `writing` / `article` | 📖 |
| (no match) | 🟢 |

Same shape as zenborg's `suggestEmojiForAreaName`. Principal can
accept (Enter) or override (type their own). MCP `create_channel`
tool gains the same optional `emoji` parameter.

### 3. Render across surfaces

- **Quick-pane Launch rows** (v0.4.8): prefix the `channel:…` handle
  with the emoji. Fallback bullet `•` when none. The "override" badge
  stays where it is.
- **OrgPicker** (v0.4.6): orgs aren't channels, so the org card stays
  emoji-less in v1. Per-vault emoji is a follow-up (likely lives on
  `Org`, not `ChannelDef`).
- **Tray popover** (when it ships): channels listed in a "recent" or
  "favorites" submenu will use the same emoji.
- **`sec channels list`**: prefix lines with the emoji.

`LaunchableChannel` Tauri DTO gains `emoji: Option<String>`. CLI list
output gains it. MCP `list_channels` tool gains it.

## Risks

### 🐇 Rabbit holes

- **Emoji validation.** Single-grapheme-cluster + extended-pictographic
  is what we want; multi-codepoint emoji (👨‍👩‍👧‍👦) is one grapheme
  cluster but multiple codepoints. Use the `unicode-segmentation`
  crate (likely already in the workspace via a transitive dep) to
  count clusters; use `unicode-properties` for extended-pictographic.
  Add a one-line accept/reject test.
- **Width across surfaces.** Emoji render width varies (1ch vs ~1.5ch
  vs ~2ch depending on platform fonts). Constrain CSS `width:
  1.5em; display: inline-flex; justify-content: center` on the prefix
  cell so the column stays aligned.
- **Skin-tone and gendered variants.** Allow them — `unicode-properties`
  accepts. Don't try to filter, the principal chose.

### 🏴 Off-sides called

- Emoji as the primary identifier. Handle stays canonical;
  emoji is an *attribute*, not a primary key. Search inputs match
  on handle/slug/name, not emoji.
- Emoji on orgs. Org cards stay text in v1. Different VO, different
  pitch.
- Emoji on individual envelopes. Channels are where this lives.
- Animated emoji / Lottie. No.
- Custom-uploaded SVGs. No — the principal can pick from the existing
  Unicode emoji set, period.

### 🥩 Fat cut

- The suggestion heuristic. v1 can ship with a "you must pass `--emoji`
  explicitly" prompt; auto-suggestion can land later. But it's ~15
  lines of code so probably worth shipping in slice 1.
- Validation. v1 can accept any short string and let the principal
  blame themselves for putting `XX` there. Validation lands cleanly
  later. Trade-off: cheap to add now.
- MCP `set_channel_emoji` verb. Today the principal edits
  `.channelDef` by hand to change it. A dedicated verb can ship in
  a follow-up if hand-editing feels too rough.

### 🧪 Domain knowledge

- Confirm `unicode-segmentation` is in the workspace (or another
  grapheme-aware crate). If not, adding it is a one-line
  Cargo.toml change.
- Confirm cmdk's filter doesn't choke on emoji prefixes — it shouldn't
  (cmdk matches on the `value` prop, which we keep as the
  handle/slug/name concatenation, not the emoji).

## Pitch

### Problem

The principal's quick-pane Launch dropdown will list 5-15 channels
once the substrate fills out. Handles are strings; visual scanning
across `channel:leggia` / `channel:journals` /
`channel:dommage-corporel` / `channel:assemblee_generale` is
character-by-character. The principal's brain is faster at glyphs
than at parsing kebab-case.

Every other tool that solves this — Slack, Linear, Things, zenborg's
areas — has an emoji per top-level entity. Secretariat doesn't,
because channels were defined as URI-shaped first. Adding an
optional emoji field is a tiny VO extension that immediately
improves every list rendering.

### The bet

One optional field on `ChannelDef`. Suggestion table on `sec channels
create`. Render in the three surfaces that list channels. Total: ~150
LoC across Rust + React. Ship as v0.4.9.

The bet pays off the first time the principal opens the quick-pane,
types `j`, and sees 🪶 `channel:journals` resolve faster than they
can read the next character.

### No-gos

- No new domain primitive. `emoji` rides on `ChannelDef`.
- No emoji-driven routing. The handle is still the address.
- No multi-emoji stacking.
- No required field. Channels created before this slice ship without
  emoji and render with a fallback bullet.
- No emoji on orgs in this pitch — separate concern.

## Reference

- v0.4.8 ship note (quick-pane launcher — first surface to gain
  emoji prefixes)
- zenborg's `suggestEmojiForAreaName` — pattern reference
- `~/.secretariat/queues/inbox/triage/20260517T183653Z-7qnlbs.md` —
  the originating capture
