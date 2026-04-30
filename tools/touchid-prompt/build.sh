#!/usr/bin/env bash
# Build the Touch ID helper binary into `target/touchid-prompt`.
# Run from anywhere — script resolves its own location.
set -euo pipefail

dir="$(cd "$(dirname "$0")" && pwd)"
repo_root="$(cd "$dir/../.." && pwd)"
out_dir="$repo_root/target"
mkdir -p "$out_dir"

swiftc -O "$dir/main.swift" -o "$out_dir/touchid-prompt"
chmod +x "$out_dir/touchid-prompt"
echo "built $out_dir/touchid-prompt"
