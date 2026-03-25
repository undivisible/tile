# Tile

A macOS tiling window manager with stacking, Rectangle-compatible keybinds, and drag-to-snap zones.

## Features

- **Rectangle-compatible keyboard shortcuts** — Ctrl+Opt+Arrow for halves, Ctrl+Opt+U/I/J/K for quarters, and more
- **Size cycling** — Press the same shortcut repeatedly to cycle through 1/2, 2/3, 1/3 sizes
- **BSP tiling tree** — Binary space partitioning layout with configurable gaps
- **Window stacking** — Tab-style stacking of multiple windows in a single pane
- **Drag-to-snap** — Drag windows to screen edges or existing panes to tile them
- **Snap zone overlays** — Visual feedback showing where windows will be placed
- **Menu bar app** — Runs silently in the menu bar with no dock icon

## Keyboard Shortcuts

### Halves
| Shortcut | Action |
|----------|--------|
| Ctrl+Opt+Left | Left Half |
| Ctrl+Opt+Right | Right Half |
| Ctrl+Opt+Up | Top Half |
| Ctrl+Opt+Down | Bottom Half |

### Thirds
| Shortcut | Action |
|----------|--------|
| Ctrl+Opt+D | Left Third |
| Ctrl+Opt+F | Center Third |
| Ctrl+Opt+G | Right Third |

### Two-Thirds
| Shortcut | Action |
|----------|--------|
| Ctrl+Opt+E | Left Two-Thirds |
| Ctrl+Opt+R | Center Two-Thirds |
| Ctrl+Opt+T | Right Two-Thirds |

### Quarters
| Shortcut | Action |
|----------|--------|
| Ctrl+Opt+U | Top-Left Quarter |
| Ctrl+Opt+I | Top-Right Quarter |
| Ctrl+Opt+J | Bottom-Left Quarter |
| Ctrl+Opt+K | Bottom-Right Quarter |

### Special
| Shortcut | Action |
|----------|--------|
| Ctrl+Opt+Return | Maximize |
| Ctrl+Opt+C | Center (60% of screen) |
| Ctrl+Opt+Backspace | Restore original position |
| Ctrl+Opt+= | Equalize all splits |
| Ctrl+Opt+Z | Toggle zoom |

### Tiling
| Shortcut | Action |
|----------|--------|
| Ctrl+Opt+Shift+Arrow | Move window to adjacent pane |
| Ctrl+Opt+Cmd+Arrow | Swap panes |

## Architecture

```
tile/
├── src/
│   ├── main.rs         — app entry, permission check, run loop
│   ├── app.rs          — TileApp, AppState, hotkey dispatch, status bar
│   └── drag.rs         — drag-to-snap monitor
├── crates/
│   ├── tile_core/      — zone tree (BSP), layout algorithms, snap detection, types
│   ├── tile_ax/        — Accessibility API (window list, move, resize, observe)
│   ├── tile_hotkeys/   — Carbon global hotkeys with Rectangle bindings
│   └── tile_overlay/   — NSWindow-based snap zone overlay rendering
```

## Building

```bash
cargo build --release
```

Requires:
- macOS 13+ (Ventura or later)
- Xcode Command Line Tools
- Accessibility permissions (prompted on first run)

## Permissions

Tile uses the macOS Accessibility API to move and resize windows. On first launch, it will prompt you to grant Accessibility access in **System Settings > Privacy & Security > Accessibility**.

## License

MIT
