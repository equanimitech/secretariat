//! Markdown reader/editor — file IO and pending-opens buffer.
//!
//! Domain-flavored module: pure value-object helpers that the Tauri command
//! layer in `commands::markdown` wraps for IPC. No Tauri state lives here.

pub mod file_io;
pub mod pending;
