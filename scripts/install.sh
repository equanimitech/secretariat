#!/usr/bin/env bash
# Installs Secretariat from the extracted release tarball:
#   - `sec` + `sec-mcp` → ~/.local/bin (CLI on PATH)
#   - `touchid-prompt`  → ~/.secretariat/bin (helper looked up by Touch ID gate)
#   - wires sec-mcp into Claude Desktop + Claude Code (no JSON editing)
#
# Onboarding (init, invite claim, daemon) happens AFTER this — through
# MCP tools, once Claude Code/Desktop is restarted to pick up the server.
#
# Run after extracting the release tarball:
#
#     tar xzf secretariat-darwin-arm64.tar.gz
#     cd secretariat && bash install.sh
#
# No sudo required.

set -euo pipefail

INSTALL_DIR="${HOME}/.local/bin"
HELPER_DIR="${HOME}/.secretariat/bin"
mkdir -p "${INSTALL_DIR}" "${HELPER_DIR}"

for binary in sec sec-mcp; do
  if [ ! -f "${binary}" ]; then
    echo "error: ${binary} not found in current directory" >&2
    exit 1
  fi
  install -m 0755 "${binary}" "${INSTALL_DIR}/${binary}"
  echo "installed ${INSTALL_DIR}/${binary}"
done

# Touch ID helper is macOS-only; tarball is also macOS-only so we expect it.
if [ -f "touchid-prompt" ]; then
  install -m 0755 "touchid-prompt" "${HELPER_DIR}/touchid-prompt"
  echo "installed ${HELPER_DIR}/touchid-prompt"
else
  echo "warning: touchid-prompt not found in this tarball — sec stamp will fail until you build it from tools/touchid-prompt/build.sh" >&2
fi

# Hint to add ~/.local/bin to PATH if it isn't already.
case ":${PATH}:" in
  *":${INSTALL_DIR}:"*) PATH_OK=1 ;;
  *)
    PATH_OK=0
    echo
    echo "note: ${INSTALL_DIR} is not in your PATH. Add this line to your shell rc:"
    echo "    export PATH=\"${INSTALL_DIR}:\$PATH\""
    ;;
esac

# Wire sec-mcp into Claude Desktop / Claude Code automatically. Skipped
# silently if neither is installed; a warning is enough — the user can
# always re-run `sec mcp install` later.
if [ "${PATH_OK:-0}" = "1" ] || [ -x "${INSTALL_DIR}/sec" ]; then
  echo
  echo "[install] wiring sec-mcp into Claude Desktop + Claude Code..."
  if "${INSTALL_DIR}/sec" mcp install --binary "${INSTALL_DIR}/sec-mcp"; then
    :
  else
    echo "[install] (MCP wiring skipped — re-run \`sec mcp install\` after Claude is installed.)"
  fi
fi

echo
echo "next steps:"
echo "  1. Restart Claude Code (or Claude Desktop) so the new MCP server loads."
echo "  2. In the new session, ask your assistant to run the secretariat tools:"
echo "       secretariat__init           — generate your keypair + DID"
echo "       secretariat__invite_claim   — claim an invite URL (auto-registers + adds inviter)"
echo "       secretariat__daemon_install — install the LaunchAgent (background poller)"
echo "       secretariat__daemon_status  — confirm it's running"
echo
echo "Or, if you prefer the terminal:"
if grep -q . "${HOME}/.secretariat/did" 2>/dev/null; then
  echo "  sec invite claim <url>"
  echo "  sec daemon install"
else
  echo "  sec init"
  echo "  sec invite claim <url>      (or  sec daemon register --endpoint <url>)"
  echo "  sec daemon install"
fi
