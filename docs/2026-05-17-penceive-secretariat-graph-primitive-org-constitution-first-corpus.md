---
migrated_from: equanimi.tech/project/secretariat/dev/20260517T195837Z-u2gsv3.md
---
# Penceive ↔ Secretariat: graph primitive = org constitution first, corpus later

## The synthesis

Penceive and Secretariat overlap on substrate (Tauri shell, FS-authoritative markdown, capture surface, pluggable cognition). They diverge on what's missing from each: Penceive lacks trust layer (DIDs, signing, stamps); Secretariat lacks deep writing/search/graph layer.

Penceive's tagging/extraction machinery isn't a Penceive feature — it's the **first passive agent** on Secretariat's substrate. The knowledge graph it builds is the **same primitive** as everything else in Secretariat: signed envelopes, selective stamping, optional counter-stamp.

## The reframe (the actual insight)

The graph primitive's *first* application is not knowledge extraction. It's **the company's own constitution** — who is principal, what agents exist, what channels they're authorized for, what cognition class they belong to, what stamps are required for what acts.

Constitution emerges, not decreed. Agents notice patterns ("tagger.local has written 47 times to dommage-corporel — formalize?"), propose `org.capability` envelopes, principal stamps, co-principal counter-stamps. Constitution becomes a queryable, audit-trailed, trust-graded object.

Knowledge extraction over a corpus (Themia legal data) is the **same machinery, different namespace** — `did:graph:themia.pro:case:*` instead of `did:graph:themia.pro:channel:*`. Build constitution first because:
- Small scope (2 real users) — design mistakes hurt only us
- Validates lexicon shape + daemon ACL + stamp + counter-stamp + cognition policy end-to-end
- Corpus extraction reuses every line of code

## Component decomposition

**Cognition policy per channel/queue.** Channel contract declares `cognition.allowed: [local | claude | bedrock | …]`. Daemon enforces ACL on agent DID by declared provider class. Local-only channels (Themia legal) refuse Anthropic-keyed agents. Principal owns the brain *per context*, not just globally.

**Passive AI agents.** MCP tools today are human-driven. Promote to daemon-side agents with their own DID + keypair. Subscribe to channels they're authorized for, draft signed envelopes (tags, links, summaries, capability proposals), never stamp. Principal stamps to graduate to canonical.

**Graph as envelopes.** Three lexicons under `lexicons/graph/`:
- `graph.node` — entity declaration (uri, kind, org, label)
- `graph.edge` — typed relation (from, to, relation, source, confidence)
- `graph.tag` — lightweight edge (target envelope, tag node, org)

Plus constitution layer under `lexicons/org/`:
- `org.principal`, `org.agent`, `org.channel`, `org.capability`, `org.contract`

**Three trust grades:** signed (agent proposed, ambient), stamped (one principal, org-canonical), counter-stamped (consensus). Browse surface color-codes; filter by grade. Reject = archived envelope, never deleted (audit).

## Migration from Penceive to Secretariat

| Penceive | Secretariat role |
|---|---|
| `src-tauri/src/tags.rs` | first passive agent: `tagger.local` |
| `useTags` hook | review surface for tag envelopes pending stamp |
| tantivy index | substrate-wide index, ACL-scoped per org/channel |
| frontmatter convention | source for entity extraction |
| companion pane | principal-driven graph review + LLM query over substrate |
| file explorer | ludic path browser (maps metaphor, attentional granularity) |
| WYSIWYG editor | "good enough" compose; defer power-user editing to standalone surface |
| recovery snapshots | substrate-level draft snapshots, principal-encrypted |

Penceive role post-migration (transitional): power-user writing surface pointed at `~/.secretariat/queues/`. Fold into Secretariat once its compose surface is non-painful.

## Ordering

- **v0.3 — constitution graph.** Lexicons + tagger.local + Penceive-as-review-surface + cognition policy on one org (Themia, local-only). 2-week vertical slice.
- **v0.4 — channels + multi-principal counter-stamp** (Secretariat roadmap already).
- **v0.5+ — corpus graph.** Same primitive applied to Themia legal corpus. Scale problem, not design problem.

## Adjacent ideas captured along the way

1. **LM Studio integration into Penceive companion pane.** OpenAI-compatible at `localhost:1234/v1`. `reqwest` already in `src-tauri/Cargo.toml`. Path of least resistance: Rust command + `tauri::ipc::Channel<String>` for streaming, avoids CORS, keeps endpoint config server-side. Once Secretariat merge advances, this becomes the LM-Studio-provider-class registration in the cognition layer.
2. **Federation before merge.** Don't collapse codebases yet — point Penceive's vault at `~/.secretariat/queues/`, read/write via lexicons, call `sec` daemon for stamp/send. If federation feels seamless after 2-3 weeks, merge.
3. **Tray stays anti-compulsion; Penceive is the depth surface.** Different attentional granularity, not conflicting. Tray = stamp/review ambient. Penceive = opened on purpose for deep work.
4. **Agent provider class declared by principal stamp, not agent self-claim.** At agent install, principal stamps a `provider_class` assertion. Daemon trusts principal claim. Closes the trust hole.
5. **Cross-org edges require stamps from both orgs' principals.** Natural fallout of the lexicon — `from` and `to` namespaces are different orgs → need both org's canonical authority.
6. **Audit query across constitution + corpus.** Tantivy indexes both. "Show all stamped envelopes by Rafa in did:web:themia.pro last 90 days" returns constitutional acts + corpus reviews interleaved. Audit-ready by construction.

## Open questions

- How does daemon resolve `provider_class` for an agent at request time — registered list, or live attestation? Registered list with principal-stamped assertion is simpler.
- Counter-stamp UX: does the second principal see the first principal's stamp as social proof, or is the review independent? README's process-verbaux model suggests independent review, but UX needs to make that intentional.
- Penceive's companion pane currently has no DID. Does it speak as the principal (with their key, requires Touch ID per AI response — too friction-heavy), or as a registered companion agent with its own DID? Almost certainly the latter — `companion.<principal>.local`.
- Where do `lexicons/graph/` and `lexicons/org/` actually live — under Secretariat repo, or a separate `lexicons/` repo Themia + EquanimiTech share? Probably the former for now; extract later if a third org appears.

— originally captured 2026-05-17 as 20260517T164919Z-cbu2m7.md in project:autonomous-enterprise
