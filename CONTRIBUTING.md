<!-- Generated from tmtroot/contributing.tmt. Edit that, then `tomet export .`. -->

# Contributing to Nitium

## Documentation Policy

- All Markdown files (`README.md`, `AGENTS.md`, `CONTRIBUTING.md`, etc.) are generated build artifacts.
Never edit `.md` files directly.
- Always edit the source `.tmt` files under `tmtroot/`.
- Run `just docs` (or `tomet export .`) to regenerate Markdown files, and verify with `just docs-check`.

## Commit Convention

All commits must follow the Conventional Commits format: `type(scope): description`.

### Types

- `feat`: New feature or capability.
- `fix`: Bug fix.
- `docs`: Documentation changes (`.tmt` or generated docs).
- `refactor`: Code restructuring without changing behavior.
- `perf`: Performance improvement.
- `test`: Adding or correcting tests.
- `chore`: Build scripts, dependencies, tooling, repo maintenance.

### Scopes

Use the affected app, crate, or feature as the scope:

- **Apps**: `desktop`, `mobile`
- **Crates**: `core`, `shell`, `webview`
- **Features**: `tabs`, `bookmarks`, `history`, `session`, `settings`, `shortcuts`, `controls`, `find`, `zoom`, `ime`, `ui`
- **Tooling / Infra**: `nix`, `deps`, `docs`

> [!tip]
> If a change spans multiple components or the whole workspace,
> omit the scope or use a broad category (e.g. `refactor(crates): ...` or `chore: update dependencies`).

## Clean Commits Policy

Do not include AI session metadata or trailers (e.g. `Claude-Session:`, `Co-Authored-By:`) in commit messages.
Keep commit logs clean and focused on technical changes.

## Verification

Before committing or submitting changes, ensure all checks pass:

- `just check-fast` and `just test-fast` (or `nix develop . --command cargo check --workspace` when modifying `nitium-webview`)
- `just docs-check`: Ensure documentation artifacts are up to date and valid.

