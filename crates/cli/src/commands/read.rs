//! `sec read <file>` — decrypt + verify + print the body of an envelope.
//!
//! For plaintext envelopes: parses, prints body. For encrypted envelopes:
//! parses → recovers SealedBox from body → derives our X25519 secret from
//! our ed25519 signing key → decrypts → prints plaintext to stdout.
//!
//! Substrate-for-themia Move 13 — runs a layered verify pass before
//! printing and prepends a `⚠️  STAMP INVALID — body modified since
//! stamping` (or signature-equivalent) header on tamper detection. The
//! reader sees the body but is warned that the cryptographic chain is
//! broken; receivers acting on the content should refuse.

use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use ed25519_dalek::SigningKey;
use secretariat_core::application::{verify_document_layered, SignatureOutcome, VerifyOutcome};
use secretariat_core::infrastructure::crypto::sealed::{open, signing_to_x25519, SealedBox};
use secretariat_core::infrastructure::identity_store::load_identity;
use secretariat_core::infrastructure::keys::load_signing_key;
use secretariat_core::infrastructure::markdown::parse_document;
use secretariat_core::infrastructure::{CompositeDidResolver, DidWebResolver};
use secretariat_core::EncryptionScheme;

use super::paths::key_paths;

#[derive(Parser, Debug)]
pub struct Args {
    /// Path to the envelope markdown file (e.g. `~/.secretariat/inbox/<file>`).
    file: PathBuf,
}

pub fn run(args: Args) -> Result<()> {
    // Layered verify first. Body still prints regardless of outcome (the
    // principal needs to see what they're being warned about), but tamper
    // states emit a stderr warning header so the reader has cryptographic
    // context before consuming the body.
    print_tamper_warnings(&args.file);

    let raw = std::fs::read_to_string(&args.file)
        .with_context(|| format!("reading {}", args.file.display()))?;
    let parsed = parse_document(&raw).context("parsing envelope")?;

    // `$envelope` is parsed opaquely by the markdown layer (git-native
    // teardown); deserialize the typed view here to read the encryption
    // scheme that drives the decrypt branch.
    //
    // Unsigned / frontmatter-less docs: most working docs in the substrate
    // are plain markdown with no `$envelope` block. Print the body as-is
    // rather than erroring — only envelopes carry an encryption scheme.
    let Some(envelope_value) = parsed.envelope else {
        print!("{}", parsed.body);
        return Ok(());
    };
    let envelope: secretariat_core::domain::Envelope =
        serde_yaml::from_value(envelope_value).context("parsing $envelope block")?;

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

fn print_tamper_warnings(file: &std::path::Path) {
    let Ok(paths) = key_paths() else {
        return;
    };
    let resolver = CompositeDidResolver::new(DidWebResolver::new(paths.peers_cache.clone()));
    let local_did = load_identity(&paths.identity_md)
        .ok()
        .flatten()
        .map(|id| id.did);
    let Ok(outcome) =
        verify_document_layered(file, &resolver, local_did.as_ref(), Some(&paths.root))
    else {
        return;
    };
    if let SignatureOutcome::Tampered { .. } = outcome.signature {
        eprintln!(
            "⚠️  SIGNATURE INVALID — body modified since signing. The author signature \
             no longer covers the bytes you are about to read."
        );
    }
    if let VerifyOutcome::Tampered { .. } = outcome.stamp {
        eprintln!(
            "⚠️  STAMP INVALID — body modified since stamping. The principal stamp \
             no longer covers the bytes you are about to read."
        );
    }
}

fn decrypt_with_local_key(body: &str) -> Result<Vec<u8>> {
    let paths = key_paths()?;
    let signing: SigningKey = load_signing_key(&paths.signing_key)
        .with_context(|| format!("loading signing key from {}", paths.signing_key.display()))?;
    let x25519_secret = signing_to_x25519(&signing);

    let sealed = SealedBox::parse_wire_string(body.trim())
        .map_err(|e| anyhow!("body is not a valid sealed-box wire string: {e}"))?;
    let plaintext = open(&sealed, &x25519_secret).map_err(|e| anyhow!("decryption failed: {e}"))?;
    Ok(plaintext)
}
