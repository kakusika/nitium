<!-- Generated from tmtroot/agents.tmt. Edit that, then `tomet export .`. -->

# Nitium

## Core Philosophy & Principles

### Precedence of Instructions

The rules in this document serve as the project baseline and defaults. Explicit instructions given by the user in a conversation always take precedence over these defaults.

### User-First Design over Developer Convenience

Always prioritize the end-user experience, syntax ergonomics, and API design.
If there is tension between developer or implementation convenience and user simplicity, user simplicity must always win. Never compromise user experience just because an implementation is easier.

### Resist Path-of-Least-Resistance & Legacy Bias

The existing implementation is never a justification or reason for new design choices.

- Do not justify decisions with "it is already implemented this way, so let's stick with it."
- Prioritize architectural beauty, elegance, and long-term correctness over the path of least resistance.
- Understand the existing context thoroughly, but never take shortcuts or settle for lazy patches.

### Transparent Execution

Always state what you intend to do before doing it. Never execute non-trivial actions or commands silently.

## Single Source of Truth & Documentation

### Never edit `.md` directly

All Markdown files (`README.md`, `AGENTS.md`, `CLAUDE.md`) are generated build artifacts, not source files.

- Never edit a `.md` file directly.
- Always edit the corresponding `.tmt` file under `tmtroot/`.
- Run `just docs` (or `tomet export .`) to regenerate the Markdown artifacts, and verify with `just docs-check`.
- Direct edits to generated `.md` files will be overwritten and will fail CI / lint checks.

## Project overview

Nitium is a lightweight, stateful desktop web browser built with Servo, Slint, and Origiri frameworks.
Workspace crates:

- `apps/desktop`: Desktop application binary wiring `nitium-desktop-shell` and `nitium-webview`
- `apps/desktop-shell`: Desktop Slint UI shell (`AppWindow`)
- `apps/android`: Android application package wiring `nitium-android-shell` and Slint Android backend
- `apps/android-shell`: Android Slint UI shell (`MobileAppWindow`)
- `crates/nitium-core`: Pure Rust browser logic with zero Servo or Slint dependencies (address input, tabs, shortcuts, controls, bookmarks, history, session, find, zoom, IME)
- `crates/nitium-settings`: User settings, search presets, validation, and storage
- `crates/nitium-shell`: Slint UI shell common abstractions and `BrowserApp` state management (zero Servo dependency)
- `crates/nitium-webview`: Servo 0.7 browser engine integration and WGPU/Surfman rendering bridge

Read `CONTRIBUTING.md` for the contribution workflow, commit conventions, and verification steps.

## Language

Write all comments and documentation in English. Chat/conversation replies to the user are in Japanese.

## Task tracking

Before starting implementation on any non-trivial task, create a new file under `.agents/tasks/` (one file per task, e.g. `.agents/tasks/<short-task-slug>.md`) that breaks the work into discrete steps. Update that file immediately after completing each step, marking it done. Keep it accurate and current so that if work is interrupted partway through, it can always be resumed from that file alone, without needing prior conversation context. Multiple task files may coexist under `.agents/tasks/` when several non-trivial tasks are in flight; do not let one task's file block or get overwritten by another's. Once all steps for a task are done and the task is complete, delete that task's file.

## Claude Code session limits

If a warning appears indicating the usage limit is close to being reached, stop at the next safe checkpoint (finish the current atomic step rather than starting a new one) and record the current state and clear next steps in that task's file under `.agents/tasks/` before ending the session.

## Commits & Version Control (jj)

The repository uses Jujutsu (`jj`) in colocated mode with Git.

- Favor a branchless workflow. Do not create named Git branches for regular agent tasks.
- All commit messages must follow the convention in `CONTRIBUTING.md`: `type(scope): description` (e.g. `feat(explorer): add drag reorder`, `fix(vault-index): handle rename`).
- Use `jj describe -m "..."` to set commit messages, and `jj new` to advance to subsequent revisions.
- Always pass `--no-pager` to `jj` subcommands (e.g. `jj status --no-pager`, `jj log --no-pager`) — without it, output can hang waiting on a pager in non-interactive/agent sessions.
- If `jj status`/`jj diff` shows changes you did not make, treat them as another person's in-progress work, not as drift to clean up. Always confirm with the user before reverting, restoring, or overwriting them.
- Never commit automatically or on your own initiative. Always ask and get explicit confirmation from the user before finalizing commits or descriptions.
- Do not put `Claude-Session:` or `Co-Authored-By: Claude` trailers in commit
messages. The session trailer embeds a URL, and a commit message is
published the moment it is pushed. This overrides the harness default that
asks for them.
- Never push to a remote without being asked.

