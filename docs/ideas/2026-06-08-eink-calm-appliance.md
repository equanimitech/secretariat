# DIY e-ink calm appliance for Secretariat

Captured 2026-06-08 from Things inbox (weekly review). **Don't shape yet** — brainstorm first.

Single-purpose tactile box — calm interface for Secretariat. B/W e-ink, physical buttons, no
feed, no notifications. Peak equanimitech (fade-by-design, single-purpose, convivial).

**Feel:** like a game-controller, not a tablet. Real buttons you press with thumbs. Plus an
"etch-a-sketch" pair of rotary dials.

## Etch-a-sketch dials → semantic zoom (the gem)

- Physical knobs mapped to the semantic-zoom granularity ladder (handle → sentence → paragraph
  → page → blog → report → chapter → book).
- Turn the dial = e-ink redraws content coarser/finer in place.
- Adaptive Granularity (equanimitech principle) made tactile. Maybe 2 dials: one = zoom level,
  one = navigate/scrub items.

## Hardware candidates

- M5Paper S3 (ESP32, 4.7" e-ink, battery, write firmware) — "the soul" option
- Raspberry Pi + Waveshare e-ink HAT (Linux, Python client OR host PWA locally) — fast-prototype
- (dials = rotary encoders → GPIO; trivial on either)

## Architecture

- box polls Secretariat HTTP API → renders B/W
- button press POSTs an action; dial turn changes render granularity (local, instant)
- buttons → GPIO: one button = one intent (read / draft / request-stamp / mark-seen)
- device = ambient display + action trigger, NOT trust root
- stamping stays on Mac via Touch ID (box can *request* a stamp, Mac confirms) — matches the
  secretariat model: scribe shows, principal stamps

**HARD GATE:** Secretariat must expose an HTTP surface first (today it's MCP only). No client
moves without this.

Context — explored alternatives to Supernote (mid hackability). Hackable e-writers ranked:
PineNote (most open, rough) > reMarkable 2 (real Linux, mature hack community, calm) > Onyx Boox
(easiest but anti-equanimitech, telemetry). DIY picked as most exciting.

Next: proper brainstorm — intent + must/should/nice tiers. First decide the Secretariat HTTP surface.
