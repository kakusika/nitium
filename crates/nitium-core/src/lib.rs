//! Browser logic that does not depend on Servo or Slint.
//!
//! Everything here is plain data and pure functions so it can be tested quickly
//! (`cargo test -p nitium-core`) without building the rendering engine.

pub mod bookmarks;
pub mod find;
pub mod history;
pub mod ime;
pub mod input;
pub mod profile;
pub mod session;
pub mod settings;
pub mod storage;
pub mod zoom;

/// Seconds since the Unix epoch. Stores take the time as a parameter so tests stay deterministic.
pub fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}
