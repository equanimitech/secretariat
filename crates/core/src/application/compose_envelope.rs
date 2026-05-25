//! Use case: scaffold an AG-shaped envelope as a draft on a per-queue tree.
//!
//! Reads the user's customizable template at `~/.secretariat/template.md`,
//! prepends a `$envelope:` frontmatter block, and writes the result to
//! `<root>/<alias-of-to>/channels/<segments>/envelopes/YYYY/MM/DD/<rkey>.md` —
//! the per-queue day-shard tree, derived from the recipient via the
//! `queue_dir` resolver. No stamp is added — the principal stamps later
//! via `sec stamp`, which embeds the `$attestation` block in place.
//!
//! Substrate-for-themia Move 4 (2026-05-21, per
//! `docs/pitches/2026-05-21-substrate-for-themia.md`): there is one
//! envelope state and one filesystem location. The compose verb
//! writes directly into `envelopes/YYYY/MM/DD/` — no `_drafts/`
//! intermediate. The envelope's frontmatter does NOT carry the
//! `delivered:` field at compose time; absence IS the substrate's
//! signal for "undelivered / draft state." The daemon's envelope
//! watcher reacts to the new file, federates it, and writes
//! `delivered: <relay-seq-id>` in place on success.

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use ed25519_dalek::SigningKey;
use rand::Rng;
use thiserror::Error;

use crate::domain::{
    AgSource, Did, Envelope, EnvelopeBuilder, EnvelopeDepth, EnvelopeSignature, EnvelopeUrgency,
    Recipient, SignerRole,
};
use crate::infrastructure::markdown::{embed_frontmatter, MarkdownError};
use crate::infrastructure::preferences::CognitionPrefs;
use crate::infrastructure::queue_dir::AliasMap;

use super::ag_extract::{try_extract_ag, AgExtractOutcome, AuthorAgFields};

/// Author's signing identity passed to [`compose_envelope`].
/// Substrate-for-themia Move 2: every composed envelope carries an
/// author signature; the principal stamps separately and selectively.
///
/// `signer_did` MUST be the DID derived from the public half of
/// `signing_key`. The application layer enforces this at construction
/// of the signer struct (callers loading from `KeyPaths` do); the
/// domain layer trusts the pair.
pub struct ComposeSigner<'a> {
    pub signer_did: Did,
    pub signer_role: SignerRole,
    pub signing_key: &'a SigningKey,
}

impl<'a> ComposeSigner<'a> {
    pub fn new(signer_did: Did, signer_role: SignerRole, signing_key: &'a SigningKey) -> Self {
        Self {
            signer_did,
            signer_role,
            signing_key,
        }
    }
}

#[derive(Debug, Error)]
pub enum ComposeError {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("markdown error: {0}")]
    Markdown(#[from] MarkdownError),
}

#[derive(Debug, Clone)]
pub struct ComposeRequest {
    pub from: Did,
    pub recipient: Recipient,
    pub depth: EnvelopeDepth,
    pub urgency: EnvelopeUrgency,
    pub source: String,
    pub cadence_hint: Option<String>,
    /// Raw markdown body. When `Some`, it replaces the AG template entirely —
    /// caller is responsible for shape. When `None`, the user's template at
    /// `~/.secretariat/template.md` is used as a scaffold.
    pub body: Option<String>,
    /// Optional author-supplied AG fields. When any is set, the envelope is
    /// written with the author's values verbatim and the AI auto-fill pass
    /// stands down (see `compose_envelope_with_ag`).
    pub title: Option<String>,
    pub lede: Option<String>,
    pub summary: Option<String>,
}

pub fn compose_envelope(
    request: ComposeRequest,
    signer: &ComposeSigner<'_>,
    template_path: &Path,
    root: &Path,
    aliases: &AliasMap,
    now: DateTime<Utc>,
) -> Result<PathBuf, ComposeError> {
    compose_envelope_inner(request, signer, template_path, root, aliases, now, None)
}

/// Inner write — same shape as `compose_envelope`, threads an explicit
/// `ag_source` so the async wrapper can mark scribe-generated AG
/// triplets without duplicating the file-IO body.
fn compose_envelope_inner(
    request: ComposeRequest,
    signer: &ComposeSigner<'_>,
    template_path: &Path,
    root: &Path,
    aliases: &AliasMap,
    now: DateTime<Utc>,
    ag_source: Option<AgSource>,
) -> Result<PathBuf, ComposeError> {
    let mut envelope = build_envelope(&request);
    if let Some(src) = ag_source {
        envelope.ag_source = Some(src);
    }
    let queue_root = crate::infrastructure::queue_dir::queue_dir(aliases, &request.recipient, root);
    // One envelope state: write straight into the day-shard. The
    // envelope's frontmatter omits `delivered:` — absence IS the
    // substrate's "undelivered / draft" signal.
    let day_shard = now.format("%Y/%m/%d").to_string();
    let target_dir = queue_root.join("envelopes").join(&day_shard);
    fs::create_dir_all(&target_dir).map_err(|e| ComposeError::Io {
        path: target_dir.clone(),
        source: e,
    })?;

    let filename = generate_filename(now);
    let target_path = target_dir.join(filename);

    let body_owned: String;
    let body: &str = match &request.body {
        Some(b) => b.as_str(),
        None => {
            // Per-channel template override (AGENTS.md rule #5): prefer
            // `<channel-dir>/template.md` when present; fall back to the
            // principal's global template.
            let channel_template = queue_root.join("template.md");
            let chosen = if channel_template.is_file() {
                &channel_template
            } else {
                template_path
            };
            body_owned = fs::read_to_string(chosen).map_err(|e| ComposeError::Io {
                path: chosen.to_path_buf(),
                source: e,
            })?;
            strip_existing_frontmatter(&body_owned)
        }
    };

    // Substrate-for-themia Move 2: sign the body at compose. The author
    // signature is mandatory on every envelope on the wire; the
    // principal's stamp comes later (selective). Both layers share the
    // same canonical body hash, so a tamper invalidates both.
    let signature = EnvelopeSignature::sign_body(
        signer.signer_did.clone(),
        signer.signer_role,
        body,
        now,
        signer.signing_key,
    );

    let content = embed_frontmatter(body, Some(&envelope), Some(&signature), None)?;

    fs::write(&target_path, content).map_err(|e| ComposeError::Io {
        path: target_path.clone(),
        source: e,
    })?;
    Ok(target_path)
}

fn build_envelope(req: &ComposeRequest) -> Envelope {
    let mut b = EnvelopeBuilder::new(req.from.clone(), req.recipient.clone())
        .depth(req.depth)
        .urgency(req.urgency)
        .source(req.source.clone());
    if let Some(hint) = &req.cadence_hint {
        b = b.cadence_hint(hint.clone());
    }
    if let Some(t) = &req.title {
        b = b.title(t.clone());
    }
    if let Some(l) = &req.lede {
        b = b.lede(l.clone());
    }
    if let Some(s) = &req.summary {
        b = b.summary(s.clone());
    }
    b.build()
}

/// Async wrapper that runs an AG-extraction pass before composing.
///
/// When `request.title` / `lede` / `summary` are all `None` and the
/// effective body is plaintext and substantive (see
/// [`super::ag_extract::body_warrants_ag`]), call the configured
/// cognition adapter; populate the request with the result; tag
/// `ag_source = "ai"` so receivers can tell. When any of the AG fields
/// is already set, or the adapter is unconfigured, the body is too
/// short, or the adapter fails, this falls through to a normal
/// [`compose_envelope`] call. Never crashes a compose — the
/// correspondence path is not blocked by the cognition substrate.
pub async fn compose_envelope_with_ag(
    request: ComposeRequest,
    signer: &ComposeSigner<'_>,
    template_path: &Path,
    root: &Path,
    aliases: &AliasMap,
    cognition_prefs: &CognitionPrefs,
    now: DateTime<Utc>,
) -> Result<PathBuf, ComposeError> {
    let (request, ag_source) =
        enrich_with_ag(request, template_path, root, aliases, cognition_prefs).await?;
    compose_envelope_inner(request, signer, template_path, root, aliases, now, ag_source)
}

async fn enrich_with_ag(
    mut request: ComposeRequest,
    template_path: &Path,
    root: &Path,
    aliases: &AliasMap,
    cognition_prefs: &CognitionPrefs,
) -> Result<(ComposeRequest, Option<AgSource>), ComposeError> {
    let author = AuthorAgFields {
        title: request.title.clone(),
        lede: request.lede.clone(),
        summary: request.summary.clone(),
    };
    if author.any_set() {
        return Ok((request, None));
    }
    // Read the body we'd actually write so the AG pass sees real content.
    let body_string = match &request.body {
        Some(b) => b.clone(),
        None => {
            let queue_root =
                crate::infrastructure::queue_dir::queue_dir(aliases, &request.recipient, root);
            let channel_template = queue_root.join("template.md");
            let chosen = if channel_template.is_file() {
                channel_template
            } else {
                template_path.to_path_buf()
            };
            match fs::read_to_string(&chosen) {
                Ok(raw) => strip_existing_frontmatter(&raw).to_string(),
                // Don't crash AG enrichment on template-read failure; the
                // sync compose path will surface it.
                Err(_) => return Ok((request, None)),
            }
        }
    };
    let outcome = try_extract_ag(&body_string, &author, cognition_prefs).await;
    match outcome {
        AgExtractOutcome::Generated(fields) => {
            request.title = Some(fields.title);
            request.lede = Some(fields.lede);
            request.summary = Some(fields.summary);
            Ok((request, Some(AgSource::Ai)))
        }
        _ => Ok((request, None)),
    }
}

/// Decision log #7: `<utc-iso8601>-<6-char-base32-suffix>.md`.
fn generate_filename(now: DateTime<Utc>) -> String {
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz234567";
    let mut rng = rand::thread_rng();
    let suffix: String = (0..6)
        .map(|_| ALPHABET[rng.gen_range(0..ALPHABET.len())] as char)
        .collect();
    format!("{}-{}.md", now.format("%Y%m%dT%H%M%SZ"), suffix)
}

/// If the template starts with frontmatter, drop it (we'll write our own).
/// Templates are user-customizable; many users will keep examples or prior
/// envelopes around. Stripping makes composition idempotent.
fn strip_existing_frontmatter(s: &str) -> &str {
    let s = s.strip_prefix('\u{FEFF}').unwrap_or(s);
    if !(s.starts_with("---\n") || s.starts_with("---\r\n")) {
        return s;
    }
    let after_open = if let Some(r) = s.strip_prefix("---\r\n") {
        r
    } else {
        s.strip_prefix("---\n").unwrap_or(s)
    };
    let mut start = 0usize;
    while let Some(rel) = after_open[start..].find("\n---") {
        let abs = start + rel;
        let tail = &after_open[abs + 4..];
        if let Some(rest) = tail.strip_prefix('\n') {
            return rest;
        }
        if let Some(rest) = tail.strip_prefix("\r\n") {
            return rest;
        }
        if tail.is_empty() {
            return "";
        }
        start = abs + 1;
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::QueueHandle;
    use crate::infrastructure::markdown::parse_document;
    use chrono::TimeZone;
    use tempfile::TempDir;

    fn rafa_did() -> Did {
        Did::parse("did:web:rafa.equanimi.tech").unwrap()
    }

    fn marcelo_did() -> Did {
        Did::parse("did:web:marcelo.ballestiero.com").unwrap()
    }

    fn fixture_signing_key() -> SigningKey {
        SigningKey::from_bytes(&[0x42; 32])
    }

    /// Convenience for tests: a principal-role signer whose DID matches
    /// the bytes of the key. Real callers should plumb the principal /
    /// agent DID from the identity record + `KeyPaths`.
    fn signer<'a>(key: &'a SigningKey) -> ComposeSigner<'a> {
        let did = Did::from_ed25519_public_key(&key.verifying_key().to_bytes());
        ComposeSigner::new(did, SignerRole::Principal, key)
    }

    #[test]
    fn composes_to_peer_queue_envelopes_day_shard() {
        let dir = TempDir::new().unwrap();
        let template = dir.path().join("template.md");
        fs::write(&template, "# Title\n\nBody.\n").unwrap();
        let root = dir.path();
        let mut aliases = AliasMap::new(rafa_did());
        aliases.insert(marcelo_did(), "marcelo");

        let req = ComposeRequest {
            from: rafa_did(),
            recipient: Recipient::new(
                marcelo_did(),
                QueueHandle::parse("inbox:default").unwrap(),
            ),
            depth: EnvelopeDepth::Subtle,
            urgency: EnvelopeUrgency::Soon,
            source: "test".into(),
            cadence_hint: None,
            body: None,
            title: None,
            lede: None,
            summary: None,
        };

        let now = Utc.with_ymd_and_hms(2026, 4, 30, 14, 25, 0).unwrap();
        let key = fixture_signing_key();
        let sgn = signer(&key);
        let path = compose_envelope(req, &sgn, &template, root, &aliases, now).unwrap();

        // Lives under <root>/orgs/marcelo/channels/inbox/default/envelopes/YYYY/MM/DD/.
        // Move 3c — peer (non-self) recipients live under `orgs/<alias>/`.
        // No `_drafts/` intermediate — substrate-for-themia Move 4.
        assert_eq!(
            path.parent().unwrap(),
            root.join("orgs/marcelo/channels/inbox/default/envelopes/2026/04/30"),
        );
        assert!(path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("20260430T142500Z-"));

        let parsed = parse_document(&fs::read_to_string(&path).unwrap()).unwrap();
        assert!(parsed.envelope.is_some());
        assert!(parsed.stamp.is_none());
        // Substrate-for-themia Move 2: author signature mandatory at
        // compose. Verify it survives round-trip + still verifies under
        // the signing key.
        let sig = parsed.signature.as_ref().expect("$signature must be present");
        assert_eq!(sig.signer_role, SignerRole::Principal);
        assert!(sig.verify_body(&parsed.body, &key.verifying_key()));
        // Absence of `delivered:` is the draft signal — never set at compose.
        assert!(parsed.envelope.as_ref().unwrap().delivered.is_none());
        assert!(parsed.body.contains("Body."));
    }

    #[test]
    fn compose_with_agent_role_signs_with_agent_did() {
        // The principal's scribe (agent role) signs at compose. The
        // resulting envelope's `$signature` carries the agent's DID, not
        // the principal's, so receivers can distinguish.
        let dir = TempDir::new().unwrap();
        let template = dir.path().join("template.md");
        fs::write(&template, "# Agent\n\nDraft.\n").unwrap();
        let root = dir.path();
        let aliases = AliasMap::new(rafa_did());

        let req = ComposeRequest {
            from: rafa_did(),
            recipient: Recipient::new(
                rafa_did(),
                QueueHandle::parse("journal").unwrap(),
            ),
            depth: EnvelopeDepth::Gross,
            urgency: EnvelopeUrgency::Whenever,
            source: "agent-test".into(),
            cadence_hint: None,
            body: None,
            title: None,
            lede: None,
            summary: None,
        };

        // Agent key, distinct from principal's DID — a real scribe.
        let agent_key = SigningKey::from_bytes(&[0xAB; 32]);
        let agent_did = Did::from_ed25519_public_key(&agent_key.verifying_key().to_bytes());
        let sgn = ComposeSigner::new(agent_did.clone(), SignerRole::Agent, &agent_key);

        let now = Utc.with_ymd_and_hms(2026, 5, 25, 14, 30, 0).unwrap();
        let path = compose_envelope(req, &sgn, &template, root, &aliases, now).unwrap();

        let parsed = parse_document(&fs::read_to_string(&path).unwrap()).unwrap();
        let sig = parsed.signature.as_ref().expect("agent signature present");
        assert_eq!(sig.signer, agent_did);
        assert_eq!(sig.signer_role, SignerRole::Agent);
        assert!(sig.verify_body(&parsed.body, &agent_key.verifying_key()));
        // Principal's stamp: none (selective; comes later).
        assert!(parsed.stamp.is_none());
    }

    #[test]
    fn composes_self_letter_under_self_channels_root() {
        // Self-addressed envelope — owner == from. The resolver maps
        // self to `<root>/channels/<segs>/` (Move 3c — no `_self/`
        // wrapper); the file lands directly in that queue's
        // `envelopes/YYYY/MM/DD/`.
        let dir = TempDir::new().unwrap();
        let template = dir.path().join("template.md");
        fs::write(&template, "# Self\n").unwrap();
        let root = dir.path();
        let aliases = AliasMap::new(rafa_did());

        let req = ComposeRequest {
            from: rafa_did(),
            recipient: Recipient::new(
                rafa_did(),
                QueueHandle::parse("inbox:default").unwrap(),
            ),
            depth: EnvelopeDepth::Gross,
            urgency: EnvelopeUrgency::Whenever,
            source: "test".into(),
            cadence_hint: None,
            body: None,
            title: None,
            lede: None,
            summary: None,
        };

        let now = Utc.with_ymd_and_hms(2026, 4, 30, 9, 0, 0).unwrap();
        let key = fixture_signing_key();
        let sgn = signer(&key);
        let path = compose_envelope(req, &sgn, &template, root, &aliases, now).unwrap();
        assert_eq!(
            path.parent().unwrap(),
            root.join("channels/inbox/default/envelopes/2026/04/30"),
        );
        // No `_drafts/` dir should be created anywhere under the queue.
        assert!(!root
            .join("channels/inbox/default/_drafts")
            .exists());
    }

    #[test]
    fn per_channel_template_overrides_global() {
        let dir = TempDir::new().unwrap();
        let global_template = dir.path().join("template.md");
        fs::write(&global_template, "# GLOBAL\nGlobal body.\n").unwrap();
        let root = dir.path();
        let aliases = AliasMap::new(rafa_did());

        // Plant a per-channel template at the recipient's queue dir.
        let channel_dir = root.join("channels/secretariat/dev");
        fs::create_dir_all(&channel_dir).unwrap();
        fs::write(
            channel_dir.join("template.md"),
            "# CHANNEL\nChannel-specific body.\n",
        )
        .unwrap();

        let req = ComposeRequest {
            from: rafa_did(),
            recipient: Recipient::new(
                rafa_did(),
                QueueHandle::parse("secretariat:dev").unwrap(),
            ),
            depth: EnvelopeDepth::Subtle,
            urgency: EnvelopeUrgency::Whenever,
            source: "test".into(),
            cadence_hint: None,
            body: None,
            title: None,
            lede: None,
            summary: None,
        };

        let now = Utc.with_ymd_and_hms(2026, 5, 18, 12, 0, 0).unwrap();
        let key = fixture_signing_key();
        let sgn = signer(&key);
        let path = compose_envelope(req, &sgn, &global_template, root, &aliases, now).unwrap();
        let parsed = parse_document(&fs::read_to_string(&path).unwrap()).unwrap();
        assert!(parsed.body.contains("Channel-specific body"));
        assert!(!parsed.body.contains("Global body"));
    }

    #[test]
    fn strips_existing_frontmatter_from_template() {
        let dir = TempDir::new().unwrap();
        let template = dir.path().join("template.md");
        fs::write(
            &template,
            "---\nfoo: bar\n---\n# After\nbody\n",
        )
        .unwrap();
        let root = dir.path();
        let aliases = AliasMap::new(rafa_did());

        let req = ComposeRequest {
            from: rafa_did(),
            recipient: Recipient::new(
                rafa_did(),
                QueueHandle::parse("inbox:scratch").unwrap(),
            ),
            depth: EnvelopeDepth::Gross,
            urgency: EnvelopeUrgency::Whenever,
            source: "test".into(),
            cadence_hint: None,
            body: None,
            title: None,
            lede: None,
            summary: None,
        };

        let now = Utc.with_ymd_and_hms(2026, 4, 30, 9, 0, 0).unwrap();
        let key = fixture_signing_key();
        let sgn = signer(&key);
        let path = compose_envelope(req, &sgn, &template, root, &aliases, now).unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert!(!content.contains("foo: bar"));
        assert!(content.contains("# After"));
        assert!(content.contains("$envelope:"));
    }
}
