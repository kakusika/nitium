//! Where the browser keeps its data.

use std::path::{Path, PathBuf};

use directories::ProjectDirs;

/// Environment variable that overrides the profile location (useful for testing and
/// for running several isolated browsers).
pub const PROFILE_DIR_ENV: &str = "NITIUM_PROFILE_DIR";

#[derive(Debug, Clone)]
pub struct Profile {
    dir: PathBuf,
}

impl Profile {
    pub fn from_dir(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// `$NITIUM_PROFILE_DIR`, else the platform data directory (e.g. `~/.local/share/nitium`).
    pub fn default_location() -> Option<Self> {
        if let Some(dir) = std::env::var_os(PROFILE_DIR_ENV).filter(|d| !d.is_empty()) {
            return Some(Self::from_dir(dir));
        }
        ProjectDirs::from("dev", "origiri", "nitium")
            .map(|dirs| Self::from_dir(dirs.data_dir().to_path_buf()))
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn settings_path(&self) -> PathBuf {
        self.dir.join("settings.json")
    }

    pub fn bookmarks_path(&self) -> PathBuf {
        self.dir.join("bookmarks.json")
    }

    pub fn history_path(&self) -> PathBuf {
        self.dir.join("history.json")
    }

    pub fn session_path(&self) -> PathBuf {
        self.dir.join("session.json")
    }

    /// Directory for the web engine's own data (cookies, local storage, cache).
    pub fn engine_dir(&self) -> PathBuf {
        self.dir.join("engine")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_live_under_the_profile_dir() {
        let profile = Profile::from_dir("/data/nitium");
        assert_eq!(profile.settings_path(), Path::new("/data/nitium/settings.json"));
        assert_eq!(profile.bookmarks_path(), Path::new("/data/nitium/bookmarks.json"));
        assert_eq!(profile.history_path(), Path::new("/data/nitium/history.json"));
        assert_eq!(profile.session_path(), Path::new("/data/nitium/session.json"));
        assert_eq!(profile.engine_dir(), Path::new("/data/nitium/engine"));
    }
}
