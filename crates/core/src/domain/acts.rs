//! Enum value objects: stamp acts.
//!
//! Serializes as lowercase strings to match the AT-proto-shaped lexicon.

use serde::{Deserialize, Serialize};

/// What a stamp records the principal as having done.
///
/// MVP supports `Attest`. The other variants are reserved in the lexicon and
/// will be exercised once cadence + bilateral correspondence land.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum StampAct {
    /// "I read this and stand behind it."
    Attest,
    /// "Send back to queue, lower priority, with reason."
    Defer,
    /// "I'm willing to forward this with my seal."
    Vouch,
    /// "I disagree with this content."
    Dispute,
    /// "Not for me — route to X."
    Redirect,
}

impl std::fmt::Display for StampAct {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            StampAct::Attest => "attest",
            StampAct::Defer => "defer",
            StampAct::Vouch => "vouch",
            StampAct::Dispute => "dispute",
            StampAct::Redirect => "redirect",
        };
        f.write_str(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamp_act_serde_lowercase() {
        let json = serde_json::to_string(&StampAct::Attest).unwrap();
        assert_eq!(json, "\"attest\"");
        let back: StampAct = serde_json::from_str("\"defer\"").unwrap();
        assert_eq!(back, StampAct::Defer);
    }
}
