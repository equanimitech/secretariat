//! Dispatch / send mechanism — drives the scribe (headless `claude -p`) to
//! compose and send a document to an external target. Today the only target
//! is Slack via the scribe's Slack MCP tools.
//!
//! This is the SEND mechanism, not a Slack feature: the flow (compose →
//! human gate → send-verbatim) is transport-blind. The Slack-ness lives only
//! in the per-target prompt template. Add a `DispatchTarget` variant + a
//! second template when a second target earns it — see the spec's seam note.
//!
//! Trust: send = signature layer (bodies are signed automatically). No stamp,
//! no `$attestation`, no lexicon record. The Touch-ID stamp gate is unreachable
//! from this path.

// The Tauri commands that consume these items land in a later task; suppress
// dead_code until then rather than hiding real future lints with a blanket allow.
#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// Where a dispatch goes. One variant today; the enum documents the seam.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum DispatchTarget {
    Slack,
}

/// Result of the COMPOSE phase — the scribe's draft, not yet sent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct ComposeResult {
    pub channel: String,
    pub body: String,
}

/// Result of the SEND phase. `permalink` is best-effort (the scribe may or
/// may not surface one); success is determined by the CLI exit, not this field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct SendResult {
    pub permalink: Option<String>,
}

/// Build the COMPOSE prompt. The scribe reads the doc, drafts a message per
/// the principal's free-form instruction, and returns JSON `{channel, body}`
/// WITHOUT sending.
pub fn compose_prompt(target: DispatchTarget, doc_path: &str, instruction: &str) -> String {
    match target {
        DispatchTarget::Slack => format!(
            "You are the scribe. Read the markdown document at `{doc_path}`. \
The principal wants to dispatch it to Slack per this instruction: «{instruction}». \
Compose the Slack message body and identify the target channel from the instruction. \
Do NOT send anything. Reply with ONLY a JSON object, no prose, no code fence: \
{{\"channel\": \"<#channel-or-name>\", \"body\": \"<message text>\"}}."
        ),
    }
}

/// Build the SEND prompt. The scribe sends the already-confirmed body verbatim.
pub fn send_prompt(target: DispatchTarget, channel: &str, body: &str) -> String {
    match target {
        DispatchTarget::Slack => format!(
            "Send this EXACT text verbatim to Slack channel `{channel}` using the \
`slack_send_message` tool. Do not edit, summarize, translate, or add anything. \
After sending, reply with ONLY a JSON object, no prose: \
{{\"permalink\": \"<message permalink or null>\"}}. \
The text to send is:\n\n{body}"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compose_prompt_embeds_path_and_instruction() {
        let p = compose_prompt(DispatchTarget::Slack, "/docs/note.md", "send the summary to #legal");
        assert!(p.contains("/docs/note.md"));
        assert!(p.contains("send the summary to #legal"));
        assert!(p.contains("Do NOT send"));
        assert!(p.contains("\"channel\""));
        assert!(p.contains("\"body\""));
    }

    #[test]
    fn send_prompt_embeds_channel_and_body_verbatim() {
        let p = send_prompt(DispatchTarget::Slack, "#legal", "Hello team");
        assert!(p.contains("#legal"));
        assert!(p.contains("Hello team"));
        assert!(p.contains("verbatim"));
        assert!(p.contains("slack_send_message"));
    }
}
