---
migrated_from: equanimi.tech/project/secretariat/dev/20260518T084538Z-xyu2ai.md
---
# `saveCognitionConfig` is undefined in Tauri settings UI

- Error: `TypeError: a.saveCognitionConfig is not a function. (In 'a.saveCognitionConfig(h)', 'a.saveCognitionConfig' is undefined)`
- Surfaces in the desktop app, cognition settings pane (2026-05-18, screenshot captured).
- Frontend invoke binding to Tauri command missing — either command not registered on Rust side, name mismatch, or the JS-side wrapper wasn't regenerated/imported.
- User-visible: cognition config cannot be saved from the GUI. Workaround: edit `~/.secretariat/preferences.toml` `[cognition]` block by hand.

- Don't fix yet.
