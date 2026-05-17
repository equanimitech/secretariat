#!/usr/bin/env bash
# migrate-vault-v0.7.0.sh — hand-script for the v0.7.0 layout cutover.
#
# v0.7.0 bundles three slices:
#   slice 3 — identity consolidation:
#     <root>/key            → <self>/identity/key
#     <root>/did.json       → <self>/identity/did.json
#     <root>/did + profile.json → <self>/identity.md (frontmatter)
#
#   slice 4 — principal context to markdown:
#     <root>/contacts.json  → <self>/contacts.md
#     <org>/.org            → <org>/org.md (frontmatter+body)
#
#   queue_dir alignment — peer queues gain `channels/` layer:
#     <peer-alias>/<handle-segs>/ → <peer-alias>/channels/<handle-segs>/
#
# Discipline (per envelopes-never-destroyed + slice-3 pitch):
#   - `mv`-only on envelope-bearing paths AND on the key file.
#   - Legacy JSON files `mv`'d to `.archive/`, never `rm`'d.
#   - Pre-flight `tar` snapshot to ~/Documents/secretariat-snapshots/.
#   - Post-move count check (envelope plaintext .md files).
#   - On mismatch: abort + report the restore command.
#
# DO NOT RUN against a vault that isn't yours. Run once, verify, delete.

set -euo pipefail

VAULT="${SECRETARIAT_HOME:-$HOME/.secretariat}"
SNAPSHOT_DIR="$HOME/Documents/secretariat-snapshots"
TS="$(date +%Y-%m-%d-%H%M%S)"
SNAPSHOT="$SNAPSHOT_DIR/$TS-pre-v0.7.0.tgz"
SELF_ROOT="$VAULT/_self"
ARCHIVE="$VAULT/.archive"

if [[ ! -d "$VAULT" ]]; then
  echo "vault not found at $VAULT — nothing to migrate" >&2
  exit 0
fi

echo "[migrate] vault:      $VAULT"
echo "[migrate] self root:  $SELF_ROOT"
echo "[migrate] snapshot:   $SNAPSHOT"
echo "[migrate] archive:    $ARCHIVE"

# ----- pre-flight --------------------------------------------------------
mkdir -p "$SNAPSHOT_DIR"
if [[ -e "$SNAPSHOT" ]]; then
  echo "[migrate] snapshot already exists at $SNAPSHOT — refusing to overwrite" >&2
  exit 2
fi
echo "[migrate] taking snapshot…"
tar -czf "$SNAPSHOT" -C "$(dirname "$VAULT")" "$(basename "$VAULT")" \
  --exclude='*.sock' 2>/dev/null || true

# Sweep macOS .DS_Store noise so rmdir-on-empty checks don't false-positive.
echo "[migrate] sweeping .DS_Store…"
find "$VAULT" -name '.DS_Store' -delete 2>/dev/null || true

count_envelopes() {
  find "$1" -type f -name '*.md' -path '*/envelopes/*' 2>/dev/null | wc -l | tr -d ' '
}

PRE_COUNT="$(count_envelopes "$VAULT")"
echo "[migrate] pre-migration envelope count: $PRE_COUNT"

mkdir -p "$SELF_ROOT"
mkdir -p "$ARCHIVE"

# ----- slice 3 — identity consolidation ----------------------------------
if [[ -f "$VAULT/key" && ! -f "$SELF_ROOT/identity/key" ]]; then
  echo "[migrate] slice 3: consolidating identity → $SELF_ROOT/identity{.md,/key,/did.json}"
  mkdir -p "$SELF_ROOT/identity"

  # Pull current values for the new identity.md frontmatter.
  did_val=""
  if [[ -f "$VAULT/did" ]]; then
    did_val="$(tr -d '\n\r ' < "$VAULT/did")"
  fi
  display_name="Principal"
  full_name=""
  if [[ -f "$VAULT/profile.json" ]]; then
    if command -v jq >/dev/null 2>&1; then
      display_name="$(jq -r '.display_name // "Principal"' "$VAULT/profile.json")"
      full_name="$(jq -r '.full_name // ""' "$VAULT/profile.json")"
    fi
  fi
  did_method="did:key"
  case "$did_val" in
    did:web:*) did_method="did:web" ;;
    did:key:*) did_method="did:key" ;;
  esac
  now="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"

  # Emit identity.md.
  {
    echo "---"
    echo "\$type: tech.equanimi.secretariat.identity"
    echo "did: $did_val"
    echo "did_method: $did_method"
    echo "display_name: $display_name"
    if [[ -n "$full_name" ]]; then
      echo "full_name: $full_name"
    fi
    echo "key_path: identity/key"
    echo "key_type: ed25519"
    echo "key_created_at: $now"
    echo "key_rotations: []"
    echo "created_at: $now"
    echo "---"
    echo ""
    echo "# identity"
    echo ""
  } > "$SELF_ROOT/identity.md"

  # Move key + did.json (NEVER rm; mv preserves inode).
  mv "$VAULT/key" "$SELF_ROOT/identity/key"
  [[ -f "$VAULT/did.json" ]] && mv "$VAULT/did.json" "$SELF_ROOT/identity/did.json"

  # Archive the consumed JSON/text inputs.
  [[ -f "$VAULT/did" ]] && mv "$VAULT/did" "$ARCHIVE/did"
  [[ -f "$VAULT/profile.json" ]] && mv "$VAULT/profile.json" "$ARCHIVE/profile.json"
else
  echo "[migrate] slice 3: skipped (identity already at new path or no legacy key)"
fi

# ----- slice 4 — contacts.json → _self/contacts.md -----------------------
if [[ -f "$VAULT/contacts.json" && ! -f "$SELF_ROOT/contacts.md" ]]; then
  echo "[migrate] slice 4: converting contacts.json → $SELF_ROOT/contacts.md"
  if ! command -v python3 >/dev/null 2>&1; then
    echo "[migrate] python3 required for contacts conversion; aborting" >&2
    exit 6
  fi
  python3 - "$VAULT/contacts.json" "$SELF_ROOT/contacts.md" <<'PYEOF'
import json, sys
src, dst = sys.argv[1], sys.argv[2]
data = json.load(open(src))
out = ["# Contacts\n"]
for c in data.get("contacts", []):
    name = c.get("display_name") or c.get("did") or "unknown"
    out.append(f"\n## {name}\n\n")
    out.append("---\n")
    out.append("$type: tech.equanimi.secretariat.contact\n")
    for k in ("did", "display_name", "full_name", "relay_endpoint", "added_at"):
        v = c.get(k)
        if v not in (None, ""):
            out.append(f"{k}: {v}\n")
    out.append("---\n")
with open(dst, "w") as f:
    f.write("".join(out))
PYEOF
  chmod 600 "$SELF_ROOT/contacts.md" || true
  mv "$VAULT/contacts.json" "$ARCHIVE/contacts.json"
else
  echo "[migrate] slice 4 contacts: skipped (already migrated or no legacy file)"
fi

# ----- slice 4 — orgs/<alias>/.org → orgs/<alias>/org.md -----------------
if [[ -d "$VAULT/orgs" ]]; then
  for org_dir in "$VAULT/orgs"/*/; do
    [[ -d "$org_dir" ]] || continue
    legacy="$org_dir.org"
    new="$org_dir""org.md"
    if [[ -f "$legacy" && ! -f "$new" ]]; then
      alias_name="$(basename "$org_dir")"
      echo "[migrate] slice 4: converting $legacy → $new"
      if ! command -v python3 >/dev/null 2>&1; then
        echo "[migrate] python3 required; aborting" >&2
        exit 6
      fi
      python3 - "$legacy" "$new" <<'PYEOF'
import json, sys
src, dst = sys.argv[1], sys.argv[2]
data = json.load(open(src))
alias = data.get("alias", "")
name = data.get("name") or alias
description = data.get("description", "")
did = data.get("did", "")
created_at = data.get("created_at", "")
lines = ["---\n", "$type: tech.equanimi.secretariat.org\n"]
if alias: lines.append(f"alias: {alias}\n")
if did:   lines.append(f"did: {did}\n")
if name:  lines.append(f"name: {name}\n")
if description: lines.append(f"description: {description}\n")
if created_at:  lines.append(f"created_at: {created_at}\n")
lines.append("---\n\n")
lines.append(f"# {name}\n\n")
with open(dst, "w") as f:
    f.writelines(lines)
PYEOF
      # Archive legacy under .archive/orgs/<alias>/.
      mkdir -p "$ARCHIVE/orgs/$alias_name"
      mv "$legacy" "$ARCHIVE/orgs/$alias_name/.org"
    fi
  done
else
  echo "[migrate] slice 4 orgs: no orgs/ dir"
fi

# ----- queue_dir alignment — peer queues gain channels/ layer ------------
# Walk top-level dirs. Skip reserved names. For each remaining alias
# directory, if it has subdirs that aren't `channels/`, move them under
# a new `channels/` layer.
echo "[migrate] queue_dir alignment: peer queues → channels/ layer"
for peer_dir in "$VAULT"/*/; do
  [[ -d "$peer_dir" ]] || continue
  name="$(basename "$peer_dir")"
  case "$name" in
    _self|orgs|bin|peers|logs|.archive|.runtime) continue ;;
  esac

  # Already migrated?
  if [[ -d "$peer_dir/channels" ]]; then
    continue
  fi

  # Collect dirs that need wrapping.
  shopt -s nullglob
  to_move=("$peer_dir"*/)
  shopt -u nullglob
  [[ ${#to_move[@]} -eq 0 ]] && continue

  echo "[migrate]   $name: wrapping in channels/"
  mkdir -p "$peer_dir/channels"
  for sub in "${to_move[@]}"; do
    sub_name="$(basename "$sub")"
    [[ "$sub_name" == "channels" ]] && continue
    mv "$sub" "$peer_dir/channels/$sub_name"
  done
done

# ----- gate: envelope count must match -----------------------------------
POST_COUNT="$(count_envelopes "$VAULT")"
echo "[migrate] post-migration envelope count: $POST_COUNT"

if [[ "$PRE_COUNT" != "$POST_COUNT" ]]; then
  echo "[migrate] ENVELOPE COUNT MISMATCH ($PRE_COUNT → $POST_COUNT)" >&2
  echo "[migrate] restore from snapshot:" >&2
  echo "  rm -rf $VAULT && tar -xzf $SNAPSHOT -C $(dirname "$VAULT")" >&2
  exit 5
fi

# ----- gate: DID roundtrip -----------------------------------------------
if [[ -f "$SELF_ROOT/identity.md" ]]; then
  new_did="$(grep -E '^did:' "$SELF_ROOT/identity.md" | head -1 | sed 's/^did: *//')"
  echo "[migrate] DID in identity.md: $new_did"
  if [[ -z "$new_did" ]]; then
    echo "[migrate] identity.md missing 'did:' line — aborting" >&2
    exit 7
  fi
fi

echo "[migrate] done. envelopes preserved ($PRE_COUNT)."
echo "[migrate] snapshot at: $SNAPSHOT"
echo "[migrate] legacy JSON archived at: $ARCHIVE"
echo "[migrate] after a week of clean operation, delete this script + snapshot + archive."
