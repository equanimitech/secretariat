---
tag: pitch
appetite: small
status: draft
source: docs/ideas/2026-05-31-edit-with-claude-button.md
slice_id: A
---

# Pitch — Edit-with-Claude button

**Bet:** Add one button to the markdown editor that launches Claude in the doc's repo, with the doc handed in as the thing to edit — closing the git-native loop (human edits, Claude assists, human stamps) in one click.

**Why it matters:** Post-teardown, Secretariat is "markdown editor + Touch-ID stamp over git repos." The editor and the stamp exist; the *Claude-assist* leg is a manual `cd <repo> && claude "edit this doc"`. The button is the missing third leg of the core loop.

---

## Boundaries

**JBTD:** When I'm reading or editing a doc in the Secretariat editor and want Claude to draft or refine it, I want to hand the doc to Claude without leaving the app, so I stay in the read → assist → stamp loop. Baseline today: I leave the app, open a terminal, `cd` to the repo by hand, run `claude`, and re-type which file I mean.

**Out:**
- In-app chat / streaming edits inside the editor pane. The button launches the *external* cognition session (`CognitionLaunching`), not the in-process `CognitionSession`. Diff-in-place is a separate, later slice.
- New cognition plumbing. Reuse `PrefsLauncher` + `launch_macos_in` verbatim — no new launch port, no SDK wiring.
- A return path / round-trip sync. Claude edits the file on disk; the editor already reloads via ⌘R + the conflict guard. "And back" is the existing reload, not new code.

## Elements

- **The button** (`src/components/markdown/MarkdownTitlebar.tsx:30`). A `Sparkles` ghost button in the titlebar action row, next to Reload/Reveal. The titlebar's own NOTE says "Launch Claude" used to live here and moved out because it was *channel*-scoped — git-native makes the **repo** the unit, so a *doc*-scoped launch belongs here again.
- **Repo-root walk** (new, mirrors `find_enclosing_channel_dir`, `src-tauri/src/commands/secretariat.rs:991`). Walk up from the doc path to the nearest `.git/` instead of the nearest `channel.md`. That dir is the cwd. One helper, same shape as the channel-dir walk it replaces.
- **`launch_claude_on_doc` command** (new, models `launch_claude_at`, `secretariat.rs:974`). Takes the file path, resolves repo root, builds the plan via `PrefsLauncher::from_prefs`, appends the doc path as Claude's initial positional prompt, spawns via `launch_macos_in(target, &shell, Some(repo_root))` (`secretariat.rs:656`).
- **Initial-context string.** Pass `claude "Edit @<relpath>"` (repo-relative path) as the trailing positional arg in the shell line built at `secretariat.rs:1130`. This is the one genuinely new behavior — every other piece is reuse.

## Risks

**🐇 Rabbit holes:**
- Prompt-passing across substrates. `claude` takes a positional prompt; a future Ollama/aider wrapper may not. Keep it to the Claude adapter's arg convention for v1; don't generalize the launch port now.
- Terminal-target matrix. `launch_macos_in` already branches over Terminal/iTerm/WezTerm/Alacritty/ClaudeDesktop with per-target quoting (`secretariat.rs:656`). Appending a quoted prompt arg multiplies the escaping surface. Reuse the existing per-target quoting; don't rewrite it.

**🏴 Off-sides:**
- Streaming in-editor edits + accept/reject diffs. Tempting ("real" Edit-with-Claude), but that's the in-process `CognitionSession` port and a multi-day slice. The button ships the external loop first.

**🥩 Fat cut:**
- Per-doc prompt templates ("shape this idea" on `docs/ideas/*`, "implement" on a pitch) from the git-native note. Real, but it's a `.claude/` convention layer, not this button. Ship the bare hand-off; templates land when the review walker does.

**🧪 Domain knowledge:**
- Confirm the doc isn't always inside a registered repo. Captures in the personal-knowledge home repo are fine; a doc opened from outside any `.git/` must fail gracefully (toast: "not in a git repo"), mirroring `launch_claude_at`'s `ok_or_else` error path.
- Confirm `claude "<prompt>"` reads the positional as an opening turn (not a slash command) in the installed CLI version before wiring the arg.

## Acceptance

1. A `Sparkles` button renders in the editor titlebar for any open doc.
2. Clicking it, with a doc inside a git repo, opens the configured terminal at the repo root running `claude` with the doc's repo-relative path in the opening prompt.
3. Clicking it on a doc *not* inside any git repo shows a toast error and launches nothing.
4. After Claude writes the file, ⌘R (or the reload button) shows the new content; the existing conflict dialog fires if there were unsaved editor edits.
5. The launch honors the principal's configured terminal target and `launch_env` (LM Studio routing still works) — i.e. it routes through `PrefsLauncher`, not a hardcoded `claude`.

---

_Drafted by Claude (scribe). Source: `docs/ideas/2026-05-31-edit-with-claude-button.md`._
