//! Infrastructure — concrete adapters. Populated incrementally per the
//! implementation sequencing in the plan.

pub mod biometric;
pub mod cognition;
pub mod composite_did_resolver;
pub mod contact_store;
pub mod crypto;
pub mod did_key_resolver;
pub mod did_web_resolver;
pub mod ed25519_signer;
pub mod keys;
pub mod markdown;
pub mod profile_store;
#[cfg(target_os = "macos")]
pub mod touchid;
pub mod transport;

pub use biometric::{build_signer, pick_gate, AnyGate};
pub use composite_did_resolver::CompositeDidResolver;
pub use contact_store::{ContactBook, ContactStoreError};
pub use did_key_resolver::DidKeyResolver;
pub use did_web_resolver::DidWebResolver;
pub use ed25519_signer::{AlwaysAllowGate, BiometricGate, Ed25519Signer};
pub use keys::{generate_keypair, load_signing_key, save_signing_key, write_did_document, KeyError, KeyPaths};
pub use markdown::{embed_stamp, parse_document, MarkdownError, ParsedDocument};
pub use profile_store::{load_profile, save_profile, PrincipalProfile, ProfileStoreError};
#[cfg(target_os = "macos")]
pub use touchid::TouchIdGate;
