//! Tauri-side cognition adapters.
//!
//! Some adapters live here rather than `crates/core/src/infrastructure/cognition/`
//! because they depend on Tauri runtime types (tauri-plugin-shell for
//! sidecar spawn, AppHandle for lifecycle). HTTP-style adapters (Anthropic
//! API, OpenAI-compat) stay in core where they belong — no Tauri leakage.

pub mod claude_code_sdk;
