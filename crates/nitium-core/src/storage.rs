//! Small JSON persistence helpers shared by every store.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde::de::DeserializeOwned;

/// Loads a JSON file.
///
/// * A missing file yields `T::default()` (first run).
/// * A file that cannot be parsed is moved to `<name>.corrupt` so the user's data is
///   never silently overwritten by the next save, and `T::default()` is returned.
pub fn load_or_default<T: DeserializeOwned + Default>(path: &Path) -> T {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return T::default(),
        Err(err) => {
            eprintln!("nitium-core: cannot read {}: {err}", path.display());
            return T::default();
        }
    };
    match serde_json::from_str(&text) {
        Ok(value) => value,
        Err(err) => {
            let aside = corrupt_path(path);
            eprintln!(
                "nitium-core: {} is corrupt ({err}); moving it to {}",
                path.display(),
                aside.display()
            );
            let _ = fs::rename(path, aside);
            T::default()
        }
    }
}

/// Writes `value` as pretty JSON, atomically (temp file + rename) so a crash mid-write
/// cannot leave a truncated file behind.
pub fn save<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(value).map_err(io::Error::other)?;
    let tmp = sibling_with_suffix(path, ".tmp");
    fs::write(&tmp, json)?;
    fs::rename(&tmp, path)
}

fn corrupt_path(path: &Path) -> PathBuf {
    sibling_with_suffix(path, ".corrupt")
}

fn sibling_with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(suffix);
    path.with_file_name(name)
}

#[cfg(test)]
pub(crate) mod test_util {
    use std::path::PathBuf;

    /// A fresh, empty directory unique to this test process and `name`.
    pub fn scratch_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("nitium-core-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create scratch dir");
        dir
    }
}

#[cfg(test)]
mod tests {
    use super::test_util::scratch_dir;
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
    struct Sample {
        n: u32,
    }

    #[test]
    fn missing_file_is_default() {
        let dir = scratch_dir("storage-missing");
        let loaded: Sample = load_or_default(&dir.join("none.json"));
        assert_eq!(loaded, Sample::default());
    }

    #[test]
    fn save_then_load_round_trips_and_creates_parents() {
        let dir = scratch_dir("storage-roundtrip");
        let path = dir.join("nested/deeper/sample.json");
        save(&path, &Sample { n: 7 }).expect("save");
        assert_eq!(load_or_default::<Sample>(&path), Sample { n: 7 });
        assert!(!sibling_with_suffix(&path, ".tmp").exists());
    }

    #[test]
    fn corrupt_file_is_preserved() {
        let dir = scratch_dir("storage-corrupt");
        let path = dir.join("sample.json");
        fs::write(&path, "{ not json").expect("write");
        assert_eq!(load_or_default::<Sample>(&path), Sample::default());
        assert!(!path.exists());
        assert_eq!(
            fs::read_to_string(dir.join("sample.json.corrupt")).expect("kept"),
            "{ not json"
        );
    }
}
