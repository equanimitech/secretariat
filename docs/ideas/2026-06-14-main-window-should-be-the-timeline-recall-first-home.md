---
$signature:
  $type: tech.equanimi.secretariat.signature
  signer: did:key:z6MkpcX3mHt44yNEDPDWJic8ocJdagzERxx5u2Qh1dWcVRVN
  signerRole: agent
  docHash: sha256:4fe16f7e3aa16d3001c689268f40e90b9bef749f0677aacd9086009073514eb8
  signedAt: 2026-06-14T12:29:32.632519Z
  signature: ed25519:zBR61VVnAMLeuCS+qacZxXBQAeJ+e7RB+pqTeTsdVcgvVbvpj/2BZyjCTD0irswrlzaAtvYoyg5aCKmxnjDaAg==
type: idea
---
# Main window should be the timeline (recall-first home)

- The main window currently earns its keep only as settings; that is a thin
  reason to keep a window. Make its primary content the timeline: a
  recall-first home over the substrate.
- Reuses the same `build_timeline` core the CLI and MCP tool already use. The
  window is a third surface, not a new data layer.
- Recall-first shape: timeline as the landing view (day/repo grouping, the
  zoom dial day/week/month); clicking a doc opens it in the editor. Settings
  demotes to a panel or a command-palette entry.
- Slots into the existing "Post-teardown window model — main window, sidebar,
  quick captures" note: this answers what the main window is for.
- Questions:
  - Recall-first (timeline home, click opens editor), or editor embedded
    alongside a timeline rail?
  - Does the window call the core via a Tauri command, or shell out to the
    same path the CLI/MCP use?
  - Default range/zoom on open (today? 7d?); does it scope to the active repo?

Don't shape yet.
