---
$attestation:
  $type: tech.equanimi.secretariat.stamp
  signer: did:key:z6MkjB8PQaN1vuUzdtnJsxyXR2f8d3tckGHkUYZMDytQsfak
  act: attest
  docHash: sha256:5b0eca83da004ecc9408d36d2647ab8fd0ddd51ff294e6b70f643523c79481e9
  docFilename: 2026-06-03-document-as-workflow-node.md
  stampedAt: 2026-06-03T21:53:28.935249Z
  signature: ed25519:IbovXapkHlqkASjiAoJB8kw9tjicSwhMBcurrRLhAR1FFO2tD9aVnn5U0WaNHIxh4GU3w17FHKY6Vr8cicNUDA==
---
# Document as workflow node

* From the editor, **run any custom prompt / skill with the current markdown as input** — the doc becomes a workflow node, skills/prompts are operations on it.

* Already half-true by invariant #5: a repo's doc surface *is* the activation surface (`cd repo && claude` activates context for free). This makes it an explicit **in-editor affordance** instead of a terminal ritual.

* Fits the Compose/Attend split: is "run X on this" a **Compose** action (transform the draft in place) or an **Attend** action (act on the sealed record, output travels onward)? Probably both, with different provenance rules.

* The contested half — **"can stamping automatically cause stuff to happen?"** This is the exact tension simmering in \[\[delegation as a sealable decision]] (the daemon-on-seal kill-shot). Keep the distinction sharp:

  * **(a) Consented seal-and-run** — the human chooses, *in the same turn*, to seal AND dispatch. Still **pull** (= the delegation idea's "pull-at-dispatch"). Defensible.

  * **(b) Standing daemon auto-fires on every seal** — push in a trenchcoat, automation treadmill, no structural terminator. The trap.

  * Line: a stamp records **disposition**; whether it *fires* an action must be the human's explicit choice in the moment, never a standing rule. **Seal as terminus, optionally a one-shot consented dispatch — not a trigger.**

* Questions:

  * Compose-affordance, Attend-affordance, or both? Different output/provenance per mode.

  * What's the registry the editor exposes — the installed skills? a curated subset? custom prompts saved per repo?

  * Output target — new doc, in-place edit, or a child node? Where does the result land and how is it named (the git-native `<date>-<slug>.md` convention)?

  * **Provenance travel** — does the output carry the input's seal/signature as its origin? (Same question as the delegation idea.)

  * Loop/terminator — a doc-node that produces a doc-node that re-enters: same infinite-gauntlet risk. What stops it?

  * If standing rules are ever allowed (b), who can define them, and how is that not a notification/automation treadmill? (Default: don't allow; pull only.)

Don't shape yet.
