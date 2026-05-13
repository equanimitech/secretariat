//! Infrastructure — concrete adapters. Populated incrementally per the
//! implementation sequencing in the plan.

pub mod biometric;
pub mod channel_def_store;
pub mod cognition;
pub mod contract_store;
pub mod composite_did_resolver;
pub mod contact_store;
pub mod crypto;
pub mod did_key_resolver;
pub mod did_web_resolver;
pub mod ed25519_signer;
pub mod keys;
pub mod markdown;
pub mod org_store;
pub mod preferences;
pub mod profile_store;
pub mod queue_dir;
#[cfg(target_os = "macos")]
pub mod touchid;
pub mod transport;

pub use biometric::{build_signer, pick_gate, AnyGate};
pub use composite_did_resolver::CompositeDidResolver;
pub use contact_store::{ContactBook, ContactStoreError};
pub use did_key_resolver::DidKeyResolver;
pub use did_web_resolver::DidWebResolver;
pub use ed25519_signer::{AlwaysAllowGate, BiometricGate, Ed25519Signer};
pub use channel_def_store::{
    channel_def_path, channel_dir, delete_channel as delete_channel_tree, load_channel_def,
    save_channel_def, ChannelDefStoreError, CHANNEL_DEF_FILENAME,
};
pub use contract_store::{
    channel_contract_path, load_contract, org_contract_path, save_contract, save_stub_if_absent,
    ContractStoreError, CONTRACT_FILENAME,
};
pub use keys::{generate_keypair, load_signing_key, save_signing_key, write_did_document, KeyError, KeyPaths};
pub use markdown::{embed_stamp, parse_document, MarkdownError, ParsedDocument};
pub use org_store::{
    delete_org as delete_org_tree, list_org_dirs, load_org, org_channels_root, org_dir,
    org_metadata_path, save_org, OrgStoreError, ORG_METADATA_FILENAME,
};
pub use preferences::{
    load_or_migrate as load_or_migrate_preferences, CognitionPrefs, CognitionProvider,
    CompositionPrefs, DeliveryPrefs, Preferences, PreferencesError,
};
pub use profile_store::{load_profile, save_profile, PrincipalProfile, ProfileStoreError};
pub use queue_dir::{
    ciphertext_dir, envelopes_dir, outbox_dir, queue_dir, AliasMap, SELF_ALIAS,
};
#[cfg(target_os = "macos")]
pub use touchid::TouchIdGate;
