# Editor & Envelope-Reader Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Split the single markdown editor surface into two explicit intents — **Compose** (edit) and **Attend** (read / verify / seal) — over one document, with verify state made first-class and the seal re-weighted as a sober ceremony in the reading posture.

**Architecture:** Keep one `MarkdownWindow` component; add an in-component `intent` state (`'compose' | 'attend'`) toggled with one key, wrapped in React 19 `<ViewTransition>` for a calm cross-fade. Surface real signature/stamp state by adding a `verify_envelope` Tauri command that calls the existing `secretariat_core::application::verify_document_layered` use case (the same path `sec verify --json` uses) — no subprocess. A new `TrustChip` + `TrustBanner` render the four-state vocabulary; the stamp ceremony relocates to Attend's bounded end. Editing a sealed doc raises a calm break-the-seal interstitial.

**Tech Stack:** Rust (Tauri v2 commands, `secretariat-core` use cases), tauri-specta (TS binding generation), React 19.2 + TypeScript, Tailwind v4 (OKLCH tokens), Milkdown Crepe editor, shadcn/Radix primitives.

---

## Reference reading (do this first, ~10 min)

- `docs/superpowers/specs/2026-06-03-editor-reader-redesign-design.md` — the spec this plan implements. The Shape-Up scope tiers (must / should / nice) map to the task ordering below.
- `AGENTS.md` hard rule #4 (stamp ceremony — principal-attested, render body verbatim, explicit consent in same turn) and #5 (verify before trusting; failed signature → quarantine, not surface). The UI must encode these, not weaken them.
- `crates/cli/src/commands/verify.rs` — the canonical `sec verify --json` shape. **Critical:** the real JSON is `{ signature, stamp }` only. There is **no `counter_stamps` key** — the spec assumed one; counter-stamp is reserved (no record type ships). The plan reflects the real shape; the counter row in Attend renders as a static greyed "reserved", fed by nothing.
- `crates/core/src/application/verify_document.rs` — `verify_document_layered(file, resolver, local_principal_did, manifest_cache_root) -> LayeredVerifyOutcome { signature: SignatureOutcome, stamp: VerifyOutcome }`. Study the two enums; the TS `TrustState` is derived from their pairing.
- `src-tauri/src/commands/secretariat.rs` — mirror `stamp_envelope` (lines 196-245) for the new command; reuse `load_self_did` (line 600) and `KeyPaths::discover()`.
- `src/components/markdown/MarkdownWindow.tsx` — the orchestrator you'll extend. Children: `MarkdownTitlebar`, `CrepeEditor`, `EnvelopeFooter`, `FrontmatterPanel`.

### Trust-state derivation (single source of truth — implement once, in Task 5)

Derive one coarse `TrustState` from the layered outcome. This table is the contract every later task depends on:

| `signature.outcome`                              | `stamp.outcome`        | `TrustState` | Chip glyph · token         |
| ------------------------------------------------ | ---------------------- | ------------ | -------------------------- |
| `tampered` / `signatureInvalid` / `invalid`      | (any)                  | `tampered`   | ⚠ · `--trust-tampered` (red) |
| (any)                                            | `tampered`             | `tampered`   | ⚠ · `--trust-tampered`     |
| `ok` / `verifiedAgent` / `okUnverifiedAgent`     | `verified`             | `sealed`     | ✓ · `--trust-sealed` (teal/ink) |
| `ok` / `verifiedAgent` / `okUnverifiedAgent`     | `none`                 | `signed`     | ◷ · `--trust-signed` (slate) |
| `none`                                           | `verified`             | `sealed`     | ✓ · `--trust-sealed`       |
| `none`                                           | `none`                 | `unsigned`   | ○ · `--trust-unsigned` (muted) |
| `signerUnresolvable`                             | (not tampered)         | `signed`*    | ◷ · `--trust-signed`       |

\* `signerUnresolvable` is "can't confirm", not "tampered" — degrade to `signed` (informational), never to `sealed`. Tampered always wins (precedence top-to-bottom).

---

## Scope ordering

This plan covers all 10 spec elements. **PR #1 (this session) ships Tasks 1-3 only** — the verify binding keystone. Tasks 4-12 (the frontend redesign) are sequenced for follow-up PRs against this same plan. Each task is independently committable.

- **PR #1 — verify keystone (must-have #2 foundation):** Tasks 1-3.
- **PR #2 — two intents + trust surface (must-have #1, #2 UI, #3):** Tasks 4-8.
- **PR #3 — seal-breaking + polish (must-have #4, should/nice):** Tasks 9-12.

---

## Task 1: `verify_envelope` Tauri command (Rust)

**Files:**
- Modify: `src-tauri/src/commands/secretariat.rs` (add command + result types near `stamp_envelope`, ~line 195)
- Test: same file, `#[cfg(test)]` module at end

**What it does:** Wraps `verify_document_layered` for the frontend, flattening the two Rust enums into a serde-friendly `LayeredVerifyResult` the TS layer can switch on. Mirrors the CLI's `print_json` field names (`outcome`, `signer`, `signerRole`, `signedAt`, `stampedAt`, `act`, `claimedHash`, `computedHash`, `cause`) so the wire vocabulary is identical across CLI / MCP / Tauri.

- [ ] **Step 1: Write the failing test**

Add at the end of `src-tauri/src/commands/secretariat.rs`:

```rust
#[cfg(test)]
mod verify_tests {
    use super::*;

    #[test]
    fn unsigned_document_maps_to_none_layers() {
        // A bare markdown file with no $signature and no $attestation.
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("doc.md");
        std::fs::write(&path, "# just a draft\n").unwrap();

        let out = verify_envelope_inner(path.to_string_lossy().to_string())
            .expect("verify should succeed on a plain file");

        assert_eq!(out.signature.outcome, "none");
        assert_eq!(out.stamp.outcome, "none");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p secretariat verify_tests::unsigned_document_maps_to_none_layers`
Expected: FAIL — `verify_envelope_inner` not found, `VerifyLayerResult`/fields unresolved.

> Note: `secretariat` is the `src-tauri` crate name (see `src-tauri/Cargo.toml`). Confirm with `cargo test -p secretariat --no-run` if unsure.

- [ ] **Step 3: Write the result types**

Add near `StampReport` (after line 194) in `src-tauri/src/commands/secretariat.rs`:

```rust
/// One trust layer (signature OR stamp), flattened for the frontend.
/// `outcome` is the discriminant; the other fields are populated per
/// variant exactly as the CLI's `sec verify --json` emits them, so the
/// wire vocabulary is identical across CLI / MCP / Tauri.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct VerifyLayerResult {
    /// Signature layer: none | ok | verifiedAgent | okUnverifiedAgent | tampered | signerUnresolvable | invalid
    /// Stamp layer:     none | verified | tampered | signerUnresolvable | signatureInvalid
    pub outcome: String,
    pub signer: Option<String>,
    pub signer_role: Option<String>,
    pub principal: Option<String>,
    pub agent: Option<String>,
    pub signed_at: Option<String>,
    pub stamped_at: Option<String>,
    pub act: Option<String>,
    pub claimed_hash: Option<String>,
    pub computed_hash: Option<String>,
    pub cause: Option<String>,
}

impl VerifyLayerResult {
    fn empty(outcome: &str) -> Self {
        Self {
            outcome: outcome.to_string(),
            signer: None,
            signer_role: None,
            principal: None,
            agent: None,
            signed_at: None,
            stamped_at: None,
            act: None,
            claimed_hash: None,
            computed_hash: None,
            cause: None,
        }
    }
}

/// Layered verify result: author signature + principal stamp, reported
/// independently. There is intentionally NO counter-stamp field — no
/// counter-stamp record type ships (AGENTS.md "out of scope").
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct LayeredVerifyResult {
    pub signature: VerifyLayerResult,
    pub stamp: VerifyLayerResult,
}
```

- [ ] **Step 4: Write the mapping + command**

Add directly below the types:

```rust
fn map_signature(out: &secretariat_core::application::SignatureOutcome) -> VerifyLayerResult {
    use secretariat_core::application::SignatureOutcome as S;
    match out {
        S::None => VerifyLayerResult::empty("none"),
        S::Ok { signer, signer_role, signed_at } => VerifyLayerResult {
            signer: Some(signer.as_str().to_string()),
            signer_role: Some(signer_role.as_str().to_string()),
            signed_at: Some(signed_at.to_rfc3339()),
            ..VerifyLayerResult::empty("ok")
        },
        S::VerifiedAgent { agent, principal, signed_at } => VerifyLayerResult {
            agent: Some(agent.as_str().to_string()),
            principal: Some(principal.as_str().to_string()),
            signed_at: Some(signed_at.to_rfc3339()),
            ..VerifyLayerResult::empty("verifiedAgent")
        },
        S::OkUnverifiedAgent { signer, signed_at } => VerifyLayerResult {
            signer: Some(signer.as_str().to_string()),
            signed_at: Some(signed_at.to_rfc3339()),
            ..VerifyLayerResult::empty("okUnverifiedAgent")
        },
        S::Tampered { claimed_hash, computed_hash } => VerifyLayerResult {
            claimed_hash: Some(claimed_hash.to_string()),
            computed_hash: Some(computed_hash.to_string()),
            ..VerifyLayerResult::empty("tampered")
        },
        S::SignerUnresolvable { signer, cause } => VerifyLayerResult {
            signer: Some(signer.as_str().to_string()),
            cause: Some(cause.to_string()),
            ..VerifyLayerResult::empty("signerUnresolvable")
        },
        S::Invalid { signer } => VerifyLayerResult {
            signer: Some(signer.as_str().to_string()),
            ..VerifyLayerResult::empty("invalid")
        },
    }
}

fn map_stamp(out: &secretariat_core::application::VerifyOutcome) -> VerifyLayerResult {
    use secretariat_core::application::VerifyOutcome as V;
    match out {
        V::Unsigned => VerifyLayerResult::empty("none"),
        V::Verified { signer, stamped_at, act } => VerifyLayerResult {
            signer: Some(signer.as_str().to_string()),
            stamped_at: Some(stamped_at.to_rfc3339()),
            act: Some(format!("{act}")),
            ..VerifyLayerResult::empty("verified")
        },
        V::Tampered { claimed_hash, computed_hash } => VerifyLayerResult {
            claimed_hash: Some(claimed_hash.to_string()),
            computed_hash: Some(computed_hash.to_string()),
            ..VerifyLayerResult::empty("tampered")
        },
        V::SignerUnresolvable { signer, cause } => VerifyLayerResult {
            signer: Some(signer.as_str().to_string()),
            cause: Some(cause.to_string()),
            ..VerifyLayerResult::empty("signerUnresolvable")
        },
        V::SignatureInvalid { signer } => VerifyLayerResult {
            signer: Some(signer.as_str().to_string()),
            ..VerifyLayerResult::empty("signatureInvalid")
        },
    }
}

/// Inner, sync, testable core of `verify_envelope`. Resolves DIDs via the
/// same composite resolver the CLI uses, passing the local principal DID
/// (when present) so agent-self-loop short-circuits work, and the manifest
/// cache root so agent→principal binding can promote to VerifiedAgent.
fn verify_envelope_inner(file_path: String) -> Result<LayeredVerifyResult, String> {
    use secretariat_core::application::verify_document_layered;
    use secretariat_core::infrastructure::identity_store::load_identity;
    use secretariat_core::infrastructure::{CompositeDidResolver, DidWebResolver};

    let paths = KeyPaths::discover().map_err(|e| format!("resolving ~/.secretariat: {e}"))?;
    let resolver = CompositeDidResolver::new(DidWebResolver::new(paths.peers_cache.clone()));
    let local_did = load_identity(&paths.identity_md).ok().flatten().map(|id| id.did);

    let outcome = verify_document_layered(
        &std::path::PathBuf::from(&file_path),
        &resolver,
        local_did.as_ref(),
        Some(&paths.root),
    )
    .map_err(|e| format!("verifying {file_path}: {e}"))?;

    Ok(LayeredVerifyResult {
        signature: map_signature(&outcome.signature),
        stamp: map_stamp(&outcome.stamp),
    })
}

/// Layered verify for the front-end: author `$signature` + principal
/// `$attestation`, each reported independently. Read-only, no biometric
/// gate. AGENTS.md rule #5 ("verify before trusting") — the UI derives
/// its trust chip from this.
#[tauri::command]
#[specta::specta]
pub async fn verify_envelope(file_path: String) -> Result<LayeredVerifyResult, String> {
    tauri::async_runtime::spawn_blocking(move || verify_envelope_inner(file_path))
        .await
        .map_err(|e| format!("join error: {e}"))?
}
```

- [ ] **Step 5: Verify exports exist**

The mapping references `secretariat_core::application::{SignatureOutcome, VerifyOutcome, LayeredVerifyOutcome, verify_document_layered}` and `infrastructure::{CompositeDidResolver, DidWebResolver}`. Confirm they're re-exported (the CLI imports the same paths in `crates/cli/src/commands/verify.rs:13-17`):

Run: `rg -n 'SignatureOutcome|VerifyOutcome|verify_document_layered' crates/core/src/application/mod.rs`
Expected: all three appear in `pub use`. If `SignerRole::as_str()` or `DocHash`/`Did` `to_string()`/`as_str()` don't compile, check the value-object surfaces in `crates/core/src/domain/` and adjust the accessor (e.g. `.as_str()` vs `.to_string()`).

- [ ] **Step 6: Run test to verify it passes**

Run: `cargo test -p secretariat verify_tests::unsigned_document_maps_to_none_layers`
Expected: PASS.

- [ ] **Step 7: Run clippy on the crate**

Run: `cargo clippy -p secretariat -- -D warnings`
Expected: no warnings. Fix any (likely `needless_return` or unused imports).

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/commands/secretariat.rs
git commit -m "feat(tauri): verify_envelope command — layered signature+stamp for the frontend"
```

---

## Task 2: Register the command + regenerate TS bindings

**Files:**
- Modify: `src-tauri/src/bindings.rs:30` (add to `collect_commands!`)
- Generated: `src/lib/bindings.ts` (do not hand-edit; regenerated)

- [ ] **Step 1: Register the command**

In `src-tauri/src/bindings.rs`, add after line 30 (`secretariat::stamp_envelope,`):

```rust
        secretariat::verify_envelope,
```

- [ ] **Step 2: Regenerate bindings**

Run: `cargo test -p secretariat export_bindings -- --ignored`
Expected: `✓ TypeScript bindings exported to ../src/lib/bindings.ts`

- [ ] **Step 3: Confirm the binding landed**

Run: `rg -n 'verifyEnvelope|LayeredVerifyResult|VerifyLayerResult' src/lib/bindings.ts`
Expected: a `verifyEnvelope(filePath: string)` async fn and the two generated types appear.

- [ ] **Step 4: Typecheck the frontend**

Run: `pnpm tsc --noEmit` (or `pnpm typecheck` if defined in package.json)
Expected: no errors introduced by the new types.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/bindings.rs src/lib/bindings.ts
git commit -m "feat(bindings): register verify_envelope, regenerate TS bindings"
```

---

## Task 3: Frontend `TrustState` derivation + unit test (no UI yet)

**Files:**
- Create: `src/lib/markdown/trust.ts`
- Test: `src/lib/markdown/trust.test.ts`

**What it does:** Pure function mapping the `LayeredVerifyResult` to one coarse `TrustState`, per the derivation table in the reference section. Pure + tested now so every UI task downstream consumes a verified contract. This is the last task in PR #1 — it makes the verify keystone usable without any rendering yet.

- [ ] **Step 1: Write the failing test**

Create `src/lib/markdown/trust.test.ts`:

```ts
import { describe, expect, it } from 'vitest'
import { deriveTrustState } from './trust'
import type { LayeredVerifyResult } from '../bindings'

const layer = (outcome: string) => ({
  outcome, signer: null, signerRole: null, principal: null, agent: null,
  signedAt: null, stampedAt: null, act: null, claimedHash: null,
  computedHash: null, cause: null,
})
const result = (sig: string, stamp: string): LayeredVerifyResult => ({
  signature: layer(sig), stamp: layer(stamp),
})

describe('deriveTrustState', () => {
  it('sealed when signature ok and stamp verified', () => {
    expect(deriveTrustState(result('ok', 'verified'))).toBe('sealed')
  })
  it('signed when signature ok but stamp absent', () => {
    expect(deriveTrustState(result('ok', 'none'))).toBe('signed')
  })
  it('unsigned when both absent', () => {
    expect(deriveTrustState(result('none', 'none'))).toBe('unsigned')
  })
  it('tampered when signature tampered, regardless of stamp', () => {
    expect(deriveTrustState(result('tampered', 'verified'))).toBe('tampered')
  })
  it('tampered when stamp tampered', () => {
    expect(deriveTrustState(result('ok', 'tampered'))).toBe('tampered')
  })
  it('signed (not sealed) when signer unresolvable', () => {
    expect(deriveTrustState(result('signerUnresolvable', 'none'))).toBe('signed')
  })
  it('sealed when only the stamp layer is present', () => {
    expect(deriveTrustState(result('none', 'verified'))).toBe('sealed')
  })
})
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm vitest run src/lib/markdown/trust.test.ts`
Expected: FAIL — cannot find module `./trust`.

- [ ] **Step 3: Implement**

Create `src/lib/markdown/trust.ts`:

```ts
import type { LayeredVerifyResult, VerifyLayerResult } from '../bindings'

/** Coarse trust chip state — the spec's four-state vocabulary. */
export type TrustState = 'sealed' | 'signed' | 'unsigned' | 'tampered'

const SIG_OK = new Set(['ok', 'verifiedAgent', 'okUnverifiedAgent'])
const TAMPER = new Set(['tampered', 'signatureInvalid', 'invalid'])

/**
 * Derive the coarse trust state from the layered verify result.
 * Tampered wins over everything (AGENTS.md rule #5 — a failed-signature
 * doc is quarantined). `signerUnresolvable` is "can't confirm", not
 * "tampered" — it degrades to `signed`, never up to `sealed`.
 */
export function deriveTrustState(r: LayeredVerifyResult): TrustState {
  const sig: VerifyLayerResult = r.signature
  const stamp: VerifyLayerResult = r.stamp

  if (TAMPER.has(sig.outcome) || stamp.outcome === 'tampered') return 'tampered'

  const sealed = stamp.outcome === 'verified' && (sig.outcome === 'none' || SIG_OK.has(sig.outcome))
  if (sealed) return 'sealed'

  if (SIG_OK.has(sig.outcome) || sig.outcome === 'signerUnresolvable') return 'signed'

  return 'unsigned'
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm vitest run src/lib/markdown/trust.test.ts`
Expected: PASS (7 tests).

- [ ] **Step 5: Commit**

```bash
git add src/lib/markdown/trust.ts src/lib/markdown/trust.test.ts
git commit -m "feat(markdown): deriveTrustState — coarse four-state trust vocabulary"
```

> **PR #1 boundary.** Stop here for the first PR. Run the gates (`cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `pnpm vitest run`), open the PR with the plan + spec + Tasks 1-3. Tasks 4-12 below are the frontend redesign for follow-up PRs.

---

## Task 4: Trust design tokens (sober palette, retire amber-as-good)

**Files:**
- Modify: `src/theme-variables.css` (add `--trust-*` tokens to `:root` and `.dark`)

**What it does:** Defines the four sober trust colors as OKLCH tokens so no component hardcodes amber. Spec problem #3: a seal is an achievement of trust, not a hazard.

- [ ] **Step 1: Add tokens**

In `src/theme-variables.css`, inside the `:root { … }` block add:

```css
  /* Trust-state vocabulary — sober, never caution-amber for a good seal. */
  --trust-sealed: oklch(0.52 0.07 195);   /* deep teal/ink */
  --trust-sealed-fg: oklch(0.98 0.01 195);
  --trust-signed: oklch(0.55 0.02 250);   /* neutral slate */
  --trust-signed-fg: oklch(0.98 0.005 250);
  --trust-unsigned: oklch(0.65 0 0);      /* muted grey */
  --trust-unsigned-fg: oklch(0.99 0 0);
  --trust-tampered: oklch(0.55 0.20 25);  /* red — the only alarm */
  --trust-tampered-fg: oklch(0.99 0.02 25);
```

And the dark-mode counterparts inside `.dark { … }`:

```css
  --trust-sealed: oklch(0.70 0.09 195);
  --trust-sealed-fg: oklch(0.18 0.03 195);
  --trust-signed: oklch(0.72 0.02 250);
  --trust-signed-fg: oklch(0.18 0.01 250);
  --trust-unsigned: oklch(0.55 0 0);
  --trust-unsigned-fg: oklch(0.16 0 0);
  --trust-tampered: oklch(0.68 0.18 25);
  --trust-tampered-fg: oklch(0.16 0.03 25);
```

- [ ] **Step 2: Map into the Tailwind `@theme inline` block**

Where `theme-variables.css` maps CSS vars to Tailwind color names (`@theme inline { --color-…: var(--…) }`), add:

```css
  --color-trust-sealed: var(--trust-sealed);
  --color-trust-sealed-fg: var(--trust-sealed-fg);
  --color-trust-signed: var(--trust-signed);
  --color-trust-signed-fg: var(--trust-signed-fg);
  --color-trust-unsigned: var(--trust-unsigned);
  --color-trust-unsigned-fg: var(--trust-unsigned-fg);
  --color-trust-tampered: var(--trust-tampered);
  --color-trust-tampered-fg: var(--trust-tampered-fg);
```

This makes `bg-trust-sealed text-trust-sealed-fg` valid utilities.

- [ ] **Step 3: Verify the app still builds**

Run: `pnpm build` (Vite build; or `pnpm tauri:dev` briefly)
Expected: no CSS errors.

- [ ] **Step 4: Commit**

```bash
git add src/theme-variables.css
git commit -m "feat(theme): sober trust-state color tokens (retire amber-as-good)"
```

---

## Task 5: `TrustChip` component (coarse rung)

**Files:**
- Create: `src/components/markdown/TrustChip.tsx`
- Test: `src/components/markdown/TrustChip.test.tsx`

**What it does:** One chip, four states, sober palette. The always-visible coarse provenance rung. Glyphs: `✓ Sealed`, `◷ Signed`, `○ Unsigned`, `⚠ Tampered`.

- [ ] **Step 1: Write the failing test**

Create `src/components/markdown/TrustChip.test.tsx`:

```tsx
import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { TrustChip } from './TrustChip'

describe('TrustChip', () => {
  it('renders the sealed label', () => {
    render(<TrustChip state="sealed" />)
    expect(screen.getByText(/sealed/i)).toBeInTheDocument()
  })
  it('renders the tampered label with an alert role', () => {
    render(<TrustChip state="tampered" />)
    expect(screen.getByRole('status')).toHaveTextContent(/tampered/i)
  })
})
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm vitest run src/components/markdown/TrustChip.test.tsx`
Expected: FAIL — cannot find `./TrustChip`.

- [ ] **Step 3: Implement**

Create `src/components/markdown/TrustChip.tsx`:

```tsx
import { BadgeCheck, CircleDashed, Circle, TriangleAlert } from 'lucide-react'
import type { TrustState } from '@/lib/markdown/trust'

const CONFIG: Record<TrustState, { label: string; Icon: typeof BadgeCheck; cls: string }> = {
  sealed: { label: 'Sealed', Icon: BadgeCheck, cls: 'bg-trust-sealed text-trust-sealed-fg' },
  signed: { label: 'Signed', Icon: CircleDashed, cls: 'bg-trust-signed text-trust-signed-fg' },
  unsigned: { label: 'Unsigned', Icon: Circle, cls: 'bg-trust-unsigned text-trust-unsigned-fg' },
  tampered: { label: 'Tampered', Icon: TriangleAlert, cls: 'bg-trust-tampered text-trust-tampered-fg' },
}

export function TrustChip({ state }: { state: TrustState }) {
  const { label, Icon, cls } = CONFIG[state]
  return (
    <span
      role="status"
      className={`inline-flex h-7 items-center gap-1.5 rounded-full px-3 text-xs font-medium ${cls}`}
    >
      <Icon size={14} aria-hidden />
      {label}
    </span>
  )
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm vitest run src/components/markdown/TrustChip.test.tsx`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/components/markdown/TrustChip.tsx src/components/markdown/TrustChip.test.tsx
git commit -m "feat(markdown): TrustChip — coarse four-state trust rung"
```

---

## Task 6: `useVerify` hook — fetch + cache layered verify per file

**Files:**
- Create: `src/components/markdown/useVerify.ts`

**What it does:** Calls `commands.verifyEnvelope(filePath)` on mount and on demand (after save / stamp / external reload), exposes `{ verify, state, refresh, loading }`. Keeps `MarkdownWindow` thin.

- [ ] **Step 1: Implement**

Create `src/components/markdown/useVerify.ts`:

```ts
import { useCallback, useEffect, useState } from 'react'
import { commands, type LayeredVerifyResult } from '@/lib/bindings'
import { deriveTrustState, type TrustState } from '@/lib/markdown/trust'

export function useVerify(filePath: string) {
  const [verify, setVerify] = useState<LayeredVerifyResult | null>(null)
  const [loading, setLoading] = useState(false)

  const refresh = useCallback(async () => {
    setLoading(true)
    try {
      const res = await commands.verifyEnvelope(filePath)
      // tauri-specta Result: { status: 'ok', data } | { status: 'error', error }
      if (res.status === 'ok') setVerify(res.data)
      else setVerify(null)
    } finally {
      setLoading(false)
    }
  }, [filePath])

  useEffect(() => {
    void refresh()
  }, [refresh])

  const state: TrustState = verify ? deriveTrustState(verify) : 'unsigned'
  return { verify, state, refresh, loading }
}
```

> Confirm the `commands.verifyEnvelope` return shape against `src/lib/bindings.ts` (tauri-specta wraps in `{ status, data | error }`). Adjust the unwrap if the project's generated shape differs.

- [ ] **Step 2: Typecheck**

Run: `pnpm tsc --noEmit`
Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add src/components/markdown/useVerify.ts
git commit -m "feat(markdown): useVerify hook — layered verify per file"
```

---

## Task 7: Intent toggle in `MarkdownWindow` (Compose ⇄ Attend) with View Transition

**Files:**
- Modify: `src/components/markdown/MarkdownWindow.tsx`
- Modify: `src/components/markdown/MarkdownTitlebar.tsx` (add the mode toggle button + ambient save dot)

**What it does:** Adds `intent` state, a one-key toggle (⇥ / a button), wraps the body swap in React 19 `<ViewTransition>` for a calm cross-fade. Compose shows `CrepeEditor`; Attend shows a read-optimized render + `TrustBanner` (Task 8). Per spec open-question: in-component state, not separate routes — preserves scroll/identity.

- [ ] **Step 1: Add intent state + keybinding**

In `MarkdownWindow.tsx`, near the other `useState` (lines 53-61):

```tsx
const [intent, setIntent] = useState<'compose' | 'attend'>('compose')
const verify = useVerify(filePath)
```

Add a keydown handler (alongside the existing Cmd/Ctrl+R handler) toggling on the Tab key when not focused in the editor, or a dedicated shortcut (e.g. `Cmd/Ctrl+E`):

```tsx
useEffect(() => {
  const onKey = (e: KeyboardEvent) => {
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'e') {
      e.preventDefault()
      setIntent((m) => (m === 'compose' ? 'attend' : 'compose'))
    }
  }
  window.addEventListener('keydown', onKey)
  return () => window.removeEventListener('keydown', onKey)
}, [])
```

- [ ] **Step 2: Wrap the body in a View Transition**

Import at top: `import { unstable_ViewTransition as ViewTransition } from 'react'` (React 19.2; confirm the exact export name in the installed version — may be `ViewTransition` unprefixed). Replace the direct `<CrepeEditor … />` render with:

```tsx
<ViewTransition>
  {intent === 'compose' ? (
    <CrepeEditor key={editorKey} initialValue={body} onChange={onBodyChange} />
  ) : (
    <AttendView
      body={body}
      frontmatter={frontmatter}
      verify={verify}
      filePath={filePath}
      selfDid={selfDid}
      selfDisplayName={selfDisplayName}
      stamping={stamping}
      onStamp={onStamp}
    />
  )}
</ViewTransition>
```

`AttendView` is built in Task 8. After a successful `onStamp` or save, call `verify.refresh()`.

- [ ] **Step 3: Add the toggle button + ambient save dot to the titlebar**

In `MarkdownTitlebar.tsx`, extend props:

```tsx
interface MarkdownTitlebarProps {
  title: string
  saving: boolean
  filePath: string
  intent: 'compose' | 'attend'
  onToggleIntent: () => void
  trust: TrustState
  onReload: () => void
  reloading: boolean
}
```

Replace the inline `Saving…` text (lines ~43-45) with an ambient dot (Task 11 refines the pulse), and add a toggle button labeled `Attend ⇥` / `Compose ⇥` plus the `TrustChip` for at-a-glance state. Wire `intent`, `onToggleIntent`, `trust` from `MarkdownWindow`.

- [ ] **Step 4: Manual smoke (the app must run)**

Run: `pnpm tauri:dev`, open a markdown file, press `Cmd+E`.
Expected: body cross-fades between the Crepe editor and the read view; `TrustChip` reflects the file's real state.

- [ ] **Step 5: Commit**

```bash
git add src/components/markdown/MarkdownWindow.tsx src/components/markdown/MarkdownTitlebar.tsx
git commit -m "feat(markdown): Compose/Attend intent toggle with View Transition"
```

---

## Task 8: `AttendView` — read-optimized render + `TrustBanner` + relocated ceremony

**Files:**
- Create: `src/components/markdown/AttendView.tsx`
- Create: `src/components/markdown/TrustBanner.tsx`

**What it does:** The reading posture. Renders the body read-optimized (read-only Crepe config or a markdown renderer — decide at build per the spec open-question; default to read-only Crepe to keep typography identical), a `TrustBanner` at the top (coarse, expandable to medium→fine rungs), and the bounded end-of-document marker where `Seal this document` becomes available (unsealed) or the trust state is restated (sealed). The ceremony protocol is unchanged — hard rule #4 still governs: render full body verbatim (the reader IS reading it), explicit consent, Touch ID.

- [ ] **Step 1: Build `TrustBanner`**

Create `src/components/markdown/TrustBanner.tsx`: a banner using the `TrustChip` for the coarse rung, a `[details ▾]` disclosure that expands the medium rung (who sealed, when; signer; frontmatter `$type`) and the fine rung (signature hex, doc-hash preimage, `act`, a greyed **Counter — none (reserved)** row, and a copy-to-clipboard of the raw `LayeredVerifyResult` JSON). **Acceptance gate (spec):** the medium-rung copy is plain language, not lexicon jargon — e.g. "Signed by you, sealed 2026-06-01" not "did:key:z6Mk… signerRole=principal".

- [ ] **Step 2: Build `AttendView`**

Compose `TrustBanner` + read-only body + bounded end. When `state === 'tampered'`, dim the body and show a quarantine notice instead of the seal affordance (Task 12 refines). When `state === 'signed'` (unsealed), surface `[ Seal this document ]` at the bounded end, which calls the existing `onStamp` flow.

- [ ] **Step 3: Manual smoke**

Run: `pnpm tauri:dev`, switch to Attend on a signed-only doc.
Expected: banner shows `◷ Signed`, `[details ▾]` expands plain-language provenance, bounded end offers Seal.

- [ ] **Step 4: Commit**

```bash
git add src/components/markdown/AttendView.tsx src/components/markdown/TrustBanner.tsx
git commit -m "feat(markdown): AttendView + TrustBanner — provenance-forward reading posture"
```

---

## Task 9: Break-the-seal interstitial

**Files:**
- Modify: `src/components/markdown/MarkdownWindow.tsx`
- Create: `src/components/markdown/BreakSealDialog.tsx`

**What it does:** When the principal begins editing a sealed doc (Compose intent + `state === 'sealed'` + first body change), raise a calm interstitial: _"Editing breaks the current seal. The record reverts to signed-only until you re-stamp."_ — Confirm / Cancel. Not silent invalidation (spec Strategic Friction / construct ES-16).

- [ ] **Step 1: Build the dialog**

Create `BreakSealDialog.tsx` using the shadcn `AlertDialog` primitive. Calm copy, no red alarm (this is care, not error). Confirm → allow the edit + mark a local `sealBroken` flag so it doesn't re-prompt every keystroke. Cancel → revert the pending change and stay in Attend.

- [ ] **Step 2: Wire the guard in `MarkdownWindow`**

In `onBodyChange`, if `intent === 'compose' && verify.state === 'sealed' && !sealBroken`, intercept the first change: hold the pending value, open the dialog. On confirm, apply + set `sealBroken`; on cancel, drop it.

- [ ] **Step 3: Manual smoke**

Run: `pnpm tauri:dev`, open a sealed doc, edit a character.
Expected: interstitial appears once; confirm lets editing proceed; the chip will read `Signed` after the next verify refresh.

- [ ] **Step 4: Commit**

```bash
git add src/components/markdown/MarkdownWindow.tsx src/components/markdown/BreakSealDialog.tsx
git commit -m "feat(markdown): break-the-seal interstitial on editing a sealed doc"
```

---

## Task 10: Inline collapsed frontmatter summary (should-have #7)

**Files:**
- Modify: `src/components/markdown/MarkdownWindow.tsx`
- Create: `src/components/markdown/FrontmatterSummary.tsx`

**What it does:** A one-line legible summary at the foot of the Compose body (`type · $type · N agents`) that expands to the existing `FrontmatterPanel`. Frontmatter is part of reading the document's nature (spec problem #6), no longer hidden behind the offcanvas as the only access.

- [ ] **Step 1: Build `FrontmatterSummary`** rendering a compact line from the frontmatter object, with an expand affordance that opens the existing sidebar panel.
- [ ] **Step 2: Mount it** at the foot of the Compose body in `MarkdownWindow`.
- [ ] **Step 3: Manual smoke** — summary shows, expands to the full field editor.
- [ ] **Step 4: Commit**

```bash
git add src/components/markdown/FrontmatterSummary.tsx src/components/markdown/MarkdownWindow.tsx
git commit -m "feat(markdown): inline collapsed frontmatter summary"
```

---

## Task 11: Ambient save glyph (should-have #5)

**Files:**
- Modify: `src/components/markdown/MarkdownTitlebar.tsx`

**What it does:** Replace the flickering `Saving…` text with a single ambient glyph: calm `● synced` when idle, a slow (not anxious) pulse while a save is pending/in-flight. Reflects **disk only** — never implies a server (spec Sovereignty / Convivial Infrastructure). Use a CSS animation with a long period (e.g. 2s ease-in-out), not a spinner.

- [ ] **Step 1: Implement the glyph** keyed off the existing `saving` flag; add a `@keyframes trust-pulse` (slow opacity breathe) to `markdown-window.css`.
- [ ] **Step 2: Manual smoke** — type, watch the dot breathe, settle to solid `synced`.
- [ ] **Step 3: Commit**

```bash
git add src/components/markdown/MarkdownTitlebar.tsx src/markdown-window.css
git commit -m "feat(markdown): ambient save glyph replacing 'Saving…' churn"
```

---

## Task 12: Nice-to-haves — Edit-raw, bounded-end marker, tamper quarantine

**Files:**
- Modify: `src/components/markdown/MarkdownTitlebar.tsx` (Edit raw button)
- Modify: `src/components/markdown/AttendView.tsx` (bounded end + quarantine)
- Possibly modify: `src-tauri/src/commands/secretariat.rs` or `settings.rs` (reuse `reveal_in_finder`; add an `open_in_default_editor` command if none exists)

**What it does:** (8) "Edit raw" escape hatch — open the underlying `.md` in the system editor (Modification Rights). (9) Bounded end-of-document marker — a centered hairline + restated trust state, never an autoplay next-doc. (10) Tamper quarantine — when `state === 'tampered'`, dim the body and block editing until re-derived (makes AGENTS.md rule #5 a UI invariant).

- [ ] **Step 1: Edit-raw** — check for an existing open-in-editor command; if absent, add `open_in_default_editor(path)` mirroring `reveal_in_finder`, register + regenerate bindings, wire a titlebar button.
- [ ] **Step 2: Bounded end** — centered hairline + "signed-only — not yet sealed" / "sealed by you · <date>" in `AttendView`.
- [ ] **Step 3: Tamper quarantine** — dim + `pointer-events-none` on the body, a quarantine banner, edit blocked, when `state === 'tampered'`.
- [ ] **Step 4: Manual smoke** each.
- [ ] **Step 5: Commit** (one commit per sub-step is fine).

```bash
git commit -m "feat(markdown): edit-raw hatch, bounded-end marker, tamper quarantine"
```

---

## Final verification (before each PR)

- [ ] `cargo test --workspace` — green
- [ ] `cargo clippy --workspace -- -D warnings` — clean
- [ ] `pnpm vitest run` — green
- [ ] `pnpm tsc --noEmit` — clean
- [ ] `pnpm tauri:dev` smoke of the slice the PR ships
- [ ] Lexicon check: this redesign changes **no record shapes** (it reads existing `$signature`/`$attestation`, adds no fields) — so AGENTS.md hard rule #3 needs no `lexicons/` diff. Confirm no Rust record struct gained/lost a field.

## Self-review notes (gaps & decisions surfaced)

- **Spec said `{signature, stamp, counter_stamps}`; reality is `{signature, stamp}`.** Counter-stamp ships nothing — rendered as a static greyed "reserved" row (Task 8), fed by no data. Flagged in the open-questions; resolved here.
- **`signerUnresolvable` precedence.** Not in the spec's 4-state table explicitly; mapped to `signed` (informational, "can't confirm") rather than `tampered`. Documented in the derivation table + tested (Task 3).
- **ViewTransition export name** varies by React 19 minor (`unstable_ViewTransition` vs `ViewTransition`). Task 7 step 2 flags confirming against the installed version — see the `vercel-react-view-transitions` skill / React 19 release notes.
- **Read-mode renderer** (spec open-question): default to read-only Crepe config so typography is identical between intents; revisit if Crepe read-only proves glitchy in 7.x (the codebase already notes 7.x quirks in `CrepeEditor.tsx`).
