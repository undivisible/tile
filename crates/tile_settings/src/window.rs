//! Crepuscularity-powered Tile settings and about windows.

use crepuscularity_gpui::prelude::*;
use gpui::{
    point, px, size, uniform_list, App, Bounds, ClickEvent, SharedString,
    UniformListScrollHandle, Window, WindowBounds, WindowKind, WindowOptions,
    TitlebarOptions,
};
use log::info;

use crate::config::{
    action_display_name, action_group, format_binding, TileConfig, TilingModeConfig,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TilePanel {
    Settings,
    About,
}

#[derive(Debug, Clone)]
struct BindingRow {
    display_name: String,
    group: &'static str,
    shortcut_text: String,
}

pub struct SettingsWindow {
    config: TileConfig,
    rows: Vec<BindingRow>,
    scroll_handle: UniformListScrollHandle,
}

impl SettingsWindow {
    pub fn new(config: TileConfig) -> Self {
        Self {
            rows: build_rows(&config),
            config,
            scroll_handle: UniformListScrollHandle::new(),
        }
    }

    fn reset_defaults(&mut self, _: &ClickEvent, _window: &mut Window, cx: &mut Context<Self>) {
        self.config = TileConfig::default();
        self.rows = build_rows(&self.config);
        info!("Reset keybindings to defaults");
        cx.notify();
    }

    fn save(&mut self, _: &ClickEvent, _window: &mut Window, _cx: &mut Context<Self>) {
        match self.config.save() {
            Ok(()) => info!("Settings saved"),
            Err(e) => log::error!("Failed to save config: {}", e),
        }
    }

    fn set_mode_snap(&mut self, _: &ClickEvent, _window: &mut Window, cx: &mut Context<Self>) {
        self.config.tiling_mode = TilingModeConfig::Snap;
        cx.notify();
    }

    fn set_mode_bsp(&mut self, _: &ClickEvent, _window: &mut Window, cx: &mut Context<Self>) {
        self.config.tiling_mode = TilingModeConfig::Bsp;
        cx.notify();
    }
}

pub struct AboutWindow;

impl AboutWindow {
    pub fn new() -> Self {
        Self
    }
}

impl Render for AboutWindow {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let version = env!("CARGO_PKG_VERSION");

        view! {r#"
            div w-full h-full bg-zinc-950 text-zinc-100 flex flex-col p-8 gap-6
                div flex flex-col gap-2
                    div text-4xl font-bold tracking-tight
                        "Tile"
                    div text-sm text-zinc-400
                        "Version {version}"
                    div text-base text-zinc-300 leading-relaxed max-w-[560px]
                        "A macOS tiling window manager with Rectangle-style shortcuts, drag-to-snap overlays, and an in-progress multiplexer mode."

                div bg-zinc-900 border border-zinc-800 rounded-2xl p-5 flex flex-col gap-3
                    div text-xs uppercase tracking-widest text-zinc-500
                        "What works today"
                    div text-sm text-zinc-300
                        "Global hotkeys for halves, thirds, quarters, maximize, center, restore, display movement, and drag-based snap previews."
                    div text-sm text-zinc-300
                        "Persistent BSP management exists in the standalone app state, but some menu and settings polish is still catching up."

                div bg-zinc-900 border border-zinc-800 rounded-2xl p-5 flex flex-col gap-3
                    div text-xs uppercase tracking-widest text-zinc-500
                        "Permissions"
                    div text-sm text-zinc-300 leading-relaxed
                        "Tile needs Accessibility access to list windows, observe focus, and move or resize them. Enable it in System Settings > Privacy & Security > Accessibility."

                div text-xs text-zinc-500
                    "Built with Rust, GPUI, and Crepuscularity."
        "#}
    }
}

impl Render for SettingsWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let row_count = self.rows.len();
        let rows = self.rows.clone();
        let outer_gap = format!("{:.0}px", self.config.gap_outer);
        let inner_gap = format!("{:.0}px", self.config.gap_inner);
        let is_snap = self.config.tiling_mode == TilingModeConfig::Snap;
        let is_bsp  = self.config.tiling_mode == TilingModeConfig::Bsp;

        let save_button = div()
            .id("save-btn")
            .px(px(12.0))
            .py(px(4.0))
            .bg(rgb(0x89b4fa))
            .text_color(rgb(0x1e1e2e))
            .rounded(px(6.0))
            .cursor_pointer()
            .text_size(px(12.0))
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .hover(|s| s.opacity(0.8))
            .on_click(cx.listener(Self::save))
            .child("Save");
        let reset_button = div()
            .id("reset-btn")
            .px(px(12.0))
            .py(px(4.0))
            .bg(rgb(0x45475a))
            .rounded(px(6.0))
            .cursor_pointer()
            .text_size(px(12.0))
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .hover(|s| s.opacity(0.8))
            .on_click(cx.listener(Self::reset_defaults))
            .child("Reset Defaults");

        let snap_btn = div()
            .id("mode-snap")
            .px(px(14.0))
            .py(px(6.0))
            .bg(if is_snap { rgb(0x89b4fa) } else { rgb(0x27272a) })
            .text_color(if is_snap { rgb(0x1e1e2e) } else { rgb(0xa1a1aa) })
            .rounded(px(6.0))
            .cursor_pointer()
            .text_size(px(12.0))
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .hover(|s| s.opacity(0.8))
            .on_click(cx.listener(Self::set_mode_snap))
            .child("Snap");
        let bsp_btn = div()
            .id("mode-bsp")
            .px(px(14.0))
            .py(px(6.0))
            .bg(if is_bsp { rgb(0xa6e3a1) } else { rgb(0x27272a) })
            .text_color(if is_bsp { rgb(0x1e1e2e) } else { rgb(0xa1a1aa) })
            .rounded(px(6.0))
            .cursor_pointer()
            .text_size(px(12.0))
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .hover(|s| s.opacity(0.8))
            .on_click(cx.listener(Self::set_mode_bsp))
            .child("BSP");

        let list = uniform_list("keybindings", row_count, move |range, _window, _cx| {
            range
                .map(|ix| render_row(ix, &rows[ix]))
                .collect()
        })
        .track_scroll(self.scroll_handle.clone());

        view! {r#"
            div w-full h-full bg-zinc-950 text-zinc-100 flex flex-col
                div px-5 py-4 border-b border-zinc-800 flex items-center justify-between
                    div flex flex-col gap-1
                        div text-2xl font-bold tracking-tight
                            "Tile Settings"
                        div text-sm text-zinc-500
                            "Keybindings and layout defaults."
                    div flex gap-2
                        {reset_button}
                        {save_button}

                div px-5 py-4 border-b border-zinc-800 flex flex-col gap-3
                    div text-xs uppercase tracking-widest text-zinc-500
                        "Tiling Mode"
                    div flex gap-3 items-start
                        {snap_btn}
                        {bsp_btn}
                    div text-xs text-zinc-500 leading-relaxed max-w-[500px]
                        "Snap: use hotkeys to position windows. BSP: all windows are auto-tiled in a persistent grid — drag dividers to resize, Opt+Ctrl drag to snap beside. Restart Tile after changing."

                div px-5 py-2 border-b border-zinc-800 flex text-xs uppercase tracking-widest text-zinc-500
                    div w-[110px]
                        "Group"
                    div flex-1
                        "Action"
                    div w-[210px]
                        "Shortcut"

                div flex-1
                    {list}

                div px-5 py-4 border-t border-zinc-800 flex gap-4 text-sm text-zinc-400
                    div bg-zinc-900 border border-zinc-800 rounded-xl px-3 py-2
                        "Outer gap: {outer_gap}"
                    div bg-zinc-900 border border-zinc-800 rounded-xl px-3 py-2
                        "Inner gap: {inner_gap}"
        "#}
    }
}

fn build_rows(config: &TileConfig) -> Vec<BindingRow> {
    let group_order = [
        "Halves",
        "Thirds",
        "Two-Thirds",
        "Quarters",
        "Special",
        "Move Focus",
        "Swap Panes",
    ];

    let mut rows = Vec::new();
    for group in &group_order {
        for (name, binding) in &config.bindings {
            if action_group(name) == *group {
                rows.push(BindingRow {
                    display_name: action_display_name(name),
                    group,
                    shortcut_text: format_binding(binding),
                });
            }
        }
    }
    rows
}

fn render_row(ix: usize, row: &BindingRow) -> impl IntoElement {
    let bg = if ix.is_multiple_of(2) {
        rgb(0x12161d)
    } else {
        rgb(0x171b23)
    };

    div()
        .flex()
        .items_center()
        .px(px(20.0))
        .py(px(8.0))
        .bg(bg)
        .hover(|s| s.bg(rgb(0x202633)))
        .child(
            div()
                .w(px(110.0))
                .text_size(px(11.0))
                .text_color(rgb(0x71717a))
                .child(SharedString::from(row.group.to_string())),
        )
        .child(
            div()
                .flex_grow()
                .child(SharedString::from(row.display_name.clone())),
        )
        .child(
            div()
                .w(px(210.0))
                .child(
                    div()
                        .px(px(8.0))
                        .py(px(2.0))
                        .bg(rgb(0x27272a))
                        .rounded(px(4.0))
                        .text_size(px(12.0))
                        .child(SharedString::from(row.shortcut_text.clone())),
                ),
        )
}

pub fn open_panel_window(cx: &mut App, panel: TilePanel) {
    match panel {
        TilePanel::Settings => open_settings_window(cx),
        TilePanel::About => open_about_window(cx),
    }
}

pub fn open_settings_window(cx: &mut App) {
    let config = TileConfig::load();
    let bounds = Bounds::centered(None, size(px(760.0), px(640.0)), cx);

    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some("Tile Settings".into()),
                appears_transparent: true,
                traffic_light_position: Some(point(px(12.0), px(12.0))),
            }),
            kind: WindowKind::Normal,
            focus: true,
            is_movable: true,
            is_resizable: true,
            ..Default::default()
        },
        |_window, cx| cx.new(|_cx| SettingsWindow::new(config)),
    )
    .ok();
}

pub fn open_about_window(cx: &mut App) {
    let bounds = Bounds::centered(None, size(px(680.0), px(520.0)), cx);

    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some("About Tile".into()),
                appears_transparent: true,
                traffic_light_position: Some(point(px(12.0), px(12.0))),
            }),
            kind: WindowKind::Normal,
            focus: true,
            is_movable: true,
            is_resizable: false,
            ..Default::default()
        },
        |_window, cx| cx.new(|_cx| AboutWindow::new()),
    )
    .ok();
}
