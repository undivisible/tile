# Tile — Agent Instructions

## What is Tile

Tile is a standalone macOS tiling window manager built in Rust. It runs as a menu bar app and provides Rectangle-compatible keyboard shortcuts, BSP tiling, window stacking, and drag-to-snap zones.

## Architecture

```
tile (binary, src/)
├── main.rs     — app entry, accessibility permission check
├── app.rs      — TileApp, AppState, hotkey dispatch, status bar menu
└── drag.rs     — drag-to-snap mouse event monitoring

crates/
├── tile_core   — Node tree (BSP), layout computation, snap zone detection, types
├── tile_ax     — Accessibility API: window enumeration, move/resize, AX observers
├── tile_hotkeys — Carbon RegisterEventHotKey with Rectangle-compatible bindings
└── tile_overlay — NSWindow-based transparent overlay for snap zone preview
```

## Key Technical Decisions

- **objc2** (not old `objc`/`cocoa`) for all AppKit/Foundation access
- **Raw FFI** for Accessibility API (`ApplicationServices` framework) since objc2 doesn't wrap it
- **Raw FFI** for Carbon hotkeys (`Carbon` framework) — same approach as Rectangle/Amethyst
- **core-foundation** + **core-graphics** crates for CF/CG types needed by AX API
- Coordinate system: Accessibility API uses **top-left origin**; AppKit uses **bottom-left origin**. Conversions happen at the boundary (tile_ax::screen, tile_overlay).

## Build

```bash
cargo build -p tile          # debug
cargo build -p tile --release  # release
cargo test -p tile_core      # unit tests (no macOS APIs needed)
```

## Do Not

- Use the old `objc` or `cocoa` crates — use `objc2`, `objc2-foundation`, `objc2-app-kit`
- Use `open -a` to launch apps — use AppleScript or NSWorkspace
- Use `brew install` — use `wax install` for CLI tools
- Assume Accessibility API works without permissions — always check/prompt first
- Mix coordinate systems — AX = top-left, AppKit = bottom-left
