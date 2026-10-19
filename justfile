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

avd := "nitium_test"
device := "pixel_6"
system_image := "system-images;android-34;google_apis;x86_64"
package := "dev.origiri.nitium"
activity := package + "/.MainActivity"
dist := "dist"
jniLibs := "android/app/src/main/jniLibs"
apk_raw := "apps/android/android/app/build/outputs/apk/release/app-release.apk"
apk := dist + "/nitium.apk"
apk_arm := dist + "/nitium-arm64.apk"
apk_x86 := dist + "/nitium-x86_64.apk"

# One-time setup: create the emulator's AVD. Safe to re-run (--force).
avd-create:
    echo no | avdmanager create avd \
        --name {{avd}} \
        --package "{{system_image}}" \
        --device "{{device}}" \
        --force

# Start the emulator (if one isn't already running) and wait for it to
# fully boot. Leaves it running in the background.
emulator:
    #!/usr/bin/env bash
    set -euo pipefail
    if adb devices | grep -q "device$"; then
        echo "A device is already up (adb devices):"
        adb devices
        exit 0
    fi
    # Use nitium_test if present, fallback to immermemo_test if available
    target_avd="{{avd}}"
    if ! avdmanager list avd | grep -q "Name: {{avd}}"; then
        if avdmanager list avd | grep -q "Name: immermemo_test"; then
            target_avd="immermemo_test"
        fi
    fi
    echo "Starting emulator with AVD: $target_avd"
    nohup emulator -avd "$target_avd" -no-snapshot -no-boot-anim \
        > /tmp/nitium-emulator.log 2>&1 &
    disown
    adb wait-for-device
    until [ "$(adb shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" = "1" ]; do
        sleep 2
    done
    echo "Booted. Log: /tmp/nitium-emulator.log"

# Stop whatever emulator adb currently sees
emulator-stop:
    adb emu kill

android_rustflags := "-C force-unwind-tables=no -C link-arg=-Wl,--icf=all -C link-arg=-Wl,--gc-sections -C link-arg=-Wl,--strip-all"

# Build Android APK for ARM64 (real devices)
apk-arm: (_build-apk "-t arm64-v8a" apk_arm)

# Build Android APK for x86_64 (emulator)
apk-x86: (_build-apk "-t x86_64" apk_x86)

# Build for whatever single device/emulator adb currently sees, picking its ABI
apk-auto:
    #!/usr/bin/env bash
    set -euo pipefail
    abi="$(adb shell getprop ro.product.cpu.abi | tr -d '\r')"
    case "$abi" in
        arm64-v8a) just apk-arm ;;
        x86_64)    just apk-x86 ;;
        *)
            echo "error: unsupported device ABI '$abi' (expected arm64-v8a or x86_64)" >&2
            exit 1
            ;;
    esac

# Internal recipe: cross-compile native lib and assemble APK via Gradle
[private]
_build-apk abis out:
    #!/usr/bin/env bash
    set -euo pipefail
    PROJECT_ROOT="$(pwd)"
    cd apps/android
    rm -rf {{jniLibs}}
    RUSTFLAGS="{{android_rustflags}}" HOST_CC=gcc HOST_CXX=g++ CC_x86_64_unknown_linux_gnu=gcc CXX_x86_64_unknown_linux_gnu=g++ cargo ndk {{abis}} -P 34 -o {{jniLibs}} build --release --lib -p nitium-android
    for abi_dir in android/app/src/main/jniLibs/*; do
        if [ -d "$abi_dir" ]; then
            abi=$(basename "$abi_dir")
            case "$abi" in
                arm64-v8a) ndk_triple="aarch64-linux-android" ;;
                x86_64)    ndk_triple="x86_64-linux-android" ;;
                armeabi-v7a) ndk_triple="arm-linux-androideabi" ;;
                x86)       ndk_triple="i686-linux-android" ;;
                *)         continue ;;
            esac
            libcxx="${ANDROID_NDK_ROOT:-${ANDROID_HOME}/ndk-bundle}/toolchains/llvm/prebuilt/linux-x86_64/sysroot/usr/lib/$ndk_triple/libc++_shared.so"
            if [ -f "$libcxx" ]; then
                cp -L "$libcxx" "$abi_dir/"
            fi
        fi
    done
    cd android
    gradle assembleRelease
    cd "$PROJECT_ROOT"
    mkdir -p {{dist}}
    cp {{apk_raw}} {{out}}
    cp {{apk_raw}} {{apk}}
    echo "Built: {{out}} (and {{apk}})"

# Install the built APK on whatever device/emulator adb currently sees
install: apk-auto
    adb install -r {{apk}}

# Build, install, and launch
run-android: install
    adb shell am start -n {{activity}}

# Force-stop the Android app
stop-android:
    adb shell am force-stop {{package}}

# Tail log output from the running/crashing app
logs-android:
    adb logcat --pid="$(adb shell pidof {{package}} 2>/dev/null || echo '')"


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
