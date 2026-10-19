//! The set of open tabs, saved so the next launch can pick up where this one left off.

use std::io;

use serde::{Deserialize, Serialize};

use crate::input::is_web_url;
use crate::profile::Profile;
use crate::storage;

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Session {
    pub tabs: Vec<String>,
    /// Index into `tabs` of the tab that was in front.
    pub active: usize,
}

impl Session {
    /// Builds a session from the current tabs, keeping only real pages. Blank tabs are
    /// not worth restoring; `active` follows the tab it pointed at, or the nearest
    /// kept tab if that one was dropped.
    pub fn from_tabs(urls: &[String], active: usize) -> Self {
        let mut kept = Vec::new();
        let mut new_active = None;
        for (index, url) in urls.iter().enumerate() {
            if !is_web_url(url) {
                continue;
            }
            if index <= active {
                new_active = Some(kept.len());
            }
            kept.push(url.clone());
        }
        let active = new_active.unwrap_or(0).min(kept.len().saturating_sub(1));
        Self { tabs: kept, active }
    }

    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    pub fn load(profile: &Profile) -> Self {
        let mut session: Session = storage::load_or_default(&profile.session_path());
        session.tabs.retain(|url| is_web_url(url));
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

    fn tabs(urls: &[&str]) -> Vec<String> {
        urls.iter().map(|u| u.to_string()).collect()
    }

    #[test]
    fn keeps_pages_and_the_active_one() {
        let session = Session::from_tabs(&tabs(&["https://a.test", "https://b.test"]), 1);
        assert_eq!(session.tabs, tabs(&["https://a.test", "https://b.test"]));
        assert_eq!(session.active, 1);
    }

    #[test]
    fn blank_tabs_are_dropped_and_active_follows() {
        let urls = tabs(&["about:blank", "https://a.test", "about:blank", "https://b.test"]);
        let session = Session::from_tabs(&urls, 3);
        assert_eq!(session.tabs, tabs(&["https://a.test", "https://b.test"]));
        assert_eq!(session.active, 1);
    }

    #[test]
    fn active_blank_tab_falls_back_to_the_previous_page() {
        let urls = tabs(&["https://a.test", "about:blank", "https://b.test"]);
        let session = Session::from_tabs(&urls, 1);
        assert_eq!(session.active, 0);
    }

    #[test]
    fn all_blank_is_empty() {
        let session = Session::from_tabs(&tabs(&["about:blank"]), 0);
        assert!(session.is_empty());
        assert_eq!(session.active, 0);
    }

    #[test]
    fn load_repairs_a_bad_file() {
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
        let session = Session::from_tabs(&tabs(&["https://a.test", "https://b.test"]), 0);
        session.save(&profile).expect("save");
        assert_eq!(Session::load(&profile), session);
    }
}
