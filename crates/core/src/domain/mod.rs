//! Pure domain: bounded context = human attestation of AI-mediated documents.
//!
//! No IO. Types are the architectural guardrails — illegal states unrepresentable.
//! Aggregates (`AttestedDocument`) enforce cross-entity invariants at construction.

mod acts;
mod agent;
mod agent_manifest;
mod attested_document;
mod channel_binding;
mod channel_contract;
mod channel_def;
mod contact;
mod envelope;
mod identity;
mod org;
mod org_alias;
mod queue_handle;
mod recipient;
mod root;
mod signature;
mod stamp;

pub use acts::{EnvelopeDepth, EnvelopeUrgency, StampAct};
pub use agent::{
    Agent, AgentName, AgentNameParseError, AgentRole, AgentRoleParseError, AgentSubstrate,
    AgentSubstrateParseError,
};
pub use agent_manifest::{
    AgentManifest, AgentManifestFrontmatter, AgentManifestParseError, ManifestTarget,
    ManifestTargetParseError, AGENT_MANIFEST_TYPE,
};
pub use attested_document::{canonical_body_hash, AttestedDocument, DocumentInvariantError};
pub use channel_binding::ChannelBinding;
pub use channel_contract::{ChannelContract, TrustGate};
pub use channel_def::ChannelDef;
pub use contact::{
    DisplayName, DisplayNameParseError, RelayEndpoint, RelayEndpointParseError,
};
pub use envelope::{AgSource, EncryptionScheme, Envelope, EnvelopeBuilder};
pub use identity::{Did, DidMethod, DidParseError, DocHash, Signature, SignatureParseError};
pub use org::Org;
pub use org_alias::{OrgAlias, OrgAliasError};
pub use queue_handle::{QueueHandle, QueueHandleError};
pub use recipient::Recipient;
pub use root::Root;
pub use signature::{EnvelopeSignature, SignerRole, SignerRoleParseError};
pub use stamp::Stamp;
