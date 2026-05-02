//! `sec read <file>` — decrypt + verify + print the body of an envelope.
//!
//! For plaintext envelopes: parses, prints body. For encrypted envelopes:
//! parses → recovers SealedBox from body → derives our X25519 secret from
//! our ed25519 signing key → decrypts → prints plaintext to stdout.
//!
//! Verification of the ed25519 signature is **not** performed here — that's
//! `sec verify`. A future enhancement could combine both, gated by a flag.
//! Decryption + verification compose: do `sec verify` first if you don't
//! trust the inbox file's origin.

use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use ed25519_dalek::SigningKey;
use secretariat_core::infrastructure::crypto::sealed::{open, signing_to_x25519, SealedBox};
use secretariat_core::infrastructure::keys::load_signing_key;
use secretariat_core::infrastructure::markdown::parse_document;
use secretariat_core::EncryptionScheme;

use super::paths::key_paths;

#[derive(Parser, Debug)]
pub struct Args {
    /// Path to the envelope markdown file (e.g. `~/.secretariat/inbox/<file>`).
    file: PathBuf,
}

pub fn run(args: Args) -> Result<()> {
    let raw = std::fs::read_to_string(&args.file)
        .with_context(|| format!("reading {}", args.file.display()))?;
    let parsed = parse_document(&raw).context("parsing envelope")?;

    let envelope = parsed
        .envelope
        .ok_or_else(|| anyhow!("envelope frontmatter missing"))?;

    match envelope.encryption {
        None => {
            // Plaintext body — just print it.
            print!("{}", parsed.body);
            Ok(())
        }
        Some(EncryptionScheme::X25519XChaCha20Poly1305) => {
            let plaintext = decrypt_with_local_key(&parsed.body)?;
            std::io::Write::write_all(&mut std::io::stdout(), &plaintext)
                .context("writing plaintext to stdout")?;
            Ok(())
        }
    }
}

fn decrypt_with_local_key(body: &str) -> Result<Vec<u8>> {
    let paths = key_paths()?;
    let signing: SigningKey = load_signing_key(&paths.signing_key)
        .with_context(|| format!("loading signing key from {}", paths.signing_key.display()))?;
    let x25519_secret = signing_to_x25519(&signing);

    let sealed = SealedBox::parse_wire_string(body.trim())
        .map_err(|e| anyhow!("body is not a valid sealed-box wire string: {e}"))?;
    let plaintext = open(&sealed, &x25519_secret)
        .map_err(|e| anyhow!("decryption failed: {e}"))?;
    Ok(plaintext)
}
