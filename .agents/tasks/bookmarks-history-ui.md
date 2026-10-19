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

# Bookmarks list and history page

Bookmarks (`nitium-core::bookmarks`) and history (`nitium-core::history`) are only reachable through the
address bar suggestions and the star button. There is no way to browse, search, edit or delete them.

## Steps
- [x] Bookmarks panel (toolbar button / Ctrl+Shift+O): list, search, open (click / middle-click for new tab), remove
- [x] History panel (Ctrl+H): grouped by date, search, open, remove entry, clear all
- [ ] Optional bookmarks bar under the toolbar
- [x] Rename / edit title; folders are out of scope until needed
- [x] `nitium-core`: add `Bookmarks::rename`, `History::remove`, `History::recent(limit)` with tests as needed
- [x] Reuse `MenuLayer`/`DialogLayer` patterns in `apps/nitium/ui/app.slint`
