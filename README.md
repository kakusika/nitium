<!-- Generated from tmtroot/readme.tmt. Edit that, then `tomet export .`. -->

# Nitium

Web browser powered by Servo and Slint, built with the Origiri Frameworks.

## Overview

Nitium is a modern, responsive desktop web browser that embeds Mozilla's Servo browser engine inside Slint UI with Vulkan/WGPU rendering integration.

## Workspace Layout

- `apps/desktop` (`nitium-desktop`) -- Desktop application binary wiring `nitium-shell` and `nitium-webview`.
- `apps/android` (`nitium-android`) -- Android application package wiring `nitium-shell` and Slint Android backend.
- `crates/nitium-core` (`nitium-core`) -- Pure Rust domain logic with zero Servo or Slint dependencies (address input, tabs, shortcuts, controls, bookmarks, history, session, find, zoom, IME).
- `crates/nitium-settings` (`nitium-settings`) -- User settings, search presets, validation, and storage.
- `crates/nitium-shell` (`nitium-shell`) -- Slint UI shell and `BrowserApp` state management (zero Servo dependency).
- `crates/nitium-webview` (`nitium-webview`) -- Servo 0.7 browser engine integration and WGPU/Surfman rendering bridge.

## Building and Running

### Prerequisites

- Rust stable (2024 edition, 1.88+)
- Wayland / X11 graphics libraries (`libxkbcommon`, `fontconfig`, `wayland`, `vulkan-loader`, `mesa`, `libGL`)
- Or use Nix: `nix develop`

### Run Application

```bash
cargo run -p nitium-desktop
```

### Android APK

```bash
just apk-arm  # Real devices (aarch64)
just apk-x86  # Emulator (x86_64)
```

### Fast Check & Tests (No Servo Build)

```bash
just check-fast
just test-fast
```

