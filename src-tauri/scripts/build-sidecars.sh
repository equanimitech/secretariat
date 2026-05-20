#!/usr/bin/env bash
# Build `sec` and `sec-mcp` for the current target triple and stage them
# under `src-tauri/binaries/<bin>-<triple>` for Tauri's `bundle.externalBin`.
#
# Invoked by Tauri's `beforeBundleCommand`. The triple suffix is required by
# Tauri sidecars: it picks up `binaries/sec-<triple>` at bundle time and
# strips the suffix when copying into `Contents/MacOS/`.
#
# Override the target by exporting TARGET (CI sets this for cross-builds).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$WORKSPACE_ROOT"

TARGET="${TARGET:-$(rustc -vV | sed -n 's|host: ||p')}"

echo "[sidecars] target = $TARGET"
echo "[sidecars] building sec + sec-mcp (release)"
cargo build --release --target "$TARGET" --bin sec --bin sec-mcp

DEST="$WORKSPACE_ROOT/src-tauri/binaries"
mkdir -p "$DEST"
cp "target/$TARGET/release/sec"     "$DEST/sec-$TARGET"
cp "target/$TARGET/release/sec-mcp" "$DEST/sec-mcp-$TARGET"

# Cognition sidecar — Bun-compiled TS wrapping @anthropic-ai/claude-agent-sdk.
# Drives the in-process streaming chat for the Tauri tab strip
# (CognitionSession port).
COG_DIR="$WORKSPACE_ROOT/crates/cognition-claude-sdk"
if [ -d "$COG_DIR" ]; then
  echo "[sidecars] building cognition-claude-sdk via bun"
  (
    cd "$COG_DIR"
    if [ ! -d node_modules ]; then bun install; fi
    bun build --compile --minify --target=bun \
      --outfile "dist/cognition-claude-sdk" index.ts
  )
  cp "$COG_DIR/dist/cognition-claude-sdk" "$DEST/cognition-claude-sdk-$TARGET"
fi

echo "[sidecars] staged:"
ls -1 "$DEST"
