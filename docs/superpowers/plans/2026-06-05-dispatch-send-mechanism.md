---
$attestation:
  $type: tech.equanimi.secretariat.stamp
  signer: did:key:z6MkjB8PQaN1vuUzdtnJsxyXR2f8d3tckGHkUYZMDytQsfak
  act: attest
  docHash: sha256:f31d25bf674d62dd51c90a6ceb67cd668b4d04afc0bc0a5df5c55714d8f91bb6
  docFilename: 2026-06-05-dispatch-send-mechanism.md
  stampedAt: 2026-06-05T18:42:33.570185Z
  signature: ed25519:dVRK7pikMHr2LPRDFvi4Q33x07aTuy0DNrFdJWPNdgmtYlX+7UCJXdPimGr4vlKwsdYg83PJwRzhetqS5vq1BA==
---
# Dispatch / Send Mechanism Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an editor toolbar "Send" button that dispatches the open document to Slack by driving the scribe (headless `claude -p`) through a compose → human-gate → send-verbatim flow.

**Architecture:** Two thin Tauri commands (`dispatch_compose`, `dispatch_send`) shell out to the configured cognition CLI headless (`claude -p --output-format json`), each carrying a fixed per-target prompt. Compose returns `{channel, body}` without sending; the frontend renders the body verbatim for principal confirmation; Send posts it verbatim. Mechanism is transport-blind (`DispatchTarget` enum, one variant `Slack` today); Slack-ness lives only in the prompt template. **No core domain, no `lexicons/` diff** — prose over the wire, not a record. Send = signature layer; the Touch-ID stamp gate is never reached from this path.

> **Spec refinement (principal-approved 2026-06-05):** the stamped design diagram named the `claude_code_sdk` streaming bridge. That bridge is interactive/multi-session and returns no final string. This plan shells `claude -p` headless instead — one spawn, one parseable result, deterministic. SDK-session path comes later. Mechanism (compose→gate→send-verbatim) is unchanged.

**Tech Stack:** Rust (Tauri v2, `tauri-plugin-shell`, `serde`, `specta`/`tauri-specta`), React + TypeScript (sonner toasts, generated `commands` bindings), `claude` CLI on PATH.

**Spec:** `docs/superpowers/specs/2026-06-05-dispatch-send-mechanism-design.md`

---

## File Structure

| File | Responsibility | New? |
|------|----------------|------|
| `src-tauri/src/commands/dispatch.rs` | `DispatchTarget` enum, pure prompt builders, pure output parsers, the two `#[tauri::command]`s | **create** |
| `src-tauri/src/commands/mod.rs` | declare `pub mod dispatch;` | modify |
| `src-tauri/src/cognition/claude_code_sdk.rs` | make `resolve_claude_path` reusable (`pub(crate)`) | modify (1 line) |
| `src-tauri/src/bindings.rs` | register the two commands in `collect_commands!` | modify |
| `src/lib/dispatch/dispatch-client.ts` | thin async wrapper over `commands.dispatchCompose` / `commands.dispatchSend`, normalizing the `{status}` result | **create** |
| `src/components/markdown/DispatchComposer.tsx` | dialog: instruction input → compose → show body → confirm → send → toast | **create** |
| `src/components/markdown/MarkdownTitlebar.tsx` | mount the Send button that opens the composer | modify |

---

## Task 1: Rust — `DispatchTarget` + prompt builders (pure, TDD)

**Files:**
- Create: `src-tauri/src/commands/dispatch.rs`
- Modify: `src-tauri/src/commands/mod.rs`

- [ ] **Step 1: Create the module file with types, prompt builders, and failing tests**

Create `src-tauri/src/commands/dispatch.rs`:

```rust
//! Dispatch / send mechanism — drives the scribe (headless `claude -p`) to
//! compose and send a document to an external target. Today the only target
//! is Slack via the scribe's Slack MCP tools.
//!
//! This is the SEND mechanism, not a Slack feature: the flow (compose →
//! human gate → send-verbatim) is transport-blind. The Slack-ness lives only
//! in the per-target prompt template. Add a `DispatchTarget` variant + a
//! second template when a second target earns it — see the spec's seam note.
//!
//! Trust: send = signature layer (bodies are signed automatically). No stamp,
//! no `$attestation`, no lexicon record. The Touch-ID stamp gate is unreachable
//! from this path.

use serde::{Deserialize, Serialize};

/// Where a dispatch goes. One variant today; the enum documents the seam.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum DispatchTarget {
    Slack,
}

/// Result of the COMPOSE phase — the scribe's draft, not yet sent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct ComposeResult {
    pub channel: String,
    pub body: String,
}

/// Result of the SEND phase. `permalink` is best-effort (the scribe may or
/// may not surface one); success is determined by the CLI exit, not this field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct SendResult {
    pub permalink: Option<String>,
}

/// Build the COMPOSE prompt. The scribe reads the doc, drafts a message per
/// the principal's free-form instruction, and returns JSON `{channel, body}`
/// WITHOUT sending.
pub fn compose_prompt(target: DispatchTarget, doc_path: &str, instruction: &str) -> String {
    match target {
        DispatchTarget::Slack => format!(
            "You are the scribe. Read the markdown document at `{doc_path}`. \
The principal wants to dispatch it to Slack per this instruction: «{instruction}». \
Compose the Slack message body and identify the target channel from the instruction. \
Do NOT send anything. Reply with ONLY a JSON object, no prose, no code fence: \
{{\"channel\": \"<#channel-or-name>\", \"body\": \"<message text>\"}}."
        ),
    }
}

/// Build the SEND prompt. The scribe sends the already-confirmed body verbatim.
pub fn send_prompt(target: DispatchTarget, channel: &str, body: &str) -> String {
    match target {
        DispatchTarget::Slack => format!(
            "Send this EXACT text verbatim to Slack channel `{channel}` using the \
`slack_send_message` tool. Do not edit, summarize, translate, or add anything. \
After sending, reply with ONLY a JSON object, no prose: \
{{\"permalink\": \"<message permalink or null>\"}}. \
The text to send is:\n\n{body}"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compose_prompt_embeds_path_and_instruction() {
        let p = compose_prompt(DispatchTarget::Slack, "/docs/note.md", "send the summary to #legal");
        assert!(p.contains("/docs/note.md"));
        assert!(p.contains("send the summary to #legal"));
        assert!(p.contains("Do NOT send"));
        assert!(p.contains("\"channel\""));
        assert!(p.contains("\"body\""));
    }

    #[test]
    fn send_prompt_embeds_channel_and_body_verbatim() {
        let p = send_prompt(DispatchTarget::Slack, "#legal", "Hello team");
        assert!(p.contains("#legal"));
        assert!(p.contains("Hello team"));
        assert!(p.contains("verbatim"));
        assert!(p.contains("slack_send_message"));
    }
}
```

Add to `src-tauri/src/commands/mod.rs` (alongside the other `pub mod` lines):

```rust
pub mod dispatch;
```

- [ ] **Step 2: Run the tests to verify they pass**

Run: `cargo test -p secretariat dispatch::tests -- --nocapture`
Expected: PASS (2 tests). If the crate name differs, use the package that owns `src-tauri` — check `src-tauri/Cargo.toml` `[package] name`; commands below assume `secretariat`.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/commands/dispatch.rs src-tauri/src/commands/mod.rs
git commit -m "feat(dispatch): DispatchTarget + per-target prompt builders"
```

---

## Task 2: Rust — output parsers (pure, TDD)

The CLI is invoked with `--output-format json`, which wraps the agent's reply in an envelope like `{"type":"result","subtype":"success","is_error":false,"result":"<agent text>"}`. We extract `result`, then parse the agent's JSON out of it (tolerating an accidental ```json fence).

**Files:**
- Modify: `src-tauri/src/commands/dispatch.rs`

- [ ] **Step 1: Add the parser fns + failing tests**

Append to `src-tauri/src/commands/dispatch.rs` (above the `#[cfg(test)]` module, then add the new tests inside it):

```rust
/// Pull the agent's reply text out of the `claude -p --output-format json`
/// envelope. Errors if the envelope reports `is_error` or has no `result`.
pub fn extract_result_text(stdout: &str) -> Result<String, String> {
    let v: serde_json::Value = serde_json::from_str(stdout.trim())
        .map_err(|e| format!("cognition CLI returned non-JSON output: {e}"))?;
    if v.get("is_error").and_then(|b| b.as_bool()) == Some(true) {
        let msg = v.get("result").and_then(|r| r.as_str()).unwrap_or("unknown error");
        return Err(format!("scribe reported an error: {msg}"));
    }
    v.get("result")
        .and_then(|r| r.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "cognition CLI output had no `result` field".to_string())
}

/// Strip an optional ```json … ``` fence and surrounding whitespace.
fn strip_fence(text: &str) -> &str {
    let t = text.trim();
    let t = t.strip_prefix("```json").or_else(|| t.strip_prefix("```")).unwrap_or(t);
    t.trim().strip_suffix("```").unwrap_or(t).trim()
}

/// Parse the COMPOSE agent reply into `{channel, body}`.
pub fn parse_compose_output(text: &str) -> Result<ComposeResult, String> {
    serde_json::from_str::<ComposeResult>(strip_fence(text))
        .map_err(|e| format!("could not parse composed message (expected {{channel, body}}): {e}"))
}

/// Parse the SEND agent reply. Missing/garbled permalink is non-fatal — the
/// CLI exit already told us the send succeeded — so fall back to `None`.
pub fn parse_send_output(text: &str) -> SendResult {
    let permalink = serde_json::from_str::<SendResult>(strip_fence(text))
        .ok()
        .and_then(|r| r.permalink);
    SendResult { permalink }
}
```

Add these tests inside the existing `#[cfg(test)] mod tests`:

```rust
    #[test]
    fn extract_result_text_pulls_result_field() {
        let env = r#"{"type":"result","is_error":false,"result":"hello"}"#;
        assert_eq!(extract_result_text(env).unwrap(), "hello");
    }

    #[test]
    fn extract_result_text_errors_on_is_error() {
        let env = r#"{"type":"result","is_error":true,"result":"boom"}"#;
        assert!(extract_result_text(env).unwrap_err().contains("boom"));
    }

    #[test]
    fn parse_compose_output_handles_bare_and_fenced_json() {
        let bare = r#"{"channel":"#legal","body":"Hi"}"#;
        assert_eq!(
            parse_compose_output(bare).unwrap(),
            ComposeResult { channel: "#legal".into(), body: "Hi".into() }
        );
        let fenced = "```json\n{\"channel\":\"#legal\",\"body\":\"Hi\"}\n```";
        assert_eq!(parse_compose_output(fenced).unwrap().channel, "#legal");
    }

    #[test]
    fn parse_send_output_tolerates_missing_permalink() {
        assert_eq!(parse_send_output("not json").permalink, None);
        assert_eq!(
            parse_send_output(r#"{"permalink":"https://x"}"#).permalink,
            Some("https://x".into())
        );
    }
```

- [ ] **Step 2: Run the tests**

Run: `cargo test -p secretariat dispatch::tests -- --nocapture`
Expected: PASS (6 tests total).

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/commands/dispatch.rs
git commit -m "feat(dispatch): pure parsers for claude -p json output"
```

---

## Task 3: Rust — the two Tauri commands (shell-out)

These wrap shell + the pure helpers. They are not unit-tested (they spawn a real agent — verified by dogfood in Task 6, per the project convention that infra tests use real integrations, not mocks).

**Files:**
- Modify: `src-tauri/src/commands/dispatch.rs`
- Modify: `src-tauri/src/cognition/claude_code_sdk.rs` (make resolver reusable)

- [ ] **Step 1: Make the claude-path resolver reusable**

In `src-tauri/src/cognition/claude_code_sdk.rs`, change the resolver's visibility:

```rust
pub(crate) fn resolve_claude_path() -> Option<std::path::PathBuf> {
```

(The existing body — `which claude` — is unchanged. Config-driven cognition-command resolution is a deferred refinement; for the keystone we reuse this PATH lookup.)

- [ ] **Step 2: Add the commands to `dispatch.rs`**

Add near the top of `src-tauri/src/commands/dispatch.rs` (after the `use serde::...` line):

```rust
use tauri::AppHandle;
use tauri_plugin_shell::ShellExt;

use crate::cognition::claude_code_sdk::resolve_claude_path;

/// Run the configured cognition CLI headless with `prompt`, return the agent's
/// reply text (already unwrapped from the `--output-format json` envelope).
async fn run_scribe(app: &AppHandle, prompt: &str) -> Result<String, String> {
    let claude = resolve_claude_path()
        .ok_or_else(|| "cognition CLI (`claude`) not found on PATH".to_string())?;
    let output = app
        .shell()
        .command(claude)
        .args(["-p", prompt, "--output-format", "json"])
        .output()
        .await
        .map_err(|e| format!("could not run the scribe: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("scribe exited with an error: {}", stderr.trim()));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    extract_result_text(&stdout)
}

/// COMPOSE: draft a message from the document. Does NOT send.
#[tauri::command]
#[specta::specta]
pub async fn dispatch_compose(
    app: AppHandle,
    target: DispatchTarget,
    doc_path: String,
    instruction: String,
) -> Result<ComposeResult, String> {
    let prompt = compose_prompt(target, &doc_path, &instruction);
    let text = run_scribe(&app, &prompt).await?;
    parse_compose_output(&text)
}

/// SEND: post the principal-confirmed body verbatim.
#[tauri::command]
#[specta::specta]
pub async fn dispatch_send(
    app: AppHandle,
    target: DispatchTarget,
    channel: String,
    body: String,
) -> Result<SendResult, String> {
    let prompt = send_prompt(target, &channel, &body);
    let text = run_scribe(&app, &prompt).await?;
    Ok(parse_send_output(&text))
}
```

- [ ] **Step 3: Register the commands in `bindings.rs`**

In `src-tauri/src/bindings.rs`, add `dispatch` to the `use crate::commands::{…}` list, and add two lines inside `collect_commands![…]` (after the `agent::*` lines):

```rust
        dispatch::dispatch_compose,
        dispatch::dispatch_send,
```

- [ ] **Step 4: Build to verify it compiles**

Run: `cd src-tauri && cargo check -p secretariat`
Expected: compiles clean. (If a clean clone fails on a missing sidecar resource, run `src-tauri/scripts/build-sidecars.sh` once first — see AGENTS.md.)

- [ ] **Step 5: Clippy gate**

Run: `cargo clippy -p secretariat -- -D warnings`
Expected: no warnings.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/commands/dispatch.rs src-tauri/src/cognition/claude_code_sdk.rs src-tauri/src/bindings.rs
git commit -m "feat(dispatch): dispatch_compose + dispatch_send tauri commands"
```

---

## Task 4: Regenerate TypeScript bindings + frontend client

**Files:**
- Modify (generated): `src/lib/bindings.ts`
- Create: `src/lib/dispatch/dispatch-client.ts`

- [ ] **Step 1: Regenerate the specta bindings**

Run: `cd src-tauri && cargo test export_bindings -- --ignored`
Expected: PASS; `src/lib/bindings.ts` now contains `dispatchCompose`, `dispatchSend`, and the `ComposeResult` / `SendResult` / `DispatchTarget` types.

- [ ] **Step 2: Verify the generated surface**

Run: `grep -n "dispatchCompose\|dispatchSend\|ComposeResult\|DispatchTarget" src/lib/bindings.ts`
Expected: matches for all four. Note the exact import path the existing code uses for `commands` — `MarkdownWindow.tsx` imports from `@/lib/tauri-bindings`. Use the SAME specifier in the next step.

- [ ] **Step 3: Write the client wrapper**

Create `src/lib/dispatch/dispatch-client.ts`:

```ts
import { commands, type ComposeResult, type SendResult } from '@/lib/tauri-bindings'

/** Draft a Slack message from a document. Does not send. */
export async function compose(
  docPath: string,
  instruction: string,
): Promise<ComposeResult> {
  const res = await commands.dispatchCompose('slack', docPath, instruction)
  if (res.status === 'error') throw new Error(res.error)
  return res.data
}

/** Send a confirmed body verbatim to a Slack channel. */
export async function send(channel: string, body: string): Promise<SendResult> {
  const res = await commands.dispatchSend('slack', channel, body)
  if (res.status === 'error') throw new Error(res.error)
  return res.data
}
```

- [ ] **Step 4: Typecheck**

Run: `pnpm tsc --noEmit` (or the project's typecheck script — check `package.json` `scripts`; likely `pnpm typecheck`)
Expected: no errors. If `commands.dispatchCompose` expects a different arg shape than positional `('slack', docPath, instruction)`, match whatever `bindings.ts` generated (some tauri-specta versions take a single object); adjust the wrapper accordingly.

- [ ] **Step 5: Commit**

```bash
git add src/lib/bindings.ts src/lib/dispatch/dispatch-client.ts
git commit -m "feat(dispatch): regenerate bindings + frontend dispatch client"
```

---

## Task 5: `DispatchComposer` component (the human gate)

**Files:**
- Create: `src/components/markdown/DispatchComposer.tsx`

- [ ] **Step 1: Write the component**

Create `src/components/markdown/DispatchComposer.tsx`. It uses the existing shadcn `Dialog` + `Button` + `Textarea` primitives (confirm they exist under `src/components/ui/`; the repo already uses `alert-dialog` and `button` — if `dialog`/`textarea` are absent, add them via the project's shadcn flow or fall back to `alert-dialog` + a styled `textarea`).

```tsx
import { useState } from 'react'
import { toast } from 'sonner'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { Textarea } from '@/components/ui/textarea'
import { compose, send } from '@/lib/dispatch/dispatch-client'
import type { ComposeResult } from '@/lib/tauri-bindings'

interface DispatchComposerProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  /** Absolute path of the document being dispatched. */
  docPath: string
}

type Phase = 'instruct' | 'composing' | 'review' | 'sending'

export function DispatchComposer({ open, onOpenChange, docPath }: DispatchComposerProps) {
  const [phase, setPhase] = useState<Phase>('instruct')
  const [instruction, setInstruction] = useState('')
  const [draft, setDraft] = useState<ComposeResult | null>(null)

  function reset() {
    setPhase('instruct')
    setInstruction('')
    setDraft(null)
  }

  async function handleCompose() {
    if (!instruction.trim()) return
    setPhase('composing')
    try {
      const result = await compose(docPath, instruction.trim())
      setDraft(result)
      setPhase('review')
    } catch (e) {
      toast.error(`Compose failed: ${e instanceof Error ? e.message : String(e)}`)
      setPhase('instruct')
    }
  }

  async function handleSend() {
    if (!draft) return
    setPhase('sending')
    try {
      const result = await send(draft.channel, draft.body)
      toast.success(
        result.permalink ? `Sent to ${draft.channel}` : `Sent to ${draft.channel}`,
        result.permalink ? { description: result.permalink } : undefined,
      )
      onOpenChange(false)
      reset()
    } catch (e) {
      toast.error(`Send failed: ${e instanceof Error ? e.message : String(e)}`)
      setPhase('review') // keep the draft for retry
    }
  }

  return (
    <Dialog
      open={open}
      onOpenChange={(o) => {
        onOpenChange(o)
        if (!o) reset()
      }}
    >
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Send to Slack</DialogTitle>
          <DialogDescription>
            The scribe drafts the message; you review the exact text before it sends.
            Sends the saved document.
          </DialogDescription>
        </DialogHeader>

        {phase !== 'review' && phase !== 'sending' ? (
          <Textarea
            placeholder="e.g. send a short summary to #legal"
            value={instruction}
            onChange={(e) => setInstruction(e.target.value)}
            disabled={phase === 'composing'}
            rows={3}
          />
        ) : (
          <div className="space-y-2">
            <div className="text-sm text-muted-foreground">
              Channel: <span className="font-mono">{draft?.channel}</span>
            </div>
            <Textarea readOnly value={draft?.body ?? ''} rows={8} className="font-mono text-sm" />
          </div>
        )}

        <DialogFooter>
          {phase === 'review' || phase === 'sending' ? (
            <>
              <Button variant="outline" onClick={() => setPhase('instruct')} disabled={phase === 'sending'}>
                Back
              </Button>
              <Button onClick={handleSend} disabled={phase === 'sending'}>
                {phase === 'sending' ? 'Sending…' : 'Send'}
              </Button>
            </>
          ) : (
            <Button onClick={handleCompose} disabled={phase === 'composing' || !instruction.trim()}>
              {phase === 'composing' ? 'Composing…' : 'Compose'}
            </Button>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
```

- [ ] **Step 2: Typecheck**

Run: `pnpm tsc --noEmit`
Expected: no errors. If `dialog`/`textarea` primitives are missing, add them (shadcn) before this passes.

- [ ] **Step 3: Commit**

```bash
git add src/components/markdown/DispatchComposer.tsx src/components/ui/
git commit -m "feat(dispatch): DispatchComposer dialog with compose-review-send gate"
```

---

## Task 6: Mount the Send button in `MarkdownTitlebar`

**Files:**
- Modify: `src/components/markdown/MarkdownTitlebar.tsx`

- [ ] **Step 1: Inspect the titlebar to match its existing button pattern**

Run: `sed -n '1,80p' src/components/markdown/MarkdownTitlebar.tsx`
Note: how it receives the file path (prop name), how existing icon buttons (reload/reveal/archive) are rendered, and the icon import source (`lucide-react`). Mirror that exact pattern in the next step.

- [ ] **Step 2: Add the button + composer**

In `MarkdownTitlebar.tsx`: import `Send` from `lucide-react`, import `DispatchComposer`, add `const [dispatchOpen, setDispatchOpen] = useState(false)`, render a Send icon button beside the existing actions wired to `onClick={() => setDispatchOpen(true)}`, and render `<DispatchComposer open={dispatchOpen} onOpenChange={setDispatchOpen} docPath={filePath} />` (use whatever the file-path prop is actually named in this component). Match the existing buttons' size/variant classes exactly.

Example button (adapt classes to the neighbours):

```tsx
<button
  type="button"
  aria-label="Send to Slack"
  title="Send to Slack"
  className={/* same classes as the reload/reveal buttons */ ''}
  onClick={() => setDispatchOpen(true)}
>
  <Send className="size-4" />
</button>
```

- [ ] **Step 3: Typecheck + lint**

Run: `pnpm tsc --noEmit && pnpm lint`
Expected: no errors.

- [ ] **Step 4: Commit**

```bash
git add src/components/markdown/MarkdownTitlebar.tsx
git commit -m "feat(dispatch): send-to-slack button in editor titlebar"
```

---

## Task 7: Dogfood — simmer first (manual, gated)

Per the project's "dogfood a workflow on itself, simmer first" rule: exercise compose-only before any real send, with the principal's eyes on each step. No auto-advance.

- [ ] **Step 1: Build and launch the app**

Run: `pnpm tauri:dev`
Expected: app launches; sidecars staged automatically by `beforeBuildCommand`.

- [ ] **Step 2: Compose-only smoke (no send)**

Open any throwaway markdown doc. Click Send → enter `draft a one-line summary to #<a test channel you own>` → Compose. Confirm the review pane shows a sane channel + body. Click **Back/Cancel — do NOT send yet.** Confirm nothing posted to Slack.

- [ ] **Step 3: One real send to a test channel**

Repeat, and this time click **Send**. Confirm: (a) the message lands in the test channel verbatim, (b) the success toast fires, (c) the doc body itself was unchanged. Note latency (two spawns).

- [ ] **Step 4: Error-path spot check**

With Slack MCP intentionally unreachable (or a nonsense channel), confirm the error toast surfaces and the composed body is preserved for retry rather than lost.

- [ ] **Step 5: Record the dogfood result**

Append a short note (date, what worked, any friction) to the spec or a `/log` entry. Do not mark the feature "shipped" — release (8-manifest lockstep bump) is a separate, principal-gated step and is explicitly NOT part of this plan.

---

## Out of scope (deferred — do NOT build here)

- Config-driven cognition-command resolution (keystone reuses `which claude`).
- Channel autocomplete / picker; default-channel preference.
- `slack_schedule_message` / draft variants; targets beyond Slack.
- Release ceremony / version bumps / DMG.

## Self-review notes

- **Spec coverage:** architecture (Tasks 1–6), trust posture (no stamp path touched — Tasks 1/3 prose + commands never call stamp), two prompts (Task 1), error handling (Task 3 `run_scribe` + Task 5 toasts/retain-body), testing pure+live split (Tasks 1–2 pure, Task 7 live), scope fork + seam note (Out-of-scope section). ✓
- **Type consistency:** `DispatchTarget`, `ComposeResult{channel,body}`, `SendResult{permalink}` used identically across Rust (Tasks 1–3) and TS (Tasks 4–5). `dispatch_compose`/`dispatch_send` → `dispatchCompose`/`dispatchSend` (specta camelCase). ✓
- **Placeholder scan:** the two intentional "verify the real shape in the repo" steps (crate name in Task 1/3; arg shape in Task 4; prop name + button classes in Task 6) are inspection instructions with concrete fallbacks, not deferred work. ✓
