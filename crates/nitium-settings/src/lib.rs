//! User settings (`settings.json`) and search presets for the Nitium browser.

use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Search engine preset containing a display name and a query template containing `%s`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchPreset {
    pub name: &'static str,
    pub url_template: &'static str,
}

/// Known search engines as `(name, URL template)`; `%s` is replaced with the query.
pub const SEARCH_PRESETS: &[SearchPreset] = &[
    SearchPreset {
        name: "Google",
        url_template: "https://www.google.com/search?q=%s",
    },
    SearchPreset {
        name: "DuckDuckGo",
        url_template: "https://duckduckgo.com/?q=%s",
    },
    SearchPreset {
        name: "Bing",
        url_template: "https://www.bing.com/search?q=%s",
    },
    SearchPreset {
        name: "Brave",
        url_template: "https://search.brave.com/search?q=%s",
    },
    SearchPreset {
        name: "Yahoo",
        url_template: "https://search.yahoo.com/search?p=%s",
    },
    SearchPreset {
        name: "Ecosia",
        url_template: "https://www.ecosia.org/search?q=%s",
    },
];

pub const DEFAULT_HOME_URL: &str = "https://servo.org";

/// Theme preference for the application window and page content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    #[default]
    System = 0,
    Light = 1,
    Dark = 2,
}

impl ThemeMode {
    pub fn from_index(index: usize) -> Self {
        match index {
            1 => ThemeMode::Light,
            2 => ThemeMode::Dark,
            _ => ThemeMode::System,
        }
    }

    pub fn as_index(self) -> usize {
        self as usize
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SettingsError {
    #[error("Search URL must contain '%s' as the query placeholder")]
    MissingQueryPlaceholder,
    #[error("URL cannot be empty")]
    EmptyUrl,
    #[error("Invalid URL format: {0}")]
    InvalidUrl(String),
}

/// Display entry for theme preset selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemePresetEntry {
    pub id: &'static str,
    pub name: &'static str,
    pub is_dark: bool,
}

pub const THEME_PRESETS: &[ThemePresetEntry] = &[
    ThemePresetEntry { id: "system", name: "System", is_dark: true },
    ThemePresetEntry { id: "dark", name: "Origiri Dark", is_dark: true },
    ThemePresetEntry { id: "light", name: "Origiri Light", is_dark: false },
    ThemePresetEntry { id: "tokyonight-night", name: "Tokyo Night", is_dark: true },
    ThemePresetEntry { id: "tokyonight-day", name: "Tokyo Day", is_dark: false },
    ThemePresetEntry { id: "catppuccin-mocha", name: "Catppuccin Mocha", is_dark: true },
    ThemePresetEntry { id: "catppuccin-latte", name: "Catppuccin Latte", is_dark: false },
    ThemePresetEntry { id: "flexoki-dark", name: "Flexoki Dark", is_dark: true },
    ThemePresetEntry { id: "flexoki-light", name: "Flexoki Light", is_dark: false },
];

pub const CORNER_RADIUS_PRESETS: &[(&str, &str, f32)] = &[
    ("disabled", "None (0px)", 0.0),
    ("small", "Small (4px)", 4.0),
    ("medium", "Medium (8px)", 8.0),
    ("large", "Large (14px)", 14.0),
];

pub const ACCENT_COLOR_PRESETS: &[(&str, &str)] = &[
    ("default", "Default"),
    ("#d12a55", "Crimson"),
    ("#3b82f6", "Blue"),
    ("#10b981", "Emerald"),
    ("#8b5cf6", "Purple"),
    ("#f97316", "Orange"),
];

/// Persistent user configuration for Nitium.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Page opened at startup when there is nothing to restore.
    pub home_url: String,
    /// Search URL template containing `%s`.
    pub search_url: String,
    /// Reopen the tabs of the previous session at startup.
    pub restore_session: bool,
    /// UI theme mode (System, Light, Dark).
    pub theme: ThemeMode,
    /// Origiri theme preset id ("system", "dark", "light", "tokyonight-night", etc.).
    pub theme_preset: String,
    /// Custom accent color in hex (e.g. "#d12a55") or None for preset default.
    pub accent_color: Option<String>,
    /// Corner radius preset ("disabled", "small", "medium", "large").
    pub corner_radius: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            home_url: DEFAULT_HOME_URL.to_string(),
            search_url: SEARCH_PRESETS[0].url_template.to_string(),
            restore_session: true,
            theme: ThemeMode::default(),
            theme_preset: "system".to_string(),
            accent_color: None,
            corner_radius: "small".to_string(),
        }
    }
}

impl Settings {
    /// Loads settings from a file path, returning defaults if the file does not exist or is invalid.
    pub fn load_from_path(path: &Path) -> Self {
        let content = match fs::read_to_string(path) {
            Ok(content) => content,
            Err(_) => return Self::default(),
        };

        serde_json::from_str::<Settings>(&content)
            .map(Self::sanitized)
            .unwrap_or_default()
    }

    /// Saves settings atomically to the given file path.
    pub fn save_to_path(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let json = serde_json::to_string_pretty(self)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        // Write to temporary sibling file and rename for atomicity.
        let tmp_path = path.with_extension("tmp");
        fs::write(&tmp_path, json.as_bytes())?;
        fs::rename(&tmp_path, path)?;
        Ok(())
    }

    /// Sanitizes values that cannot work (e.g. search URL without `%s` or empty home URL).
    pub fn sanitized(mut self) -> Self {
        let defaults = Settings::default();
        if !self.search_url.contains("%s") {
            self.search_url = defaults.search_url;
        }
        if self.home_url.trim().is_empty() {
            self.home_url = defaults.home_url;
        }
        if self.theme_preset.trim().is_empty() {
            self.theme_preset = defaults.theme_preset;
        }
        if self.corner_radius.trim().is_empty() {
            self.corner_radius = defaults.corner_radius;
        }
        self
    }

    /// Finds matching preset index (if `search_url` corresponds to a known preset).
    pub fn search_preset_index(&self) -> Option<usize> {
        SEARCH_PRESETS
            .iter()
            .position(|preset| preset.url_template == self.search_url)
    }

    /// Sets the search URL from a preset index.
    pub fn set_search_preset(&mut self, index: usize) -> bool {
        if let Some(preset) = SEARCH_PRESETS.get(index) {
            self.search_url = preset.url_template.to_string();
            true
        } else {
            false
        }
    }

    /// Validates a candidate search URL template.
    pub fn validate_search_url(template: &str) -> Result<(), SettingsError> {
        let trimmed = template.trim();
        if trimmed.is_empty() {
            return Err(SettingsError::EmptyUrl);
        }
        if !trimmed.contains("%s") {
            return Err(SettingsError::MissingQueryPlaceholder);
        }
        Ok(())
    }

    /// Validates a candidate home URL.
    pub fn validate_home_url(url: &str) -> Result<(), SettingsError> {
        let trimmed = url.trim();
        if trimmed.is_empty() {
            return Err(SettingsError::EmptyUrl);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_usable() {
        let settings = Settings::default();
        assert!(settings.search_url.contains("%s"));
        assert!(settings.restore_session);
        assert_eq!(settings.theme, ThemeMode::System);
        assert_eq!(settings.theme_preset, "system");
        assert_eq!(settings.accent_color, None);
        assert_eq!(settings.corner_radius, "small");
        assert!(SEARCH_PRESETS.iter().all(|preset| preset.url_template.contains("%s")));
        assert!(THEME_PRESETS.iter().any(|preset| preset.id == "system"));
        assert!(THEME_PRESETS.iter().any(|preset| preset.id == "catppuccin-mocha"));
    }

    #[test]
    fn presets_matching() {
        let mut settings = Settings::default();
        assert_eq!(settings.search_preset_index(), Some(0)); // Google

        assert!(settings.set_search_preset(1)); // DuckDuckGo
        assert_eq!(settings.search_url, SEARCH_PRESETS[1].url_template);
        assert_eq!(settings.search_preset_index(), Some(1));

        settings.search_url = "https://custom.search/?q=%s".into();
        assert_eq!(settings.search_preset_index(), None);
    }

    #[test]
    fn theme_mode_indexing() {
        assert_eq!(ThemeMode::from_index(0), ThemeMode::System);
        assert_eq!(ThemeMode::from_index(1), ThemeMode::Light);
        assert_eq!(ThemeMode::from_index(2), ThemeMode::Dark);
        assert_eq!(ThemeMode::from_index(99), ThemeMode::System);

        assert_eq!(ThemeMode::System.as_index(), 0);
        assert_eq!(ThemeMode::Light.as_index(), 1);
        assert_eq!(ThemeMode::Dark.as_index(), 2);
    }

    #[test]
    fn validation() {
        assert!(Settings::validate_search_url("https://example.com/?q=%s").is_ok());
        assert_eq!(
            Settings::validate_search_url("https://example.com/?q=test"),
            Err(SettingsError::MissingQueryPlaceholder)
        );
        assert_eq!(Settings::validate_search_url(""), Err(SettingsError::EmptyUrl));

        assert!(Settings::validate_home_url("https://servo.org").is_ok());
        assert_eq!(Settings::validate_home_url(""), Err(SettingsError::EmptyUrl));
    }

    #[test]
    fn round_trip_persistence() {
        let temp_dir = std::env::temp_dir().join(format!("nitium-test-settings-{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp_dir);
        let path = temp_dir.join("settings.json");

        let settings = Settings {
            home_url: "https://example.com".into(),
            search_url: "https://duckduckgo.com/?q=%s".into(),
            restore_session: false,
            theme: ThemeMode::Dark,
            theme_preset: "catppuccin-mocha".into(),
            accent_color: Some("#d12a55".into()),
            corner_radius: "large".into(),
        };

        settings.save_to_path(&path).expect("save");
        let loaded = Settings::load_from_path(&path);
        assert_eq!(loaded, settings);

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn sanitization_repairs_invalid_values() {
        let broken = Settings {
            home_url: "".into(),
            search_url: "https://broken.com".into(),
            restore_session: true,
            theme: ThemeMode::System,
            theme_preset: "".into(),
            accent_color: None,
            corner_radius: "   ".into(),
        };
        let sanitized = broken.sanitized();
        assert_eq!(sanitized.home_url, DEFAULT_HOME_URL);
        assert_eq!(sanitized.search_url, SEARCH_PRESETS[0].url_template);
        assert_eq!(sanitized.theme_preset, "system");
        assert_eq!(sanitized.corner_radius, "small");
    }
}
