//! Use case: compose a document into a registered repo — place by
//! convention, sign at birth, commit pathspec-scoped.
//!
//! The write-side of the three-layer trust model (AGENTS.md hard rule #4):
//! every body composed through here carries the scribe's `$signature` the
//! moment it exists. The commit is the "dispatch = signature" tier — local,
//! scoped to the one file just written, never touching co-mingled state.
//! Stamping stays the principal's separate act.
//!
//! Pitch: `docs/pitches/2026-06-10-compose-keystone-slice.md`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use chrono::{DateTime, Utc};
use thiserror::Error;

use crate::application::agent_ops::{list_agents, AgentOpsError};
use crate::domain::{AgentRole, Did, EnvelopeSignature, SignerRole};
use crate::infrastructure::keys::{load_signing_key, KeyError, KeyPaths};
use crate::infrastructure::markdown::{
    embed_frontmatter_with_extra, lift_leading_frontmatter, LiftFrontmatterError, MarkdownError,
};
use crate::infrastructure::repo_registry::RepoEntry;

// ---------------------------------------------------------------------------
// DocType
// ---------------------------------------------------------------------------

/// The doc-type taxonomy compose owns. Each type maps to a bucket directory
/// under the repo root; the type also lands in the doc's editorial
/// frontmatter as `type:` so search can facet on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocType {
    Idea,
    Pain,
    Decision,
    Pitch,
    Note,
}

#[derive(Debug, Error)]
pub enum DocTypeParseError {
    #[error("unknown doc type `{0}` (known: idea, pain, decision, pitch, note)")]
    Unknown(String),
}

impl DocType {
    pub fn parse(s: &str) -> Result<Self, DocTypeParseError> {
        match s {
            "idea" => Ok(Self::Idea),
            "pain" => Ok(Self::Pain),
            "decision" => Ok(Self::Decision),
            "pitch" => Ok(Self::Pitch),
            "note" => Ok(Self::Note),
            other => Err(DocTypeParseError::Unknown(other.to_string())),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Idea => "idea",
            Self::Pain => "pain",
            Self::Decision => "decision",
            Self::Pitch => "pitch",
            Self::Note => "note",
        }
    }

    /// Bucket directory relative to the repo root. Mirrors the existing
    /// repo conventions (`docs/ideas/`, `docs/pain/`, …); `note` lands flat
    /// in `docs/`.
    pub fn bucket(&self) -> &'static str {
        match self {
            Self::Idea => "docs/ideas",
            Self::Pain => "docs/pain",
            Self::Decision => "docs/decisions",
            Self::Pitch => "docs/pitches",
            Self::Note => "docs",
        }
    }
}

impl std::fmt::Display for DocType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// Errors / outcome
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum ComposeError {
    #[error("repo `{0}` is not registered; run `sec repo add {0}` first")]
    NotRegistered(PathBuf),
    #[error("`{0}` is not a git repository")]
    NotAGitRepo(PathBuf),
    #[error("title produces an empty slug")]
    EmptyTitle,
    #[error("target already exists: {0} — compose never overwrites")]
    PathExists(PathBuf),
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("markdown error: {0}")]
    Markdown(#[from] MarkdownError),
    #[error(transparent)]
    Lift(#[from] LiftFrontmatterError),
}

/// What compose did. `committed == false` is not an error — the file is on
/// disk and signed regardless; `commit_skipped` says why the commit tier
/// didn't run (mid-rebase, detached HEAD, git failure).
#[derive(Debug)]
pub struct ComposeOutcome {
    pub path: PathBuf,
    pub committed: bool,
    pub commit_skipped: Option<String>,
    pub signature: EnvelopeSignature,
}

// ---------------------------------------------------------------------------
// Use case
// ---------------------------------------------------------------------------

/// Everything compose needs, resolved by the caller (CLI/MCP): the registry
/// slice, the scribe's DID + signing key, and the clock. Time and identity
/// enter via parameters; the use case does no discovery of its own.
pub struct ComposeRequest<'a> {
    pub registry: &'a [RepoEntry],
    pub repo_path: &'a Path,
    pub doc_type: DocType,
    pub title: &'a str,
    pub body: &'a str,
    pub signer: Did,
    pub signing_key: &'a ed25519_dalek::SigningKey,
    pub now: DateTime<Utc>,
}

/// Compose a doc into the request's repo: resolve the bucket from
/// `doc_type`, name it `<date>-<slug>.md`, lift any caller-supplied leading
/// frontmatter (reserved cryptographic keys rejected), sign the body with
/// the scribe's key, write, and commit the single path.
pub fn compose_document(req: ComposeRequest<'_>) -> Result<ComposeOutcome, ComposeError> {
    let ComposeRequest {
        registry,
        repo_path,
        doc_type,
        title,
        body,
        signer,
        signing_key,
        now,
    } = req;
    let repo = repo_path.canonicalize().map_err(|e| ComposeError::Io {
        path: repo_path.to_path_buf(),
        source: e,
    })?;

    if !registry.iter().any(|e| e.path == repo) {
        return Err(ComposeError::NotRegistered(repo));
    }
    if !repo.join(".git").exists() {
        return Err(ComposeError::NotAGitRepo(repo));
    }

    let slug = slugify(title);
    if slug.is_empty() {
        return Err(ComposeError::EmptyTitle);
    }

    // Lift caller-supplied leading frontmatter into the canonical single
    // block; rejects `$envelope` / `$signature` / `$attestation` injection.
    let lifted = lift_leading_frontmatter(body)?;
    let mut extra = lifted.extra;
    // Compose owns `type` — it is the search facet, not caller prose.
    extra.insert(
        "type".to_string(),
        serde_yaml::Value::String(doc_type.as_str().to_string()),
    );

    let rel =
        PathBuf::from(doc_type.bucket()).join(format!("{}-{slug}.md", now.format("%Y-%m-%d")));
    let abs = repo.join(&rel);
    if abs.exists() {
        return Err(ComposeError::PathExists(abs));
    }
    if let Some(parent) = abs.parent() {
        fs::create_dir_all(parent).map_err(|e| ComposeError::Io {
            path: parent.to_path_buf(),
            source: e,
        })?;
    }

    let signature =
        EnvelopeSignature::sign_body(signer, SignerRole::Agent, &lifted.body, now, signing_key);
    let content = embed_frontmatter_with_extra(&lifted.body, None, Some(&signature), None, extra)?;
    fs::write(&abs, content).map_err(|e| ComposeError::Io {
        path: abs.clone(),
        source: e,
    })?;

    // Commit tier. Soft by design: any reason the commit can't run leaves
    // the written+signed file in place and reports why.
    let (committed, commit_skipped) = match commit_single_path(&repo, &rel, doc_type, title) {
        Ok(()) => (true, None),
        Err(reason) => (false, Some(reason)),
    };

    Ok(ComposeOutcome {
        path: abs,
        committed,
        commit_skipped,
        signature,
    })
}

#[derive(Debug, Error)]
pub enum ScribeResolveError {
    #[error("no scribe agent provisioned — run `sec agent add <name> --role scribe` first")]
    NoScribe,
    #[error("{0} scribe agents found — compose signs as the sole scribe; selection (`--as`) lands when cardinality does")]
    MultipleScribes(usize),
    #[error("agent ops: {0}")]
    Agent(#[from] AgentOpsError),
    #[error("loading scribe key: {0}")]
    Key(#[from] KeyError),
}

/// Resolve the principal's sole scribe agent to a (DID, signing key) pair —
/// the identity compose signs as. Single-scribe assumption by design: zero
/// or multiple scribes is an error, not a picker.
pub fn resolve_sole_scribe(
    paths: &KeyPaths,
) -> Result<(Did, ed25519_dalek::SigningKey), ScribeResolveError> {
    let scribes: Vec<_> = list_agents(paths)?
        .into_iter()
        .filter(|a| a.role == AgentRole::Scribe)
        .collect();
    match scribes.len() {
        0 => Err(ScribeResolveError::NoScribe),
        1 => {
            let agent = scribes.into_iter().next().expect("len checked");
            let key = load_signing_key(&paths.agent_signing_key_path(agent.name.as_str()))?;
            Ok((agent.did, key))
        }
        n => Err(ScribeResolveError::MultipleScribes(n)),
    }
}

/// Kebab-case slug: lowercase alphanumeric runs joined by `-`, capped at 60
/// chars. Mirrors the `<date>-<slug>.md` naming already used across `docs/`.
fn slugify(title: &str) -> String {
    let mut out = String::new();
    let mut pending_dash = false;
    for c in title.chars() {
        if c.is_alphanumeric() {
            if pending_dash && !out.is_empty() {
                out.push('-');
            }
            pending_dash = false;
            for lc in c.to_lowercase() {
                out.push(lc);
            }
        } else {
            pending_dash = true;
        }
        if out.len() >= 60 {
            break;
        }
    }
    out
}

/// Commit exactly `rel` inside `repo`. Refuses (returns the reason) when the
/// repo is mid-rebase/merge or on a detached HEAD — composing must never
/// entangle an in-progress git operation. Never stages anything beyond the
/// one path; co-mingled working-tree state is untouched.
fn commit_single_path(
    repo: &Path,
    rel: &Path,
    doc_type: DocType,
    title: &str,
) -> Result<(), String> {
    if let Some(state) = unsafe_repo_state(repo) {
        return Err(state);
    }

    let rel_str = rel.to_string_lossy();
    run_git(repo, &["add", "--", &rel_str])?;
    let message = format!("docs({doc_type}): {title}");
    run_git(repo, &["commit", "-m", &message, "--", &rel_str])?;
    Ok(())
}

/// `Some(reason)` when the repo is in a state where an automated commit is
/// unsafe: detached HEAD (commit would orphan) or an in-progress
/// rebase/merge/cherry-pick.
fn unsafe_repo_state(repo: &Path) -> Option<String> {
    let head_ok = Command::new("git")
        .args(["symbolic-ref", "-q", "HEAD"])
        .current_dir(repo)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !head_ok {
        return Some("detached HEAD — commit skipped".to_string());
    }

    let git_dir_out = Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .current_dir(repo)
        .output()
        .ok()?;
    let git_dir = PathBuf::from(String::from_utf8_lossy(&git_dir_out.stdout).trim());
    let git_dir = if git_dir.is_absolute() {
        git_dir
    } else {
        repo.join(git_dir)
    };
    for (marker, label) in [
        ("rebase-merge", "rebase in progress"),
        ("rebase-apply", "rebase in progress"),
        ("MERGE_HEAD", "merge in progress"),
        ("CHERRY_PICK_HEAD", "cherry-pick in progress"),
    ] {
        if git_dir.join(marker).exists() {
            return Some(format!("{label} — commit skipped"));
        }
    }
    None
}

fn run_git(repo: &Path, args: &[&str]) -> Result<(), String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .map_err(|e| format!("git {}: {e}", args.join(" ")))?;
    if !out.status.success() {
        return Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::keys::generate_keypair;
    use crate::infrastructure::markdown::parse_document;
    use crate::infrastructure::repo_registry::RepoRole;
    use chrono::TimeZone;
    use tempfile::TempDir;

    fn test_now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 6, 10, 12, 0, 0).unwrap()
    }

    /// Positional shorthand over [`compose_document`] for test brevity.
    #[allow(clippy::too_many_arguments)]
    fn compose(
        registry: &[RepoEntry],
        repo_path: &Path,
        doc_type: DocType,
        title: &str,
        body: &str,
        signer: Did,
        signing_key: &ed25519_dalek::SigningKey,
        now: DateTime<Utc>,
    ) -> Result<ComposeOutcome, ComposeError> {
        compose_document(ComposeRequest {
            registry,
            repo_path,
            doc_type,
            title,
            body,
            signer,
            signing_key,
            now,
        })
    }

    fn scribe() -> (Did, ed25519_dalek::SigningKey) {
        let key = generate_keypair();
        let did = Did::from_ed25519_public_key(key.verifying_key().as_bytes());
        (did, key)
    }

    /// Init a real git repo in a tempdir and return (tempdir, canonical
    /// path, one-entry registry).
    fn test_repo() -> (TempDir, PathBuf, Vec<RepoEntry>) {
        let dir = TempDir::new().unwrap();
        let repo = dir.path().canonicalize().unwrap();
        for args in [
            vec!["init", "-q", "-b", "main"],
            vec!["config", "user.email", "test@example.com"],
            vec!["config", "user.name", "Test"],
        ] {
            let ok = Command::new("git")
                .args(&args)
                .current_dir(&repo)
                .status()
                .unwrap()
                .success();
            assert!(ok, "git {args:?} failed");
        }
        let registry = vec![RepoEntry {
            path: repo.clone(),
            role: RepoRole::Project,
            tags: vec![],
        }];
        (dir, repo, registry)
    }

    fn git_stdout(repo: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .args(args)
            .current_dir(repo)
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    #[test]
    fn composes_signs_and_commits() {
        let (_dir, repo, registry) = test_repo();
        let (did, key) = scribe();

        let out = compose(
            &registry,
            &repo,
            DocType::Idea,
            "Test Title",
            "# Test Title\n\nbody\n",
            did.clone(),
            &key,
            test_now(),
        )
        .unwrap();

        assert!(out.committed, "skip reason: {:?}", out.commit_skipped);
        assert_eq!(out.path, repo.join("docs/ideas/2026-06-10-test-title.md"));

        let parsed = parse_document(&fs::read_to_string(&out.path).unwrap()).unwrap();
        let sig = parsed.signature.expect("$signature embedded");
        assert_eq!(sig.signer, did);
        assert_eq!(sig.signer_role, SignerRole::Agent);
        assert!(sig.verify_body(&parsed.body, &key.verifying_key()));
        assert!(parsed.stamp.is_none(), "compose never stamps");
        assert_eq!(
            parsed.extra.get("type").and_then(|v| v.as_str()),
            Some("idea")
        );

        assert_eq!(
            git_stdout(&repo, &["log", "-1", "--format=%s"]),
            "docs(idea): Test Title"
        );
        assert_eq!(git_stdout(&repo, &["status", "--porcelain"]), "");
    }

    #[test]
    fn commit_leaves_comingled_state_untouched() {
        let (_dir, repo, registry) = test_repo();
        let (did, key) = scribe();
        fs::write(repo.join("unrelated.txt"), "dirty").unwrap();

        let out = compose(
            &registry,
            &repo,
            DocType::Pain,
            "a bug",
            "body\n",
            did,
            &key,
            test_now(),
        )
        .unwrap();

        assert!(out.committed);
        // The unrelated file is still untracked — not staged, not committed.
        assert_eq!(
            git_stdout(&repo, &["status", "--porcelain"]),
            "?? unrelated.txt"
        );
    }

    #[test]
    fn lifts_caller_frontmatter_and_owns_type() {
        let (_dir, repo, registry) = test_repo();
        let (did, key) = scribe();

        let body = "---\ntags: [transport]\ntype: decision\n---\n# T\n";
        let out = compose(
            &registry,
            &repo,
            DocType::Idea,
            "T",
            body,
            did,
            &key,
            test_now(),
        )
        .unwrap();

        let parsed = parse_document(&fs::read_to_string(&out.path).unwrap()).unwrap();
        // Caller keys survive; compose's `type` wins over the caller's.
        assert!(parsed.extra.contains_key("tags"));
        assert_eq!(
            parsed.extra.get("type").and_then(|v| v.as_str()),
            Some("idea")
        );
        // Single frontmatter block: the body must not start with a second one.
        assert!(!parsed.body.trim_start().starts_with("---"));
    }

    #[test]
    fn rejects_reserved_key_injection() {
        let (_dir, repo, registry) = test_repo();
        let (did, key) = scribe();
        let body = "---\n\"$attestation\": {forged: true}\n---\nbody\n";
        let r = compose(
            &registry,
            &repo,
            DocType::Note,
            "x",
            body,
            did,
            &key,
            test_now(),
        );
        assert!(matches!(r, Err(ComposeError::Lift(_))));
    }

    #[test]
    fn rejects_unregistered_repo() {
        let (_dir, repo, _registry) = test_repo();
        let (did, key) = scribe();
        let r = compose(&[], &repo, DocType::Idea, "x", "b", did, &key, test_now());
        assert!(matches!(r, Err(ComposeError::NotRegistered(_))));
    }

    #[test]
    fn rejects_existing_path() {
        let (_dir, repo, registry) = test_repo();
        let (did, key) = scribe();
        let _ = compose(
            &registry,
            &repo,
            DocType::Idea,
            "same",
            "b1",
            did.clone(),
            &key,
            test_now(),
        )
        .unwrap();
        let r = compose(
            &registry,
            &repo,
            DocType::Idea,
            "same",
            "b2",
            did,
            &key,
            test_now(),
        );
        assert!(matches!(r, Err(ComposeError::PathExists(_))));
    }

    #[test]
    fn skips_commit_on_detached_head() {
        let (_dir, repo, registry) = test_repo();
        let (did, key) = scribe();
        // Need one commit to detach onto.
        let _ = compose(
            &registry,
            &repo,
            DocType::Note,
            "first",
            "b",
            did.clone(),
            &key,
            test_now(),
        )
        .unwrap();
        assert!(Command::new("git")
            .args(["checkout", "-q", "--detach"])
            .current_dir(&repo)
            .status()
            .unwrap()
            .success());

        let out = compose(
            &registry,
            &repo,
            DocType::Note,
            "second",
            "b",
            did,
            &key,
            test_now(),
        )
        .unwrap();
        assert!(!out.committed);
        assert!(out.commit_skipped.unwrap().contains("detached"));
        // File still written + signed.
        assert!(out.path.exists());
    }

    #[test]
    fn empty_slug_rejected() {
        let (_dir, repo, registry) = test_repo();
        let (did, key) = scribe();
        let r = compose(
            &registry,
            &repo,
            DocType::Idea,
            "???",
            "b",
            did,
            &key,
            test_now(),
        );
        assert!(matches!(r, Err(ComposeError::EmptyTitle)));
    }

    #[test]
    fn resolve_sole_scribe_paths() {
        use crate::application::agent_ops::add_agent;
        use crate::domain::{AgentName, AgentSubstrate, DisplayName};
        use crate::infrastructure::identity_store::{save_identity, PrincipalIdentity};
        use crate::infrastructure::keys::{save_signing_key, KeyPaths};

        let tmp = TempDir::new().unwrap();
        let paths = KeyPaths::under(tmp.path().to_path_buf());
        paths.ensure_dirs().unwrap();
        let principal_key = generate_keypair();
        let did = Did::from_ed25519_public_key(&principal_key.verifying_key().to_bytes());
        save_signing_key(&paths.signing_key, &principal_key).unwrap();
        let when = test_now();
        let id = PrincipalIdentity {
            did,
            did_method: "did:key".to_string(),
            display_name: DisplayName::parse("Test").unwrap(),
            full_name: None,
            key_path: "identity/key".to_string(),
            key_type: "ed25519".to_string(),
            key_created_at: when,
            key_rotations: vec![],
            authorized_agents: vec![],
            created_at: when,
            signature: None,
            body: String::new(),
        };
        save_identity(&paths.identity_md, &id, &principal_key).unwrap();

        // No scribe yet.
        assert!(matches!(
            resolve_sole_scribe(&paths),
            Err(ScribeResolveError::NoScribe)
        ));

        let agent = add_agent(
            &paths,
            AgentName::parse("claude").unwrap(),
            AgentRole::Scribe,
            AgentSubstrate::parse("claude-code").unwrap(),
            when,
        )
        .unwrap();

        let (scribe_did, scribe_key) = resolve_sole_scribe(&paths).unwrap();
        assert_eq!(scribe_did, agent.did);
        // The resolved key must be the one whose pubkey the DID encodes.
        assert_eq!(
            Did::from_ed25519_public_key(&scribe_key.verifying_key().to_bytes()),
            scribe_did
        );
    }

    #[test]
    fn slugify_shapes() {
        assert_eq!(slugify("Test Title"), "test-title");
        assert_eq!(slugify("  MCP — compose & search!  "), "mcp-compose-search");
        assert_eq!(slugify("été à Paris"), "été-à-paris");
    }

    #[test]
    fn doc_type_buckets() {
        assert_eq!(DocType::parse("idea").unwrap().bucket(), "docs/ideas");
        assert_eq!(DocType::parse("note").unwrap().bucket(), "docs");
        assert!(DocType::parse("memo").is_err());
    }
}
