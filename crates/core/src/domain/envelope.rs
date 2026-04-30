//! `Envelope` — one bid for the receiver's attention.
//!
//! Composed by the scribe (AI) and addressed by the principal. The envelope is
//! routing metadata; in MVP the cryptographic stamp covers only the body, not
//! the envelope (see decision log #1 in the plan). v2 may add envelope signing
//! for bilateral bound enforcement.

use serde::{Deserialize, Serialize};

use super::{Did, EnvelopeDepth, EnvelopeUrgency};

/// Lexicon: `app.equanimi.secretariat.envelope`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Envelope {
    pub from: Did,
    pub to: Option<Did>,
    pub depth: EnvelopeDepth,
    pub urgency: EnvelopeUrgency,
    pub source: String,
    pub cadence_hint: Option<String>,
}

impl Envelope {
    pub const TYPE_ID: &'static str = "app.equanimi.secretariat.envelope";

    pub fn builder(from: Did) -> EnvelopeBuilder {
        EnvelopeBuilder::new(from)
    }
}

#[derive(Serialize, Deserialize)]
struct EnvelopeWire {
    #[serde(rename = "$type")]
    type_id: String,
    from: Did,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    to: Option<Did>,
    depth: EnvelopeDepth,
    urgency: EnvelopeUrgency,
    source: String,
    #[serde(rename = "cadenceHint", default, skip_serializing_if = "Option::is_none")]
    cadence_hint: Option<String>,
}

impl Serialize for Envelope {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        EnvelopeWire {
            type_id: Self::TYPE_ID.to_string(),
            from: self.from.clone(),
            to: self.to.clone(),
            depth: self.depth,
            urgency: self.urgency,
            source: self.source.clone(),
            cadence_hint: self.cadence_hint.clone(),
        }
        .serialize(s)
    }
}

impl<'de> Deserialize<'de> for Envelope {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let w = EnvelopeWire::deserialize(d)?;
        if w.type_id != Self::TYPE_ID {
            return Err(serde::de::Error::custom(format!(
                "expected $type {}, got {}",
                Self::TYPE_ID,
                w.type_id
            )));
        }
        Ok(Envelope {
            from: w.from,
            to: w.to,
            depth: w.depth,
            urgency: w.urgency,
            source: w.source,
            cadence_hint: w.cadence_hint,
        })
    }
}

/// Fluent builder. The only mandatory field is `from`. Defaults:
/// `depth = Subtle`, `urgency = Whenever`, `source = ""`.
#[derive(Debug, Clone)]
pub struct EnvelopeBuilder {
    from: Did,
    to: Option<Did>,
    depth: EnvelopeDepth,
    urgency: EnvelopeUrgency,
    source: String,
    cadence_hint: Option<String>,
}

impl EnvelopeBuilder {
    pub fn new(from: Did) -> Self {
        Self {
            from,
            to: None,
            depth: EnvelopeDepth::Subtle,
            urgency: EnvelopeUrgency::Whenever,
            source: String::new(),
            cadence_hint: None,
        }
    }

    pub fn to(mut self, to: Did) -> Self {
        self.to = Some(to);
        self
    }

    pub fn depth(mut self, depth: EnvelopeDepth) -> Self {
        self.depth = depth;
        self
    }

    pub fn urgency(mut self, urgency: EnvelopeUrgency) -> Self {
        self.urgency = urgency;
        self
    }

    pub fn source(mut self, source: impl Into<String>) -> Self {
        self.source = source.into();
        self
    }

    pub fn cadence_hint(mut self, hint: impl Into<String>) -> Self {
        self.cadence_hint = Some(hint.into());
        self
    }

    pub fn build(self) -> Envelope {
        Envelope {
            from: self.from,
            to: self.to,
            depth: self.depth,
            urgency: self.urgency,
            source: self.source,
            cadence_hint: self.cadence_hint,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Envelope {
        Envelope::builder(Did::parse("did:web:rafa.equanimi.tech").unwrap())
            .to(Did::parse("did:web:marcelo.ballestiero.com").unwrap())
            .depth(EnvelopeDepth::Subtle)
            .urgency(EnvelopeUrgency::Soon)
            .source("claude-code-2026-04-30T14:22:00Z")
            .cadence_hint("morning")
            .build()
    }

    #[test]
    fn envelope_roundtrip_yaml() {
        let e = fixture();
        let yaml = serde_yaml::to_string(&e).unwrap();
        let back: Envelope = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(e, back);
    }

    #[test]
    fn envelope_emits_camel_case_keys_and_type() {
        let yaml = serde_yaml::to_string(&fixture()).unwrap();
        assert!(yaml.contains("$type: app.equanimi.secretariat.envelope"));
        assert!(yaml.contains("cadenceHint"));
    }

    #[test]
    fn envelope_self_addressed_when_no_to() {
        let e = Envelope::builder(Did::parse("did:web:rafa.equanimi.tech").unwrap()).build();
        let yaml = serde_yaml::to_string(&e).unwrap();
        assert!(!yaml.contains("to:"));
    }
}
