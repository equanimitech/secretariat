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

use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_plugin_shell::ShellExt;

use crate::cognition::claude_code_sdk::resolve_claude_path;

/// Run the configured cognition CLI headless with `prompt`, return the agent's
/// reply text (already unwrapped from the `--output-format json` envelope).
async fn run_scribe(app: &AppHandle, prompt: &str) -> Result<String, String> {
    let claude = resolve_claude_path()
        .ok_or_else(|| "cognition CLI (`claude`) not found on PATH".to_string())?;
    let output = app
        .shell()
        .command(claude)
        .args(["-p", prompt, "--output-format", "json"])
        .output()
        .await
        .map_err(|e| format!("could not run the scribe: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("scribe exited with an error: {}", stderr.trim()));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    extract_result_text(&stdout)
}

/// COMPOSE: draft a message from the document. Does NOT send.
#[tauri::command]
#[specta::specta]
pub async fn dispatch_compose(
    app: AppHandle,
    target: DispatchTarget,
    doc_path: String,
    instruction: String,
) -> Result<ComposeResult, String> {
    let prompt = compose_prompt(target, &doc_path, &instruction);
    let text = run_scribe(&app, &prompt).await?;
    parse_compose_output(&text)
}

/// SEND: post the principal-confirmed body verbatim.
#[tauri::command]
#[specta::specta]
pub async fn dispatch_send(
    app: AppHandle,
    target: DispatchTarget,
    channel: String,
    body: String,
) -> Result<SendResult, String> {
    let prompt = send_prompt(target, &channel, &body);
    let text = run_scribe(&app, &prompt).await?;
    Ok(parse_send_output(&text))
}

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
///
/// The `body` must be principal-confirmed before this is called — the human gate lives in the
/// frontend composer, not in this function. Do not call with unreviewed content.
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

/// Pull the agent's reply text out of the `claude -p --output-format json`
/// envelope. Errors if the envelope reports `is_error` or has no `result`.
pub fn extract_result_text(stdout: &str) -> Result<String, String> {
    let v: serde_json::Value = serde_json::from_str(stdout.trim())
        .map_err(|e| format!("cognition CLI returned non-JSON output: {e}"))?;
    if v.get("is_error").and_then(|b| b.as_bool()) == Some(true) {
        let msg = v.get("result").and_then(|r| r.as_str()).unwrap_or("unknown error");
        return Err(format!("scribe reported an error: {msg}"));
    }
    v.get("result")
        .and_then(|r| r.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "cognition CLI output had no `result` field".to_string())
}

/// Strip an optional ```json … ``` fence and surrounding whitespace.
fn strip_fence(text: &str) -> &str {
    let t = text.trim();
    let t = t.strip_prefix("```json").or_else(|| t.strip_prefix("```")).unwrap_or(t);
    t.trim().strip_suffix("```").unwrap_or(t).trim()
}

/// Parse the COMPOSE agent reply into `{channel, body}`.
pub fn parse_compose_output(text: &str) -> Result<ComposeResult, String> {
    serde_json::from_str::<ComposeResult>(strip_fence(text))
        .map_err(|e| format!("could not parse composed message (expected {{channel, body}}): {e}"))
}

/// Parse the SEND agent reply. Missing/garbled permalink is non-fatal — the
/// CLI exit already told us the send succeeded — so fall back to `None`.
pub fn parse_send_output(text: &str) -> SendResult {
    let permalink = serde_json::from_str::<SendResult>(strip_fence(text))
        .ok()
        .and_then(|r| r.permalink);
    SendResult { permalink }
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

    #[test]
    fn extract_result_text_pulls_result_field() {
        let env = r#"{"type":"result","is_error":false,"result":"hello"}"#;
        assert_eq!(extract_result_text(env).unwrap(), "hello");
    }

    #[test]
    fn extract_result_text_errors_on_is_error() {
        let env = r#"{"type":"result","is_error":true,"result":"boom"}"#;
        assert!(extract_result_text(env).unwrap_err().contains("boom"));
    }

    #[test]
    fn parse_compose_output_handles_bare_and_fenced_json() {
        let bare = "{\"channel\":\"#legal\",\"body\":\"Hi\"}";
        assert_eq!(
            parse_compose_output(bare).unwrap(),
            ComposeResult { channel: "#legal".into(), body: "Hi".into() }
        );
        let fenced = "```json\n{\"channel\":\"#legal\",\"body\":\"Hi\"}\n```";
        assert_eq!(parse_compose_output(fenced).unwrap().channel, "#legal");
    }

    #[test]
    fn parse_compose_output_handles_plain_fence() {
        let fenced_plain = "```\n{\"channel\":\"#legal\",\"body\":\"Hi\"}\n```";
        assert_eq!(parse_compose_output(fenced_plain).unwrap().channel, "#legal");
    }

    #[test]
    fn parse_send_output_tolerates_missing_permalink() {
        assert_eq!(parse_send_output("not json").permalink, None);
        assert_eq!(
            parse_send_output(r#"{"permalink":"https://x"}"#).permalink,
            Some("https://x".into())
        );
    }
}
