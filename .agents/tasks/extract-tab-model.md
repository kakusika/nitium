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

# Extract the tab model into nitium-core

The tab list logic (selection after close, relative/indexed selection, reopen-closed stack, session snapshot) lives
in `apps/nitium/src/browser.rs` where it cannot be unit tested without building Servo.

## Steps
- [ ] Design a `TabList` in `nitium-core` over plain data (id, url, title, ...), independent of Slint
- [ ] Move: successor choice on close, `select_relative`, `select_index`, closed-URL stack (cap 25), session snapshot
- [ ] Keep `BrowserApp` as the adapter that mirrors it into the Slint `VecModel`
- [ ] Tests for the cases that matter: close active tab picks the neighbour, close last quits, wrap-around, reopen order
- [ ] Also covers the old "unit tests for tab model" item; URL, bookmark, history, session, find, ime, zoom tests already exist (62 in `nitium-core`)
