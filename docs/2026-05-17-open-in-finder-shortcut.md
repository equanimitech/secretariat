---
migrated_from: equanimi.tech/project/secretariat/editor/ideas/20260517T192003Z-o2j7yq.md
---
# Open-in-Finder shortcut

From the markdown window: one-click (or cmd-shortcut) to reveal the current file in Finder. Standard macOS verb; Tauri's opener plugin exposes `revealItemInDir`.

UI placement: titlebar overflow menu, or a small Finder-icon button next to the filename. Cmd-Shift-R is the macOS convention.

Windows equivalent: `explorer.exe /select`. Tauri's `revealItemInDir` already handles cross-platform dispatch.
