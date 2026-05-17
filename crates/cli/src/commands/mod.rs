//! Subcommand handlers for the `sec` CLI.
//!
//! Each command exposes an `Args` struct (clap-derived) and a `run(Args)` entrypoint.

pub mod capture;
pub mod channels;
pub mod compose;
pub mod orgs;
pub mod contact;
pub mod daemon;
pub mod init;
pub mod invite;
pub mod launch;
pub mod list;
pub mod mcp;
pub mod profile;
pub mod read;
pub mod stamp;
pub mod verify;

mod biometric;
pub(crate) mod paths;
