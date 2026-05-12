//! JSON-RPC 2.0 wire shapes for the daemon's Unix-socket protocol.
//!
//! Deliberately a thin reuse of the JSON-RPC envelope rather than a
//! bespoke protocol — keeps the surface boring, well-understood, and
//! easy for any future caller to speak. One request, one response, one
//! line each. Streaming responses (push subscriptions to per-channel
//! sequences) land in Slice 5 with their own framing.

use serde::{Deserialize, Serialize};

pub const JSONRPC_VERSION: &str = "2.0";

/// JSON-RPC error codes the daemon emits.
///
/// We stay inside the reserved server-error range (-32099 to -32000)
/// for daemon-specific failures, and the standard pre-defined codes for
/// protocol-level issues.
pub mod codes {
    /// `-32601`. Method does not exist on this daemon.
    pub const METHOD_NOT_FOUND: i32 = -32601;
    /// `-32700`. Parsing the request failed.
    pub const PARSE_ERROR: i32 = -32700;
    /// `-32600`. Request shape was wrong (missing field, wrong type).
    pub const INVALID_REQUEST: i32 = -32600;
    /// `-32000`. Catch-all for application-layer failures inside a
    /// dispatched method (e.g. `tick` failed inside `sync_now`).
    pub const INTERNAL_ERROR: i32 = -32000;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub jsonrpc: String,
    pub id: u64,
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub jsonrpc: String,
    pub id: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
}

impl Response {
    pub fn ok(id: u64, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.into(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn err(id: u64, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.into(),
            id,
            result: None,
            error: Some(RpcError {
                code,
                message: message.into(),
            }),
        }
    }
}
