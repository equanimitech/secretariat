---
migrated_from: equanimi.tech/project/secretariat/dev/20260517T200114Z-rbmoga.md
---
# Idea: Secretariat vault as a git repo

What if `~/.secretariat/` is itself a git working tree? Captures, channels, envelopes, contracts, archived/ subdirs — all under version control.

## Why this might be powerful
- **History for free.** Every envelope mutation (archive moves, channel renames, contract edits) shows up in `git log`. Audit trail without bespoke event-sourcing.
- **Cross-device sync.** Push/pull instead of a custom relay. Conflicts surface as merge conflicts (handle markdown well).
- **Branching for hypotheticals.** "What would my channel tree look like if I reorganized?" — branch, try, discard.
- **Atomic operations.** A bulk reroute (this session's stale-namespace cleanup) becomes one commit; revert if regretted.
- **Public publishability.** Push a subtree (`channel:articles`) to a public mirror = poor-man's publishing protocol.
- **Time-travel review.** "What was in my inbox 3 months ago?" — `git checkout` a tag.

## Tensions / open questions
- **Substrate writes are frequent and small.** Every capture = a write. Auto-commit per capture vs batched commit on stamp/archive/review?
- **Encrypted envelopes.** If/when relay-bound envelopes are E2E-encrypted, the ciphertext goes into the repo. Fine for backup but defeats text diff for canonical content. Keep cleartext locally; encrypt only at relay boundary?
- **Signing collision.** Secretariat already signs envelopes (Touch ID stamp). Git also has its own commit signing. Are they orthogonal layers (stamp = content attestation; commit = state transition) or do they conflict?
- **DID identity.** Commit author = principal DID? Or system user? Probably principal DID via git config per vault.
- **Remote model.** Self-hosted (`themia.pro/git/<principal-did>`) or peer-to-peer (each principal's relay also serves git)?
- **GUI implications.** Tray could surface "you have 12 uncommitted captures since last review." Aligns with strategic-friction philosophy — git push as the deliberate intentional act.
- **Performance at scale.** ~1k envelopes/year per principal × 5 years × N principals. Git handles much larger trees; just plan packfile/gc cadence.

## Composes with
- `channel:secretariat:dev` Penceive ↔ Secretariat synthesis (envelope 20260517T195837Z-u2gsv3): the constitution-graph layer benefits from git's audit trail.
- `channel:articles` 9-principles roadmap: "Local-First Ownership" and "Modification Rights" are reinforced by user-owned git history.
- Stamp ceremony: could include `git commit -S` (GPG-signed commit) at stamp time, gluing two attestation models.

## Smallest next step
Prototype: `git init ~/.secretariat` on a test vault. Auto-commit hook on `capture`/`archive`/`stamp` tool calls. Observe: does the commit cadence feel right? What's the diff noise like? Does merge work for two devices?

— captured during 2026-05-17 review session as a substrate idea.
