---
$signature:
  $type: tech.equanimi.secretariat.signature
  signer: did:key:z6MkpcX3mHt44yNEDPDWJic8ocJdagzERxx5u2Qh1dWcVRVN
  signerRole: agent
  docHash: sha256:61600682c2d490f8b06d88e60c40eea46fbd15b71aca213e953fd959844d5371
  signedAt: 2026-06-14T12:37:09.581198Z
  signature: ed25519:3RbXkKKYUx9dxsv0vxwB7lDVBQug3Ae5DIL+SZzIboSvybBxkHK119136rSE4XwRqtMF3i2rI2MSxyZUxErEAw==
type: idea
---
# Remove the home window for now (settings from individual windows)

- The home/main window opens on every launch, and right now it only does
  settings — thin justification for a window that auto-appears each start.
- Interim move: remove the home/main window for now; make settings openable
  from any individual window (app menu / command palette / settings sheet).
- This is the stopgap before the longer-term direction where the main window
  becomes the timeline (see idea "Main window should be the timeline
  (recall-first home)"). Remove now; reintroduce as the timeline later.
- Questions:
  - Where does settings live with no home window: app menu, per-window
    command, or a settings sheet?
  - On launch with no home, what is the entry surface — last doc, quick-open,
    or nothing until a doc is opened?
  - Is "remove" a hide/disable, or actually deleting the route?

Don't shape yet.
