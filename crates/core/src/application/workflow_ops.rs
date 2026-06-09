//! Workflow use case: load + parse `.secretariat/workflows/*.md`, resolve which
//! fire for a stamped doc. Pure orchestration; IO is fs reads + `Preferences`.

use std::fs;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::domain::{
    doc_type_from_path, StampEvent, Trigger, Workflow, WorkflowMatch, WorkflowParseError,
};
use crate::infrastructure::preferences::{Preferences, PreferencesError};

#[derive(Debug, Error)]
pub enum WorkflowError {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("workflow `{name}`: {source}")]
    Parse {
        name: String,
        #[source]
        source: WorkflowParseError,
    },
    #[error(transparent)]
    Preferences(#[from] PreferencesError),
}

#[derive(serde::Deserialize)]
struct RawTrigger {
    on: String,
    #[serde(default)]
    r#match: RawMatch,
}

#[derive(serde::Deserialize, Default)]
struct RawMatch {
    #[serde(default)]
    r#type: Vec<String>,
    #[serde(default)]
    tags: Vec<String>,
}

/// Split a leading `---\n…\n---` frontmatter block from the body.
fn split_frontmatter(s: &str) -> Option<(&str, &str)> {
    let rest = s.strip_prefix("---\n")?;
    let idx = rest.find("\n---")?;
    let yaml = &rest[..idx];
    let after = &rest[idx + 4..]; // skip "\n---"
    let body = after.strip_prefix('\n').unwrap_or(after);
    Some((yaml, body))
}

/// Parse one workflow file's content into a `Workflow`.
pub fn parse_workflow(name: &str, content: &str) -> Result<Workflow, WorkflowParseError> {
    let (yaml, body) = split_frontmatter(content).ok_or(WorkflowParseError::BadFrontmatter)?;
    let raw: RawTrigger =
        serde_yaml::from_str(yaml).map_err(|e| WorkflowParseError::Yaml(e.to_string()))?;
    let on = StampEvent::parse(&raw.on)?;
    Ok(Workflow {
        name: name.to_string(),
        trigger: Trigger {
            on,
            match_: WorkflowMatch {
                types: raw.r#match.r#type,
                tags: raw.r#match.tags,
            },
        },
        prompt: body.trim().to_string(),
    })
}

/// Load + parse every `.secretariat/workflows/*.md` in `repo`. An absent
/// directory is not an error — it means "no workflows".
pub fn load_workflows(repo: &Path) -> Result<Vec<Workflow>, WorkflowError> {
    let dir = repo.join(".secretariat/workflows");
    let rd = match fs::read_dir(&dir) {
        Ok(rd) => rd,
        Err(_) => return Ok(Vec::new()),
    };
    let mut out = Vec::new();
    for entry in rd {
        let entry = entry.map_err(|source| WorkflowError::Io {
            path: dir.clone(),
            source,
        })?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let content = fs::read_to_string(&path).map_err(|source| WorkflowError::Io {
            path: path.clone(),
            source,
        })?;
        let wf = parse_workflow(&name, &content).map_err(|source| WorkflowError::Parse {
            name: name.clone(),
            source,
        })?;
        out.push(wf);
    }
    Ok(out)
}

/// Read a `type:` value from a doc's own frontmatter, if present.
fn frontmatter_type(doc_abs: &Path) -> Option<String> {
    let content = fs::read_to_string(doc_abs).ok()?;
    let (yaml, _) = split_frontmatter(&content)?;
    #[derive(serde::Deserialize)]
    struct Fm {
        r#type: Option<String>,
    }
    serde_yaml::from_str::<Fm>(yaml).ok()?.r#type
}

/// Workflows that fire for a just-stamped doc. Type = frontmatter `type:` if
/// present, else the path's immediate `docs/` subdir. Tags from the registry.
pub fn match_workflows(
    prefs_path: &Path,
    repo: &Path,
    doc_rel: &Path,
) -> Result<Vec<Workflow>, WorkflowError> {
    let workflows = load_workflows(repo)?;
    let doc_abs = repo.join(doc_rel);
    let doc_type = frontmatter_type(&doc_abs).or_else(|| doc_type_from_path(doc_rel));

    let prefs = Preferences::load(prefs_path)?;
    let abs = std::fs::canonicalize(repo).unwrap_or_else(|_| repo.to_path_buf());
    let tags = prefs
        .registry()
        .find(&abs)
        .map(|e| e.tags.clone())
        .unwrap_or_default();

    Ok(workflows
        .into_iter()
        .filter(|w| {
            matches!(w.trigger.on, StampEvent::Stamp)
                && w.trigger.match_.matches(doc_type.as_deref(), &tags)
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    const SAMPLE: &str =
        "---\non: stamp\nmatch:\n  type: [idea, pain]\n  tags: [themia]\n---\nDo the thing.\n";

    #[test]
    fn parse_extracts_trigger_and_prompt() {
        let wf = parse_workflow("to-linear", SAMPLE).unwrap();
        assert_eq!(wf.name, "to-linear");
        assert_eq!(wf.trigger.on, StampEvent::Stamp);
        assert_eq!(wf.trigger.match_.types, vec!["idea", "pain"]);
        assert_eq!(wf.trigger.match_.tags, vec!["themia"]);
        assert_eq!(wf.prompt, "Do the thing.");
    }

    #[test]
    fn parse_rejects_unknown_trigger() {
        let bad = "---\non: push\n---\nx";
        assert!(matches!(
            parse_workflow("x", bad),
            Err(WorkflowParseError::UnknownTrigger(_))
        ));
    }

    #[test]
    fn parse_rejects_missing_frontmatter() {
        assert!(matches!(
            parse_workflow("x", "no frontmatter here"),
            Err(WorkflowParseError::BadFrontmatter)
        ));
    }

    /// A git repo with one workflow file + a registered prefs entry tagged `themia`.
    fn repo_with_workflow() -> (TempDir, PathBuf, PathBuf) {
        let d = TempDir::new().unwrap();
        let repo = d.path().join("minerva");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        std::fs::create_dir_all(repo.join(".secretariat/workflows")).unwrap();
        std::fs::write(repo.join(".secretariat/workflows/to-linear.md"), SAMPLE).unwrap();
        let prefs = d.path().join("preferences.toml");
        crate::application::repo_ops::register_repo(
            &prefs,
            &repo,
            crate::infrastructure::RepoRole::Project,
            vec!["themia".into()],
        )
        .unwrap();
        (d, repo, prefs)
    }

    #[test]
    fn load_reads_all_workflow_files() {
        let (_d, repo, _prefs) = repo_with_workflow();
        let wfs = load_workflows(&repo).unwrap();
        assert_eq!(wfs.len(), 1);
        assert_eq!(wfs[0].name, "to-linear");
    }

    #[test]
    fn load_absent_dir_is_empty_not_error() {
        let d = TempDir::new().unwrap();
        assert!(load_workflows(d.path()).unwrap().is_empty());
    }

    #[test]
    fn match_fires_for_typed_tagged_doc() {
        let (_d, repo, prefs) = repo_with_workflow();
        // type from path = "pain", repo tag = "themia" → matches
        let hits = match_workflows(&prefs, &repo, Path::new("docs/pain/x.md")).unwrap();
        assert_eq!(hits.len(), 1);
        // flat doc → untyped → type filter present → no match
        let none = match_workflows(&prefs, &repo, Path::new("docs/flat.md")).unwrap();
        assert!(none.is_empty());
    }

    #[test]
    fn match_respects_frontmatter_type_override() {
        let (_d, repo, prefs) = repo_with_workflow();
        // a flat doc that DECLARES type: idea in its own frontmatter → matches
        std::fs::create_dir_all(repo.join("docs")).unwrap();
        std::fs::write(repo.join("docs/flat.md"), "---\ntype: idea\n---\nbody").unwrap();
        let hits = match_workflows(&prefs, &repo, Path::new("docs/flat.md")).unwrap();
        assert_eq!(hits.len(), 1);
    }
}
