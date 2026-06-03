# Send markdown to Supernote (read on e-ink)

- A button in Secretariat: **send the current markdown to my Supernote** so I can read formatted markdown on the small e-ink device.
- Natural fit with **Attend mode** (the reading posture from the editor-reader redesign): "read this elsewhere, on e-ink" is a reading affordance, not an editing one. The button belongs in Attend, next to the trust banner.
- Local-first, no cloud needed (ideally) — over LAN, matching the no-server / convivial-infrastructure stance.

- Mechanism (grounded in `~/Developer/supynote`):
  - supynote is a Python CLI over the Supernote **"Export via LAN"** feature (Settings → System → Export via LAN): device runs a LAN web server; supynote **auto-discovers** it (`device_finder.py`), lists/downloads files, converts `.note`→vector PDF (Cairo).
  - **Direction wrinkle:** supynote today is *download-from-device*. "Export via LAN" is primarily browse/download — it may **not** accept uploads. Sending *to* the device is the inverse path and is the open feasibility question.
  - Likely shape: render markdown → a format the Supernote reads well (**PDF**, reusing the Cairo render supynote already has, but markdown→PDF is a new direction), then deliver it — via device-side import if LAN upload exists, else `~/Developer/supernote-cloud` (cloud path), USB, or a watched folder the device syncs.

- Questions:
  - Does Supernote "Export via LAN" support **upload**, or is it download-only? (Decides whether this is pure-LAN or needs cloud/USB.)
  - Render target — **PDF** (vector, Cairo, reflow-fixed) vs something reflowable the device prefers? Formatted-markdown legibility on small e-ink is the whole point.
  - Reuse supynote as a **dependency/sidecar** (Python) vs reimplement the LAN discovery in Rust? Secretariat is Rust/Tauri; supynote is Python — a sidecar or a shell-out, or port `device_finder`.
  - Is this **Secretariat** scope or a **supynote** feature Secretariat calls? Cleanest: supynote grows a `send`/`push` verb; Secretariat's button shells out to it (keeps the device protocol in the tool that already owns it).
  - Stamp interaction: send the *signed/sealed* version so what you read on e-ink carries provenance? Or raw body for reading comfort?

Don't shape yet.
