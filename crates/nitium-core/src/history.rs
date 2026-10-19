//! Browsing history (`history.json`) and address bar suggestions.

use std::io;

use serde::{Deserialize, Serialize};

use crate::input::{is_web_url, without_fragment};
use crate::profile::Profile;
use crate::storage;

/// Oldest entries are dropped beyond this many pages.
const MAX_ENTRIES: usize = 5000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub url: String,
    pub title: String,
    pub visits: u32,
    /// Unix seconds.
    pub last_visit: u64,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct History {
    entries: Vec<HistoryEntry>,
    /// The page recorded last, so redirects and in-page navigation (`#fragment`,
    /// repeated URL notifications) do not inflate the visit count.
    #[serde(skip)]
    last_recorded: Option<String>,
}

impl History {
    pub fn load(profile: &Profile) -> Self {
        storage::load_or_default(&profile.history_path())
    }

    pub fn save(&self, profile: &Profile) -> io::Result<()> {
        storage::save(&profile.history_path(), self)
    }

    pub fn entries(&self) -> &[HistoryEntry] {
        &self.entries
    }

    /// Records that the user is now on `url`. Non-page URLs are ignored.
    pub fn record_visit(&mut self, url: &str, now: u64) {
        if !is_web_url(url) {
            return;
        }
        let key = without_fragment(url);
        if self.last_recorded.as_deref() == Some(key) {
            return;
        }
        self.last_recorded = Some(key.to_string());

        match self.entries.iter_mut().find(|e| without_fragment(&e.url) == key) {
            Some(entry) => {
                entry.visits = entry.visits.saturating_add(1);
                entry.last_visit = now;
            }
            None => {
                self.entries.push(HistoryEntry {
                    url: key.to_string(),
                    title: String::new(),
                    visits: 1,
                    last_visit: now,
                });
                if self.entries.len() > MAX_ENTRIES {
                    self.entries.sort_by(|a, b| b.last_visit.cmp(&a.last_visit));
                    self.entries.truncate(MAX_ENTRIES);
                }
            }
        }
    }

    /// Remembers the title of a page the history already knows.
    pub fn set_title(&mut self, url: &str, title: &str) {
        let title = title.trim();
        if title.is_empty() {
            return;
        }
        let key = without_fragment(url);
        if let Some(entry) = self.entries.iter_mut().find(|e| without_fragment(&e.url) == key) {
            entry.title = title.to_string();
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.last_recorded = None;
    }

    /// Pages matching every whitespace-separated term of `query` (in the URL or the
    /// title, case-insensitive), best first: sites whose address starts with the
    /// query, then most visited, then most recent.
    pub fn suggest(&self, query: &str, limit: usize) -> Vec<&HistoryEntry> {
        let query = query.trim().to_lowercase();
        if query.is_empty() || limit == 0 {
            return Vec::new();
        }
        let terms: Vec<&str> = query.split_whitespace().collect();

        let mut matches: Vec<(&HistoryEntry, bool)> = self
            .entries
            .iter()
            .filter_map(|entry| {
                let url = entry.url.to_lowercase();
                let title = entry.title.to_lowercase();
                let all_terms = terms.iter().all(|t| url.contains(t) || title.contains(t));
                all_terms.then(|| (entry, address_starts_with(&url, &query)))
            })
            .collect();
        matches.sort_by(|(a, a_prefix), (b, b_prefix)| {
            b_prefix
                .cmp(a_prefix)
                .then(b.visits.cmp(&a.visits))
                .then(b.last_visit.cmp(&a.last_visit))
        });
        matches.into_iter().take(limit).map(|(entry, _)| entry).collect()
    }
}

/// `example.com/x` starts with `exa` once the scheme and a leading `www.` are ignored.
fn address_starts_with(lower_url: &str, lower_query: &str) -> bool {
    let rest = lower_url
        .split_once("://")
        .map_or(lower_url, |(_, rest)| rest);
    rest.strip_prefix("www.").unwrap_or(rest).starts_with(lower_query)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::test_util::scratch_dir;

    fn urls<'a>(entries: &[&'a HistoryEntry]) -> Vec<&'a str> {
        entries.iter().map(|e| e.url.as_str()).collect()
    }

    #[test]
    fn records_and_counts_visits() {
        let mut history = History::default();
        history.record_visit("https://a.test/", 1);
        history.record_visit("https://b.test/", 2);
        history.record_visit("https://a.test/", 3);
        assert_eq!(history.entries().len(), 2);
        let a = &history.entries()[0];
        assert_eq!((a.visits, a.last_visit), (2, 3));
    }

    #[test]
    fn repeated_notifications_and_fragments_count_once() {
        let mut history = History::default();
        history.record_visit("https://a.test/doc", 1);
        history.record_visit("https://a.test/doc", 1);
        history.record_visit("https://a.test/doc#intro", 2);
        assert_eq!(history.entries()[0].visits, 1);
        assert_eq!(history.entries()[0].url, "https://a.test/doc");
    }

    #[test]
    fn ignores_non_pages() {
        let mut history = History::default();
        history.record_visit("about:blank", 1);
        history.record_visit("data:text/html,x", 1);
        assert!(history.entries().is_empty());
    }

    #[test]
    fn titles_attach_to_known_pages_only() {
        let mut history = History::default();
        history.set_title("https://nowhere.test/", "Ghost");
        assert!(history.entries().is_empty());
        history.record_visit("https://a.test/", 1);
        history.set_title("https://a.test/#frag", "  A page ");
        history.set_title("https://a.test/", "   ");
        assert_eq!(history.entries()[0].title, "A page");
    }

    #[test]
    fn suggestions_match_url_and_title_terms() {
        let mut history = History::default();
        history.record_visit("https://docs.rs/serde", 1);
        history.set_title("https://docs.rs/serde", "serde - Rust");
        history.record_visit("https://example.com/", 2);
        assert_eq!(urls(&history.suggest("rust serde", 5)), ["https://docs.rs/serde"]);
        assert!(history.suggest("nothing", 5).is_empty());
        assert!(history.suggest("   ", 5).is_empty());
        assert!(history.suggest("docs", 0).is_empty());
    }

    #[test]
    fn suggestions_rank_prefix_then_visits_then_recency() {
        let mut history = History::default();
        // Contains "ex" only in the path; most visited.
        for t in 1..=5 {
            history.record_visit("https://other.test/ex", t);
            history.record_visit("https://zzz.test/", t);
        }
        history.record_visit("https://www.example.com/", 10);
        history.record_visit("https://zzz.test/", 11);
        history.record_visit("https://example.org/", 12);
        history.record_visit("https://zzz.test/", 13);
        let found = history.suggest("ex", 10);
        // Address-prefix matches first (more recent one first on equal visits), then the rest.
        assert_eq!(
            urls(&found),
            ["https://example.org/", "https://www.example.com/", "https://other.test/ex"]
        );
    }

    #[test]
    fn suggestion_limit_applies() {
        let mut history = History::default();
        for i in 0..10 {
            history.record_visit(&format!("https://site{i}.test/"), i);
        }
        assert_eq!(history.suggest("site", 3).len(), 3);
    }

    #[test]
    fn history_is_capped() {
        let mut history = History::default();
        for i in 0..(MAX_ENTRIES as u64 + 10) {
            history.record_visit(&format!("https://site{i}.test/"), i);
        }
        assert_eq!(history.entries().len(), MAX_ENTRIES);
        // The oldest were dropped.
        assert!(history.entries().iter().all(|e| e.last_visit >= 10));
    }

    #[test]
    fn clear_forgets_everything() {
        let mut history = History::default();
        history.record_visit("https://a.test/", 1);
        history.clear();
        assert!(history.entries().is_empty());
        history.record_visit("https://a.test/", 2);
        assert_eq!(history.entries().len(), 1);
    }

    #[test]
    fn round_trip() {
        let profile = Profile::from_dir(scratch_dir("history-roundtrip"));
        let mut history = History::default();
        history.record_visit("https://a.test/", 1);
        history.set_title("https://a.test/", "A");
        history.save(&profile).expect("save");
        assert_eq!(History::load(&profile).entries(), history.entries());
    }
}
