//! Selects the biometric gate at runtime based on env + debug build flags.
//!
//! Lifted from the CLI so MCP and any future GUI shell can build the same
//! signer without duplicating policy.
//!
//! Precedence:
//! - `SECRETARIAT_BIOMETRIC=always_allow`  → AlwaysAllowGate (debug builds only, or with `allow_test_biometrics=true`)
//! - `SECRETARIAT_BIOMETRIC=always_deny`   → AlwaysDenyGate (same constraint)
//! - default → NativeBiometricGate (in-process: macOS LAContext / Windows Hello)

use anyhow::{anyhow, Result};
use ed25519_dalek::SigningKey;

use crate::infrastructure::ed25519_signer::{
    AlwaysAllowGate, AlwaysDenyGate, BiometricGate, Ed25519Signer,
};
use crate::infrastructure::native_biometric::NativeBiometricGate;
use crate::ports::SignerError;
use crate::Did;

pub enum AnyGate {
    Native(NativeBiometricGate),
    AlwaysAllow,
    AlwaysDeny,
}

impl BiometricGate for AnyGate {
    fn prompt(&self, reason: &str) -> Result<(), SignerError> {
        match self {
            AnyGate::Native(g) => g.prompt(reason),
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
        Some("native") | None => Ok(AnyGate::Native(NativeBiometricGate::new())),
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
