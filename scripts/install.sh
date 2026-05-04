#!/usr/bin/env bash
# Installs Secretariat from the extracted release tarball:
#   - `sec` + `sec-mcp` → ~/.local/bin (CLI on PATH)
#   - `touchid-prompt`  → ~/.secretariat/bin (helper looked up by Touch ID gate)
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
if grep -q . "${HOME}/.secretariat/did" 2>/dev/null; then
  echo "  sec invite claim <url>                  # if someone sent you an invite"
  echo "  sec invite create --purpose first-contact   # to invite someone yourself"
else
  echo "  sec init                                # generate keypair + DID"
  echo "  sec invite claim <url>                  # if you have an invite"
  echo "      OR"
  echo "  sec daemon register --endpoint <url>    # register manually with a relay"
  echo "  sec contact add --did <did> --name <n>  # add a peer"
fi
