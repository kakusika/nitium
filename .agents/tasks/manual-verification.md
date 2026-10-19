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

# Manual verification of Phases 1-4 (user runs `just run`)

Everything below was written without being run by the agent. Report failures (with logs) back.

## Tabs and shortcuts
- [ ] Frame appears after switching tabs (assumes `show()` triggers a repaint; if the old tab's pixels linger,
      call `paint()` and emit a frame in `ServoBrowser::activate_tab`)
- [ ] Ctrl+T opens a blank tab with the address bar focused; Ctrl+W closes; closing the last tab quits;
      Ctrl+Shift+T reopens; Ctrl+Tab / Ctrl+1..9 switch
- [ ] `target=_blank` / `window.open` open and activate a new tab
- [ ] Ctrl+C / V / X / A reach the page (clipboard uses Servo's `arboard`; on Wayland it may need XWayland)
- [ ] HiDPI (scale > 1): page layout width matches the window (the `resize` physical-pixel fix)
- [ ] Dark/light theme applies to the first tab at startup

## Input
- [ ] Japanese IME: composition chip appears over the field, commit lands in the field, clicking inside a
      focused field still moves the caret, focus leaving mid-composition does not wedge the field
- [ ] Ctrl+F opens the find bar and focuses it; Enter / Shift+Enter / F3 step; counter shows `n/m`;
      Esc closes and clears highlights; bar closes on tab switch and navigation
- [ ] Ctrl + / = / - / 0 zoom; the % pill appears and a click resets
- [ ] Cursor changes over links and text fields

## Persistence
- [ ] Ctrl+D toggles the star; bookmarks survive a restart (`$NITIUM_PROFILE_DIR` can point at a scratch dir)
- [ ] Address bar suggestions (history + bookmarks), Up/Down/Enter/click
- [ ] Session restore after closing the window with several tabs open

## Page questions
- [ ] `alert()`, `confirm()`, `prompt()` dialogs; Enter/Esc; heading shows the origin
- [ ] A permission prompt (e.g. `navigator.geolocation.getCurrentPosition`) and an HTTP-auth page
- [ ] `<select>` single and `multiple`; right-click menu on a link, an image, a text field and plain text

## Known risks to look at first
- `TextInput.preedit-text` is documented by Slint as internal ("only exposed for IME"); a Slint upgrade may drop it.
- The IME bridge relies on Servo sending `show_embedder_control(InputMethod)`; it is not re-sent when switching back to a tab.

## Notes
- `crates/nitium-webview/src/rendering_context/{servo_rendering_adapter,surfman_context}.rs` carry uncommitted
  changes that are not the agent's (Linux uses CPU read-back instead of the Vulkan texture). Leave them alone;
  confirm with the user before reverting or overwriting.
