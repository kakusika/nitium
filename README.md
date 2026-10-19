<!-- Generated from tmtroot/readme.tmt. Edit that, then `tomet export .`. -->

# Nitium

Web browser powered by Servo and Slint, built with the Origiri Frameworks.

## Overview

Nitium is a modern, responsive desktop web browser that embeds Mozilla's Servo browser engine inside Slint UI with Vulkan/WGPU rendering integration.

## Workspace Layout

- `apps/nitium` (`nitium`) -- Desktop web browser application.
- `crates/nitium-core` (`nitium-core`) -- Servo-free browser logic (address input, settings, bookmarks, history, session, find, zoom, IME).
- `crates/nitium-webview` (`nitium-webview`) -- Servo browser engine integration and Slint widget bindings.

## Building and Running

### Prerequisites

- Rust stable (2024 edition, 1.85+)
- Wayland / X11 graphics libraries (`libxkbcommon`, `fontconfig`, `wayland`, `vulkan-loader`, `mesa`, `libGL`)
- Or use Nix: `nix develop`

### Run Application

```bash
cargo run -p nitium
```

### Run Tests

```bash
cargo test -p nitium-core
```

