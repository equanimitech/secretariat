//! Touch ID biometric gate. Shells out to the compiled Swift helper at
//! `tools/touchid-prompt/` (built separately via `bash tools/touchid-prompt/build.sh`).
//!
//! Per decision-log #11: the reason string is `"Stamp Secretariat envelope: <basename>"`,
//! constructed by the application layer.

use std::path::PathBuf;

use crate::infrastructure::ed25519_signer::BiometricGate;
use crate::ports::SignerError;

#[derive(Debug, Clone)]
pub struct TouchIdGate {
    binary_path: PathBuf,
}

impl TouchIdGate {
    pub fn new(binary_path: PathBuf) -> Self {
        Self { binary_path }
    }

    /// Find the Swift helper. Search order:
    /// 1. `$SECRETARIAT_TOUCHID_BINARY` if set.
    /// 2. `<workspace target dir>/touchid-prompt` (set via `SECRETARIAT_TARGET_DIR` if needed).
    /// 3. `~/.secretariat/bin/touchid-prompt`.
    /// 4. `touchid-prompt` on `$PATH`.
    pub fn discover() -> Result<Self, std::io::Error> {
        if let Ok(p) = std::env::var("SECRETARIAT_TOUCHID_BINARY") {
            return Ok(Self::new(PathBuf::from(p)));
        }

        if let Ok(target) = std::env::var("SECRETARIAT_TARGET_DIR") {
            let candidate = PathBuf::from(target).join("touchid-prompt");
            if candidate.exists() {
                return Ok(Self::new(candidate));
            }
        }

        if let Some(home) = dirs::home_dir() {
            let candidate = home.join(".secretariat/bin/touchid-prompt");
            if candidate.exists() {
                return Ok(Self::new(candidate));
            }
        }

        // Fall back to `touchid-prompt` on PATH; let std::process::Command resolve.
        Ok(Self::new(PathBuf::from("touchid-prompt")))
    }
}

impl BiometricGate for TouchIdGate {
    fn prompt(&self, reason: &str) -> Result<(), SignerError> {
        let status = std::process::Command::new(&self.binary_path)
            .arg(reason)
            .status()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        format!(
                            "touchid-prompt helper not found at {} — install it with \
                             `bash tools/touchid-prompt/build.sh && \
                             install -m 0755 target/touchid-prompt ~/.secretariat/bin/touchid-prompt`, \
                             or rerun `sec init` (best-effort builds it via swiftc). \
                             Override with $SECRETARIAT_TOUCHID_BINARY",
                            self.binary_path.display()
                        ),
                    )
                } else {
                    e
                }
            })
            .map_err(SignerError::Io)?;

        if status.success() {
            Ok(())
        } else {
            Err(SignerError::BiometricRefused)
        }
    }
}
