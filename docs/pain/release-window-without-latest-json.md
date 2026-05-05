---
status: open
severity: high
created: 2026-05-05
updated: 2026-05-05
---

# Release window without latest.json — auto-updater 404s during CI

Raw capture — 2026-05-05.

- During the v0.2.3 release, the in-app auto-updater on installed v0.2.2 surfaced "Update Check failed: could not check for updates" — even though `https://github.com/equanimitech/secretariat/releases/tag/v0.2.3` already existed.
- Cause: ordering between two CI workflows.
  - `.github/workflows/release.yml` (CLI tarballs) creates the GitHub release as **non-draft** immediately when it uploads the two `secretariat-darwin-{arm64,x86_64}.tar.gz`. Takes ~3 min.
  - `.github/workflows/tauri-release.yml` builds + signs + notarizes the .app, generates `latest.json`, uploads to the same release. Takes ~7 min.
  - Between minute 3 and minute 7, the latest release exists, IS marked latest by GitHub, but has no `latest.json` asset yet. Apps querying `/releases/latest/download/latest.json` get 302→404.
- Real-world impact: every release window has 4+ minutes where existing installs throw "could not check for updates". Looks broken.
- Where observed: 2026-05-05 v0.2.3 push.
- Questions:
  - Fold both workflows into a single workflow with sequential jobs (CLI → Tauri → publish)?
  - Have `release.yml` create as draft (`prerelease: true` or `draft: true`), let `tauri-release.yml`'s publish step flip both?
  - Drop `release.yml` entirely — bundle the CLI tarballs as artifacts of the Tauri workflow?
- Don't fix yet.
