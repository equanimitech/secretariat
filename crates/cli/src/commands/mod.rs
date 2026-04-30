//! Subcommand handlers for the `sec` CLI.
//!
//! Each command exposes an `Args` struct (clap-derived) and a `run(Args)` entrypoint.

pub mod compose;
pub mod init;
pub mod list;
pub mod stamp;
pub mod verify;

mod biometric;
mod paths;
