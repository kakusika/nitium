## Context
Nitium is a Servo + Slint desktop browser (`apps/nitium` = UI and glue, `crates/nitium-webview` = Servo
integration, `crates/nitium-core` = Servo-free logic with unit tests). Phases 1-4 of the daily-driver
effort (tabs, shortcuts, IME, find, zoom, persistence, dialogs, menus) are written and `cargo check`
passes (user, 2026-10-16).

**Working rules for this task**
- Do not run cargo on this repo: Servo makes it too heavy for the user's machine. The only acceptable
  command is `cargo test -p nitium-core` (no Servo, ~11 s). Ask the user to run `cargo check` / `cargo run`.
- The agent sandbox cannot reach the Wayland compositor, so GUI behaviour must be verified by the user.
- Put testable logic in `nitium-core`; keep `apps/nitium` and `nitium-webview` thin.
- Servo 0.5.0 sources are not in the local cargo registry; fetch with
  `curl https://static.crates.io/crates/<crate>/<crate>-0.5.0.crate` into the scratchpad and read only.
- Do not commit without asking.

# Settings UI

Today `settings.json` (in the profile dir) is edited by hand. Fields: `home_url`, `search_url` (template with
`%s`), `restore_session`. Logic lives in `nitium-core::settings` (tested); `SEARCH_PRESETS` lists known engines.

## Steps
- [x] Decide where it lives: an in-app page/overlay (`SettingsLayer`, consistent with `DialogLayer`/`MenuLayer`)
- [x] Open from the UI (settings button in the toolbar) and shortcut (`Ctrl+,`)
- [x] Fields: search engine (presets + custom template, validated to contain `%s`), home page, restore session
- [x] Apply immediately: `BrowserApp.settings` is `RefCell<Settings>` and saves through `SettingsExt::save`
- [x] "Clear history", "Clear bookmarks", and "Clear cookies & cache" (via Servo 0.7 `SiteDataManager` & `NetworkManager`)
- [x] Theme override (light/dark/system): wired to `WebEngine::set_theme` and Slint UI
- [x] Extracted dedicated `crates/nitium-settings` crate with full unit test coverage
