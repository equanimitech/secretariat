//! Infrastructure — concrete adapters. Populated incrementally per the
//! implementation sequencing in the plan.

pub mod binding_store;
pub mod biometric;
pub mod channel_def_store;
pub mod cognition;
pub mod composite_did_resolver;
pub mod contract_store;
pub mod crypto;
pub mod desktop;
pub mod did_key_resolver;
pub mod did_web_resolver;
pub mod ed25519_signer;
pub mod identity_store;
pub mod keys;
pub mod manifest_cache;
pub mod markdown;
pub mod native_biometric;
pub mod org_store;
pub mod preferences;
pub mod queue_dir;
pub mod repo_registry;
pub mod transport;
pub mod usage_ledger;

pub use binding_store::{load_channel_binding, resolve_channel_path, BindingStoreError};
pub use biometric::{build_signer, pick_gate, AnyGate};
pub use channel_def_store::{
    channel_def_exists, channel_def_exists_in_dir, channel_def_path, channel_dir,
    delete_channel as delete_channel_tree, load_channel_def, read_channel_meta_in_dir,
    save_channel_def, ChannelDefStoreError, CHANNEL_DEF_FILENAME, LEGACY_CHANNEL_DEF_FILENAME,
};
pub use cognition::PrefsLauncher;
pub use composite_did_resolver::CompositeDidResolver;
pub use desktop::open_in_secretariat;
pub use contract_store::{
    channel_contract_path, load_contract, load_contract_with_binding, org_contract_path,
    save_contract, save_stub_if_absent, ContractStoreError, CONTRACT_FILENAME,
};
pub use did_key_resolver::DidKeyResolver;
pub use did_web_resolver::DidWebResolver;
pub use ed25519_signer::{AlwaysAllowGate, BiometricGate, Ed25519Signer};
pub use identity_store::{
    load_identity, load_identity_verified, save_identity, save_identity_unsigned_for_migration,
    sign_identity, IdentityStoreError, KeyRotation, PrincipalIdentity,
};
pub use keys::{
    generate_keypair, load_signing_key, save_signing_key, write_did_document, KeyError, KeyPaths,
};
pub use markdown::{embed_stamp, parse_document, MarkdownError, ParsedDocument};
pub use native_biometric::NativeBiometricGate;
pub use org_store::{
    delete_org as delete_org_tree, list_org_dirs, load_org, org_channels_root, org_dir,
    org_metadata_path, save_org, OrgStoreError, ORG_METADATA_FILENAME,
};
pub use preferences::{
    load_or_migrate as load_or_migrate_preferences, CognitionPrefs, CognitionProvider,
    CompositionPrefs, DeliveryPrefs, Preferences, PreferencesError,
};
pub use queue_dir::{ciphertext_dir, envelopes_dir, queue_dir, AliasMap};
pub use repo_registry::{RepoEntry, RepoRegistry, RepoRole};
