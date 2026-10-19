# Development tasks.
default:
    @just --list

#[ Application ]

# Run the Nitium browser application.
run *args:
    cargo run -p nitium-desktop -- {{args}}

# Type-check UI shell and core logic without building Servo.
check-fast:
    cargo check -p nitium-settings -p nitium-core -p nitium-shell

# Run unit tests in core and shell without building Servo.
test-fast:
    cargo test -p nitium-settings -p nitium-core -p nitium-shell

# Type-check the full workspace (including Servo).
check:
    cargo check --workspace

# Run tests across the full workspace.
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
