#!/usr/bin/env bash
# Bootstrap Themia + EquanimiTech orgs and their channels into the local
# Secretariat substrate. Mirrors the Slack channel taxonomy as of
# 2026-05-12. Idempotent — re-running re-creates anything missing,
# errors loudly on anything already present (which is fine; just keep going).
#
# Run from the repo root: `bash scripts/bootstrap-themia-equanimitech.sh`

set -uo pipefail

SEC="${SEC:-./target/debug/sec}"

if [[ ! -x "$SEC" ]]; then
  echo "error: $SEC not found. Build it first: cargo build -p secretariat-cli --bin sec" >&2
  exit 1
fi

echo "==> Creating orgs"
$SEC orgs create themia.pro \
  --did did:web:themia.pro \
  --name "Themia" \
  --description "Jurimetric platform for French legal practitioners" \
  || echo "  themia.pro: already exists or error"

$SEC orgs create equanimi.tech \
  --did did:web:equanimi.tech \
  --name "EquanimiTech" \
  --description "Tooling for autonomous enterprises" \
  || echo "  equanimi.tech: already exists or error"

create_channel() {
  local org="$1"
  local handle="$2"
  local name="$3"
  shift 3
  local description="$*"
  if ! $SEC channels create "$handle" --org "$org" --name "$name" --description "$description"; then
    echo "  ${org}/${handle}: already exists or error"
  fi
}

echo ""
echo "==> Themia channels"

# analytics:*
create_channel themia.pro channel:analytics:clients         "Analytics — clients" "Client analytics + dashboards"
create_channel themia.pro channel:analytics:finances        "Analytics — finances" "Financial analytics"
create_channel themia.pro channel:analytics:mcp             "Analytics — MCP" "MCP usage analytics"
create_channel themia.pro channel:analytics:pipeline        "Analytics — pipeline" "Data pipeline analytics"

# com:*
create_channel themia.pro channel:com:analytics             "Com — analytics" "Marketing analytics"
create_channel themia.pro channel:com:blog                  "Com — blog" "Blog posts + content"
create_channel themia.pro channel:com:landing-page          "Com — landing-page" "Landing-page iteration"
create_channel themia.pro channel:com:linkedin              "Com — LinkedIn" "LinkedIn presence + posts"
create_channel themia.pro channel:com:newsletter            "Com — newsletter" "Email newsletter"
create_channel themia.pro channel:com:webinaire             "Com — webinaire" "Webinar planning + replays"

# discussion:*
create_channel themia.pro channel:discussion:acquereurs     "Discussion — acquéreurs" "Acquéreur conversations"
create_channel themia.pro channel:discussion:clients        "Discussion — clients" "Client conversations"

# ops:*
create_channel themia.pro channel:ops:audit                 "Ops — audit" "Audits + compliance"
create_channel themia.pro channel:ops:compta                "Ops — compta" "Comptabilité + bookkeeping"
create_channel themia.pro channel:ops:expenses              "Ops — expenses" "Expense tracking"
create_channel themia.pro channel:ops:finances              "Ops — finances" "Financial operations"
create_channel themia.pro channel:ops:gpt-support           "Ops — GPT support" "GPT/LLM ops"

# product (top + subtree)
create_channel themia.pro channel:product                   "Product" "Product strategy + roadmap"
create_channel themia.pro channel:product:data              "Product — data" "Data product"
create_channel themia.pro channel:product:data:baux-commerciaux \
                                                            "Product — BC" "Baux commerciaux module"
create_channel themia.pro channel:product:data:ccass        "Product — Cass" "Cour de cassation module"
create_channel themia.pro channel:product:data:construction "Product — construction" "Droit de la construction module"
create_channel themia.pro channel:product:data:travail      "Product — travail" "Droit du travail module"
create_channel themia.pro channel:product:deployments       "Product — deployments" "Release + deployment"
create_channel themia.pro channel:product:feedback          "Product — feedback" "User feedback intake"
create_channel themia.pro channel:product:onboarding        "Product — onboarding" "User onboarding flow"
create_channel themia.pro channel:product:mcpveriguard      "Product — MCP Veriguard" "Veriguard MCP verification surface"

# Unprefixed
create_channel themia.pro channel:association               "Association" "Themia association"
create_channel themia.pro channel:competition               "Competition" "Competitive landscape"
create_channel themia.pro channel:data-status               "Data status" "Data pipeline status"
create_channel themia.pro channel:encyclopedie-jurimetrie   "Encyclopédie jurimétrie" "Internal jurimetry glossary"
create_channel themia.pro channel:general                   "General" "Catch-all org-wide"
create_channel themia.pro channel:hiring                    "Hiring" "Recruiting + hiring"
create_channel themia.pro channel:market                    "Market" "Market intelligence"
create_channel themia.pro channel:questions-clients         "Questions clients" "Client questions inbox"
create_channel themia.pro channel:random                    "Random" "Off-topic"

echo ""
echo "==> EquanimiTech channels"

create_channel equanimi.tech channel:secretariat            "Secretariat" "secretariat repo + product"
create_channel equanimi.tech channel:zenborg                "Zenborg" "zenborg repo + product"
create_channel equanimi.tech channel:penceive               "Penceive" "penceive repo + product"
create_channel equanimi.tech channel:equanimi               "Equanimi" "equanimi repo (umbrella)"
create_channel equanimi.tech channel:general                "General" "Catch-all org-wide"
create_channel equanimi.tech channel:random                 "Random" "Off-topic"

echo ""
echo "==> Done. Verifying:"
echo ""
$SEC orgs list
echo ""
echo "  themia.pro:"
$SEC channels list --org themia.pro
echo ""
echo "  equanimi.tech:"
$SEC channels list --org equanimi.tech
