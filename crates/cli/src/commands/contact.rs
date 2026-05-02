//! `sec contact` — manage the local contact book.
//!
//! ```text
//! sec contact add  --did <did> --name <name> [--relay <url>]
//! sec contact list
//! sec contact show <slug>
//! sec contact remove <slug>
//! ```
//!
//! `--relay` is optional. For `did:web` peers, omit it — the relay endpoint
//! is discovered live from the DID document's `serviceEndpoint`. For
//! `did:key` peers (no published document), `--relay` is required since
//! there is no live discovery channel.

use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};

use secretariat_core::application::{
    add_contact, find_by_slug, list_contacts, remove_contact,
};
use secretariat_core::domain::DidMethod;
use secretariat_core::{Contact, Did, DisplayName, RelayEndpoint};

use super::paths::key_paths;

#[derive(Parser, Debug)]
pub struct Args {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Add a new contact.
    Add {
        /// Peer's DID — `did:web:<host>` or `did:key:z<multibase>`.
        #[arg(long)]
        did: String,

        /// Human-friendly nickname (used as the lookup slug).
        #[arg(long)]
        name: String,

        /// Relay endpoint URL. Required for `did:key` peers; optional for
        /// `did:web` (looked up from their DID document if omitted).
        #[arg(long)]
        relay: Option<String>,
    },

    /// List all known contacts.
    List,

    /// Show a single contact by name slug.
    Show {
        /// Display-name slug (case-insensitive, lowercased no-space form).
        slug: String,
    },

    /// Remove a contact by name slug.
    Remove {
        /// Display-name slug (case-insensitive, lowercased no-space form).
        slug: String,
    },
}

pub fn run(args: Args) -> Result<()> {
    let paths = key_paths()?;
    match args.cmd {
        Cmd::Add { did, name, relay } => add(&paths.contacts, &did, &name, relay.as_deref()),
        Cmd::List => list(&paths.contacts),
        Cmd::Show { slug } => show(&paths.contacts, &slug),
        Cmd::Remove { slug } => remove(&paths.contacts, &slug),
    }
}

fn add(
    path: &std::path::Path,
    did_str: &str,
    name_str: &str,
    relay_str: Option<&str>,
) -> Result<()> {
    let did = Did::parse(did_str).map_err(|e| anyhow!("invalid --did: {e}"))?;
    let name = DisplayName::parse(name_str).map_err(|e| anyhow!("invalid --name: {e}"))?;

    let relay = match relay_str {
        Some(s) => Some(RelayEndpoint::parse(s).map_err(|e| anyhow!("invalid --relay: {e}"))?),
        None => None,
    };

    // did:key has no live discovery channel — relay endpoint must be supplied.
    if did.method() == DidMethod::Key && relay.is_none() {
        return Err(anyhow!(
            "did:key peers have no DID document for live relay discovery — \
             pass --relay <url> when adding a did:key contact"
        ));
    }

    let contact = Contact::new(did.clone(), name.clone(), relay);
    add_contact(path, contact)?;
    eprintln!("[sec] added contact `{}` → {}", name, did);
    Ok(())
}

fn list(path: &std::path::Path) -> Result<()> {
    let contacts = list_contacts(path)?;
    if contacts.is_empty() {
        eprintln!("[sec] no contacts yet — add one with `sec contact add ...`");
        return Ok(());
    }
    for c in &contacts {
        match &c.relay_endpoint {
            Some(r) => println!("{:<24} {}  via {}", c.display_name, c.did, r),
            None => println!("{:<24} {}  (relay via did doc)", c.display_name, c.did),
        }
    }
    Ok(())
}

fn show(path: &std::path::Path, slug: &str) -> Result<()> {
    match find_by_slug(path, slug)? {
        None => Err(anyhow!("no contact matches `{slug}`")),
        Some(c) => {
            println!("name:  {}", c.display_name);
            println!("did:   {}", c.did);
            match &c.relay_endpoint {
                Some(r) => println!("relay: {r}"),
                None => println!("relay: (discovered live from {})", c.did),
            }
            Ok(())
        }
    }
}

fn remove(path: &std::path::Path, slug: &str) -> Result<()> {
    let removed = remove_contact(path, slug)?;
    eprintln!("[sec] removed contact `{}` ({})", removed.display_name, removed.did);
    Ok(())
}
