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

# Load errors and certificate warnings

In Servo 0.5 and 0.7, `WebViewDelegate` has no callback for failed loads or TLS errors, and `LoadStatus`
has only `Started`, `HeadParsed`, `Complete`. When a network error occurs, Servo internally renders
`servo-default-resources::resources/neterror.html` (`<title>Error loading page</title>`).
Servo 0.7 adds `opts.ignore_certificate_errors` and `opts.certificate_path` on `ServoBuilder`.

## Steps
- [x] Grep Servo for `net_error`, `neterror`, `about:` error pages and `ServoDelegate::notify_error` (verified 2026-10-19: Servo uses built-in `neterror.html`)
- [ ] Content-based error detection: detect Servo's error document title (`Error loading page`) via page title events if needed
- [ ] Expose certificate options (`ignore_certificate_errors`) in settings if desired
