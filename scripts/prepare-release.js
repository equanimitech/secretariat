#!/usr/bin/env node

import fs from 'fs'
import path from 'path'
import { execSync } from 'child_process'
import readline from 'readline'

// Every file whose version string must match the release tag.
// Adding a new versioned manifest? Add it here — the post-bump assertion
// below will refuse to release if any one of these drifts from the target.
const VERSION_FILES = [
  { path: 'package.json', kind: 'json', key: 'version' },
  { path: 'src-tauri/tauri.conf.json', kind: 'json', key: 'version' },
  { path: 'src-tauri/Cargo.toml', kind: 'toml-package' },
  { path: 'crates/core/Cargo.toml', kind: 'toml-package' },
  { path: 'crates/cli/Cargo.toml', kind: 'toml-package' },
  { path: 'crates/daemon/Cargo.toml', kind: 'toml-package' },
  { path: 'crates/mcp/Cargo.toml', kind: 'toml-package' },
  { path: 'crates/relay/Cargo.toml', kind: 'toml-package' },
]

// Match the [package] block's `version = "..."` line, anchored at the
// start of the file or after a newline so it can't drift into a [dependencies]
// sub-table whose entries also use `version = "..."`.
const PKG_VERSION_RE = /(^|\n)(\[package\][\s\S]*?\nversion = ")([^"]+)(")/

function readVersion(file) {
  const raw = fs.readFileSync(file.path, 'utf8')
  if (file.kind === 'json') return JSON.parse(raw)[file.key]
  const m = raw.match(PKG_VERSION_RE)
  return m ? m[3] : null
}

function writeVersion(file, version) {
  const raw = fs.readFileSync(file.path, 'utf8')
  if (file.kind === 'json') {
    const obj = JSON.parse(raw)
    obj[file.key] = version
    fs.writeFileSync(file.path, JSON.stringify(obj, null, 2) + '\n')
    return
  }
  const updated = raw.replace(PKG_VERSION_RE, `$1$2${version}$4`)
  if (updated === raw) {
    throw new Error(`No [package] version found in ${file.path}`)
  }
  fs.writeFileSync(file.path, updated)
}

function exec(command, options = {}) {
  try {
    return execSync(command, {
      encoding: 'utf8',
      stdio: options.silent ? 'pipe' : 'inherit',
      ...options,
    })
  } catch (error) {
    throw new Error(`Command failed: ${command}\n${error.message}`)
  }
}

function askQuestion(question) {
  const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout,
  })

  return new Promise(resolve => {
    rl.question(question, answer => {
      rl.close()
      resolve(answer.trim())
    })
  })
}

async function prepareRelease() {
  const version = process.argv[2]

  if (!version || !version.match(/^v?\d+\.\d+\.\d+$/)) {
    console.error('❌ Usage: node scripts/prepare-release.js v1.0.0')
    console.error('   or: pnpm prepare-release v1.0.0')
    process.exit(1)
  }

  const cleanVersion = version.replace('v', '')
  const tagVersion = version.startsWith('v') ? version : `v${version}`

  console.log(`🚀 Preparing release ${tagVersion}...\n`)

  try {
    // Check git status
    console.log('🔍 Checking git status...')
    const gitStatus = exec('git status --porcelain', { silent: true })
    if (gitStatus.trim()) {
      console.error(
        '❌ Working directory is not clean. Please commit or stash changes first.'
      )
      console.log('Uncommitted changes:')
      console.log(gitStatus)
      process.exit(1)
    }
    console.log('✅ Working directory is clean')

    // Run all checks first
    console.log('\n🔍 Running pre-release checks...')
    exec('npm run check:all')
    console.log('✅ All checks passed')

    // Bump every versioned manifest in lockstep.
    console.log('\n📝 Bumping version across all manifests...')
    for (const file of VERSION_FILES) {
      const before = readVersion(file)
      writeVersion(file, cleanVersion)
      console.log(`   ${file.path.padEnd(38)} ${before} → ${cleanVersion}`)
    }

    // Post-bump assertion: every manifest reports the target version.
    // This is the guard that would have caught the v0.9.0 incident where
    // tauri.conf.json drifted while everything else bumped.
    console.log('\n🔒 Verifying version consistency...')
    const mismatches = []
    for (const file of VERSION_FILES) {
      const v = readVersion(file)
      if (v !== cleanVersion) mismatches.push(`${file.path}: ${v}`)
    }
    if (mismatches.length) {
      console.error('❌ Version mismatch after bump:')
      for (const m of mismatches) console.error(`   ${m}`)
      process.exit(1)
    }
    console.log(`✅ All ${VERSION_FILES.length} manifests at ${cleanVersion}`)

    // Hold the tauri config in scope for the bundle/updater warnings below.
    const tauriConfig = JSON.parse(
      fs.readFileSync('src-tauri/tauri.conf.json', 'utf8')
    )

    // Refresh pnpm lockfile so it pins the new version. Cargo.lock will
    // be refreshed by the cargo check step below.
    console.log('\n📦 Updating lock files...')
    exec('pnpm install', { silent: true })
    console.log('✅ Lock files updated')

    // Verify configurations
    console.log('\n🔍 Verifying configurations...')

    if (!tauriConfig.bundle?.createUpdaterArtifacts) {
      console.warn(
        '⚠️  Warning: createUpdaterArtifacts not enabled in tauri.conf.json'
      )
    } else {
      console.log('✅ Updater artifacts enabled')
    }

    if (!tauriConfig.plugins?.updater?.pubkey) {
      console.warn('⚠️  Warning: Updater public key not configured')
    } else {
      console.log('✅ Updater public key configured')
    }

    // Final compile check — also refreshes Cargo.lock to pin the new
    // workspace versions so the tag commit includes lockfile updates.
    console.log('\n🔍 Running final compilation check...')
    exec('source ~/.cargo/env && cargo check --workspace')
    console.log('✅ Rust compilation check passed')

    console.log(`\n🎉 Successfully prepared release ${tagVersion}!`)
    console.log('\n📋 Git commands to execute:')
    console.log(`   git add .`)
    console.log(`   git commit -m "chore: release ${tagVersion}"`)
    console.log(`   git tag ${tagVersion}`)
    console.log(`   git push origin main --tags`)

    console.log('\n🚀 After pushing:')
    console.log('   • GitHub Actions will automatically build the release')
    console.log('   • A draft release will be created on GitHub')
    console.log("   • You'll need to manually publish the draft release")
    console.log('   • Users will receive auto-update notifications')

    // Interactive execution option
    const answer = await askQuestion(
      '\n❓ Would you like me to execute these git commands? (y/N): '
    )

    if (answer.toLowerCase() === 'y' || answer.toLowerCase() === 'yes') {
      console.log('\n⚡ Executing git commands...')

      console.log('📝 Adding changes...')
      exec('git add .')

      console.log('💾 Creating commit...')
      exec(`git commit -m "chore: release ${tagVersion}"`)

      console.log('🏷️  Creating tag...')
      exec(`git tag ${tagVersion}`)

      console.log('📤 Pushing to remote...')
      exec('git push origin main --tags')

      console.log(`\n🎊 Release ${tagVersion} has been published!`)
      console.log(
        '📱 Check GitHub Actions: https://github.com/YOUR_USERNAME/YOUR_REPO/actions'
      )
      console.log(
        '📦 Draft release will appear at: https://github.com/YOUR_USERNAME/YOUR_REPO/releases'
      )
      console.log(
        '\n⚠️  Remember: You need to manually publish the draft release on GitHub!'
      )
    } else {
      console.log('\n📝 Git commands saved for manual execution.')
      console.log("   Run them when you're ready to release.")
    }
  } catch (error) {
    console.error('\n❌ Pre-release preparation failed:', error.message)
    process.exit(1)
  }
}

// Run if this is the main module
prepareRelease()
