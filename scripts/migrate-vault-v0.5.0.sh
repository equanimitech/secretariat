#!/usr/bin/env bash
# migrate-vault-v0.5.0.sh — one-shot hand-script for the namespace-collapse
# layout (pitch: docs/pitches/2026-05-17-collapse-namespaces.md).
#
# Runs against the principal's own ~/.secretariat/ tree exactly once,
# then gets deleted. Envelopes are `mv`'d, never `rm`'d or `cp`+deleted
# (per the envelopes-never-destroyed invariant in AGENTS.md +
# memory/feedback_envelopes_never_destroyed.md).
#
# Pre-flight: a tar snapshot of the entire vault lands in
# ~/Documents/secretariat-snapshots/ before any move runs. That snapshot
# is the rollback. Delete it a week after cutover when the new vault has
# accumulated enough traffic to be trusted.
#
# Post-move: walk the new tree, count `.md` envelopes, compare with
# pre-move count. Mismatch → abort + tell the principal to restore.
#
# DO NOT RUN against a vault that doesn't belong to you. Run once,
# read the diff, delete the script.

set -euo pipefail

VAULT="${SECRETARIAT_HOME:-$HOME/.secretariat}"
SNAPSHOT_DIR="$HOME/Documents/secretariat-snapshots"
TS="$(date +%Y-%m-%d-%H%M%S)"
SNAPSHOT="$SNAPSHOT_DIR/$TS-pre-collapse.tgz"

if [[ ! -d "$VAULT" ]]; then
  echo "vault not found at $VAULT — nothing to migrate" >&2
  exit 0
fi

echo "[migrate] vault: $VAULT"
echo "[migrate] snapshot target: $SNAPSHOT"

# ----- pre-flight: tar snapshot --------------------------------------------
mkdir -p "$SNAPSHOT_DIR"
if [[ -e "$SNAPSHOT" ]]; then
  echo "[migrate] snapshot already exists at $SNAPSHOT — refusing to overwrite" >&2
  exit 2
fi
echo "[migrate] taking snapshot…"
tar -czf "$SNAPSHOT" -C "$(dirname "$VAULT")" "$(basename "$VAULT")"

count_envelopes() {
  find "$1" -type f -name '*.md' \
    -path '*/envelopes/*' \
    2>/dev/null | wc -l | tr -d ' '
}

PRE_COUNT="$(count_envelopes "$VAULT")"
echo "[migrate] pre-migration envelope count: $PRE_COUNT"

# Sweep macOS .DS_Store noise so the rmdir-on-empty checks at the end
# don't false-positive on substrate-private dirs.
echo "[migrate] sweeping .DS_Store…"
find "$VAULT" -name '.DS_Store' -delete 2>/dev/null || true

# ----- target layout --------------------------------------------------------
SELF_ROOT="$VAULT/_self"
SELF_CHANNELS="$SELF_ROOT/channels"
mkdir -p "$SELF_CHANNELS"

# ----- step 1: move top-level channels/<X>/ → _self/channels/<X>/ ----------
# (these were "personal" channels with no namespace prefix already)
if [[ -d "$VAULT/channels" ]]; then
  echo "[migrate] moving channels/* → _self/channels/*"
  for entry in "$VAULT/channels"/*; do
    [[ -e "$entry" ]] || continue
    name="$(basename "$entry")"
    if [[ -e "$SELF_CHANNELS/$name" ]]; then
      echo "[migrate] collision: $SELF_CHANNELS/$name already exists; bailing for manual rename" >&2
      exit 3
    fi
    mv "$entry" "$SELF_CHANNELS/$name"
  done
  # Empty directory only.
  rmdir "$VAULT/channels" 2>/dev/null || true
fi

# ----- step 2: queues/<ns>/<slug>/ → _self/channels/<ns>/<slug>/ -----------
# Each flat-queue namespace becomes a top-level channel segment under
# _self/channels/. `inbox/triage` → `inbox/triage`, `area/health` →
# `area/health`, `project/foo` → `project/foo`. The handle grammar in
# v0.5 happens to keep those readable as bare paths.
if [[ -d "$VAULT/queues" ]]; then
  echo "[migrate] moving queues/<ns>/* → _self/channels/<ns>/*"
  for ns_dir in "$VAULT/queues"/*; do
    [[ -d "$ns_dir" ]] || continue
    ns="$(basename "$ns_dir")"
    # Skip dotfiles like .contextification.log at queues-root level —
    # they move via step 3.
    [[ "$ns" == .* ]] && continue
    target_ns="$SELF_CHANNELS/$ns"
    mkdir -p "$target_ns"
    for slug_dir in "$ns_dir"/*; do
      [[ -e "$slug_dir" ]] || continue
      slug="$(basename "$slug_dir")"
      if [[ -e "$target_ns/$slug" ]]; then
        echo "[migrate] collision: $target_ns/$slug already exists; bailing for manual rename" >&2
        exit 3
      fi
      mv "$slug_dir" "$target_ns/$slug"
    done
    rmdir "$ns_dir" 2>/dev/null || true
  done
fi

# ----- step 3: queues/.contextification.log → _self/.contextification.log ---
if [[ -f "$VAULT/queues/.contextification.log" ]]; then
  echo "[migrate] moving .contextification.log → _self/"
  mv "$VAULT/queues/.contextification.log" "$SELF_ROOT/.contextification.log"
fi

# ----- step 4: drain leftover empty queues/ --------------------------------
if [[ -d "$VAULT/queues" ]]; then
  # Only succeed if empty.
  rmdir "$VAULT/queues" 2>/dev/null || {
    echo "[migrate] queues/ not empty after migration — inspect manually" >&2
    ls -la "$VAULT/queues"
    exit 4
  }
fi

# ----- step 5: peers/ — empty in practice ---------------------------------
if [[ -d "$VAULT/peers" ]]; then
  rmdir "$VAULT/peers" 2>/dev/null || echo "[migrate] peers/ not empty; leaving in place"
fi

# ----- gate: envelope count must match -------------------------------------
POST_COUNT="$(count_envelopes "$VAULT")"
echo "[migrate] post-migration envelope count: $POST_COUNT"

if [[ "$PRE_COUNT" != "$POST_COUNT" ]]; then
  echo "[migrate] ENVELOPE COUNT MISMATCH ($PRE_COUNT → $POST_COUNT)" >&2
  echo "[migrate] restore from snapshot:" >&2
  echo "  rm -rf $VAULT && tar -xzf $SNAPSHOT -C $(dirname "$VAULT")" >&2
  exit 5
fi

echo "[migrate] done. envelopes preserved ($PRE_COUNT)."
echo "[migrate] snapshot at: $SNAPSHOT"
echo "[migrate] after a week of clean operation, delete this script + snapshot."
