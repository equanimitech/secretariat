//! Thin re-export so existing CLI sites (`super::biometric::build_signer`,
//! `super::biometric::pick_gate`) keep compiling. The real implementation
//! lives in `secretariat_core::infrastructure::biometric`.

#[cfg(target_os = "macos")]
use anyhow::Context;
use anyhow::Result;
#[cfg(target_os = "macos")]
use std::path::PathBuf;

#[cfg(target_os = "macos")]
use secretariat_core::infrastructure::touchid::TouchIdGate;

#[allow(unused_imports)]
pub use secretariat_core::infrastructure::biometric::{build_signer, pick_gate, AnyGate};

/// For init / verify / compose: locate the Touch ID helper without instantiating
/// a signer. Surfaces a clear error if the binary is missing.
#[cfg(target_os = "macos")]
#[allow(dead_code)]
pub fn require_touchid_binary() -> Result<PathBuf> {
    let g = TouchIdGate::discover().context("locating touchid-prompt helper")?;
    let candidate =
        std::env::current_dir()?.join("target").join("touchid-prompt");
    if candidate.exists() {
        Ok(candidate)
    } else {
        let _ = g;
        Ok(PathBuf::from("touchid-prompt"))
    }
}
