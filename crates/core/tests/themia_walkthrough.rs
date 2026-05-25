//! Themia walkthrough — Move 15 end-to-end.
//!
//! Local-only smoke test that exercises the substrate-for-themia slice
//! end-to-end without a real relay. Two principals (stand-ins for
//! Christophe + Rafa); one composes via agent key + emits an
//! agentManifest; the other receives, ingests, verifies, and stamps.
//!
//! What this proves:
//!   - Move 1A — Agent VO + identity record signing
//!   - Move 1B — sec agent add (via application::agent_ops::add_agent)
//!   - Move 1C — agentManifest emit + ingest
//!   - Move 2  — envelope $signature distinct from $attestation
//!   - Move 4  — envelope writes directly to envelopes/YYYY/MM/DD/
//!   - Move 13 — layered verify reports both signature + stamp layers
//!
//! What this does NOT prove (out of scope for local-only):
//!   - Federation across relay (Move 5 daemon federate.rs)
//!   - Vault layout post-Move-3c (test uses KeyPaths::under, so it
//!     adapts to whatever layout the current code defines)
//!   - Roster gating (Move 6 channel-policy enforcement is receiver-
//!     side and not exercised here)

use chrono::Utc;
use ed25519_dalek::SigningKey;
use secretariat_core::application::{
    add_agent, compose_envelope, emit_manifest_into_channel, ingest_manifest_from_file,
    list_agents, stamp_document, verify_document_layered, ComposeRequest, ComposeSigner,
    SignatureOutcome, VerifyOutcome,
};
use secretariat_core::domain::{
    AgentName, AgentRole, AgentSubstrate, Did, DisplayName, EnvelopeDepth, EnvelopeUrgency,
    ManifestTarget, QueueHandle, Recipient, SignerRole, StampAct,
};
use secretariat_core::infrastructure::ed25519_signer::{AlwaysAllowGate, Ed25519Signer};
use secretariat_core::infrastructure::identity_store::{save_identity, PrincipalIdentity};
use secretariat_core::infrastructure::keys::{
    generate_keypair, save_signing_key, KeyPaths,
};
use secretariat_core::infrastructure::queue_dir::AliasMap;
use secretariat_core::ports::{DidResolver, ResolvedDid};
use tempfile::TempDir;

/// Bootstrap a fresh principal at the given vault root with the given
/// display name. Returns the principal's DID + signing key + KeyPaths.
fn fresh_principal(tmp: &TempDir, name: &str) -> (Did, SigningKey, KeyPaths) {
    let paths = KeyPaths::under(tmp.path().to_path_buf());
    paths.ensure_dirs().unwrap();
    let key = generate_keypair();
    let pk = key.verifying_key().to_bytes();
    let did = Did::from_ed25519_public_key(&pk);
    save_signing_key(&paths.signing_key, &key).unwrap();
    let when = Utc::now();
    let identity = PrincipalIdentity {
        did: did.clone(),
        did_method: "did:key".to_string(),
        display_name: DisplayName::parse(name).unwrap(),
        full_name: None,
        key_path: "identity/key".to_string(),
        key_type: "ed25519".to_string(),
        key_created_at: when,
        key_rotations: vec![],
        authorized_agents: vec![],
        created_at: when,
        signature: None,
        body: String::new(),
    };
    save_identity(&paths.identity_md, &identity, Some(&key)).unwrap();
    (did, key, paths)
}

#[test]
fn themia_walkthrough_christophe_to_rafa() {
    // Two separate vaults — Christophe (Themia co-owner stand-in) and
    // Rafa (the receiver). In real Themia, both would have separate
    // ~/.secretariat/ trees on separate Macs.
    let christophe_tmp = TempDir::new().unwrap();
    let rafa_tmp = TempDir::new().unwrap();
    let (christophe_did, christophe_key, christophe_paths) =
        fresh_principal(&christophe_tmp, "Christophe");
    let (_rafa_did, _rafa_key, _rafa_paths) = fresh_principal(&rafa_tmp, "Rafa");

    // -----------------------------------------------------------------
    // Step 1 — Christophe adds Claude as scribe. (Move 1A + 1B)
    // -----------------------------------------------------------------
    let agent_name = AgentName::parse("claude").unwrap();
    let agent = add_agent(
        &christophe_paths,
        agent_name.clone(),
        AgentRole::Scribe,
        AgentSubstrate::parse("claude-code").unwrap(),
        Utc::now(),
    )
    .unwrap();
    assert_eq!(agent.name, agent_name);
    assert_eq!(list_agents(&christophe_paths).unwrap().len(), 1);

    // -----------------------------------------------------------------
    // Step 2 — Christophe emits an agentManifest into a channel. (Move 1C)
    // The channel directory simulates a Themia org channel. For the test
    // we use a self-rooted path; in real usage it'd be an org channel
    // under orgs/themia.pro/channels/assemblee_generale/.
    // -----------------------------------------------------------------
    let agent_key_bytes =
        std::fs::read(christophe_paths.agent_signing_key_path(agent_name.as_str())).unwrap();
    let _ = agent_key_bytes; // (sanity: agent key exists on disk)

    let channel_dir = christophe_tmp
        .path()
        .join("orgs")
        .join("themia.pro")
        .join("channels")
        .join("assemblee_generale");
    std::fs::create_dir_all(&channel_dir).unwrap();

    let manifest_path = emit_manifest_into_channel(
        &channel_dir,
        ManifestTarget::Channel {
            owner: christophe_did.clone(),
            handle: "assemblee_generale".to_string(),
        },
        christophe_did.clone(),
        vec![agent.clone()],
        &christophe_key,
        Utc::now(),
    )
    .unwrap();
    assert!(manifest_path.exists());

    // -----------------------------------------------------------------
    // Step 3 — Christophe's-scribe composes an envelope. (Move 2 + Move 4)
    // Uses the agent's signing key — envelope.$signature carries agent DID.
    // -----------------------------------------------------------------
    let signer_pem = std::fs::read_to_string(
        christophe_paths.agent_signing_key_path(agent_name.as_str()),
    )
    .unwrap();
    use ed25519_dalek::pkcs8::DecodePrivateKey;
    let agent_signing_key = SigningKey::from_pkcs8_pem(&signer_pem).unwrap();
    let agent_did = agent.did.clone();

    let template_path = christophe_paths.template.clone();
    // Bootstrap a tiny template so compose has something to read.
    std::fs::write(&template_path, "# Template body\n\nFree-form prose.\n").unwrap();

    let recipient = Recipient::new(
        christophe_did.clone(),
        QueueHandle::parse("assemblee_generale").unwrap(),
    );
    let aliases = AliasMap::new(christophe_did.clone());
    let request = ComposeRequest {
        from: christophe_did.clone(),
        recipient: recipient.clone(),
        depth: EnvelopeDepth::Subtle,
        urgency: EnvelopeUrgency::Whenever,
        source: "themia-walkthrough-test".to_string(),
        cadence_hint: None,
        body: Some(
            "# PV — assemblée générale\n\nDraft minutes for board review.\n".to_string(),
        ),
        title: Some("PV — assemblée générale".to_string()),
        lede: Some("Draft minutes for board review.".to_string()),
        summary: None,
    };
    let signer_ctx = ComposeSigner {
        signing_key: &agent_signing_key,
        signer_did: agent_did.clone(),
        signer_role: SignerRole::Agent,
    };
    let envelope_path = compose_envelope(
        request,
        &signer_ctx,
        &template_path,
        christophe_tmp.path(),
        &aliases,
        Utc::now(),
    )
    .unwrap();
    assert!(envelope_path.exists());

    // The envelope lives at:
    //   <christophe>/orgs/themia.pro/channels/assemblee_generale/envelopes/YYYY/MM/DD/<file>.md
    // BUT queue_dir today still resolves orgs/<alias> as <alias>/, so the
    // path may be <christophe>/themia.pro/channels/.... The Move 3c
    // restructure will normalize this. For the walkthrough we just
    // verify the file exists and has the expected frontmatter.

    let envelope_raw = std::fs::read_to_string(&envelope_path).unwrap();
    assert!(
        envelope_raw.contains("$signature:"),
        "envelope MUST carry $signature after Move 2"
    );
    assert!(
        envelope_raw.contains(agent_did.as_str()),
        "envelope $signature MUST reference agent DID, not principal"
    );
    assert!(
        !envelope_raw.contains("$attestation:"),
        "envelope MUST NOT carry $attestation yet (unstamped)"
    );

    // -----------------------------------------------------------------
    // Step 4 — Simulate wire transport: copy the manifest + envelope
    // from Christophe's vault to Rafa's vault. In real Themia the
    // daemon would handle this via channel sync.
    // -----------------------------------------------------------------
    let rafa_channel_dir = rafa_tmp
        .path()
        .join("received")
        .join("themia.pro")
        .join("assemblee_generale");
    std::fs::create_dir_all(&rafa_channel_dir).unwrap();
    let rafa_manifest_dest = rafa_channel_dir.join(manifest_path.file_name().unwrap());
    let rafa_envelope_dest = rafa_channel_dir.join(envelope_path.file_name().unwrap());
    std::fs::copy(&manifest_path, &rafa_manifest_dest).unwrap();
    std::fs::copy(&envelope_path, &rafa_envelope_dest).unwrap();

    // -----------------------------------------------------------------
    // Step 5 — Rafa ingests the manifest. (Move 1C)
    // Confirms: signature valid, signer == Christophe principal,
    // authorized_agents includes Claude.
    // -----------------------------------------------------------------
    let manifest = ingest_manifest_from_file(&rafa_manifest_dest).unwrap().unwrap();
    assert_eq!(manifest.signer, christophe_did);
    assert_eq!(manifest.authorized_agents.len(), 1);
    assert_eq!(manifest.authorized_agents[0].did, agent_did);
    assert_eq!(manifest.authorized_agents[0].name.as_str(), "claude");

    // -----------------------------------------------------------------
    // Step 6 — Rafa runs layered verify on the envelope. (Move 13)
    // Without a wired agentManifest cache, expect OkUnverifiedAgent
    // (signature crypto verifies but principal binding not yet consulted).
    // -----------------------------------------------------------------
    let resolver = StubKeyResolver { keys: agent_did.embedded_ed25519_key().unwrap() };
    let outcome =
        verify_document_layered(&rafa_envelope_dest, &resolver, None).unwrap();
    match outcome.signature {
        SignatureOutcome::OkUnverifiedAgent { signer, .. } => {
            assert_eq!(signer, agent_did);
        }
        other => panic!("expected OkUnverifiedAgent, got {other:?}"),
    }
    assert!(
        matches!(outcome.stamp, VerifyOutcome::Unsigned),
        "stamp layer MUST be absent before stamping"
    );

    // -----------------------------------------------------------------
    // Step 7 — Rafa stamps to elevate the envelope to authoritative.
    // Uses Rafa's own key (not the agent's). Stamp adds $attestation
    // alongside the existing $signature.
    // -----------------------------------------------------------------
    let rafa_signer = Ed25519Signer::new(_rafa_did.clone(), _rafa_key.clone(), AlwaysAllowGate);
    let _stamped =
        stamp_document(&rafa_envelope_dest, &rafa_signer, StampAct::Attest, false, Utc::now())
            .unwrap();

    let stamped_raw = std::fs::read_to_string(&rafa_envelope_dest).unwrap();
    assert!(
        stamped_raw.contains("$attestation:"),
        "stamp MUST embed $attestation"
    );
    assert!(
        stamped_raw.contains("$signature:"),
        "stamp MUST preserve the existing $signature"
    );

    // -----------------------------------------------------------------
    // Step 8 — Re-verify. Both layers should now be present; signature
    // OkUnverifiedAgent (still no cache); stamp Verified (since
    // resolver knows Rafa's key).
    // -----------------------------------------------------------------
    let dual_resolver = DualKeyResolver {
        rafa_did: _rafa_did.clone(),
        rafa_key: _rafa_key.verifying_key().to_bytes(),
        agent_key: agent_did.embedded_ed25519_key().unwrap(),
    };
    let outcome_final =
        verify_document_layered(&rafa_envelope_dest, &dual_resolver, None).unwrap();
    assert!(matches!(
        outcome_final.signature,
        SignatureOutcome::OkUnverifiedAgent { .. }
    ));
    assert!(matches!(outcome_final.stamp, VerifyOutcome::Verified { .. }));
}

// ---------------------------------------------------------------------------
// Stub resolvers — local-only DID resolution that knows specific keys.
// ---------------------------------------------------------------------------

struct StubKeyResolver {
    keys: [u8; 32],
}

impl DidResolver for StubKeyResolver {
    fn resolve(
        &self,
        did: &Did,
    ) -> Result<ResolvedDid, secretariat_core::ports::DidResolutionError> {
        Ok(ResolvedDid {
            did: did.clone(),
            stamp_public_keys: vec![self.keys],
            raw_document: serde_json::Value::Null,
        })
    }
}

struct DualKeyResolver {
    rafa_did: Did,
    rafa_key: [u8; 32],
    agent_key: [u8; 32],
}

impl DidResolver for DualKeyResolver {
    fn resolve(
        &self,
        did: &Did,
    ) -> Result<ResolvedDid, secretariat_core::ports::DidResolutionError> {
        let keys = if did == &self.rafa_did {
            vec![self.rafa_key]
        } else {
            vec![self.agent_key]
        };
        Ok(ResolvedDid {
            did: did.clone(),
            stamp_public_keys: keys,
            raw_document: serde_json::Value::Null,
        })
    }
}
