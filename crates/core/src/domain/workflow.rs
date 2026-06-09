//! Workflow value objects — the in-repo `.secretariat/workflows/*.md` shape.
//! Pure: no IO. File reading + YAML parsing live in `application::workflow_ops`.

use std::path::Path;

/// The trigger event. Only `stamp` ships in v0.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StampEvent {
    Stamp,
}

impl StampEvent {
    pub fn parse(s: &str) -> Result<Self, WorkflowParseError> {
        match s {
            "stamp" => Ok(Self::Stamp),
            other => Err(WorkflowParseError::UnknownTrigger(other.to_string())),
        }
    }
}

/// Any-of filters. An empty vec means "unconstrained".
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkflowMatch {
    pub types: Vec<String>,
    pub tags: Vec<String>,
}

impl WorkflowMatch {
    /// True when every PRESENT filter has a non-empty intersection with the
    /// inputs. `doc_type = None` only passes a type filter that is empty.
    pub fn matches(&self, doc_type: Option<&str>, repo_tags: &[String]) -> bool {
        let type_ok =
            self.types.is_empty() || doc_type.is_some_and(|t| self.types.iter().any(|x| x == t));
        let tag_ok =
            self.tags.is_empty() || self.tags.iter().any(|x| repo_tags.iter().any(|rt| rt == x));
        type_ok && tag_ok
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trigger {
    pub on: StampEvent,
    pub match_: WorkflowMatch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workflow {
    pub name: String,
    pub trigger: Trigger,
    pub prompt: String,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum WorkflowParseError {
    #[error("unknown trigger `{0}` (expected: stamp)")]
    UnknownTrigger(String),
    #[error("missing or malformed frontmatter")]
    BadFrontmatter,
    #[error("yaml error: {0}")]
    Yaml(String),
}

/// A doc's type = the immediate subdir under `docs/`. `None` for a flat
/// `docs/x.md` or any path not nested under `docs/`. Frontmatter `type:` (read
/// in `application`) overrides this.
pub fn doc_type_from_path(doc_rel: &Path) -> Option<String> {
    let mut comps = doc_rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned());
    if comps.next().as_deref() != Some("docs") {
        return None;
    }
    let sub = comps.next()?; // immediate child of docs/
                             // It is a directory only if at least one more component (the file) follows.
    comps.next().map(|_| sub)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn match_intersects_present_filters() {
        let m = WorkflowMatch {
            types: vec!["idea".into(), "pain".into()],
            tags: vec!["themia".into()],
        };
        assert!(m.matches(Some("pain"), &["themia".into()]));
        assert!(!m.matches(Some("spec"), &["themia".into()])); // type miss
        assert!(!m.matches(Some("idea"), &["equanimitech".into()])); // tag miss
        assert!(!m.matches(None, &["themia".into()])); // untyped doc, type filter present
    }

    #[test]
    fn empty_filter_is_unconstrained() {
        let m = WorkflowMatch::default();
        assert!(m.matches(None, &[]));
        assert!(m.matches(Some("anything"), &["whatever".into()]));
    }

    #[test]
    fn type_is_immediate_subdir_under_docs() {
        assert_eq!(
            doc_type_from_path(Path::new("docs/pain/x.md")).as_deref(),
            Some("pain")
        );
        assert_eq!(
            doc_type_from_path(Path::new("docs/ideas/y.md")).as_deref(),
            Some("ideas")
        );
        // nested → immediate child only (per spec)
        assert_eq!(
            doc_type_from_path(Path::new("docs/superpowers/specs/z.md")).as_deref(),
            Some("superpowers")
        );
        // flat doc → untyped
        assert_eq!(doc_type_from_path(Path::new("docs/flat.md")), None);
        // not under docs/ → untyped
        assert_eq!(doc_type_from_path(Path::new("README.md")), None);
    }
}
