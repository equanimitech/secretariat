//! Subcommand handlers for the `sec` CLI.
//!
//! Each command exposes an `Args` struct (clap-derived) and a `run(Args)` entrypoint.

pub mod agent;
pub mod daemon;
pub mod init;
pub mod launch;
pub mod mcp;
pub mod profile;
pub mod read;
pub mod stamp;
pub mod verify;
pub mod view;

pub(crate) mod paths;
