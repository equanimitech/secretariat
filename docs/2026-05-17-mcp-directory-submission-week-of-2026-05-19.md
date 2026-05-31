---
migrated_from: equanimi.tech/project/secretariat/dev/20260517T195748Z-mgphsc.md
---
# MCP Directory Submission — Week-of 2026-05-19 implementation queue

**Spec:** `docs/specs/mcp_v1/2026-05-16-directory-submission-v2-plan.md`
**Memory:** `project_mcp_directory_submission.md`
**Target submit date:** 2026-05-26 (Mon) — slip to 2026-05-29 (Thu) if legal blocks.
**Strategic frame:** Anthropic launched Claude for Legal 2026-05-12 with no French civil-law jurimetrics partner. Themia owns that slot. Don't slip past the launch attention window.

---

## Mon 2026-05-19 — Manifest + Auth setup

- [ ] **W1 — Manifest sync.** Rewrite `apps/api/app/mcp/manifest.json` to reflect actual 23-item surface (20 module tools + 2 standalone + 1 prompt). Bump `version` to `1.1.0`. Broaden description from DC-only to multi-module. Add `localization.resources["en-US"]` block with EN short descriptions per tool. Add keywords: `Cour de cassation`, `bail commercial`, `civil law`, `France`. **~2h.**
- [ ] **W3 — Clerk redirect URIs.** Add both `https://claude.ai/api/mcp/oauth_redirect` AND `https://claude.com/api/mcp/oauth_redirect` to Clerk dashboard. (`claude.com` is Anthropic's new domain — most-common rejection hits this.) **~30min.**
- [ ] **W3 — Reviewer test account.** Clerk user on real org, `subscriptionStatus = TRIAL`, `subscriptionTrialEndsAt` ~60d out, `FLAGS.MCP_SERVER` on, no verifier scope. Document creds in 1Password "Themia Anthropic Submission". **~1h.**

## Tue 2026-05-20 — Tool audit + Rate limit start

- [ ] **W2 — Tool surface audit.** Per-tool check: `destructiveHint: false` on `create_veille_*`; extend `enforceResponseSizeLimit` to count/options/sample tools (currently analyser-only); audit `echantillon_decisions_*` for party-name PII (strip if found); verify `visualize` prompt has no encoded directives. Fill the audit table in §4 of plan doc. **~1d.**
- [ ] **W5 — Rate limit scaffolding.** Begin `apps/api/app/mcp/rate-limit.ts` against Upstash. Sliding window per-userId: 60/min, 1000/day. **~half-day kickoff.**
- [ ] **W7 — README rewrite kickoff.** Multi-module rewrite plan; 12 worked examples drafted (3 per module). **~1h scaffolding.**

## Wed 2026-05-21 — Rate limit complete + Docs

- [ ] **W5 — Rate limit finish.** Wire into route, 429 French response, PostHog `mcp.tool.rate_limited` event, integration test for 70 calls in 60s. **~half-day.**
- [ ] **W7 — README finish.** All 4 modules covered, 12 examples, "Outils en écriture" section for veille, troubleshooting expanded. **~3h.**
- [ ] **W7 — `REVIEWER_GUIDE.md`.** Step-by-step: connect → example 1 expected output → example 2 → example 3. **~1h.**

## Thu 2026-05-22 — Privacy + Observability audit

- [ ] **W4 — Privacy policy MCP section.** Verify `https://themia.pro/legal/privacy` live. If section missing, add "Données collectées via le serveur MCP" listing exactly what `instrumentation.ts` logs (tool name, filter field names, tier, response time — NOT filter values, conversation context, or query text). **Owner: rafa for page, Christophe for legal sign-off.**
- [ ] **W6 — Observability audit memo.** `docs/specs/mcp_v1/2026-05-22-mcp-observability-audit.md`. Every logged field classified (necessary for op / debugging / extras). Decision on `McpUsage.metadata` filter values (likely keep, doc retention as 90 days). **~4h.**
- [ ] **W4 — DPA URL.** Publish standard Themia DPA at stable HTTPS URL. Link from form.

## Fri 2026-05-23 — Branding pack + Public docs

- [ ] **W8 — Logo SVG.** Themia owl wordmark, transparent BG, color + monochrome variants.
- [ ] **W8 — Favicons.** Verify `app.themia.pro` + `api.themia.pro` favicons via `https://www.google.com/s2/favicons?domain=<host>&sz=64`. Fix if missing.
- [ ] **W8 — 3 listing screenshots.** ≥1000px wide PNGs: (1) distribution query end-to-end with visualize chart, (2) DC trend, (3) create_veille flow.
- [ ] **W8 — 200-word EN directory blurb.** Smart-Brevity, editorial register.
- [ ] **W7 — Public docs page.** `themia.pro/docs/mcp` live (English, single page, mirrors README). **~2h.**

## Sat–Sun 2026-05-24/25 — Burn-in window

- [ ] **W9 — 48h burn-in.** Deploy everything to prod. PostHog `mcp.*` dashboard monitoring. Manual smoke test all 23 surface items via Claude.ai custom connector — rafa + one alpha user (Katia or Christophe). Zero P0 issues at submission time.

## Mon 2026-05-26 — Submit

- [ ] **W10 — Form submission.** `clau.de/mcp-directory-submission`. All fields filled per checklist §8. Linear "MCP Directory submission tracker" issue created with stages submitted → review-in-progress → feedback → resubmit → accepted.
- [ ] Slack alert wired on `mcp-review@anthropic.com` replies.
- [ ] Internal status check scheduled every Wednesday.

---

## Open items to resolve mid-week

1. Veille tools in scope? Default yes — pull only if W2 audit surprises.
2. EN tool descriptions: manifest only, not runtime. Confirmed cheap.
3. DPA URL: link existing org-wide DPA, not MCP-specific.
4. Listing copy register: lead EN "jurimetrics" then (jurimétrie).
5. `MCP_SERVER` flag default state in PostHog: re-verify pre-submission.

## After acceptance — Phase 2 (W11, MCP App)

Earliest start 2026-06-09 assuming 2-week review. Discovery → renderer reuse from rapports v1 → postMessage drill-down → deep-links → 5 screenshots → second submission. ~2 weeks elapsed.

**Strategic note:** Phase 1 ships us into the directory alongside Westlaw / Harvey. Phase 2 is where the moat is — "Themia inside Claude" not "Themia queryable from Claude." Different conversion funnel.

## Risks to watch

- `echantillon_decisions_*` PII surface — court decisions public but parties' names might appear. Strip if real.
- Subscription gate blocking reviewer — trial-tier test account documented.
- French-only responses confusing non-FR reviewer — EN tool descriptions + EN-comment expected outputs in README.
- `MCP_SERVER` default-off for new users — re-verify before submission.

## References

- Plan: `docs/specs/mcp_v1/2026-05-16-directory-submission-v2-plan.md`
- Policy: https://support.claude.com/en/articles/13145358-anthropic-software-directory-policy
- Submission form: clau.de/mcp-directory-submission
- Escalation: mcp-review@anthropic.com

— originally captured 2026-05-16 as 20260516T215517Z-pmiogq.md in product:mcp
