//! Timeline use case: a chronological view of docs across every registered
//! repo, grouped by date, each badged by stamp state.
//!
//! Answers "what did I create today / over the last days / last month" by
//! globbing the `docs/` tree of each repo in the substrate manifest. Dates
//! come from the `<date>-<slug>.md` filename convention (no file read needed
//! to bucket); state is derived from the frontmatter blocks (no decryption):
//!
//! - `$attestation` present            → **stamped** (principal committed)
//! - `$signature` present, no stamp    → **signed** (scribe-composed, informational)
//! - neither                           → **raw** (plain markdown)
//!
//! Pure orchestration: filesystem IO via `std::fs` (application layer, like
//! `repo_ops`), but `today` enters as a parameter so the range logic stays
//! testable and the core never calls `Utc::now()`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use chrono::{Datelike, Duration, NaiveDate};
use thiserror::Error;

use crate::application::repo_ops::{list_repos, RepoOpsError};

#[derive(Debug, Error)]
pub enum TimelineError {
    #[error(transparent)]
    Repos(#[from] RepoOpsError),
    #[error("invalid range `{spec}`: {reason}")]
    BadRange { spec: String, reason: String },
}

/// Stamp state of a doc, derived from its frontmatter (no decryption).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocState {
    /// `$attestation` present — principal Touch-ID-attested. Authoritative.
    Stamped,
    /// `$signature` present, no stamp — scribe-composed, informational.
    Signed,
    /// Neither block — plain markdown.
    Raw,
}

impl DocState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Stamped => "stamped",
            Self::Signed => "signed",
            Self::Raw => "raw",
        }
    }

    /// Parse a filter string (`stamped` | `signed` | `raw`).
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "stamped" => Some(Self::Stamped),
            "signed" => Some(Self::Signed),
            "raw" => Some(Self::Raw),
            _ => None,
        }
    }
}

/// One dated doc on the timeline.
#[derive(Debug, Clone)]
pub struct TimelineEntry {
    pub date: NaiveDate,
    /// Absolute path — ready to hand to `read`.
    pub abs_path: PathBuf,
    /// Path relative to the repo root (e.g. `docs/decisions/2026-06-14-foo.md`).
    pub rel_path: PathBuf,
    pub repo_path: PathBuf,
    pub repo_tags: Vec<String>,
    /// Top-level dir under `docs/` (e.g. `decisions`), or `None` if the doc
    /// sits directly in `docs/`.
    pub bucket: Option<String>,
    /// Filename slug with the date prefix and `.md` stripped.
    pub slug: String,
    pub state: DocState,
    /// First markdown heading in the body, if any.
    pub title: Option<String>,
}

impl TimelineEntry {
    /// The repo's display name (its directory basename, e.g. `keel`).
    pub fn repo_name(&self) -> &str {
        self.repo_path
            .file_name()
            .and_then(|s| s.to_str())
            .or_else(|| self.repo_path.to_str())
            .unwrap_or("?")
    }
}

/// Per-day state histogram.
#[derive(Debug, Clone)]
pub struct DayBucket {
    pub date: NaiveDate,
    pub stamped: usize,
    pub signed: usize,
    pub raw: usize,
}

impl DayBucket {
    pub fn total(&self) -> usize {
        self.stamped + self.signed + self.raw
    }
}

/// The assembled timeline. `entries` sorted date-descending; `by_day` likewise.
#[derive(Debug, Clone)]
pub struct Timeline {
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub entries: Vec<TimelineEntry>,
    pub by_day: Vec<DayBucket>,
}

/// Optional filters applied while walking.
#[derive(Debug, Default, Clone)]
pub struct TimelineFilter {
    /// Only repos carrying this tag.
    pub tag: Option<String>,
    /// Only docs in this state.
    pub state: Option<DocState>,
    /// Only docs in this bucket (top-level dir under `docs/`).
    pub bucket: Option<String>,
}

/// Build the timeline over the registered repos.
pub fn build_timeline(
    prefs_path: &Path,
    today: NaiveDate,
    range_spec: &str,
    filter: &TimelineFilter,
) -> Result<Timeline, TimelineError> {
    let (from, to) = resolve_range(range_spec, today)?;
    let repos = list_repos(prefs_path, filter.tag.as_deref())?;

    let mut entries = Vec::new();
    for repo in repos {
        let docs_dir = repo.path.join("docs");
        if !docs_dir.is_dir() {
            continue;
        }
        let mut md = Vec::new();
        collect_md(&docs_dir, &mut md);
        for abs in md {
            let Some(filename) = abs.file_name().and_then(|f| f.to_str()) else {
                continue;
            };
            let Some((date, slug)) = parse_doc_filename(filename) else {
                continue; // undated doc (architecture notes, README, …) — skip
            };
            if date < from || date > to {
                continue;
            }
            let bucket = docs_relative_bucket(&abs, &docs_dir);
            if let Some(want) = &filter.bucket {
                if bucket.as_deref() != Some(want.as_str()) {
                    continue;
                }
            }
            let Ok(content) = std::fs::read_to_string(&abs) else {
                continue;
            };
            let (state, body) = detect_state_and_body(&content);
            if let Some(want) = filter.state {
                if state != want {
                    continue;
                }
            }
            let rel_path = abs
                .strip_prefix(&repo.path)
                .map(Path::to_path_buf)
                .unwrap_or_else(|_| abs.clone());
            entries.push(TimelineEntry {
                date,
                abs_path: abs.clone(),
                rel_path,
                repo_path: repo.path.clone(),
                repo_tags: repo.tags.clone(),
                bucket,
                slug,
                state,
                title: first_heading(&body),
            });
        }
    }

    // Date descending; within a day, by repo then slug for stable output.
    entries.sort_by(|a, b| {
        b.date
            .cmp(&a.date)
            .then_with(|| a.repo_path.cmp(&b.repo_path))
            .then_with(|| a.slug.cmp(&b.slug))
    });

    let by_day = aggregate_by_day(&entries);
    Ok(Timeline {
        from,
        to,
        entries,
        by_day,
    })
}

/// Resolve a range spec against `today` (inclusive bounds). Accepts:
/// `today`, `Nd` (last N days incl. today), `YYYY-MM` (whole month),
/// `YYYY-MM-DD`, or `YYYY-MM-DD..YYYY-MM-DD`.
pub fn resolve_range(spec: &str, today: NaiveDate) -> Result<(NaiveDate, NaiveDate), TimelineError> {
    let spec = spec.trim();
    let bad = |reason: &str| TimelineError::BadRange {
        spec: spec.to_string(),
        reason: reason.to_string(),
    };

    if let Some((a, b)) = spec.split_once("..") {
        let from = NaiveDate::parse_from_str(a.trim(), "%Y-%m-%d").map_err(|e| bad(&e.to_string()))?;
        let to = NaiveDate::parse_from_str(b.trim(), "%Y-%m-%d").map_err(|e| bad(&e.to_string()))?;
        return Ok(order(from, to));
    }

    if spec == "today" {
        return Ok((today, today));
    }

    if let Some(n) = spec.strip_suffix('d') {
        if let Ok(days) = n.parse::<i64>() {
            if days < 1 {
                return Err(bad("day count must be >= 1"));
            }
            return Ok((today - Duration::days(days - 1), today));
        }
    }

    // `YYYY-MM` — whole calendar month.
    if spec.len() == 7 {
        if let Ok(first) = NaiveDate::parse_from_str(&format!("{spec}-01"), "%Y-%m-%d") {
            return Ok((first, last_day_of_month(first)));
        }
    }

    if let Ok(d) = NaiveDate::parse_from_str(spec, "%Y-%m-%d") {
        return Ok((d, d));
    }

    Err(bad(
        "expected: today | Nd | YYYY-MM | YYYY-MM-DD | YYYY-MM-DD..YYYY-MM-DD",
    ))
}

/// Extract a leading `YYYY-MM-DD` date and the trailing slug from a doc
/// filename. `2026-06-14-foo-bar.md` → `(2026-06-14, "foo-bar")`. Returns
/// `None` when there's no dated prefix or no slug after it.
pub fn parse_doc_filename(filename: &str) -> Option<(NaiveDate, String)> {
    let stem = filename.strip_suffix(".md")?;
    if stem.len() < 12 {
        return None; // need "YYYY-MM-DD-" + ≥1 slug char
    }
    let (date_part, rest) = stem.split_at(10);
    let date = NaiveDate::parse_from_str(date_part, "%Y-%m-%d").ok()?;
    let slug = rest.strip_prefix('-')?;
    if slug.is_empty() {
        return None;
    }
    Some((date, slug.to_string()))
}

/// Tolerant frontmatter peek. Derives stamp state from the *presence* of the
/// `$attestation` / `$signature` keys via a generic YAML parse — no typed
/// deserialization, so schema drift or a malformed block never silently hides
/// a doc from the listing. Returns the state and the body (for title
/// extraction). Mirrors the delimiter handling in `infrastructure::markdown`.
fn detect_state_and_body(content: &str) -> (DocState, String) {
    let stripped = content.strip_prefix('\u{FEFF}').unwrap_or(content);
    let Some(after_open) = stripped
        .strip_prefix("---\n")
        .or_else(|| stripped.strip_prefix("---\r\n"))
    else {
        return (DocState::Raw, stripped.to_string());
    };
    let Some((yaml, body)) = split_closing_delim(after_open) else {
        return (DocState::Raw, stripped.to_string());
    };
    let state = match serde_yaml::from_str::<serde_yaml::Value>(yaml) {
        Ok(serde_yaml::Value::Mapping(map)) => {
            let has = |k: &str| map.contains_key(serde_yaml::Value::String(k.to_string()));
            if has("$attestation") {
                DocState::Stamped
            } else if has("$signature") {
                DocState::Signed
            } else {
                DocState::Raw
            }
        }
        _ => DocState::Raw,
    };
    (state, body.to_string())
}

/// Split a post-opening-delimiter string into `(yaml, body)` at the closing
/// `---` line. Returns `None` if no closing delimiter is found.
fn split_closing_delim(s: &str) -> Option<(&str, &str)> {
    let mut start = 0;
    while let Some(rel) = s[start..].find("\n---") {
        let abs = start + rel;
        let tail = &s[abs + 4..];
        if let Some(rest) = tail.strip_prefix('\n') {
            return Some((&s[..abs], rest));
        }
        if let Some(rest) = tail.strip_prefix("\r\n") {
            return Some((&s[..abs], rest));
        }
        if tail.is_empty() {
            return Some((&s[..abs], ""));
        }
        start = abs + 1;
    }
    None
}

fn first_heading(body: &str) -> Option<String> {
    body.lines()
        .map(str::trim)
        .find(|l| l.starts_with('#'))
        .map(|l| l.trim_start_matches('#').trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Top-level dir under `docs/`, or `None` if the file sits directly in `docs/`.
fn docs_relative_bucket(abs: &Path, docs_dir: &Path) -> Option<String> {
    let rel = abs.strip_prefix(docs_dir).ok()?;
    let mut comps = rel.components();
    let first = comps.next()?;
    // Another component after the first means `first` is a directory (bucket).
    if comps.next().is_some() {
        Some(first.as_os_str().to_string_lossy().into_owned())
    } else {
        None
    }
}

/// Recursively collect `*.md` files under `dir`.
fn collect_md(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_md(&path, out);
        } else if path.extension().is_some_and(|e| e == "md") {
            out.push(path);
        }
    }
}

fn aggregate_by_day(entries: &[TimelineEntry]) -> Vec<DayBucket> {
    let mut map: BTreeMap<NaiveDate, DayBucket> = BTreeMap::new();
    for e in entries {
        let b = map.entry(e.date).or_insert(DayBucket {
            date: e.date,
            stamped: 0,
            signed: 0,
            raw: 0,
        });
        match e.state {
            DocState::Stamped => b.stamped += 1,
            DocState::Signed => b.signed += 1,
            DocState::Raw => b.raw += 1,
        }
    }
    // BTreeMap iterates ascending; reverse to date-descending.
    map.into_values().rev().collect()
}

fn order(a: NaiveDate, b: NaiveDate) -> (NaiveDate, NaiveDate) {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}

fn last_day_of_month(d: NaiveDate) -> NaiveDate {
    let (y, m) = (d.year(), d.month());
    let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
    NaiveDate::from_ymd_opt(ny, nm, 1)
        .expect("first of next month is valid")
        .pred_opt()
        .expect("day before first-of-month is valid")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::repo_ops::register_repo;
    use crate::infrastructure::RepoRole;
    use tempfile::TempDir;

    fn date(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    // -- range parsing --------------------------------------------------------

    #[test]
    fn range_today() {
        let t = date("2026-06-14");
        assert_eq!(resolve_range("today", t).unwrap(), (t, t));
    }

    #[test]
    fn range_n_days_includes_today() {
        let t = date("2026-06-14");
        assert_eq!(
            resolve_range("7d", t).unwrap(),
            (date("2026-06-08"), date("2026-06-14"))
        );
        assert_eq!(resolve_range("1d", t).unwrap(), (t, t));
    }

    #[test]
    fn range_whole_month() {
        let t = date("2026-06-14");
        assert_eq!(
            resolve_range("2026-06", t).unwrap(),
            (date("2026-06-01"), date("2026-06-30"))
        );
        // February leap-year boundary.
        assert_eq!(
            resolve_range("2024-02", t).unwrap(),
            (date("2024-02-01"), date("2024-02-29"))
        );
    }

    #[test]
    fn range_explicit_span_and_single_day() {
        let t = date("2026-06-14");
        assert_eq!(
            resolve_range("2026-06-01..2026-06-10", t).unwrap(),
            (date("2026-06-01"), date("2026-06-10"))
        );
        // Reversed span is normalized.
        assert_eq!(
            resolve_range("2026-06-10..2026-06-01", t).unwrap(),
            (date("2026-06-01"), date("2026-06-10"))
        );
        assert_eq!(
            resolve_range("2026-06-09", t).unwrap(),
            (date("2026-06-09"), date("2026-06-09"))
        );
    }

    #[test]
    fn range_rejects_garbage() {
        let t = date("2026-06-14");
        assert!(resolve_range("last-week", t).is_err());
        assert!(resolve_range("0d", t).is_err());
    }

    // -- filename parsing -----------------------------------------------------

    #[test]
    fn filename_parsing() {
        assert_eq!(
            parse_doc_filename("2026-06-14-runway-call.md"),
            Some((date("2026-06-14"), "runway-call".to_string()))
        );
        assert_eq!(parse_doc_filename("secretariat-architecture.md"), None);
        assert_eq!(parse_doc_filename("README.md"), None);
        assert_eq!(parse_doc_filename("2026-06-14.md"), None); // no slug
    }

    // -- end-to-end over a fixture substrate ----------------------------------

    /// Write a doc with the given frontmatter blocks present.
    fn write_doc(repo: &Path, rel: &str, signed: bool, stamped: bool, body: &str) {
        let path = repo.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut fm = String::new();
        if signed || stamped {
            fm.push_str("---\n");
            if signed {
                fm.push_str("$signature:\n  $type: tech.equanimi.secretariat.signature\n  signer: did:web:rafa.equanimi.tech\n");
            }
            if stamped {
                fm.push_str("$attestation:\n  $type: tech.equanimi.secretariat.stamp\n  signer: did:web:rafa.equanimi.tech\n");
            }
            fm.push_str("---\n");
        }
        std::fs::write(path, format!("{fm}{body}")).unwrap();
    }

    fn fixture() -> (TempDir, PathBuf) {
        let d = TempDir::new().unwrap();
        let repo = d.path().join("equanimitech/secretariat");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        let prefs = d.path().join("preferences.toml");
        register_repo(&prefs, &repo, RepoRole::Project, vec!["equanimitech".into()]).unwrap();

        write_doc(
            &repo,
            "docs/decisions/2026-06-14-runway-call.md",
            true,
            true,
            "# Runway call\nbody",
        );
        write_doc(
            &repo,
            "docs/ideas/2026-06-14-chronological-zoom.md",
            true,
            false,
            "# Chronological zoom\nbody",
        );
        write_doc(
            &repo,
            "docs/2026-06-13-loose-note.md",
            false,
            false,
            "just raw text",
        );
        // Undated doc — must be ignored.
        write_doc(&repo, "docs/developer/architecture.md", true, false, "# Arch");
        // Out-of-range doc.
        write_doc(
            &repo,
            "docs/ideas/2026-05-01-old-thought.md",
            false,
            false,
            "# Old",
        );
        (d, prefs)
    }

    #[test]
    fn builds_timeline_with_states_and_buckets() {
        let (_d, prefs) = fixture();
        let tl = build_timeline(
            &prefs,
            date("2026-06-14"),
            "7d",
            &TimelineFilter::default(),
        )
        .unwrap();

        assert_eq!(tl.from, date("2026-06-08"));
        assert_eq!(tl.to, date("2026-06-14"));
        // 3 in-range, dated docs (old-thought out of range, architecture undated).
        assert_eq!(tl.entries.len(), 3);

        let runway = tl
            .entries
            .iter()
            .find(|e| e.slug == "runway-call")
            .unwrap();
        assert_eq!(runway.state, DocState::Stamped);
        assert_eq!(runway.bucket.as_deref(), Some("decisions"));
        assert_eq!(runway.title.as_deref(), Some("Runway call"));

        let idea = tl
            .entries
            .iter()
            .find(|e| e.slug == "chronological-zoom")
            .unwrap();
        assert_eq!(idea.state, DocState::Signed);

        let note = tl.entries.iter().find(|e| e.slug == "loose-note").unwrap();
        assert_eq!(note.state, DocState::Raw);
        assert_eq!(note.bucket, None); // directly in docs/

        // by_day: 14th has 2 docs (1 stamped, 1 signed), 13th has 1 raw.
        let d14 = tl.by_day.iter().find(|d| d.date == date("2026-06-14")).unwrap();
        assert_eq!((d14.stamped, d14.signed, d14.raw), (1, 1, 0));
        assert_eq!(tl.by_day[0].date, date("2026-06-14")); // descending
    }

    #[test]
    fn state_filter_narrows_results() {
        let (_d, prefs) = fixture();
        let tl = build_timeline(
            &prefs,
            date("2026-06-14"),
            "30d",
            &TimelineFilter {
                state: Some(DocState::Stamped),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(tl.entries.len(), 1);
        assert_eq!(tl.entries[0].slug, "runway-call");
    }

    #[test]
    fn bucket_filter_narrows_results() {
        let (_d, prefs) = fixture();
        let tl = build_timeline(
            &prefs,
            date("2026-06-14"),
            "30d",
            &TimelineFilter {
                bucket: Some("ideas".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(tl.entries.len(), 1);
        assert_eq!(tl.entries[0].slug, "chronological-zoom");
    }
}
