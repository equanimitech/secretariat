---
migrated_from: equanimi.tech/project/secretariat/editor/bugs/20260517T192419Z-hmmng6.md
---

# Windows portability — markdown reader doesn't work yet

Audit 2026-05-17 (subagent). Mac-only today. Christophe-readiness requires the following fixes. Severity-tagged.

## BLOCKERS (Windows won't open `.md` at all)

1. **`RunEvent::Opened`** **is macOS/iOS/Android only.** `src-tauri/src/lib.rs:380-394`. Windows passes file paths as `argv[1]` — no `Opened` event fires. The single-instance callback (`lib.rs:57-82`) handles *subsequent* opens into a running instance, but the **first** invocation needs the primary process to walk `std::env::args().skip(1)` in `setup()` and call `spawn_markdown_window` directly.

2. **`sec view`** **is hard-bailed on non-macOS.** `crates/cli/src/commands/view.rs:44-46` raises `anyhow::bail!("`sec view` is macOS-only…")`. Add a `#[cfg(target_os = "windows")]` arm that locates the installed `Secretariat.exe` (`%LOCALAPPDATA%\Secretariat\Secretariat.exe` or `%PROGRAMFILES%\Secretariat\Secretariat.exe`) and `Command::new(exe).arg(abs).spawn()`. The `secretariat://` deep-link scheme is also registered (`tauri.conf.json:74-79`) — could use that instead.

## IMPORTANT (silent breakage on first Windows build)

3. **`bundle.fileAssociations`** **lives only in** **`tauri.macos.conf.json`.** Move to base `tauri.conf.json` — it's cross-platform (NSIS/MSI installer writes registry entries from the same block). Right-click → "Open with → Secretariat" is dead on Windows until this moves.

4. **`fs::rename`** **over open file fails on Windows.** `src-tauri/src/markdown/file_io.rs:49-51`. POSIX rename-over-existing is atomic; Windows returns `ERROR_ACCESS_DENIED` if anything holds a lock. Add retry-with-backoff (3×, 50ms) or switch to `tempfile::persist` / `atomic-write-file`. At minimum surface a friendlier error than raw `os error 5`.

5. **`basenameWithoutExt`** **splits on** **`/`** **only.** `src/lib/markdown/title.ts:19-22`. `C:\Users\rafa\notes\my-file.md` → entire path as one segment, title broken. Fix: `p.split(/[/\\]/).pop()`. Add Windows-path test case.

6. **`tauri.windows.conf.json`** **placeholder title.** `:5` still reads `"title": "tauri-app"` — template leftover. Mirror macOS conf: `"title": "Secretariat"`, `minWidth`, `decorations: true`, `visible: false`.

7. **`beforeBuildCommand`** **is bash-only.** `tauri.conf.json:10` runs `bash src-tauri/scripts/build-sidecars.sh`. Windows builder without WSL/Git-Bash on `PATH` fails before build starts. Sidecars are required for MCP + daemon wiring (`lib.rs:264-269`). Port to cross-platform: per-platform `beforeBuildCommand` in `tauri.{platform}.conf.json`, or a `cargo xtask`-style Rust script. Also: `lib.rs:459-460` `dir.join("sec")` — on Windows the sidecar is `sec.exe`; Tauri's externalBin convention appends `.exe`, verify the lookup.

## NICE-TO-HAVE

8. **Mixed-case path opens window twice.** `src-tauri/src/commands/markdown.rs:91` (`window_label`). Same file with different separator casing (`C:\foo\bar.md` vs `C:/foo/bar.md`) hashes to two different labels → two windows. Canonicalize via `dunce::canonicalize` before hashing.

9. **Custom titlebar + native decorations stack on Windows.** `MarkdownTitlebar.tsx:17` adds `data-tauri-drag-region` header; `spawn_markdown_window` doesn't pass `.decorations(false)`. macOS overlays traffic-lights on the drag region — works. Windows stacks two title bars. Per-platform: either `.decorations(false)` + ship custom close/min/max trio, or `.decorations(true)` and drop the custom header on Windows.

10. **UNC** **`\\?\`** **paths from** **`canonicalize`.** `crates/cli/src/commands/view.rs:24-25`. Ugly in titlebar display. Mitigate with `dunce::canonicalize`. Downstream of fixing #2.

11. **First-use notification permission UX.** `capabilities/markdown.json:23` (`notification:default`). `tauri-plugin-notification` handles Windows via WinRT `ToastNotification`; first notification on Windows 10+ pops a system permission flow. No code change — just document for onboarding copy.

## CONFIRMED NON-ISSUES

* **`urlencoding`** **round-trip with backslashes.** Percent-encodes `\` to `%5C`; `decodeURIComponent` reverses cleanly. `PathBuf::from(&path_str)` on Rust side accepts both separators.

* **Dependency portability.** All deps cross-platform. `tauri-nspanel` correctly gated to `target_os = "macos"` in Cargo.toml:62. No unix-only crates.

* **`dirs::home_dir`.** Resolves `%USERPROFILE%` on Windows. `~/.secretariat/` lands at `C:\Users\<name>\.secretariat\` — unusual on Windows but consistent with the project's filesystem-authoritative principle.

## Order of operations when we ship Windows

1. Move fileAssociations to base conf (#3) — unblocks "open with"
2. Fix Windows title placeholder (#6) — visible
3. Walk argv in setup (#1) — opens windows from "open with"
4. `sec view` Windows arm (#2) — opens from CLI
5. Atomic rename retry (#4) — autosave survives concurrent editors
6. Title basename regex (#5) — titlebar reads correctly
7. build-sidecars port (#7) — Windows CI can actually build
8. Polish: decorations + canonicalization (#8, #9, #10)

Roughly a day's work for blockers + importants. Nice-to-haves another half-day. Stop work today; revisit before Christophe-on-Windows milestone.

## Sources

* [Tauri 2.0 Stable Release notes](https://v2.tauri.app/blog/tauri-20/)

* [Tauri file-association PR #4320](https://github.com/tauri-apps/tauri/commit/3b98141aa26f74c641a4090874247b97079bd58a)

* [File Associations on Mobile (also covers desktop)](https://v2.tauri.app/learn/mobile-file-associations/)

