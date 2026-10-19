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

# Zoom and input polish

## Steps
- [ ] Ctrl + mouse wheel zoom: check whether Slint 1.18 `PointerScrollEvent` exposes `modifiers`; wire into `WebviewLogic.scroll`
- [ ] Pinch zoom (touchpads): `WebView::adjust_pinch_zoom`; needs a Slint gesture source
- [ ] Per-site zoom persistence: store `host -> zoom` in `nitium-core` (new store next to bookmarks), apply on navigation
- [ ] IME after switching back to a tab: Servo does not re-send `show_embedder_control(InputMethod)`; either
      remember the field per tab or make the user click the field again (current behaviour)
- [-] Physical key codes (`KeyboardEvent.code`): blocked. Slint's `KeyEvent` has no physical code and deriving
      it from the typed character is wrong on non-US layouts. Revisit if Slint adds one
