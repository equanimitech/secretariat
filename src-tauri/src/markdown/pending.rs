//! Buffer for file paths received via `RunEvent::Opened` before the frontend
//! is ready to consume them.
//!
//! macOS delivers `Opened` before the webview's `markdown://pending-opens-added`
//! listener has been registered, so we stash paths here and drain them once
//! the frontend calls `take_pending_opens`.

use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Default)]
pub struct PendingOpens(Mutex<Vec<PathBuf>>);

impl PendingOpens {
    pub fn push(&self, path: PathBuf) {
        let mut g = self.0.lock().unwrap();
        if !g.contains(&path) {
            g.push(path);
        }
    }

    pub fn drain(&self) -> Vec<PathBuf> {
        let mut g = self.0.lock().unwrap();
        std::mem::take(&mut *g)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn drain_returns_and_clears() {
        let p = PendingOpens::default();
        p.push(PathBuf::from("/a.md"));
        p.push(PathBuf::from("/b.md"));
        let drained = p.drain();
        assert_eq!(drained.len(), 2);
        assert!(p.drain().is_empty());
    }

    #[test]
    fn deduplicates_on_push() {
        let p = PendingOpens::default();
        p.push(PathBuf::from("/a.md"));
        p.push(PathBuf::from("/a.md"));
        assert_eq!(p.drain().len(), 1);
    }
}
