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

Servo 0.5's `WebViewDelegate` has no callback for failed loads or TLS errors, and `LoadStatus` has no failure
state. What the user sees today is whatever Servo renders by itself (unverified).

## Steps
- [ ] User: open `https://expired.badssl.com/`, `https://self-signed.badssl.com/` and an unreachable host; record what Servo shows
- [ ] Grep Servo 0.5 for `net_error`, `neterror`, `about:` error pages and `ServoDelegate::notify_error`
- [ ] If no hook exists: consider a content-based approach (detect Servo's error document title via `Title` events)
      and a friendly page; otherwise leave it and note the upstream gap
- [ ] If a hook exists: show an interstitial with "Go back" and (for certificates) an explicit, per-host override
