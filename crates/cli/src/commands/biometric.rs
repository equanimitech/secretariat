//! Selects the biometric gate at runtime based on env + debug build flags.
//!
//! Precedence (matches plan's "Biometric mocking for CI / dev" section):
//! - `SECRETARIAT_BIOMETRIC=always_allow`  → AlwaysAllowGate (debug builds only, or with --allow-test-biometrics)
//! - `SECRETARIAT_BIOMETRIC=always_deny`   → AlwaysDenyGate (same constraint)
//! - `SECRETARIAT_BIOMETRIC=touchid` (default on Mac) → TouchIdGate (shells out to Swift helper)

use anyhow::{anyhow, Context, Result};
use std::path::PathBuf;

use secretariat_core::infrastructure::ed25519_signer::{
    AlwaysAllowGate, AlwaysDenyGate, BiometricGate, Ed25519Signer,
};
use secretariat_core::infrastructure::touchid::TouchIdGate;
use secretariat_core::ports::SignerError;
use secretariat_core::Did;
use ed25519_dalek::SigningKey;

/// One of the three real gates plus a deny gate, behind a thin enum so the
/// `Ed25519Signer` can be parameterized without monomorphizing per call site.
pub enum AnyGate {
    TouchId(TouchIdGate),
    AlwaysAllow,
    AlwaysDeny,
}

impl BiometricGate for AnyGate {
    fn prompt(&self, reason: &str) -> Result<(), SignerError> {
        match self {
            AnyGate::TouchId(g) => g.prompt(reason),
            AnyGate::AlwaysAllow => AlwaysAllowGate.prompt(reason),
            AnyGate::AlwaysDeny => AlwaysDenyGate.prompt(reason),
        }
    }
}

pub fn pick_gate(allow_test_biometrics: bool) -> Result<AnyGate> {
    let env = std::env::var("SECRETARIAT_BIOMETRIC").ok();
    let test_biometrics_ok = cfg!(debug_assertions) || allow_test_biometrics;

    match env.as_deref() {
        Some("always_allow") if test_biometrics_ok => {
            eprintln!("[sec] biometric=always_allow — DO NOT USE IN PRODUCTION");
            Ok(AnyGate::AlwaysAllow)
        }
        Some("always_allow") => Err(anyhow!(
            "SECRETARIAT_BIOMETRIC=always_allow is only honored in debug builds or with --allow-test-biometrics"
        )),
        Some("always_deny") if test_biometrics_ok => {
            eprintln!("[sec] biometric=always_deny");
            Ok(AnyGate::AlwaysDeny)
        }
        Some("always_deny") => Err(anyhow!(
            "SECRETARIAT_BIOMETRIC=always_deny is only honored in debug builds or with --allow-test-biometrics"
        )),
        Some("touchid") | None => Ok(AnyGate::TouchId(
            TouchIdGate::discover().context("locating touchid-prompt helper")?,
        )),
        Some(other) => Err(anyhow!("unknown SECRETARIAT_BIOMETRIC value: {other}")),
    }
}

pub fn build_signer(
    did: Did,
    key: SigningKey,
    allow_test_biometrics: bool,
) -> Result<Ed25519Signer<AnyGate>> {
    let gate = pick_gate(allow_test_biometrics)?;
    Ok(Ed25519Signer::new(did, key, gate))
}

/// For init / verify / compose: locate the Touch ID helper without instantiating
/// a signer. Surfaces a clear error if the binary is missing.
#[allow(dead_code)]
pub fn require_touchid_binary() -> Result<PathBuf> {
    let g = TouchIdGate::discover().context("locating touchid-prompt helper")?;
    // TouchIdGate doesn't expose its path; fall back to a probe against the
    // expected workspace location for a friendly error.
    let candidate =
        std::env::current_dir()?.join("target").join("touchid-prompt");
    if candidate.exists() {
        Ok(candidate)
    } else {
        // Trust TouchIdGate::discover succeeded — the binary is somewhere on PATH or env.
        let _ = g;
        Ok(PathBuf::from("touchid-prompt"))
    }
}
