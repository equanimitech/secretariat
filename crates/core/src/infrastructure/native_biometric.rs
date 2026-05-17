//! In-process biometric gate. Calls the platform's native presence API
//! directly from the host process — no subprocess helper, no Swift binary
//! to ship.
//!
//! - macOS: `LAContext` from LocalAuthentication.framework via
//!   `objc2-local-authentication`. Touch ID / Apple Watch unlock.
//! - Windows: `UserConsentVerifier` via the `windows` crate. Windows Hello
//!   (face, fingerprint, PIN — picker resolved by the OS).
//! - Other platforms: returns `BiometricRefused`. Secretariat does not run
//!   here in v0.5.
//!
//! Threading idiom on macOS (mpsc bridge from `evaluatePolicy` callback)
//! adapted from `Choochmeque/tauri-plugin-biometry` (MIT).

use crate::infrastructure::ed25519_signer::BiometricGate;
use crate::ports::SignerError;

/// In-process presence gate. Unit struct — all state lives in the OS.
#[derive(Debug, Default, Clone, Copy)]
pub struct NativeBiometricGate;

impl NativeBiometricGate {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(target_os = "macos")]
impl BiometricGate for NativeBiometricGate {
    fn prompt(&self, reason: &str) -> Result<(), SignerError> {
        use objc2_foundation::{NSError, NSString};
        use objc2_local_authentication::{LAContext, LAPolicy};

        let context = unsafe { LAContext::new() };
        let policy = LAPolicy::DeviceOwnerAuthenticationWithBiometrics;

        // Fast-fail when no biometric is enrolled / sensor absent.
        unsafe {
            context
                .canEvaluatePolicy_error(policy)
                .map_err(|_| SignerError::BiometricRefused)?;
        }

        let reason_str = NSString::from_str(reason);
        let (tx, rx) = std::sync::mpsc::channel::<bool>();
        let tx_block = tx.clone();

        unsafe {
            context.evaluatePolicy_localizedReason_reply(
                policy,
                &reason_str,
                &block2::StackBlock::new(
                    move |success: objc2::runtime::Bool, _err: *mut NSError| {
                        let _ = tx_block.send(success.as_bool());
                    },
                ),
            );
        }

        match rx.recv() {
            Ok(true) => Ok(()),
            _ => Err(SignerError::BiometricRefused),
        }
    }
}

#[cfg(target_os = "windows")]
impl BiometricGate for NativeBiometricGate {
    fn prompt(&self, reason: &str) -> Result<(), SignerError> {
        use windows::core::HSTRING;
        use windows::Security::Credentials::UI::{
            UserConsentVerificationResult, UserConsentVerifier,
        };

        let reason_h = HSTRING::from(reason);
        let op = UserConsentVerifier::RequestVerificationAsync(&reason_h)
            .map_err(|_| SignerError::BiometricRefused)?;
        let result = op.get().map_err(|_| SignerError::BiometricRefused)?;

        match result {
            UserConsentVerificationResult::Verified => Ok(()),
            _ => Err(SignerError::BiometricRefused),
        }
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
impl BiometricGate for NativeBiometricGate {
    fn prompt(&self, _reason: &str) -> Result<(), SignerError> {
        Err(SignerError::BiometricRefused)
    }
}
