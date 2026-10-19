# Development tasks.
default:
    @just --list

#[ Application ]

# Run the Nitium browser application.
run *args:
    cargo run -p nitium -- {{args}}

# Type-check the workspace.
check:
    cargo check --workspace

# Run tests across the workspace.
test:
    cargo test --workspace

#[ Documentation ]

# Generate and update all derived documentation across the workspace.
docs:
    tomet export .
    tomet format -i .

# Check that all documents parse, format cleanly, and match export targets.
docs-check:
    tomet check .
    tomet format --check .
    tomet export --check .
    @if [ -f .writ.tmt ] && command -v twrit >/dev/null 2>&1; then \
        twrit check .; \
    fi
