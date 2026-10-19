//! User settings (`settings.json`).

use std::io;

use serde::{Deserialize, Serialize};

use crate::profile::Profile;
use crate::storage;

/// Known search engines as `(name, URL template)`; `%s` is the query.
pub const SEARCH_PRESETS: &[(&str, &str)] = &[
    ("Google", "https://www.google.com/search?q=%s"),
    ("DuckDuckGo", "https://duckduckgo.com/?q=%s"),
    ("Bing", "https://www.bing.com/search?q=%s"),
    ("Brave", "https://search.brave.com/search?q=%s"),
];

const DEFAULT_HOME_URL: &str = "https://servo.org";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Page opened at startup when there is nothing to restore.
    pub home_url: String,
    /// Search URL template containing `%s`.
    pub search_url: String,
    /// Reopen the tabs of the previous session at startup.
    pub restore_session: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            home_url: DEFAULT_HOME_URL.to_string(),
            search_url: SEARCH_PRESETS[0].1.to_string(),
            restore_session: true,
        }
    }
}

impl Settings {
    /// Loads settings, repairing values that cannot work (a search URL without `%s`
    /// would send every query to the same page).
    pub fn load(profile: &Profile) -> Self {
        storage::load_or_default::<Settings>(&profile.settings_path()).sanitized()
    }

    pub fn save(&self, profile: &Profile) -> io::Result<()> {
        storage::save(&profile.settings_path(), self)
    }

    pub fn sanitized(mut self) -> Self {
        let defaults = Settings::default();
        if !self.search_url.contains("%s") {
            self.search_url = defaults.search_url;
        }
        if self.home_url.trim().is_empty() {
            self.home_url = defaults.home_url;
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::test_util::scratch_dir;

    #[test]
    fn defaults_are_usable() {
        let settings = Settings::default();
        assert!(settings.search_url.contains("%s"));
        assert!(settings.restore_session);
        assert!(SEARCH_PRESETS.iter().all(|(_, url)| url.contains("%s")));
    }

    #[test]
    fn first_run_uses_defaults() {
        let profile = Profile::from_dir(scratch_dir("settings-first-run"));
        assert_eq!(Settings::load(&profile), Settings::default());
    }

    #[test]
    fn round_trip() {
        let profile = Profile::from_dir(scratch_dir("settings-roundtrip"));
        let settings = Settings {
            home_url: "https://example.com".into(),
            search_url: "https://duckduckgo.com/?q=%s".into(),
            restore_session: false,
        };
        settings.save(&profile).expect("save");
        assert_eq!(Settings::load(&profile), settings);
    }

    #[test]
    fn partial_files_fill_in_defaults() {
        let dir = scratch_dir("settings-partial");
        std::fs::write(dir.join("settings.json"), r#"{ "home_url": "https://a.test" }"#)
            .expect("write");
        let settings = Settings::load(&Profile::from_dir(dir));
        assert_eq!(settings.home_url, "https://a.test");
        assert_eq!(settings.search_url, Settings::default().search_url);
    }

    #[test]
    fn broken_values_are_repaired() {
        let settings = Settings {
            home_url: "  ".into(),
            search_url: "https://no-placeholder.test/".into(),
            restore_session: true,
        }
        .sanitized();
        assert_eq!(settings, Settings::default());
    }
}
