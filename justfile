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

#[ Android ]

dist := "dist"
jniLibs := "android/app/src/main/jniLibs"
apk_raw := "apps/android/android/app/build/outputs/apk/release/app-release.apk"
apk := dist + "/nitium.apk"
apk_arm := dist + "/nitium-arm64.apk"
apk_x86 := dist + "/nitium-x86_64.apk"

android_rustflags := "-C force-unwind-tables=no -C link-arg=-Wl,--icf=all -C link-arg=-Wl,--gc-sections -C link-arg=-Wl,--strip-all"

# Build Android APK for ARM64 (real devices)
apk-arm: (_build-apk "-t arm64-v8a" apk_arm)

# Build Android APK for x86_64 (emulator)
apk-x86: (_build-apk "-t x86_64" apk_x86)

# Internal recipe: cross-compile native lib and assemble APK via Gradle
_build-apk abis out:
    cd apps/android && rm -rf {{jniLibs}}
    cd apps/android && RUSTFLAGS="{{android_rustflags}}" HOST_CC=gcc HOST_CXX=g++ CC_x86_64_unknown_linux_gnu=gcc CXX_x86_64_unknown_linux_gnu=g++ cargo ndk {{abis}} -P 34 -o {{jniLibs}} build --release --lib -p nitium-android
    cd apps/android/android && gradle assembleRelease


    @mkdir -p {{dist}}
    cp {{apk_raw}} {{out}}
    cp {{apk_raw}} {{apk}}
    @echo "Built: {{out}} (and {{apk}})"

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
