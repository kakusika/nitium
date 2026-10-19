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

# Remaining page controls

`show_embedder_control` in `crates/nitium-webview/src/delegate.rs` currently drops file and colour pickers,
which tells the page they were cancelled.

## Steps
- [ ] File picker (`<input type=file>`): `EmbedderControl::FilePicker` (`filter_patterns`, `allow_select_multiple`,
      `select`, `submit`, `dismiss`). Needs a native dialog: evaluate the `rfd` crate (xdg-desktop-portal on Wayland)
- [ ] Colour picker (`<input type=color>`): `EmbedderControl::ColorPicker`; a small popup with a hex field and a swatch row
- [ ] Keyboard navigation in `MenuLayer` (Up/Down/Enter/Esc, type-ahead for `<select>`); mouse only today
- [ ] Date/time inputs (`InputMethodType::Date`, ...) get a plain text IME today; a picker is optional
- [ ] Wrap each new control like `controls.rs` does (opaque handle, dropping == dismiss)
