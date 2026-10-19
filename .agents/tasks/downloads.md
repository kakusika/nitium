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

# Downloads (blocked upstream)

Servo 0.5 has no download API: no delegate callback for responses that should be saved, and `WebResourceLoad`
cannot hand a body to the embedder as a file. Links to files currently render (or fail) as pages.

## Steps
- [ ] On each Servo upgrade, re-check `webview_delegate.rs` / `servo_delegate.rs` for a download hook
- [ ] Possible workaround to evaluate: intercept via `WebViewDelegate::load_web_resource` (`WebResourceLoad::intercept`)
      and fetch the URL ourselves for `Content-Disposition: attachment` / non-renderable types (needs an HTTP client dependency)
- [ ] UI: download progress list, destination setting (needs `settings-ui.md`), "open folder"
