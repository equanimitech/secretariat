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
2. Add to GitHub repo secrets as `TAURI_SIGNING_PRIVATE_KEY` — the
   secret value is **the raw file contents**, not a base64-encoded
   version. The file is already base64-armored (it starts with
   `untrusted comment: rsign encrypted secret key` after one decode);
   double-encoding fails CI with `Missing encoded key in secret key`.

```bash
# Set secret directly from file contents.
gh secret set TAURI_SIGNING_PRIVATE_KEY \
  --repo equanimitech/secretariat \
  < .tauri-keys/secretariat-updater
```

If the key file has no password (current state — generated with
`tauri signer generate --ci`), no `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`
is needed.

## Apple Developer ID — runbook

Step-by-step, ordered. Each block is a discrete action you can pause between.
Validated end-to-end against the equanimitech/zenborg release on 2026-05-04.

### 1. Enroll + create certs

- Apple Developer Program enrollment ($99/yr) at developer.apple.com.
- Xcode → Settings → Accounts → "Manage Certificates" → `+` → **Developer ID
  Application**. (Add **Developer ID Installer** only if shipping `.pkg`.)
- Note the Team ID at developer.apple.com → Membership (10-char string).

### 2. Export `.p12` files

In Keychain Access → My Certificates → right-click each cert → Export →
`.p12`. Set a strong export password (you'll need it as a secret).

```bash
ls ~/Downloads/mac_dev_id.p12         # Developer ID Application
ls ~/Downloads/mac_installer_id.p12   # Developer ID Installer (optional)
```

### 3. Import to local keychain + capture signing identity

Double-click each `.p12` to import. Then:

```bash
security find-identity -v -p codesigning | grep "Developer ID"
# → 1) <SHA1> "Developer ID Application: <Your Name> (<TEAM_ID>)"
```

Copy the full quoted string — that's `APPLE_SIGNING_IDENTITY`.

### 4. App-specific password for notarization

account.apple.com → Sign-In Security → **App-Specific Passwords** → `+` →
name it `tauri-notarize` → copy the 16-char password.

This is **not** your Apple ID password. App-specific passwords are scoped
and revocable. Treat as a secret — anyone with it can submit notarization
runs under your developer account.

### 5. Push the 6 GitHub secrets

```bash
REPO=equanimitech/secretariat   # adjust as needed

gh secret set APPLE_ID --repo $REPO --body "<your-apple-id@example.com>"
gh secret set APPLE_TEAM_ID --repo $REPO --body "<TEAM_ID>"
gh secret set APPLE_SIGNING_IDENTITY --repo $REPO \
  --body "Developer ID Application: <Your Name> (<TEAM_ID>)"

# Stdin variants avoid leaking values into shell history.
gh secret set APPLE_PASSWORD --repo $REPO            # paste app-specific pw, ctrl-D
gh secret set APPLE_CERTIFICATE_PASSWORD --repo $REPO # paste .p12 export pw, ctrl-D

# Cert: base64 the .p12 directly into the secret.
base64 -i ~/Downloads/mac_dev_id.p12 | gh secret set APPLE_CERTIFICATE --repo $REPO
```

Verify:

```bash
gh secret list --repo $REPO
# Expect 6 APPLE_* + 2 TAURI_SIGNING_* secrets.
```

### 6. Trigger the workflow

```bash
git tag v0.2.0 && git push origin v0.2.0
gh run watch --repo $REPO
```

`tauri-action` handles import → sign → notarize → staple → updater sig
end-to-end. ~3 min build + ~3-7 min notarization. Released artifacts attach
to the GitHub release the workflow creates/updates.

### What Tauri does with these env vars

`tauri build` orchestrates the Apple toolchain automatically when these
env vars are set:

- `APPLE_CERTIFICATE` + `APPLE_CERTIFICATE_PASSWORD` → imports `.p12` into
  an ephemeral keychain on the runner
- `APPLE_SIGNING_IDENTITY` → passed to `codesign --sign`
- `APPLE_ID` + `APPLE_PASSWORD` + `APPLE_TEAM_ID` → passed to
  `xcrun notarytool submit --wait`, then `xcrun stapler staple`

If any of the notarization trio is missing, the build still signs but
**skips notarization** (CI logs `Warn skipping app notarization, no
APPLE_ID & APPLE_PASSWORD & APPLE_TEAM_ID …`). The resulting `.dmg`
triggers Gatekeeper warnings on first launch — fine for self-install,
not for distribution.

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

1. Build for both `aarch64-apple-darwin` and `x86_64-apple-darwin` (universal)
2. Sign with Developer ID
3. Notarize via `notarytool`
4. Staple
5. Sign updater bundle with Tauri Updater key
6. Generate `latest.json`
7. Upload all artifacts to the GitHub release

See `.github/workflows/tauri-release.yml`. Action: [`tauri-apps/tauri-action@v0`](https://github.com/tauri-apps/tauri-action)
([env reference](https://github.com/tauri-apps/tauri-action#inputs)).

### Local-build gotcha: Homebrew `xattr` shadows system `xattr`

Tauri's bundler runs `xattr -cr` on the `.app`. Homebrew installs an older
Python-based `xattr` that doesn't support `-r`, breaking `pnpm tauri build`
locally with `failed to run xattr`. CI is unaffected (clean macOS runner).

Local fix — force system `xattr` first in PATH for the build script. Edit
`package.json`:

```jsonc
"build:tauri": "PATH=/usr/bin:$PATH tauri build"
```

Verify with `which xattr` → `/usr/bin/xattr`. The system tool supports
`-cr`; the Homebrew/Python ones don't.

If the project also loads multiple env files, `dotenv-cli` accepts a
comma-list:

```jsonc
"build:tauri": "PATH=/usr/bin:$PATH dotenv -f .env.development.local,.env tauri build"
```

## Notarization expectations

- Required for **every** distributed binary on macOS 10.15+, including
  patch updates. Tauri's minisign signature ≠ Apple notarization (different
  threats: server-compromise vs. malware). Both required for a clean
  install + auto-update UX.
- Cost: free. Time: ~3-7 min per submission (Apple's queue).
- Total release wall-clock on CI: ~7-10 min (Rust build + notarize).
- For solo / pre-release testing: skip the Apple secrets and accept
  Gatekeeper warnings. `right-click → Open` on first launch bypasses.

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
