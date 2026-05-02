//! Selects the biometric gate at runtime based on env + debug build flags.
//!
//! Lifted from the CLI so MCP and any future GUI shell can build the same
//! signer without duplicating policy. CLI re-exports this module.
//!
//! Precedence:
//! - `SECRETARIAT_BIOMETRIC=always_allow`  → AlwaysAllowGate (debug builds only, or with `allow_test_biometrics=true`)
//! - `SECRETARIAT_BIOMETRIC=always_deny`   → AlwaysDenyGate (same constraint)
//! - `SECRETARIAT_BIOMETRIC=touchid` (default on Mac) → TouchIdGate (shells out to Swift helper)

use anyhow::{anyhow, Result};
#[cfg(target_os = "macos")]
use anyhow::Context;
use ed25519_dalek::SigningKey;

use crate::infrastructure::ed25519_signer::{
    AlwaysAllowGate, AlwaysDenyGate, BiometricGate, Ed25519Signer,
};
#[cfg(target_os = "macos")]
use crate::infrastructure::touchid::TouchIdGate;
use crate::ports::SignerError;
use crate::Did;

pub enum AnyGate {
    #[cfg(target_os = "macos")]
    TouchId(TouchIdGate),
    AlwaysAllow,
    AlwaysDeny,
}

impl BiometricGate for AnyGate {
    fn prompt(&self, reason: &str) -> Result<(), SignerError> {
        match self {
            #[cfg(target_os = "macos")]
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
        #[cfg(target_os = "macos")]
        Some("touchid") | None => Ok(AnyGate::TouchId(
            TouchIdGate::discover().context("locating touchid-prompt helper")?,
        )),
        #[cfg(not(target_os = "macos"))]
        Some("touchid") | None => Err(anyhow!(
            "Touch ID gate is macOS-only. On non-mac builds, set \
             SECRETARIAT_BIOMETRIC=always_allow with --allow-test-biometrics for tests."
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
