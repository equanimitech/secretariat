---
migrated_from: equanimi.tech/project/secretariat/dev/20260525T213853Z-xi2oab.md
$attestation:
  $type: tech.equanimi.secretariat.stamp
  signer: did:key:z6MkjB8PQaN1vuUzdtnJsxyXR2f8d3tckGHkUYZMDytQsfak
  act: attest
  docHash: sha256:ceeccf55c6365eae83a8c5b0b498b9a54354681362eee36696325c8ba43ef284
  docFilename: 20260525T213853Z-xi2oab.md
  stampedAt: 2026-05-26T18:37:32.471639Z
  signature: ed25519:8Hc0Mw0/Ns0o6ohKc0TouApF0ecyNY+kAHRLfefeA85Xc3iAcwK+4ZsQgt7rmX4gP9/XwaBdaUjDTXih+CODCA==
---

# MCP secretariat — gaps découverts en usage (2026-05-25)

Capture issue d'une session où je devais lire 7+ envelopes du canal `module:cassation` (themia.pro) pour synthétiser un feedback cofounder. J'ai par défaut utilisé `Bash` (`find`, `grep`, `ls`) + `Read` sur filesystem direct — pas les tools MCP `mcp__secretariat__*`. Pourquoi : les tools MCP existants ne couvrent pas les verbes dont j'avais besoin.

Pain : les patterns d'usage agent (« trouve tout ce qui touche X dans le canal Y », « résous ce short-code ») n'ont pas de tool dédié, donc bypass filesystem. Bypass marche, mais court-circuite contrats du canal + ACL + déchiffrement propre.

## Tools manquants — par ordre de douleur

1. **`search_envelopes`** — full-text grep à travers envelopes d'un canal (ou multi-canaux), filtres `since` / `until` / `tags` / `source` / `from`. **Le plus pénible.** Sans ça je tombe sur `find | xargs grep` au filesystem.

2. **`list_envelopes`** — énumérer envelopes d'un canal avec pagination + tri (newest-first par défaut). `read_channel` existe — pas vérifié s'il retourne juste le `channel.md` ou aussi un index d'envelopes. À clarifier dans tous les cas : un `list_envelopes(handle, limit, since)` discret serait propre.

3. **`get_envelope_by_short_code`** — les envelopes se citent entre elles par short code (`4qiqpb`, `i4djks`, `eeye4p`, `gvl4ru`). Pour résoudre une référence aujourd'hui : grep filesystem sur le code. Un `read --short-code 4qiqpb` (avec scope optionnel : `org` / `handle`) serait trivial à implémenter et débloque la lecture de threads.

4. **`thread_walk`** **/ backlinks** — frontmatters contiennent déjà `companion-to:`, `supersedes:`, et des `[[name]]` inline. Le graph existe, manque la traversée. Verbe : « étant donné cet envelope, donne-moi tous les envelopes qui le citent ou qu'il cite ». Permet aux agents de remonter le contexte sans grep aveugle.

5. **`filter_by_tag`** — frontmatter porte déjà `tags: [should-have, defer, v1.2, validation-kit, ...]`. Aucun filtre MCP n'en tire parti. Un `list_envelopes(tags=["v1.2"])` suffirait.

6. **`cross_handle_query`** — « tous les envelopes mentionnant *cassation* à travers tous les canaux où je suis membre ». Cassation aujourd'hui touche `module:cassation`, `dev:leggia`, `product:jurimetria`, `inbox:ideas`. Sans ce verbe, l'agent doit énumérer les canaux puis grep chacun. Pourrait être implémenté comme `search_envelopes` sans scope (cf. #1) — gap de la même famille mais utility séparée.

## Pourquoi ça compte

* **Bypass = perte d'audit trail.** Quand l'agent lit le filesystem direct, le canal ne sait pas qu'on a lu. Pas grave aujourd'hui (single-principal) — devient important dès qu'on a roster + read-receipts ou métriques d'attention par canal.

* **Bypass = perte de contrats.** Le canal a un `contract.local.md` qui dicte comment je consomme. Lecture filesystem ignore les overrides (filtres, surface).

* **Friction agent.** Plus le manque est marqué, plus l'agent par défaut tape filesystem — habitude qui scale mal le jour où on a des envelopes chiffrés au repos ou un backend non-filesystem (sync, remote queue).

## Note de cadrage

Pas urgent (le bypass marche), mais les tools manquants sont précisément ceux que les agents demandent en pratique. Ordre raisonnable de ship : `get_envelope_by_short_code` (trivial, gros gain) → `search_envelopes` (le verbe central) → `list_envelopes` (probablement déjà à moitié fait dans `read_channel`) → `thread_walk` (joli mais derrière le reste).

## Session contexte

Canal cible : `themia.pro / module:cassation`. Tâche : synthétiser le feedback praticien de Christophe sur la dimension jurimétrique Cassation/Rejet × cas d'ouverture × matière × juridiction attaquée. Le résultat est capturé séparément dans `module:cassation` même.
