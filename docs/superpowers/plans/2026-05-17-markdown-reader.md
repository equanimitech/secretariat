# Markdown reader/editor — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship Secretariat as the principal's default macOS markdown reader/editor. Each `.md` opens in its own Tauri window with Milkdown Crepe WYSIWYG body, a structured frontmatter form, a pretty title bar, and a stamp button.

**Architecture:** New Tauri webview entry `markdown-window.html` + `src/markdown-window-main.tsx`. Rust module `src-tauri/src/commands/markdown.rs` exposes `read_markdown` / `write_markdown` / `open_markdown_window` / `take_pending_opens`. macOS file-open routed via `RunEvent::Opened` → `PendingOpens` buffer → frontend drain on `main` window ready (and on single-instance reentry). YAML frontmatter parsed JS-side with `gray-matter`; body round-trips through Crepe.

**Tech Stack:** Tauri 2, React 19, Vite (rolldown), Milkdown Crepe, `gray-matter`, Vitest, tauri-specta.

***

## File Structure

**Create — frontend:**

* `markdown-window.html` — Vite entry HTML (mirrors `quick-pane.html`)

* `src/markdown-window-main.tsx` — React root

* `src/markdown-window.css` — entry styles

* `src/components/markdown/MarkdownWindow.tsx` — top-level layout + data flow

* `src/components/markdown/CrepeEditor.tsx` — Milkdown/Crepe React wrapper

* `src/components/markdown/FrontmatterPanel.tsx` — collapsible FM form

* `src/components/markdown/FrontmatterField.tsx` — per-row type-dispatched input

* `src/components/markdown/MarkdownTitlebar.tsx` — pretty title + stamp button

* `src/components/markdown/StampDialog.tsx` — verbatim-body consent modal

* `src/lib/markdown/parse.ts` — `gray-matter` wrapper + types

* `src/lib/markdown/parse.test.ts`

* `src/lib/markdown/title.ts` — title resolution rules

* `src/lib/markdown/title.test.ts`

* `src/lib/markdown/field-type.ts` — FM value-type inference

* `src/lib/markdown/field-type.test.ts`

* `src/lib/markdown/open.ts` — IPC to `open_markdown_window`

**Create — backend:**

* `src-tauri/src/commands/markdown.rs` — Tauri commands

* `src-tauri/src/markdown/mod.rs` — module root

* `src-tauri/src/markdown/pending.rs` — `PendingOpens` state buffer

* `src-tauri/src/markdown/file_io.rs` — atomic read/write helpers

* `src-tauri/tests/markdown_round_trip.rs` — integration test

**Modify:**

* `package.json` — add `@milkdown/crepe`, `@milkdown/core`, `gray-matter`

* `src-tauri/Cargo.toml` — add `sha2`, `sha1`, `urlencoding` (verify which already pulled in transitively)

* `vite.config.ts` — add `markdown-window` to `rolldownOptions.input`

* `src-tauri/src/lib.rs` — register `PendingOpens` state, add `RunEvent::Opened` arm, wire single-instance argv parsing

* `src-tauri/src/commands/mod.rs` — `pub mod markdown;`

* `src-tauri/src/bindings.rs` — add markdown commands to `collect_commands!`

* `src-tauri/tauri.macos.conf.json` — add `bundle.fileAssociations`

***

## Task 1: Add dependencies

**Files:**

* Modify: `package.json`

* Modify: `src-tauri/Cargo.toml`

* [ ] **Step 1: Install JS deps**

```bash
pnpm add @milkdown/crepe @milkdown/core gray-matter
```

* [ ] **Step 2: Add Rust deps**

Append under `[dependencies]` in `src-tauri/Cargo.toml`:

```toml
sha2 = "0.10"
sha1 = "0.10"
urlencoding = "2"
```

* [ ] **Step 3: Verify Rust build**

Run: `pnpm rust:fmt:check && cd src-tauri && cargo check`
Expected: success.

* [ ] **Step 4: Commit**

```bash
git add package.json pnpm-lock.yaml src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "deps: milkdown/crepe, gray-matter, sha2/sha1/urlencoding"
```

***

## Task 2: Frontmatter type inference (TDD)

**Files:**

* Create: `src/lib/markdown/field-type.ts`

* Create: `src/lib/markdown/field-type.test.ts`

* [ ] **Step 1: Write failing tests**

```ts
// src/lib/markdown/field-type.test.ts
import { describe, it, expect } from 'vitest'
import { inferFieldType } from './field-type'

describe('inferFieldType', () => {
  it('classifies short string as text', () => {
    expect(inferFieldType('hello')).toBe('text')
  })
  it('classifies multiline string as multiline', () => {
    expect(inferFieldType('line1\nline2')).toBe('multiline')
  })
  it('classifies boolean as boolean', () => {
    expect(inferFieldType(true)).toBe('boolean')
    expect(inferFieldType(false)).toBe('boolean')
  })
  it('classifies ISO date string as date', () => {
    expect(inferFieldType('2026-05-17')).toBe('date')
    expect(inferFieldType('2026-05-17T10:30:00Z')).toBe('date')
  })
  it('classifies array of scalars as list', () => {
    expect(inferFieldType(['a', 'b'])).toBe('list')
  })
  it('classifies number as number', () => {
    expect(inferFieldType(42)).toBe('number')
  })
  it('classifies plain object as nested', () => {
    expect(inferFieldType({ k: 1 })).toBe('nested')
  })
  it('classifies null/undefined as text', () => {
    expect(inferFieldType(null)).toBe('text')
    expect(inferFieldType(undefined)).toBe('text')
  })
})
```

* [ ] **Step 2: Run, verify fail**

Run: `pnpm test:run src/lib/markdown/field-type.test.ts`
Expected: FAIL (module not found).

* [ ] **Step 3: Implement**

```ts
// src/lib/markdown/field-type.ts
export type FieldType =
  | 'text'
  | 'multiline'
  | 'boolean'
  | 'date'
  | 'list'
  | 'number'
  | 'nested'

const ISO_DATE = /^\d{4}-\d{2}-\d{2}(T\d{2}:\d{2}(:\d{2})?(\.\d+)?(Z|[+-]\d{2}:?\d{2})?)?$/

export function inferFieldType(value: unknown): FieldType {
  if (value === null || value === undefined) return 'text'
  if (typeof value === 'boolean') return 'boolean'
  if (typeof value === 'number') return 'number'
  if (Array.isArray(value)) return 'list'
  if (typeof value === 'object') return 'nested'
  if (typeof value === 'string') {
    if (value.includes('\n')) return 'multiline'
    if (ISO_DATE.test(value)) return 'date'
    return 'text'
  }
  return 'text'
}
```

* [ ] **Step 4: Run, verify pass**

Run: `pnpm test:run src/lib/markdown/field-type.test.ts`
Expected: PASS.

* [ ] **Step 5: Commit**

```bash
git add src/lib/markdown/field-type.ts src/lib/markdown/field-type.test.ts
git commit -m "feat(markdown): field type inference for frontmatter UI"
```

***

## Task 3: gray-matter parse wrapper (TDD)

**Files:**

* Create: `src/lib/markdown/parse.ts`

* Create: `src/lib/markdown/parse.test.ts`

* [ ] **Step 1: Write failing tests**

```ts
// src/lib/markdown/parse.test.ts
import { describe, it, expect } from 'vitest'
import { parseMarkdown, serializeMarkdown } from './parse'

describe('parseMarkdown', () => {
  it('parses file with frontmatter', () => {
    const src = '---\ntitle: Hello\ntags: [a, b]\n---\n# Body\n\nText.'
    const { frontmatter, body } = parseMarkdown(src)
    expect(frontmatter).toEqual({ title: 'Hello', tags: ['a', 'b'] })
    expect(body).toBe('# Body\n\nText.')
  })

  it('returns empty frontmatter when none present', () => {
    const { frontmatter, body } = parseMarkdown('# Just body')
    expect(frontmatter).toEqual({})
    expect(body).toBe('# Just body')
  })

  it('preserves unknown keys', () => {
    const src = '---\ncustom_field: value\n---\nBody'
    const { frontmatter } = parseMarkdown(src)
    expect(frontmatter).toEqual({ custom_field: 'value' })
  })
})

describe('serializeMarkdown', () => {
  it('round-trips frontmatter + body', () => {
    const src = '---\ntitle: Hello\n---\nBody.'
    const { frontmatter, body } = parseMarkdown(src)
    const out = serializeMarkdown(frontmatter, body)
    expect(parseMarkdown(out)).toEqual({ frontmatter, body })
  })

  it('omits frontmatter delimiters when empty', () => {
    const out = serializeMarkdown({}, '# Body')
    expect(out).toBe('# Body')
  })
})
```

* [ ] **Step 2: Run, verify fail**

Run: `pnpm test:run src/lib/markdown/parse.test.ts`
Expected: FAIL.

* [ ] **Step 3: Implement**

```ts
// src/lib/markdown/parse.ts
import matter from 'gray-matter'

export type Frontmatter = Record<string, unknown>

export interface ParsedMarkdown {
  frontmatter: Frontmatter
  body: string
}

export function parseMarkdown(source: string): ParsedMarkdown {
  const parsed = matter(source)
  return {
    frontmatter: parsed.data ?? {},
    body: parsed.content.replace(/^\n+/, ''),
  }
}

export function serializeMarkdown(frontmatter: Frontmatter, body: string): string {
  if (Object.keys(frontmatter).length === 0) {
    return body
  }
  return matter.stringify(body, frontmatter)
}
```

* [ ] **Step 4: Run, verify pass**

Run: `pnpm test:run src/lib/markdown/parse.test.ts`
Expected: PASS.

* [ ] **Step 5: Commit**

```bash
git add src/lib/markdown/parse.ts src/lib/markdown/parse.test.ts
git commit -m "feat(markdown): gray-matter parse/serialize wrapper"
```

***

## Task 4: Title resolution (TDD)

**Files:**

* Create: `src/lib/markdown/title.ts`

* Create: `src/lib/markdown/title.test.ts`

* [ ] **Step 1: Write failing tests**

```ts
// src/lib/markdown/title.test.ts
import { describe, it, expect } from 'vitest'
import { resolveTitle } from './title'

describe('resolveTitle', () => {
  it('prefers frontmatter.title when set', () => {
    expect(resolveTitle({ title: 'FM' }, '# Body H1', '/x/file.md')).toBe('FM')
  })
  it('falls back to first H1 in body', () => {
    expect(resolveTitle({}, '# Heading\n\nText', '/x/file.md')).toBe('Heading')
  })
  it('skips inline #-prefixed text that is not a heading', () => {
    expect(resolveTitle({}, 'intro\n# Real Heading', '/x/file.md')).toBe('Real Heading')
  })
  it('falls back to file basename without extension', () => {
    expect(resolveTitle({}, 'plain text', '/x/notes/my-file.md')).toBe('my-file')
  })
  it('returns basename when title is empty string', () => {
    expect(resolveTitle({ title: '' }, '', '/x/f.md')).toBe('f')
  })
  it('handles markdown extension variants', () => {
    expect(resolveTitle({}, '', '/x/f.markdown')).toBe('f')
    expect(resolveTitle({}, '', '/x/f.mdown')).toBe('f')
  })
})
```

* [ ] **Step 2: Run, verify fail**

Run: `pnpm test:run src/lib/markdown/title.test.ts`
Expected: FAIL.

* [ ] **Step 3: Implement**

```ts
// src/lib/markdown/title.ts
import type { Frontmatter } from './parse'

const H1 = /^#\s+(.+)$/m

export function resolveTitle(
  frontmatter: Frontmatter,
  body: string,
  filePath: string,
): string {
  const fmTitle = frontmatter.title
  if (typeof fmTitle === 'string' && fmTitle.trim().length > 0) {
    return fmTitle.trim()
  }
  const match = body.match(H1)
  if (match) return match[1].trim()
  return basenameWithoutExt(filePath)
}

function basenameWithoutExt(p: string): string {
  const base = p.split('/').pop() ?? p
  return base.replace(/\.(md|markdown|mdown|mkd)$/i, '')
}
```

* [ ] **Step 4: Run, verify pass**

Run: `pnpm test:run src/lib/markdown/title.test.ts`
Expected: PASS.

* [ ] **Step 5: Commit**

```bash
git add src/lib/markdown/title.ts src/lib/markdown/title.test.ts
git commit -m "feat(markdown): title resolution (fm → h1 → basename)"
```

***

## Task 5: Rust `markdown::file_io` — atomic read/write (TDD)

**Files:**

* Create: `src-tauri/src/markdown/mod.rs`

* Create: `src-tauri/src/markdown/file_io.rs`

* [ ] **Step 1: Register module**

Add to `src-tauri/src/lib.rs` near other `mod` declarations (after `mod commands;`):

```rust
mod markdown;
```

Create `src-tauri/src/markdown/mod.rs`:

```rust
pub mod file_io;
pub mod pending;
```

* [ ] **Step 2: Write failing test**

```rust
// src-tauri/src/markdown/file_io.rs
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn read_returns_content_and_sha256() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("a.md");
        std::fs::write(&path, b"hello world").unwrap();
        let result = read_file(&path).unwrap();
        assert_eq!(result.content, "hello world");
        assert_eq!(result.sha256.len(), 64);
    }

    #[test]
    fn write_succeeds_when_expected_sha_matches() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("b.md");
        std::fs::write(&path, b"v1").unwrap();
        let first = read_file(&path).unwrap();
        let new_sha = write_file(&path, "v2", &first.sha256).unwrap();
        assert_ne!(new_sha, first.sha256);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "v2");
    }

    #[test]
    fn write_rejects_when_disk_sha_diverged() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("c.md");
        std::fs::write(&path, b"orig").unwrap();
        let stale = "0".repeat(64);
        let err = write_file(&path, "new", &stale).unwrap_err();
        assert!(matches!(err, WriteError::Conflict { .. }));
    }
}
```

* [ ] **Step 3: Run, verify fail**

Add `tempfile = "3"` to `src-tauri/Cargo.toml` `[dev-dependencies]`.
Run: `cd src-tauri && cargo test markdown::file_io -- --nocapture`
Expected: FAIL (functions don't exist).

* [ ] **Step 4: Implement**

```rust
// src-tauri/src/markdown/file_io.rs (replace the test-module-only file)
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

pub struct ReadResult {
    pub content: String,
    pub sha256: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ReadError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("not utf-8")]
    NotUtf8,
}

#[derive(Debug, thiserror::Error)]
pub enum WriteError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("file changed on disk")]
    Conflict { current_sha256: String },
}

pub fn read_file(path: &Path) -> Result<ReadResult, ReadError> {
    let bytes = fs::read(path)?;
    let content = String::from_utf8(bytes).map_err(|_| ReadError::NotUtf8)?;
    let sha256 = hash(content.as_bytes());
    Ok(ReadResult { content, sha256 })
}

pub fn write_file(
    path: &Path,
    new_content: &str,
    expected_sha256: &str,
) -> Result<String, WriteError> {
    if path.exists() {
        let current = fs::read(path)?;
        let current_sha = hash(&current);
        if current_sha != expected_sha256 {
            return Err(WriteError::Conflict { current_sha256: current_sha });
        }
    }
    let tmp = path.with_extension("md.tmp");
    fs::write(&tmp, new_content.as_bytes())?;
    fs::rename(&tmp, path)?;
    Ok(hash(new_content.as_bytes()))
}

fn hash(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn read_returns_content_and_sha256() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("a.md");
        std::fs::write(&path, b"hello world").unwrap();
        let result = read_file(&path).unwrap();
        assert_eq!(result.content, "hello world");
        assert_eq!(result.sha256.len(), 64);
    }

    #[test]
    fn write_succeeds_when_expected_sha_matches() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("b.md");
        std::fs::write(&path, b"v1").unwrap();
        let first = read_file(&path).unwrap();
        let new_sha = write_file(&path, "v2", &first.sha256).unwrap();
        assert_ne!(new_sha, first.sha256);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "v2");
    }

    #[test]
    fn write_rejects_when_disk_sha_diverged() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("c.md");
        std::fs::write(&path, b"orig").unwrap();
        let stale = "0".repeat(64);
        let err = write_file(&path, "new", &stale).unwrap_err();
        assert!(matches!(err, WriteError::Conflict { .. }));
    }
}
```

Also add `thiserror = "1"` to `[dependencies]` in `src-tauri/Cargo.toml` if not present.

* [ ] **Step 5: Run, verify pass**

Run: `cd src-tauri && cargo test markdown::file_io`
Expected: 3 PASS.

* [ ] **Step 6: Commit**

```bash
git add src-tauri/src/markdown/ src-tauri/src/lib.rs src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "feat(markdown): atomic file I/O with sha256 concurrency"
```

***

## Task 6: Rust `PendingOpens` buffer (TDD)

**Files:**

* Create: `src-tauri/src/markdown/pending.rs`

* [ ] **Step 1: Write failing test**

```rust
// src-tauri/src/markdown/pending.rs
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn drain_returns_and_clears() {
        let p = PendingOpens::default();
        p.push(PathBuf::from("/a.md"));
        p.push(PathBuf::from("/b.md"));
        let drained = p.drain();
        assert_eq!(drained.len(), 2);
        assert!(p.drain().is_empty());
    }

    #[test]
    fn deduplicates_on_push() {
        let p = PendingOpens::default();
        p.push(PathBuf::from("/a.md"));
        p.push(PathBuf::from("/a.md"));
        assert_eq!(p.drain().len(), 1);
    }
}
```

* [ ] **Step 2: Run, verify fail**

Run: `cd src-tauri && cargo test markdown::pending`
Expected: FAIL.

* [ ] **Step 3: Implement**

```rust
// src-tauri/src/markdown/pending.rs
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Default)]
pub struct PendingOpens(Mutex<Vec<PathBuf>>);

impl PendingOpens {
    pub fn push(&self, path: PathBuf) {
        let mut g = self.0.lock().unwrap();
        if !g.contains(&path) {
            g.push(path);
        }
    }

    pub fn drain(&self) -> Vec<PathBuf> {
        let mut g = self.0.lock().unwrap();
        std::mem::take(&mut *g)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn drain_returns_and_clears() {
        let p = PendingOpens::default();
        p.push(PathBuf::from("/a.md"));
        p.push(PathBuf::from("/b.md"));
        let drained = p.drain();
        assert_eq!(drained.len(), 2);
        assert!(p.drain().is_empty());
    }

    #[test]
    fn deduplicates_on_push() {
        let p = PendingOpens::default();
        p.push(PathBuf::from("/a.md"));
        p.push(PathBuf::from("/a.md"));
        assert_eq!(p.drain().len(), 1);
    }
}
```

* [ ] **Step 4: Run, verify pass**

Run: `cd src-tauri && cargo test markdown::pending`
Expected: 2 PASS.

* [ ] **Step 5: Commit**

```bash
git add src-tauri/src/markdown/pending.rs
git commit -m "feat(markdown): PendingOpens buffer for RunEvent::Opened"
```

***

## Task 7: Tauri commands — read/write/open/take\_pending

**Files:**

* Create: `src-tauri/src/commands/markdown.rs`

* Modify: `src-tauri/src/commands/mod.rs`

* Modify: `src-tauri/src/bindings.rs`

* Modify: `src-tauri/src/lib.rs`

* [ ] **Step 1: Implement commands**

Create `src-tauri/src/commands/markdown.rs`:

```rust
//! Tauri commands for the markdown reader/editor.
use crate::markdown::{file_io, pending::PendingOpens};
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use specta::Type;
use std::path::PathBuf;
use tauri::{AppHandle, Manager, State, WebviewUrl, WebviewWindowBuilder};

#[derive(Serialize, Type)]
pub struct ReadMarkdownResult {
    pub content: String,
    pub sha256: String,
}

#[derive(Deserialize, Type)]
pub struct WriteMarkdownArgs {
    pub path: String,
    pub content: String,
    pub expected_sha256: String,
}

#[derive(Serialize, Type)]
#[serde(tag = "kind")]
pub enum WriteMarkdownResult {
    Ok { sha256: String },
    Conflict { current_sha256: String },
}

#[tauri::command]
#[specta::specta]
pub fn read_markdown(path: String) -> Result<ReadMarkdownResult, String> {
    let r = file_io::read_file(&PathBuf::from(&path)).map_err(|e| e.to_string())?;
    Ok(ReadMarkdownResult { content: r.content, sha256: r.sha256 })
}

#[tauri::command]
#[specta::specta]
pub fn write_markdown(args: WriteMarkdownArgs) -> Result<WriteMarkdownResult, String> {
    match file_io::write_file(&PathBuf::from(&args.path), &args.content, &args.expected_sha256) {
        Ok(sha256) => Ok(WriteMarkdownResult::Ok { sha256 }),
        Err(file_io::WriteError::Conflict { current_sha256 }) => {
            Ok(WriteMarkdownResult::Conflict { current_sha256 })
        }
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn open_markdown_window(app: AppHandle, path: String) -> Result<String, String> {
    let label = window_label(&path);
    if let Some(existing) = app.get_webview_window(&label) {
        existing.set_focus().map_err(|e| e.to_string())?;
        return Ok(label);
    }
    let encoded = urlencoding::encode(&path);
    let url = format!("markdown-window.html?path={encoded}");
    WebviewWindowBuilder::new(&app, &label, WebviewUrl::App(url.into()))
        .title("Markdown")
        .inner_size(900.0, 700.0)
        .min_inner_size(600.0, 500.0)
        .resizable(true)
        .build()
        .map_err(|e| e.to_string())?;
    Ok(label)
}

#[tauri::command]
#[specta::specta]
pub fn take_pending_opens(pending: State<'_, PendingOpens>) -> Vec<String> {
    pending
        .drain()
        .into_iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect()
}

fn window_label(path: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(path.as_bytes());
    let hex = format!("{:x}", hasher.finalize());
    format!("md:{}", &hex[..12])
}
```

* [ ] **Step 2: Register module**

Append to `src-tauri/src/commands/mod.rs`:

```rust
pub mod markdown;
```

* [ ] **Step 3: Register state + commands**

Edit `src-tauri/src/lib.rs`:

1. Add to `use` block: `use crate::markdown::pending::PendingOpens;`
2. Right after `let mut app_builder = tauri::Builder::default();` (or after single-instance, but before plugin chain runs), add:

```rust
app_builder = app_builder.manage(PendingOpens::default());
```

Edit `src-tauri/src/bindings.rs` — extend the `use` block and add to `collect_commands!`:

```rust
use crate::commands::{
    markdown, notifications, preferences, quick_pane, recovery, secretariat, settings,
};
// ...
collect_commands![
    // ... existing ...
    markdown::read_markdown,
    markdown::write_markdown,
    markdown::open_markdown_window,
    markdown::take_pending_opens,
]
```

* [ ] **Step 4: Regenerate TS bindings**

Run: `pnpm rust:bindings`
Expected: `src/lib/bindings.ts` updated with new commands.

* [ ] **Step 5: Verify build**

Run: `pnpm rust:clippy && pnpm typecheck`
Expected: success.

* [ ] **Step 6: Commit**

```bash
git add src-tauri/src/commands/markdown.rs src-tauri/src/commands/mod.rs src-tauri/src/bindings.rs src-tauri/src/lib.rs src/lib/bindings.ts
git commit -m "feat(markdown): Tauri commands (read/write/open/take_pending)"
```

***

## Task 8: macOS file association config

**Files:**

* Modify: `src-tauri/tauri.macos.conf.json`

* [ ] **Step 1: Inspect current macOS config**

Run: `cat src-tauri/tauri.macos.conf.json`
Note the structure — likely empty/minimal overrides on top of `tauri.conf.json`.

* [ ] **Step 2: Add file associations**

Replace contents of `src-tauri/tauri.macos.conf.json` (merging with whatever exists; below is the relevant addition):

```json
{
  "bundle": {
    "fileAssociations": [
      {
        "ext": ["md", "markdown", "mdown", "mkd"],
        "name": "Markdown Document",
        "description": "Markdown text document",
        "role": "Editor",
        "rank": "Alternate",
        "mimeType": "text/markdown"
      }
    ]
  }
}
```

Rationale on `rank: "Alternate"`: ship Secretariat as an *available* editor for `.md` without auto-claiming the default. Onboarding card (later task) walks the user through setting default via Finder.

* [ ] **Step 3: Build to verify Info.plist generation**

Run: `pnpm tauri:build --debug` (or `pnpm tauri:check`).
Expected: build succeeds; inspect `src-tauri/target/debug/bundle/macos/Secretariat.app/Contents/Info.plist` for `CFBundleDocumentTypes` containing `md`.

* [ ] **Step 4: Commit**

```bash
git add src-tauri/tauri.macos.conf.json
git commit -m "feat(markdown): declare .md/.markdown file association on macOS"
```

***

## Task 9: Wire `RunEvent::Opened` + single-instance argv

**Files:**

* Modify: `src-tauri/src/lib.rs`

* [ ] **Step 1: Extend the run-event callback**

In `src-tauri/src/lib.rs`, inside the `.run(|app_handle, event| match &event { ... })` block, add a new arm before the `_ => {}` (or before `RunEvent::Exit`):

```rust
RunEvent::Opened { urls } => {
    let pending = app_handle.state::<PendingOpens>();
    for url in urls {
        if let Ok(path) = url.to_file_path() {
            log::info!("RunEvent::Opened received: {}", path.display());
            pending.push(path);
        }
    }
    // Notify any open frontend windows
    let _ = app_handle.emit("markdown://pending-opens-added", ());
}
```

You need `tauri::Emitter` in scope — add `use tauri::Emitter;` near the existing `use tauri::Manager;`.

* [ ] **Step 2: Wire single-instance argv parsing**

Replace the single-instance plugin init in `src-tauri/src/lib.rs`:

```rust
app_builder = app_builder.plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
    use tauri::Emitter;
    // First arg is the program path; remaining args may be file paths
    let pending = app.state::<crate::markdown::pending::PendingOpens>();
    let mut added_any = false;
    for arg in args.iter().skip(1) {
        let p = std::path::PathBuf::from(arg);
        if p.exists() {
            pending.push(p);
            added_any = true;
        }
    }
    if added_any {
        let _ = app.emit("markdown://pending-opens-added", ());
    }
    surface_main_window(app);
}));
```

* [ ] **Step 3: Verify Rust compiles**

Run: `pnpm rust:clippy`
Expected: success.

* [ ] **Step 4: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat(markdown): route RunEvent::Opened + single-instance argv to PendingOpens"
```

***

## Task 10: Vite multi-entry for markdown window

**Files:**

* Create: `markdown-window.html`

* Create: `src/markdown-window-main.tsx`

* Create: `src/markdown-window.css`

* Modify: `vite.config.ts`

* [ ] **Step 1: Create entry HTML**

```html
<!-- markdown-window.html -->
<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <link rel="icon" type="image/svg+xml" href="/vite.svg" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Markdown</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/markdown-window-main.tsx"></script>
  </body>
</html>
```

* [ ] **Step 2: Create entry CSS**

```css
/* src/markdown-window.css */
@import 'tailwindcss';

html, body, #root {
  height: 100%;
  margin: 0;
}
```

* [ ] **Step 3: Create entry TSX (stub for now)**

```tsx
// src/markdown-window-main.tsx
import React from 'react'
import ReactDOM from 'react-dom/client'
import './markdown-window.css'
import { MarkdownWindow } from './components/markdown/MarkdownWindow'
import { ThemeProvider } from './components/ThemeProvider'

const params = new URLSearchParams(window.location.search)
const path = params.get('path') ?? ''

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <ThemeProvider>
      <MarkdownWindow filePath={decodeURIComponent(path)} />
    </ThemeProvider>
  </React.StrictMode>,
)
```

(`MarkdownWindow` is built in Task 14; this file references it but won't compile until then. Skip running typecheck here.)

* [ ] **Step 4: Add to Vite input map**

In `vite.config.ts`, change `rolldownOptions.input` to include the new entry:

```ts
rolldownOptions: {
  input: {
    main: resolve(__dirname, 'index.html'),
    'quick-pane': resolve(__dirname, 'quick-pane.html'),
    'markdown-window': resolve(__dirname, 'markdown-window.html'),
  },
},
```

* [ ] **Step 5: Commit**

```bash
git add markdown-window.html src/markdown-window-main.tsx src/markdown-window.css vite.config.ts
git commit -m "feat(markdown): vite multi-entry for markdown-window"
```

***

## Task 11: `CrepeEditor` React wrapper

**Files:**

* Create: `src/components/markdown/CrepeEditor.tsx`

* [ ] **Step 1: Implement**

```tsx
// src/components/markdown/CrepeEditor.tsx
import { useEffect, useRef } from 'react'
import { Crepe } from '@milkdown/crepe'
import '@milkdown/crepe/theme/common/style.css'
import '@milkdown/crepe/theme/frame.css'

interface CrepeEditorProps {
  initialValue: string
  onChange: (markdown: string) => void
}

export function CrepeEditor({ initialValue, onChange }: CrepeEditorProps) {
  const hostRef = useRef<HTMLDivElement>(null)
  const crepeRef = useRef<Crepe | null>(null)
  const onChangeRef = useRef(onChange)
  onChangeRef.current = onChange

  useEffect(() => {
    if (!hostRef.current) return
    const crepe = new Crepe({
      root: hostRef.current,
      defaultValue: initialValue,
    })
    crepe.on(listener => {
      listener.markdownUpdated((_ctx, markdown) => {
        onChangeRef.current(markdown)
      })
    })
    crepe.create()
    crepeRef.current = crepe
    return () => {
      crepe.destroy()
      crepeRef.current = null
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  return <div ref={hostRef} className="prose-host h-full overflow-auto" />
}
```

* [ ] **Step 2: Verify typecheck**

Run: `pnpm typecheck`
Expected: no errors for this file (errors elsewhere are fine until next tasks land).

* [ ] **Step 3: Commit**

```bash
git add src/components/markdown/CrepeEditor.tsx
git commit -m "feat(markdown): Crepe React wrapper"
```

***

## Task 12: `FrontmatterField` (per-row, type-dispatched)

**Files:**

* Create: `src/components/markdown/FrontmatterField.tsx`

* [ ] **Step 1: Implement**

```tsx
// src/components/markdown/FrontmatterField.tsx
import { inferFieldType } from '@/lib/markdown/field-type'
import { Input } from '@/components/ui/input'
import { Textarea } from '@/components/ui/textarea'
import { Switch } from '@/components/ui/switch'

interface FrontmatterFieldProps {
  fieldKey: string
  value: unknown
  onChange: (key: string, newValue: unknown) => void
}

export function FrontmatterField({ fieldKey, value, onChange }: FrontmatterFieldProps) {
  const type = inferFieldType(value)

  return (
    <div className="flex items-start gap-3 py-1.5">
      <label className="w-32 shrink-0 pt-1 text-sm text-muted-foreground">
        {fieldKey}
      </label>
      <div className="flex-1">{renderControl(type, value, v => onChange(fieldKey, v))}</div>
    </div>
  )
}

function renderControl(
  type: ReturnType<typeof inferFieldType>,
  value: unknown,
  set: (v: unknown) => void,
) {
  switch (type) {
    case 'boolean':
      return <Switch checked={Boolean(value)} onCheckedChange={set} />
    case 'multiline':
      return (
        <Textarea
          value={String(value ?? '')}
          onChange={e => set(e.target.value)}
          rows={4}
        />
      )
    case 'date':
      return (
        <Input
          type="date"
          value={String(value ?? '').slice(0, 10)}
          onChange={e => set(e.target.value)}
        />
      )
    case 'number':
      return (
        <Input
          type="number"
          value={Number(value ?? 0)}
          onChange={e => set(Number(e.target.value))}
        />
      )
    case 'list':
      return (
        <Input
          value={(value as unknown[]).map(String).join(', ')}
          onChange={e => set(e.target.value.split(',').map(s => s.trim()).filter(Boolean))}
          placeholder="comma, separated, list"
        />
      )
    case 'nested':
      return (
        <Textarea
          readOnly
          value={JSON.stringify(value, null, 2)}
          rows={4}
          className="font-mono text-xs"
        />
      )
    case 'text':
    default:
      return (
        <Input value={String(value ?? '')} onChange={e => set(e.target.value)} />
      )
  }
}
```

Verify `@/components/ui/textarea` exists: run `ls src/components/ui/`. If `textarea.tsx` is missing, add it via `pnpm dlx shadcn@latest add textarea` (the project already uses Radix + shadcn pattern based on dependency list).

* [ ] **Step 2: Verify typecheck (for this file)**

Run: `pnpm typecheck`
Expected: no errors from this file.

* [ ] **Step 3: Commit**

```bash
git add src/components/markdown/FrontmatterField.tsx
# also add ui/textarea.tsx if generated
git commit -m "feat(markdown): FrontmatterField per-row type-dispatched input"
```

***

## Task 13: `FrontmatterPanel`

**Files:**

* Create: `src/components/markdown/FrontmatterPanel.tsx`

* [ ] **Step 1: Implement**

```tsx
// src/components/markdown/FrontmatterPanel.tsx
import { useState } from 'react'
import { ChevronDown, ChevronRight } from 'lucide-react'
import type { Frontmatter } from '@/lib/markdown/parse'
import { FrontmatterField } from './FrontmatterField'

interface FrontmatterPanelProps {
  frontmatter: Frontmatter
  onChange: (next: Frontmatter) => void
}

export function FrontmatterPanel({ frontmatter, onChange }: FrontmatterPanelProps) {
  const keys = Object.keys(frontmatter)
  const [open, setOpen] = useState(keys.length < 5)

  if (keys.length === 0) return null

  return (
    <div className="border-b border-border bg-muted/30 px-6 py-3">
      <button
        type="button"
        onClick={() => setOpen(o => !o)}
        className="flex items-center gap-1 text-xs font-medium text-muted-foreground hover:text-foreground"
      >
        {open ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
        Frontmatter
      </button>
      {open && (
        <div className="mt-2">
          {keys.map(key => (
            <FrontmatterField
              key={key}
              fieldKey={key}
              value={frontmatter[key]}
              onChange={(k, v) => onChange({ ...frontmatter, [k]: v })}
            />
          ))}
        </div>
      )}
    </div>
  )
}
```

* [ ] **Step 2: Commit**

```bash
git add src/components/markdown/FrontmatterPanel.tsx
git commit -m "feat(markdown): collapsible FrontmatterPanel"
```

***

## Task 14: `MarkdownTitlebar` + `StampDialog` + `MarkdownWindow`

**Files:**

* Create: `src/components/markdown/MarkdownTitlebar.tsx`

* Create: `src/components/markdown/StampDialog.tsx`

* Create: `src/components/markdown/MarkdownWindow.tsx`

* Create: `src/lib/markdown/stamp.ts`

* [ ] **Step 1: Stamp IPC helper**

```ts
// src/lib/markdown/stamp.ts
import { Command } from '@tauri-apps/plugin-shell'

export async function stampFile(filePath: string): Promise<{ ok: boolean; message: string }> {
  const cmd = Command.sidecar('binaries/sec', ['stamp', filePath])
  const result = await cmd.execute()
  if (result.code === 0) {
    return { ok: true, message: result.stdout }
  }
  return { ok: false, message: result.stderr || result.stdout || 'stamp failed' }
}
```

If `@tauri-apps/plugin-shell` is not in `package.json`, add it: `pnpm add @tauri-apps/plugin-shell` and register in `src-tauri/src/lib.rs`: `.plugin(tauri_plugin_shell::init())` plus `tauri-plugin-shell = "2"` in `Cargo.toml`. Also extend `capabilities/default.json` to permit sidecar execution.

* [ ] **Step 2: Stamp dialog**

```tsx
// src/components/markdown/StampDialog.tsx
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'

interface StampDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  body: string
  onConfirm: () => void
  loading: boolean
}

export function StampDialog({ open, onOpenChange, body, onConfirm, loading }: StampDialogProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle>Stamp this document</DialogTitle>
          <DialogDescription>
            Review the full body below. Touch ID will gate the stamp.
          </DialogDescription>
        </DialogHeader>
        <pre className="max-h-96 overflow-auto rounded border border-border bg-muted p-3 font-mono text-xs whitespace-pre-wrap">
          {body}
        </pre>
        <DialogFooter>
          <Button variant="ghost" onClick={() => onOpenChange(false)} disabled={loading}>
            Cancel
          </Button>
          <Button onClick={onConfirm} disabled={loading}>
            {loading ? 'Stamping…' : 'Stamp'}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
```

* [ ] **Step 3: Titlebar**

```tsx
// src/components/markdown/MarkdownTitlebar.tsx
import { Button } from '@/components/ui/button'
import { Stamp } from 'lucide-react'

interface MarkdownTitlebarProps {
  title: string
  saving: boolean
  onStampClick: () => void
}

export function MarkdownTitlebar({ title, saving, onStampClick }: MarkdownTitlebarProps) {
  return (
    <header className="flex items-center justify-between border-b border-border bg-background px-6 py-2">
      <div className="flex items-center gap-2">
        <h1 className="text-base font-semibold tracking-tight">{title}</h1>
        {saving && <span className="text-xs text-muted-foreground">Saving…</span>}
      </div>
      <Button size="sm" onClick={onStampClick}>
        <Stamp size={14} className="mr-1.5" />
        Stamp
      </Button>
    </header>
  )
}
```

* [ ] **Step 4: Top-level** **`MarkdownWindow`**

```tsx
// src/components/markdown/MarkdownWindow.tsx
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { commands } from '@/lib/tauri-bindings'
import { parseMarkdown, serializeMarkdown, type Frontmatter } from '@/lib/markdown/parse'
import { resolveTitle } from '@/lib/markdown/title'
import { stampFile } from '@/lib/markdown/stamp'
import { CrepeEditor } from './CrepeEditor'
import { FrontmatterPanel } from './FrontmatterPanel'
import { MarkdownTitlebar } from './MarkdownTitlebar'
import { StampDialog } from './StampDialog'
import { toast } from 'sonner'

interface MarkdownWindowProps {
  filePath: string
}

export function MarkdownWindow({ filePath }: MarkdownWindowProps) {
  const [frontmatter, setFrontmatter] = useState<Frontmatter>({})
  const [body, setBody] = useState('')
  const [sha256, setSha256] = useState('')
  const [saving, setSaving] = useState(false)
  const [stampOpen, setStampOpen] = useState(false)
  const [stamping, setStamping] = useState(false)
  const [loaded, setLoaded] = useState(false)
  const saveTimer = useRef<number | null>(null)

  useEffect(() => {
    if (!filePath) return
    void (async () => {
      const res = await commands.readMarkdown(filePath)
      if (res.status === 'error') {
        toast.error(`Open failed: ${res.error}`)
        return
      }
      const parsed = parseMarkdown(res.data.content)
      setFrontmatter(parsed.frontmatter)
      setBody(parsed.body)
      setSha256(res.data.sha256)
      setLoaded(true)
    })()
  }, [filePath])

  const title = useMemo(
    () => resolveTitle(frontmatter, body, filePath),
    [frontmatter, body, filePath],
  )

  useEffect(() => {
    document.title = title
  }, [title])

  const scheduleSave = useCallback(
    (nextFm: Frontmatter, nextBody: string) => {
      if (saveTimer.current) window.clearTimeout(saveTimer.current)
      saveTimer.current = window.setTimeout(async () => {
        setSaving(true)
        const content = serializeMarkdown(nextFm, nextBody)
        const res = await commands.writeMarkdown({
          path: filePath,
          content,
          expectedSha256: sha256,
        })
        setSaving(false)
        if (res.status === 'error') {
          toast.error(`Save failed: ${res.error}`)
          return
        }
        if (res.data.kind === 'conflict') {
          toast.error('File changed on disk — reload to merge')
          return
        }
        setSha256(res.data.sha256)
      }, 800)
    },
    [filePath, sha256],
  )

  if (!loaded) return <div className="p-6 text-sm text-muted-foreground">Loading…</div>

  return (
    <div className="flex h-screen flex-col bg-background text-foreground">
      <MarkdownTitlebar
        title={title}
        saving={saving}
        onStampClick={() => setStampOpen(true)}
      />
      <FrontmatterPanel
        frontmatter={frontmatter}
        onChange={next => {
          setFrontmatter(next)
          scheduleSave(next, body)
        }}
      />
      <main className="flex-1 overflow-hidden">
        <CrepeEditor
          initialValue={body}
          onChange={next => {
            setBody(next)
            scheduleSave(frontmatter, next)
          }}
        />
      </main>
      <StampDialog
        open={stampOpen}
        onOpenChange={setStampOpen}
        body={serializeMarkdown(frontmatter, body)}
        loading={stamping}
        onConfirm={async () => {
          setStamping(true)
          const r = await stampFile(filePath)
          setStamping(false)
          setStampOpen(false)
          if (r.ok) {
            toast.success('Stamped')
            // re-read to pick up embedded stamp
            const res = await commands.readMarkdown(filePath)
            if (res.status === 'ok') {
              const parsed = parseMarkdown(res.data.content)
              setFrontmatter(parsed.frontmatter)
              setBody(parsed.body)
              setSha256(res.data.sha256)
            }
          } else {
            toast.error(r.message)
          }
        }}
      />
    </div>
  )
}
```

The generated `commands` shape from tauri-specta returns `{ status: 'ok', data } | { status: 'error', error }` — confirm against `src/lib/bindings.ts` after Task 7. If `sonner` isn't installed, add it: `pnpm add sonner`, and mount `<Toaster />` once in `markdown-window-main.tsx`.

* [ ] **Step 5: Mount Toaster**

Edit `src/markdown-window-main.tsx` to add `<Toaster />`:

```tsx
import { Toaster } from 'sonner'
// ...
<ThemeProvider>
  <MarkdownWindow filePath={decodeURIComponent(path)} />
  <Toaster />
</ThemeProvider>
```

* [ ] **Step 6: Typecheck**

Run: `pnpm typecheck`
Expected: success.

* [ ] **Step 7: Commit**

```bash
git add src/components/markdown/ src/lib/markdown/stamp.ts src/markdown-window-main.tsx
git commit -m "feat(markdown): MarkdownWindow + Titlebar + StampDialog"
```

***

## Task 15: Frontend bridge — drain `pending-opens` from `main`

**Files:**

* Create: `src/lib/markdown/open.ts`

* Modify: `src/App.tsx`

* [ ] **Step 1: Implement open helper**

```ts
// src/lib/markdown/open.ts
import { listen } from '@tauri-apps/api/event'
import { commands } from '@/lib/tauri-bindings'

export async function drainAndOpenPending(): Promise<void> {
  const paths = await commands.takePendingOpens()
  if (paths.status !== 'ok') return
  for (const p of paths.data) {
    await commands.openMarkdownWindow(p)
  }
}

export function watchPendingOpens(): () => void {
  // Drain once on startup (covers RunEvent::Opened that fired before frontend ready)
  void drainAndOpenPending()

  let unlistenFn: (() => void) | undefined
  void listen('markdown://pending-opens-added', () => {
    void drainAndOpenPending()
  }).then(fn => {
    unlistenFn = fn
  })

  return () => {
    if (unlistenFn) unlistenFn()
  }
}
```

* [ ] **Step 2: Wire into** **`App.tsx`**

In `src/App.tsx`, inside the existing `useEffect(() => { ... }, [])`:

```tsx
import { watchPendingOpens } from './lib/markdown/open'
// ...
const unwatch = watchPendingOpens()
// ... and return a teardown
return () => {
  unwatch()
}
```

(Compose with any existing teardown — read the file first; if the effect already has cleanup, extend it.)

* [ ] **Step 3: Smoke test manually**

Run: `pnpm tauri:dev`. In a separate terminal:

```bash
open -a Secretariat /tmp/test.md  # create test.md first with some content
```

Expected: a markdown window opens with the file's body.

* [ ] **Step 4: Commit**

```bash
git add src/lib/markdown/open.ts src/App.tsx
git commit -m "feat(markdown): drain PendingOpens on main-window mount"
```

***

## Task 16: Integration test — round-trip via commands

**Files:**

* Create: `src-tauri/tests/markdown_round_trip.rs`

* [ ] **Step 1: Write test**

```rust
// src-tauri/tests/markdown_round_trip.rs
use secretariat_lib as _; // ensure crate links

#[test]
fn round_trip_via_file_io() {
    use std::path::PathBuf;
    let dir = tempfile::tempdir().unwrap();
    let path: PathBuf = dir.path().join("note.md");
    std::fs::write(&path, b"---\ntitle: T\n---\n# H\n").unwrap();

    // We exercise the pure file-io layer (commands are tested separately at the unit level).
    // This test confirms the Cargo workspace can link the markdown module from an integration test.
    let read = secretariat_lib::markdown::file_io::read_file(&path).unwrap();
    let new_sha = secretariat_lib::markdown::file_io::write_file(
        &path,
        "---\ntitle: T2\n---\n# H\n",
        &read.sha256,
    )
    .unwrap();
    assert_ne!(new_sha, read.sha256);
}
```

(Requires `pub use` of `markdown` module from `lib.rs`. Add at top of `src-tauri/src/lib.rs`: `pub mod markdown;` instead of `mod markdown;`.)

* [ ] **Step 2: Run, verify pass**

Run: `cd src-tauri && cargo test --test markdown_round_trip`
Expected: PASS.

* [ ] **Step 3: Commit**

```bash
git add src-tauri/tests/markdown_round_trip.rs src-tauri/src/lib.rs
git commit -m "test(markdown): round-trip integration test"
```

***

## Task 17: Onboarding hint — "Set as default app"

**Files:**

* Modify: `src/components/secretariat/Onboarding.tsx` (or create a small dismissible card if onboarding is gated)

* [ ] **Step 1: Inspect existing onboarding**

Run: `head -80 src/components/secretariat/Onboarding.tsx`
Identify a sensible spot for an optional card.

* [ ] **Step 2: Add card content**

Add a card with copy along the lines of:

```tsx
<div className="rounded-md border border-border bg-muted/40 p-4">
  <h3 className="text-sm font-medium">Make Secretariat your default markdown app</h3>
  <p className="mt-1 text-xs text-muted-foreground">
    In Finder, right-click any <code>.md</code> file → Get Info → Open With → choose
    Secretariat → Change All.
  </p>
</div>
```

This v1 ships the tooltip; programmatic `LSSetDefaultRoleHandlerForContentType` is a follow-up.

* [ ] **Step 3: Commit**

```bash
git add src/components/secretariat/Onboarding.tsx
git commit -m "feat(markdown): onboarding hint for default-app"
```

***

## Task 18: Quality gates + manual smoke

**Files:** none

* [ ] **Step 1: Full check**

Run: `pnpm check:all`
Expected: typecheck, lint, format, clippy, vitest, cargo test all green.

* [ ] **Step 2: Manual smoke**

Run: `pnpm tauri:dev`. Verify, in order:

1. `open -a Secretariat /path/to/some.md` opens a new markdown window.
2. The frontmatter form renders all keys with appropriate inputs.
3. Title bar shows fm.title (or H1, or basename).
4. Edit body → wait 1s → window title shows "Saving…" briefly → file on disk has updated content.
5. Edit a fm field → same autosave path.
6. Click Stamp → modal shows full body → confirm → Touch ID dialog → file re-loaded with embedded stamp.
7. Cmd+Ctrl+F fullscreens the window.
8. Open the same file a second time → existing window focuses (no duplicate).

* [ ] **Step 3: Note any deferred polish**

If anything fails or is rough, log it in `docs/superpowers/specs/2026-05-17-markdown-reader-followups.md` (create) instead of fixing in this plan — keep scope.

***

## Self-Review (writer's pass)

**Spec coverage check:**

| Spec section                              | Task(s)           |
| ----------------------------------------- | ----------------- |
| Editor lib: Milkdown Crepe                | 1, 11             |
| Window topology (md window entry)         | 10, 14            |
| Frontmatter UI (parse, type, panel)       | 2, 3, 12, 13      |
| Pretty title resolution                   | 4, 14             |
| File I/O w/ sha256 concurrency            | 5, 7              |
| macOS file association                    | 8                 |
| RunEvent::Opened + single-instance argv   | 9                 |
| PendingOpens buffer                       | 6                 |
| Open-window IPC                           | 7                 |
| Pending drain on frontend                 | 15                |
| Stamp ceremony (verbatim → CLI → re-read) | 14                |
| Tests                                     | 2, 3, 4, 5, 6, 16 |
| Onboarding hint for default-app           | 17                |
| Quality gates                             | 18                |

**Placeholder scan:** Each step shows real code; no "TBD" / "appropriate" / "similar to".

**Type consistency:** `Frontmatter` from `parse.ts` is used in `title.ts`, `FrontmatterPanel.tsx`, `FrontmatterField.tsx`, `MarkdownWindow.tsx`. `ReadMarkdownResult` / `WriteMarkdownResult` from Rust → consumed via `commands.readMarkdown` / `commands.writeMarkdown` after Task 7 regenerates bindings. `PendingOpens` is registered via `app_builder.manage` (Task 7 step 3) before being read in commands and the `RunEvent::Opened` arm (Task 9).

**Ambiguity:** `commands.readMarkdown` response shape is asserted (Task 14 step 4 note) — engineer should confirm by reading regenerated `bindings.ts` before wiring `MarkdownWindow.tsx`. The shape `{ status: 'ok' | 'error' }` is the convention used elsewhere in `bindings.ts`.

**Risks already in spec:** Milkdown FM round-trip (mitigated — FM parsed JS-side, Crepe sees body only); macOS event ordering (mitigated — `PendingOpens` + emit event); sidecar shell capability (called out in Task 14 step 1).
