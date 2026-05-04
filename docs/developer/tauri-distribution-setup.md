# Tauri distribution setup — keys, certs, secrets

How the signed `.dmg` + auto-update pipeline is wired. Keep current as the
setup evolves. Pairs with `docs/milestones/2026-05-04-tauri-front-door.md`.

## Two independent signing trust chains

Tauri-shipped apps use **two** independent signing keys:

1. **Apple Developer ID** — signs the `.app` binary so macOS Gatekeeper
   trusts it; required for notarization. Two certs needed:
   - *Developer ID Application* — signs the binary inside the bundle
   - *Developer ID Installer* — signs the `.pkg` (only if shipping `.pkg`;
     `.dmg` doesn't need this)
2. **Tauri Updater Ed25519** — signs the update artifact + manifest so the
   running app verifies that an update came from us (not an MitM). Generated
   via `tauri signer generate`.

These are independent — Apple trusts the binary; the running app trusts
the update bundle. Different threats, different keys.

## Tauri Updater key — generated 2026-05-04

```
.tauri-keys/secretariat-updater       # private key (gitignored)
.tauri-keys/secretariat-updater.pub   # public key (also gitignored, but safe to share)
```

The **public key** is committed in `src-tauri/tauri.conf.json` under
`plugins.updater.pubkey`. The running app uses it to verify update bundles.

The **private key** is sensitive. It currently lives only in `.tauri-keys/`
on Rafa's machine. Two things to do soon:

1. Back it up to a password manager (1Password / Bitwarden) under
   "Secretariat / Tauri updater private key."
2. Add to GitHub repo secrets as `TAURI_SIGNING_PRIVATE_KEY` (base64-encoded
   contents of the file) so CI can sign updates.

```bash
# Encode for GitHub secret
base64 -i .tauri-keys/secretariat-updater | pbcopy
# Paste into: Repo Settings → Secrets and variables → Actions → New
# Name: TAURI_SIGNING_PRIVATE_KEY
```

If the key file has no password (current state), no
`TAURI_SIGNING_PRIVATE_KEY_PASSWORD` is needed.

## Apple Developer ID — TODO

Pre-reqs:

- Apple Developer Program enrollment (in place, 2026-05-04)
- Generate two certificates via developer.apple.com or Xcode:
  - *Developer ID Application* (binary signing)
  - Optional: *Developer ID Installer* (only if shipping .pkg)
- Download `.cer` files, double-click to import to keychain
- Verify with: `security find-identity -v -p codesigning`

Then add as GitHub secrets (encoded):

```bash
# Export from keychain to .p12 (use a strong password)
# Then base64 the .p12
base64 -i developer-id-app.p12 | pbcopy
```

Repo secrets needed:

| Secret | Source |
|---|---|
| `APPLE_CERTIFICATE` | base64 of `.p12` |
| `APPLE_CERTIFICATE_PASSWORD` | password set during `.p12` export |
| `APPLE_SIGNING_IDENTITY` | full cert name, e.g. `Developer ID Application: Rafa Ballestiero (XXXXXXXXXX)` |
| `APPLE_ID` | Apple account email |
| `APPLE_PASSWORD` | app-specific password generated at appleid.apple.com (NOT account password) |
| `APPLE_TEAM_ID` | 10-char team ID (developer.apple.com top-right) |

Tauri reads these env vars during `tauri build` and orchestrates
`codesign` + `xcrun notarytool` automatically.

## tauri.conf.json wiring

```jsonc
{
  "bundle": {
    "macOS": {
      "signingIdentity": "-",        // ad-hoc until Apple secrets land; flip to "$APPLE_SIGNING_IDENTITY" or just rely on env
      "entitlements": null            // may need entitlements file for keychain access / notifications
    }
  },
  "plugins": {
    "updater": {
      "endpoints": [
        "https://github.com/equanimitech/secretariat/releases/latest/download/latest.json"
      ],
      "pubkey": "<the public key, base64-armored>"
    }
  }
}
```

The updater endpoint expects a `latest.json` manifest at that URL with
this shape (Tauri generates it during `tauri build` when
`createUpdaterArtifacts: true`):

```json
{
  "version": "0.2.1",
  "notes": "release notes",
  "pub_date": "2026-05-04T19:00:00Z",
  "platforms": {
    "darwin-aarch64": {
      "signature": "<base64 ed25519 sig>",
      "url": "https://github.com/.../Secretariat_0.2.1_aarch64.app.tar.gz"
    },
    "darwin-x86_64": {
      "signature": "...",
      "url": "..."
    }
  }
}
```

Release workflow uploads both the `.dmg` (for first-install) and the
`.app.tar.gz` + `latest.json` (for updater).

## Local build (no signing)

For dev work, `tauri build --bundles app` produces an unsigned `.app` that
runs locally. Useful for iterating on UX without going through the CI loop.

```bash
pnpm tauri build --bundles app
open src-tauri/target/release/bundle/macos/Secretariat.app
```

## CI build (signed + notarized)

Once secrets are in place, the release workflow will:

1. Build for both `aarch64-apple-darwin` and `x86_64-apple-darwin`
2. Sign with Developer ID
3. Notarize via `notarytool`
4. Staple
5. Sign updater bundle with Tauri Updater key
6. Generate `latest.json`
7. Upload all artifacts to the GitHub release

See `.github/workflows/tauri-release.yml` (TBD).

## Decision log

- **No password on the Tauri Updater private key.** Trade-off: simpler CI
  setup (no `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` secret), and the private
  key only lives in two places (Rafa's machine + GitHub secrets) — both of
  which are themselves password-protected. Re-evaluate if the threat model
  ever includes a compromised CI runner.
- **Updater endpoint is GitHub Releases**, not a custom CDN. No additional
  hosting infrastructure; releases are public anyway. If we ever want
  staged rollouts or A/B updates, point the endpoint at a small relay-side
  redirector and keep GitHub Releases as the artifact origin.
