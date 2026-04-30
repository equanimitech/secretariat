//! `AttentionEnvelope` — the receiver-published bounds.
//!
//! Per Ballestiero p67 ("Goals do not cascade. Bounds propagate."): the
//! principal's published attention envelope IS the propagated bound. Senders
//! must read it before composing; the protocol detects violations.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{Did, Envelope, EnvelopeDepth, EnvelopeUrgency};

/// Lexicon: `app.equanimi.secretariat.attentionEnvelope`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttentionEnvelope {
    pub signer: Did,
    pub depths_accepted: Vec<EnvelopeDepth>,
    pub urgencies_accepted: Vec<EnvelopeUrgency>,
    pub cadence: String,
    pub override_channel: Option<String>,
    pub updated_at: DateTime<Utc>,
}

impl AttentionEnvelope {
    pub const TYPE_ID: &'static str = "app.equanimi.secretariat.attentionEnvelope";

    /// Pure predicate: does the given envelope fit within the published bounds?
    ///
    /// `true` = the envelope's depth and urgency are both in the accepted lists.
    /// `false` = at least one is out of bounds (sender's scribe should not deliver,
    /// or should escalate to its principal for an override decision).
    pub fn admits(&self, envelope: &Envelope) -> bool {
        self.depths_accepted.contains(&envelope.depth)
            && self.urgencies_accepted.contains(&envelope.urgency)
    }
}

#[derive(Serialize, Deserialize)]
struct AttentionEnvelopeWire {
    #[serde(rename = "$type")]
    type_id: String,
    signer: Did,
    #[serde(rename = "depthsAccepted")]
    depths_accepted: Vec<EnvelopeDepth>,
    #[serde(rename = "urgenciesAccepted")]
    urgencies_accepted: Vec<EnvelopeUrgency>,
    cadence: String,
    #[serde(
        rename = "overrideChannel",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    override_channel: Option<String>,
    #[serde(rename = "updatedAt")]
    updated_at: DateTime<Utc>,
}

impl Serialize for AttentionEnvelope {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        AttentionEnvelopeWire {
            type_id: Self::TYPE_ID.to_string(),
            signer: self.signer.clone(),
            depths_accepted: self.depths_accepted.clone(),
            urgencies_accepted: self.urgencies_accepted.clone(),
            cadence: self.cadence.clone(),
            override_channel: self.override_channel.clone(),
            updated_at: self.updated_at,
        }
        .serialize(s)
    }
}

impl<'de> Deserialize<'de> for AttentionEnvelope {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let w = AttentionEnvelopeWire::deserialize(d)?;
        if w.type_id != Self::TYPE_ID {
            return Err(serde::de::Error::custom(format!(
                "expected $type {}, got {}",
                Self::TYPE_ID,
                w.type_id
            )));
        }
        Ok(AttentionEnvelope {
            signer: w.signer,
            depths_accepted: w.depths_accepted,
            urgencies_accepted: w.urgencies_accepted,
            cadence: w.cadence,
            override_channel: w.override_channel,
            updated_at: w.updated_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn bounds() -> AttentionEnvelope {
        AttentionEnvelope {
            signer: Did::parse("did:web:rafa.equanimi.tech").unwrap(),
            depths_accepted: vec![EnvelopeDepth::Gross, EnvelopeDepth::Subtle],
            urgencies_accepted: vec![EnvelopeUrgency::Soon, EnvelopeUrgency::Whenever],
            cadence: "weekday mornings, 09:00–10:00 GMT".into(),
            override_channel: Some("phone call".into()),
            updated_at: Utc.with_ymd_and_hms(2026, 4, 30, 8, 0, 0).unwrap(),
        }
    }

    fn env(depth: EnvelopeDepth, urgency: EnvelopeUrgency) -> Envelope {
        Envelope::builder(Did::parse("did:web:peer.example").unwrap())
            .depth(depth)
            .urgency(urgency)
            .build()
    }

    #[test]
    fn admits_in_bounds() {
        assert!(bounds().admits(&env(EnvelopeDepth::Subtle, EnvelopeUrgency::Soon)));
    }

    #[test]
    fn rejects_out_of_bounds_urgency() {
        assert!(!bounds().admits(&env(EnvelopeDepth::Subtle, EnvelopeUrgency::Now)));
    }

    #[test]
    fn rejects_out_of_bounds_depth() {
        let mut narrow = bounds();
        narrow.depths_accepted = vec![EnvelopeDepth::Gross];
        assert!(!narrow.admits(&env(EnvelopeDepth::Subtle, EnvelopeUrgency::Soon)));
    }

    #[test]
    fn attention_envelope_roundtrip_yaml() {
        let b = bounds();
        let yaml = serde_yaml::to_string(&b).unwrap();
        let back: AttentionEnvelope = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(b, back);
    }
}
