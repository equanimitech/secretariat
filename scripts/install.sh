#!/usr/bin/env bash
# Installs `sec` + `sec-mcp` from this directory into ~/.local/bin (creating
# it + adding to PATH if needed). Run after extracting the release tarball:
#
#     tar xzf secretariat-darwin-arm64.tar.gz
#     cd secretariat && bash install.sh
#
# No sudo required.

set -euo pipefail

INSTALL_DIR="${HOME}/.local/bin"
mkdir -p "${INSTALL_DIR}"

for binary in sec sec-mcp; do
  if [ ! -f "${binary}" ]; then
    echo "error: ${binary} not found in current directory" >&2
    exit 1
  fi
  install -m 0755 "${binary}" "${INSTALL_DIR}/${binary}"
  echo "installed ${INSTALL_DIR}/${binary}"
done

# Hint to add ~/.local/bin to PATH if it isn't already.
case ":${PATH}:" in
  *":${INSTALL_DIR}:"*) ;;
  *)
    echo
    echo "note: ${INSTALL_DIR} is not in your PATH. Add this line to your shell rc:"
    echo "    export PATH=\"${INSTALL_DIR}:\$PATH\""
    ;;
esac

echo
echo "next steps:"
echo "  sec init                                            # generate keypair + DID"
echo "  sec daemon register --endpoint <relay-url>          # register with a relay"
echo "  sec contact add --did <peer-did> --name <name>      # add your first contact"
