---
migrated_from: equanimi.tech/project/secretariat/editor/ideas/20260517T192007Z-sua2ut.md
---
# Vim bindings via Crepe extensions

Crepe is ProseMirror-based. ProseMirror has `prosemirror-vim` / `codemirror-vim` patterns, but for a Crepe WYSIWYG the right path is a custom keymap extension rather than a true vim emulator (WYSIWYG selection model conflicts with modal vim).

Investigate:
- Does Crepe 7.x expose a `keymap` extension point?
- Minimum viable: normal-mode hjkl + i/a/o/escape, visual-mode selection, dd/yy/p.
- Punt: vimscript, registers, ex-commands. (Anyone who needs those uses real vim.)
- Setting: opt-in via preferences.toml; off by default.
