# Refresh project docs for the browser

Source of truth is `tmtroot/*.tmt`; `.md` files are generated (`just docs`, check with `just docs-check`).
Never edit `.md` directly.

## Steps
- [ ] `tmtroot/agents.tmt` "Project overview": replace the Tomet-vault description and the crate list
      (`apps/desktop`, `apps/mobile`, `crates/core|fs|meta|selection|vault-cache|vault-index`, `features/*`
      do not exist) with the real layout: `apps/nitium`, `crates/nitium-core`, `crates/nitium-webview`
- [ ] `agents.tmt` "Commits": the examples `feat(explorer): ...` / `fix(vault-index): ...` name scopes that do not exist
- [ ] `agents.tmt` / `contributing.tmt`: document the two-tier verification (cheap vs heavy, see below)
- [ ] `contributing.tmt` "Scopes": replace Apps/Core/Features/Extensions (desktop, mobile, canvas, board,
      node-graph, timeline, explorer, comfyui, ...) with the real ones (`nitium`, `core`, `webview`, and areas
      such as `tabs`, `input`, `find`, `bookmarks`, `history`, `session`, `settings`, `ui`) plus `nix`, `deps`, `docs`
- [ ] `contributing.tmt` "Verification": `cargo check --workspace` is the heavy path (builds Servo); add
      `cargo test -p nitium-core` as the fast path that needs no Servo
- [ ] `readme.tmt`: workspace layout lacks `nitium-core`; "Run Tests" says `cargo test --workspace` (heavy);
      add a short feature list (tabs, shortcuts, bookmarks, history, session restore, find, zoom, IME)
- [ ] `tmtroot/claude.tmt` is an empty file while `CLAUDE.md` (`@AGENTS.md`) exists: decide whether
      `claude.tmt` should generate it (`export` config) so `CLAUDE.md` is not hand-maintained
- [ ] `justfile`: `test` runs `cargo test --workspace`; consider adding `test-core` (`cargo test -p nitium-core`)
- [ ] `just docs` then `just docs-check` (both use `tomet`, not cargo)

## Notes
- `jj` is not initialised in this repo (`jj git init` needed for the jj workflow that AGENTS.md describes).
- No cargo needed for this task at all.
