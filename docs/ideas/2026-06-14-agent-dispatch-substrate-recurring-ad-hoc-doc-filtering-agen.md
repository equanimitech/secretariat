---
$signature:
  $type: tech.equanimi.secretariat.signature
  signer: did:key:z6MkpcX3mHt44yNEDPDWJic8ocJdagzERxx5u2Qh1dWcVRVN
  signerRole: agent
  docHash: sha256:909f38897e75db3d64363e6d9b3af0346e07535fa7d612d2f7e0b59641e3b5d9
  signedAt: 2026-06-14T11:08:10.603292Z
  signature: ed25519:cyc/d2825ZvsNVeZ+sLA+YyiAajqKmkW5/Q9Zgth0c49hdc9+iLlHC1RAvNkeipa2N6H6Jd54nY8sFvOS40WAA==
type: idea
---
# Agent dispatch substrate: recurring/ad-hoc doc-filtering agents triggered by stamp hook

Having agents (for Themia or equanimitech) means having recurring / ad-hoc dispatches that filter on docs and perform actions on them.

- The stamp hook should be a great trigger — a freshly-stamped doc fires a dispatch.
- Routing happens by doc type (the stamp's type/headline decides which agent picks it up).
- Two dispatch cadences: recurring (scheduled sweeps over the doc corpus) and ad-hoc (event-driven on stamp).

Don't shape yet.

Captured via /triage on 2026-06-14 from the Things inbox.