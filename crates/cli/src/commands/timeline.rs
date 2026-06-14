//! `sec timeline` — chronological view of docs across registered repos.
//!
//! - `sec timeline [--range 7d] [--zoom day|week|month] [--tag t] \
//!    [--state stamped|signed|raw] [--bucket b] [--json]`
//!
//! "What did I create today / over the last days / last month." Dates come
//! from the `<date>-<slug>.md` filename; state badges (▣ stamped, ✎ signed,
//! · raw) are derived from frontmatter. Read-only; never decrypts.

use anyhow::{bail, Context, Result};
use chrono::{Datelike, NaiveDate, Utc};
use clap::Parser;

use secretariat_core::application::timeline_ops::{
    build_timeline, DayBucket, DocState, Timeline, TimelineEntry, TimelineFilter,
};

use super::paths::key_paths;

#[derive(Parser, Debug)]
pub struct Args {
    /// Date window: today | Nd (e.g. 7d, 30d) | YYYY-MM | YYYY-MM-DD | A..B.
    #[arg(long, default_value = "7d")]
    range: String,
    /// Grouping granularity: day | week | month.
    #[arg(long, default_value = "day")]
    zoom: String,
    /// Only repos carrying this tag (e.g. equanimitech, themia).
    #[arg(long)]
    tag: Option<String>,
    /// Only this doc state: stamped | signed | raw.
    #[arg(long)]
    state: Option<String>,
    /// Only this bucket (top-level dir under docs/, e.g. decisions).
    #[arg(long)]
    bucket: Option<String>,
    /// Emit JSON instead of the rendered view.
    #[arg(long)]
    json: bool,
}

const STAMPED: &str = "▣";
const SIGNED: &str = "✎";
const RAW: &str = "·";

pub fn run(args: Args) -> Result<()> {
    let paths = key_paths()?;
    let state = match args.state.as_deref() {
        None => None,
        Some(s) => Some(
            DocState::parse(s).ok_or_else(|| anyhow::anyhow!("invalid --state `{s}` (expected stamped|signed|raw)"))?,
        ),
    };
    let zoom = args.zoom.to_lowercase();
    if !matches!(zoom.as_str(), "day" | "week" | "month") {
        bail!("invalid --zoom `{}` (expected day|week|month)", args.zoom);
    }
    let filter = TimelineFilter {
        tag: args.tag.clone(),
        state,
        bucket: args.bucket.clone(),
    };
    let today = Utc::now().date_naive();
    let tl = build_timeline(&paths.preferences, today, &args.range, &filter)
        .context("building timeline")?;

    if args.json {
        print_json(&tl);
        return Ok(());
    }

    println!(
        "{} → {}  ·  {} doc{}",
        tl.from,
        tl.to,
        tl.entries.len(),
        if tl.entries.len() == 1 { "" } else { "s" }
    );
    if tl.entries.is_empty() {
        println!("  (nothing in range)");
        return Ok(());
    }
    println!();
    match zoom.as_str() {
        "month" => render_month(&tl),
        "week" => render_week(&tl),
        _ => render_day(&tl),
    }
    Ok(())
}

/// `day` zoom — each day a section; within it, docs cluster under a repo
/// (brand-glyph) header. Each doc shows its state badge, bucket/title, and the
/// full absolute path on its own line (clickable in the terminal to open it).
fn render_day(tl: &Timeline) {
    let mut cur_date: Option<NaiveDate> = None;
    let mut cur_repo: Option<String> = None;
    for e in &tl.entries {
        if cur_date != Some(e.date) {
            if cur_date.is_some() {
                println!();
            }
            let n = tl.entries.iter().filter(|x| x.date == e.date).count();
            println!(
                "{} ({})  {} doc{}",
                e.date,
                weekday(e.date),
                n,
                if n == 1 { "" } else { "s" }
            );
            cur_date = Some(e.date);
            cur_repo = None;
        }
        let repo = e.repo_name();
        if cur_repo.as_deref() != Some(repo) {
            let n = tl
                .entries
                .iter()
                .filter(|x| x.date == e.date && x.repo_name() == repo)
                .count();
            println!("  {} {}  ({})", repo_glyph(repo), repo, n);
            cur_repo = Some(repo.to_string());
        }
        let bucket = e
            .bucket
            .as_deref()
            .map(|b| format!("{b}/"))
            .unwrap_or_default();
        let label = e.title.as_deref().unwrap_or(&e.slug);
        println!("    {} {}{}", badge(e.state), bucket, label);
        println!("       {}", e.abs_path.display());
    }
}

/// `week` zoom — one line per day, badge histogram, grouped by ISO week.
fn render_week(tl: &Timeline) {
    let mut current_week: Option<(i32, u32)> = None;
    for d in &tl.by_day {
        let iso = d.date.iso_week();
        let key = (iso.year(), iso.week());
        if current_week != Some(key) {
            if current_week.is_some() {
                println!();
            }
            println!("Week {} · {}", iso.week(), monday_of(d.date));
            current_week = Some(key);
        }
        println!(
            "  {} {:>2}  {:<12} ({})",
            weekday(d.date),
            d.date.day(),
            histogram(d),
            d.total()
        );
    }
}

/// `month` zoom — per-day counts only (compact).
fn render_month(tl: &Timeline) {
    let mut current_month: Option<(i32, u32)> = None;
    for d in &tl.by_day {
        let key = (d.date.year(), d.date.month());
        if current_month != Some(key) {
            if current_month.is_some() {
                println!();
            }
            println!("{:04}-{:02}", d.date.year(), d.date.month());
            current_month = Some(key);
        }
        println!(
            "  {:>2} {}  {:<12} ({})",
            d.date.day(),
            weekday(d.date),
            histogram(d),
            d.total()
        );
    }
}

fn badge(s: DocState) -> &'static str {
    match s {
        DocState::Stamped => STAMPED,
        DocState::Signed => SIGNED,
        DocState::Raw => RAW,
    }
}

/// Brand glyph per equanimitech repo; a neutral marker for everything else.
fn repo_glyph(name: &str) -> &'static str {
    match name {
        "keel" => "∫",
        "secretariat" => "∎",
        "zenborg" => "≋",
        "respost" => "↦",
        "site" => "≃",
        _ => "◦",
    }
}

/// A glyph run like `▣▣✎·` for a day's state counts.
fn histogram(d: &DayBucket) -> String {
    let mut s = String::new();
    s.push_str(&STAMPED.repeat(d.stamped));
    s.push_str(&SIGNED.repeat(d.signed));
    s.push_str(&RAW.repeat(d.raw));
    s
}

fn weekday(d: NaiveDate) -> &'static str {
    [
        "Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun",
    ][d.weekday().num_days_from_monday() as usize]
}

fn monday_of(d: NaiveDate) -> NaiveDate {
    d - chrono::Duration::days(d.weekday().num_days_from_monday() as i64)
}

fn print_json(tl: &Timeline) {
    let by_day: Vec<_> = tl
        .by_day
        .iter()
        .map(|d| {
            serde_json::json!({
                "date": d.date.to_string(),
                "stamped": d.stamped,
                "signed": d.signed,
                "raw": d.raw,
                "total": d.total(),
            })
        })
        .collect();
    let entries: Vec<_> = tl.entries.iter().map(entry_json).collect();
    let out = serde_json::json!({
        "from": tl.from.to_string(),
        "to": tl.to.to_string(),
        "total": tl.entries.len(),
        "by_day": by_day,
        "entries": entries,
    });
    println!("{}", serde_json::to_string_pretty(&out).unwrap());
}

fn entry_json(e: &TimelineEntry) -> serde_json::Value {
    serde_json::json!({
        "date": e.date.to_string(),
        "state": e.state.as_str(),
        "repo": e.repo_name(),
        "bucket": e.bucket,
        "slug": e.slug,
        "title": e.title,
        "repo_tags": e.repo_tags,
        "rel_path": e.rel_path.display().to_string(),
        "abs_path": e.abs_path.display().to_string(),
    })
}
