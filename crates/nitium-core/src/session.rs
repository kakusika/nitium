//! The set of open tabs, saved so the next launch can pick up where this one left off.

use std::io;

use serde::{Deserialize, Serialize};

use crate::input::is_web_url;
use crate::profile::Profile;
use crate::storage;

/// A single saved tab with its URL and document title.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionTab {
    pub url: String,
    #[serde(default)]
    pub title: String,
}

impl SessionTab {
    pub fn new(url: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            title: title.into(),
        }
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawSessionTab {
    Detailed {
        url: String,
        #[serde(default)]
        title: String,
    },
    Url(String),
}

fn deserialize_tabs<'de, D>(deserializer: D) -> Result<Vec<SessionTab>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = Vec::<RawSessionTab>::deserialize(deserializer)?;
    Ok(raw
        .into_iter()
        .map(|r| match r {
            RawSessionTab::Detailed { url, title } => SessionTab { url, title },
            RawSessionTab::Url(url) => SessionTab {
                url,
                title: String::new(),
            },
        })
        .collect())
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Session {
    #[serde(deserialize_with = "deserialize_tabs")]
    pub tabs: Vec<SessionTab>,
    /// Index into `tabs` of the tab that was in front.
    pub active: usize,
}

impl Session {
    /// Builds a session from URLs alone.
    pub fn from_tabs(urls: &[String], active: usize) -> Self {
        let entries: Vec<(String, String)> =
            urls.iter().map(|u| (u.clone(), String::new())).collect();
        Self::from_entries(&entries, active)
    }

    /// Builds a session from `(url, title)` pairs, keeping only real pages.
    pub fn from_entries(entries: &[(String, String)], active: usize) -> Self {
        let mut kept = Vec::new();
        let mut new_active = None;
        for (index, (url, title)) in entries.iter().enumerate() {
            if !is_web_url(url) {
                continue;
            }
            if index <= active {
                new_active = Some(kept.len());
            }
            kept.push(SessionTab {
                url: url.clone(),
                title: title.clone(),
            });
        }
        let active = new_active.unwrap_or(0).min(kept.len().saturating_sub(1));
        Self { tabs: kept, active }
    }

    pub fn urls(&self) -> Vec<String> {
        self.tabs.iter().map(|t| t.url.clone()).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    pub fn load(profile: &Profile) -> Self {
        let mut session: Session = storage::load_or_default(&profile.session_path());
        session.tabs.retain(|tab| is_web_url(&tab.url));
        session.active = session.active.min(session.tabs.len().saturating_sub(1));
        session
    }

    pub fn save(&self, profile: &Profile) -> io::Result<()> {
        storage::save(&profile.session_path(), self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::test_util::scratch_dir;

    fn tabs(urls: &[&str]) -> Vec<SessionTab> {
        urls.iter()
            .map(|u| SessionTab {
                url: u.to_string(),
                title: String::new(),
            })
            .collect()
    }

    fn url_strings(urls: &[&str]) -> Vec<String> {
        urls.iter().map(|u| u.to_string()).collect()
    }

    #[test]
    fn keeps_pages_and_the_active_one() {
        let session = Session::from_tabs(&url_strings(&["https://a.test", "https://b.test"]), 1);
        assert_eq!(session.tabs, tabs(&["https://a.test", "https://b.test"]));
        assert_eq!(session.active, 1);
    }

    #[test]
    fn preserves_titles() {
        let entries = vec![
            ("https://a.test".to_string(), "Page A".to_string()),
            ("https://b.test".to_string(), "Page B".to_string()),
        ];
        let session = Session::from_entries(&entries, 1);
        assert_eq!(session.tabs[0].title, "Page A");
        assert_eq!(session.tabs[1].title, "Page B");
        assert_eq!(session.active, 1);
    }

    #[test]
    fn blank_tabs_are_dropped_and_active_follows() {
        let urls = url_strings(&[
            "about:blank",
            "https://a.test",
            "about:blank",
            "https://b.test",
        ]);
        let session = Session::from_tabs(&urls, 3);
        assert_eq!(session.tabs, tabs(&["https://a.test", "https://b.test"]));
        assert_eq!(session.active, 1);
    }

    #[test]
    fn active_blank_tab_falls_back_to_the_previous_page() {
        let urls = url_strings(&["https://a.test", "about:blank", "https://b.test"]);
        let session = Session::from_tabs(&urls, 1);
        assert_eq!(session.active, 0);
    }

    #[test]
    fn all_blank_is_empty() {
        let session = Session::from_tabs(&url_strings(&["about:blank"]), 0);
        assert!(session.is_empty());
        assert_eq!(session.active, 0);
    }

    #[test]
    fn load_repairs_a_bad_file_and_accepts_legacy_urls_array() {
        let dir = scratch_dir("session-repair");
        std::fs::write(
            dir.join("session.json"),
            r#"{ "tabs": ["about:blank", "https://a.test"], "active": 9 }"#,
        )
        .expect("write");
        let session = Session::load(&Profile::from_dir(dir));
        assert_eq!(session.tabs, tabs(&["https://a.test"]));
        assert_eq!(session.active, 0);
    }

    #[test]
    fn round_trip() {
        let profile = Profile::from_dir(scratch_dir("session-roundtrip"));
        let entries = vec![
            ("https://a.test".to_string(), "Alpha".to_string()),
            ("https://b.test".to_string(), "Beta".to_string()),
        ];
        let session = Session::from_entries(&entries, 0);
        session.save(&profile).expect("save");
        assert_eq!(Session::load(&profile), session);
    }
}
