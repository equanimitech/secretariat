#!/usr/bin/env bash
# Installs Secretariat from the extracted release tarball:
#   - `sec` + `sec-mcp` → ~/.local/bin (CLI on PATH)
#   - `touchid-prompt`  → ~/.secretariat/bin (helper looked up by Touch ID gate)
#   - generates ed25519 signing key + did:key identity (`sec init`)
#   - installs LaunchAgent so the daemon polls + delivers in the background
#   - wires sec-mcp into Claude Desktop + Claude Code
#
# After this completes, the only step left for the user is:
#   1. Restart Claude Code (so it picks up the new MCP server)
#   2. Paste the invite URL into a Claude conversation
#
# No sudo required.

set -euo pipefail

INSTALL_DIR="${HOME}/.local/bin"
HELPER_DIR="${HOME}/.secretariat/bin"
mkdir -p "${INSTALL_DIR}" "${HELPER_DIR}"

# ── Stage 1: place binaries ───────────────────────────────────────────────────
for binary in sec sec-mcp; do
  if [ ! -f "${binary}" ]; then
    echo "error: ${binary} not found in current directory" >&2
    exit 1
  fi
  install -m 0755 "${binary}" "${INSTALL_DIR}/${binary}"
done

if [ -f "touchid-prompt" ]; then
  install -m 0755 "touchid-prompt" "${HELPER_DIR}/touchid-prompt"
else
  echo "warning: touchid-prompt not found in this tarball — sec stamp will fail until you build it from tools/touchid-prompt/build.sh" >&2
fi

# ── Stage 2: ensure ~/.local/bin is on PATH for THIS process so we can call sec
case ":${PATH}:" in
  *":${INSTALL_DIR}:"*) PATH_OK=1 ;;
  *)
    PATH_OK=0
    export PATH="${INSTALL_DIR}:${PATH}"
    ;;
esac

SEC="${INSTALL_DIR}/sec"

# ── Stage 3: identity (`sec init`). Idempotent — skipped if a DID already exists.
DID_FILE="${HOME}/.secretariat/did"
if [ -s "${DID_FILE}" ]; then
  DID="$(cat "${DID_FILE}")"
  INIT_DID_SOURCE="(reusing existing identity)"
else
  "${SEC}" init >/dev/null 2>&1 || {
    echo "error: \`sec init\` failed; run it manually to see the error" >&2
    exit 1
  }
  DID="$(cat "${DID_FILE}")"
  INIT_DID_SOURCE="(generated)"
fi

# ── Stage 4: install LaunchAgent so daemon runs in background.
DAEMON_INSTALL_OUT="$("${SEC}" daemon install 2>&1)" || {
  echo "warning: \`sec daemon install\` failed:" >&2
  echo "${DAEMON_INSTALL_OUT}" >&2
  echo "  re-run \`sec daemon install\` after fixing the issue." >&2
}

# ── Stage 5: wire sec-mcp into Claude.
MCP_OUT="$("${SEC}" mcp install --binary "${INSTALL_DIR}/sec-mcp" 2>&1)" || {
  echo "warning: MCP wiring failed (Claude not yet installed?):" >&2
  echo "${MCP_OUT}" >&2
  echo "  re-run \`sec mcp install\` after Claude is installed." >&2
}

# ── Stage 6: human-readable end-state. This is the screen the user sees.
cat <<EOF

╭──────────────────────────────────────────────────────────────────╮
│  ✓  Secretariat installed                                        │
╰──────────────────────────────────────────────────────────────────╯

  identity        : ${DID} ${INIT_DID_SOURCE}
  CLI binary      : ${SEC}
  MCP binary      : ${INSTALL_DIR}/sec-mcp
  daemon          : LaunchAgent loaded (polls relay in background)
  Claude wiring   : sec-mcp registered with Claude Desktop + Claude Code

EOF

if [ "${PATH_OK}" = "0" ]; then
  cat <<EOF
  ⚠  ${INSTALL_DIR} is NOT on your PATH yet. Add this line to ~/.zshrc
     (or ~/.bashrc) and open a new Terminal window:

         export PATH="${INSTALL_DIR}:\$PATH"

EOF
fi

cat <<EOF
  Next steps
  ──────────
   1. Quit Claude Code (or Claude Desktop) completely and reopen it.
      This is what loads the Secretariat MCP server into your session.

   2. In a new Claude conversation, paste the invite URL the inviter
      sent you (it looks like https://…/v0/invite/…) and ask Claude
      to claim it.

   3. That's it. Claude will report when you're connected, and surface
      envelopes as they arrive.

EOF
