//! Bookmarks (`bookmarks.json`).

use std::io;

use serde::{Deserialize, Serialize};

use crate::input::{is_web_url, without_fragment};
use crate::profile::Profile;
use crate::storage;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bookmark {
    pub url: String,
    pub title: String,
    /// Unix seconds.
    pub added: u64,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Bookmarks {
    items: Vec<Bookmark>,
}

impl Bookmarks {
    pub fn load(profile: &Profile) -> Self {
        storage::load_or_default(&profile.bookmarks_path())
    }

    pub fn save(&self, profile: &Profile) -> io::Result<()> {
        storage::save(&profile.bookmarks_path(), self)
    }

    /// Bookmarks in the order they were added.
    pub fn items(&self) -> &[Bookmark] {
        &self.items
    }

    pub fn contains(&self, url: &str) -> bool {
        let key = without_fragment(url);
        is_web_url(url) && self.items.iter().any(|b| without_fragment(&b.url) == key)
    }

    /// Adds a bookmark. Returns false for pages that cannot be bookmarked (blank pages…)
    /// and for pages that already are.
    pub fn add(&mut self, url: &str, title: &str, now: u64) -> bool {
        if !is_web_url(url) || self.contains(url) {
            return false;
        }
        let title = title.trim();
        let title = if title.is_empty() { url } else { title };
        self.items.push(Bookmark {
            url: url.to_string(),
            title: title.to_string(),
            added: now,
        });
        true
    }

    pub fn remove(&mut self, url: &str) -> bool {
        let key = without_fragment(url);
        let before = self.items.len();
        self.items.retain(|b| without_fragment(&b.url) != key);
        self.items.len() != before
    }

    /// Bookmarks matching every whitespace-separated term of `query` in the URL or
    /// the title (case-insensitive), in the order they were added.
    pub fn search(&self, query: &str, limit: usize) -> Vec<&Bookmark> {
        let query = query.trim().to_lowercase();
        if query.is_empty() {
            return Vec::new();
        }
        let terms: Vec<&str> = query.split_whitespace().collect();
        self.items
            .iter()
            .filter(|b| {
                let url = b.url.to_lowercase();
                let title = b.title.to_lowercase();
                terms.iter().all(|t| url.contains(t) || title.contains(t))
            })
            .take(limit)
            .collect()
    }

    /// Adds the page if it is not bookmarked, removes it otherwise.
    /// Returns whether the page is bookmarked afterwards.
    pub fn toggle(&mut self, url: &str, title: &str, now: u64) -> bool {
        if self.contains(url) {
            self.remove(url);
            false
        } else {
            self.add(url, title, now)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::test_util::scratch_dir;

    #[test]
    fn add_contains_remove() {
        let mut bookmarks = Bookmarks::default();
        assert!(bookmarks.add("https://example.com/a", "A", 1));
        assert!(bookmarks.contains("https://example.com/a"));
        assert!(!bookmarks.add("https://example.com/a", "again", 2), "no duplicates");
        assert_eq!(bookmarks.items().len(), 1);
        assert!(bookmarks.remove("https://example.com/a"));
        assert!(!bookmarks.remove("https://example.com/a"));
        assert!(bookmarks.items().is_empty());
    }

    #[test]
    fn fragments_do_not_make_a_different_page() {
        let mut bookmarks = Bookmarks::default();
        bookmarks.add("https://example.com/doc", "Doc", 1);
        assert!(bookmarks.contains("https://example.com/doc#section-2"));
        assert!(bookmarks.remove("https://example.com/doc#top"));
    }

    #[test]
    fn blank_and_internal_pages_are_not_bookmarkable() {
        let mut bookmarks = Bookmarks::default();
        assert!(!bookmarks.add("about:blank", "", 1));
        assert!(!bookmarks.add("data:text/html,hi", "", 1));
        assert!(!bookmarks.add("", "", 1));
        assert!(!bookmarks.toggle("about:blank", "", 1));
        assert!(!bookmarks.contains("about:blank"));
    }

    #[test]
    fn empty_title_falls_back_to_url() {
        let mut bookmarks = Bookmarks::default();
        bookmarks.add("https://example.com", "  ", 1);
        assert_eq!(bookmarks.items()[0].title, "https://example.com");
    }

    #[test]
    fn search_matches_terms_in_title_or_url() {
        let mut bookmarks = Bookmarks::default();
        bookmarks.add("https://docs.rs/serde", "Serde docs", 1);
        bookmarks.add("https://rust-lang.org", "Rust", 2);
        let found: Vec<&str> = bookmarks.search("RUST", 10).iter().map(|b| b.url.as_str()).collect();
        assert_eq!(found, ["https://rust-lang.org"]);
        let found: Vec<&str> = bookmarks.search("serde docs", 10).iter().map(|b| b.url.as_str()).collect();
        assert_eq!(found, ["https://docs.rs/serde"]);
        assert!(bookmarks.search("  ", 10).is_empty());
        assert_eq!(bookmarks.search("r", 1).len(), 1, "limit applies");
    }

    #[test]
    fn toggle_flips() {
        let mut bookmarks = Bookmarks::default();
        assert!(bookmarks.toggle("https://example.com", "E", 1));
        assert!(bookmarks.contains("https://example.com"));
        assert!(!bookmarks.toggle("https://example.com", "E", 2));
        assert!(!bookmarks.contains("https://example.com"));
    }

    #[test]
    fn round_trip() {
        let profile = Profile::from_dir(scratch_dir("bookmarks-roundtrip"));
        let mut bookmarks = Bookmarks::default();
        bookmarks.add("https://example.com", "Example", 5);
        bookmarks.add("https://rust-lang.org", "Rust", 6);
        bookmarks.save(&profile).expect("save");
        assert_eq!(Bookmarks::load(&profile), bookmarks);
    }
}
