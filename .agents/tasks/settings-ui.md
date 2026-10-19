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
- [ ] Decide where it lives: an in-app page/overlay (preferred, consistent with `DialogLayer`/`MenuLayer`) vs a separate window
- [ ] Open from the UI (menu button in the toolbar) and a shortcut (Ctrl+, )
- [ ] Fields: search engine (presets + custom template, validated to contain `%s`), home page, restore session
- [ ] Apply immediately: `BrowserApp.settings` is currently immutable (`settings: Settings`); make it
      `RefCell<Settings>` and save through `Settings::save`
- [ ] "Clear history" (`History::clear` exists) and "Clear bookmarks" with confirmation
- [ ] Theme override (light/dark/system): `ServoBrowser::set_theme` exists; the window uses Origiri `Tokens`
- [ ] Tests in `nitium-core` for any new validation
