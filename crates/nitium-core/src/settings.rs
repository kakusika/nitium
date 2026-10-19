//! User settings (`settings.json`).

use std::io;

pub use nitium_settings::*;

use crate::profile::Profile;

pub trait SettingsExt {
    fn load(profile: &Profile) -> Self;
    fn save(&self, profile: &Profile) -> io::Result<()>;
}

impl SettingsExt for Settings {
    fn load(profile: &Profile) -> Self {
        Self::load_from_path(&profile.settings_path())
    }

    fn save(&self, profile: &Profile) -> io::Result<()> {
        self.save_to_path(&profile.settings_path())
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
        assert!(SEARCH_PRESETS.iter().all(|preset| preset.url_template.contains("%s")));
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
            theme: ThemeMode::System,
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
            theme: ThemeMode::System,
        }
        .sanitized();
        assert_eq!(settings, Settings::default());
    }
}
