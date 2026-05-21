//! Crepuscularity-powered Tile settings and about windows.

use crepuscularity_gpui::prelude::*;
use gpui::{
    point, px, size, uniform_list, App, Bounds, ClickEvent, DispatchPhase, Entity, KeyDownEvent,
    SharedString, TitlebarOptions, UniformListScrollHandle, Window, WindowBounds, WindowKind,
    WindowOptions,
};
use log::info;

use crate::config::{
    action_display_name, action_group, binding_from_keystroke, format_binding, TileConfig,
    TilingModeConfig,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TilePanel {
    Settings,
    About,
}

#[derive(Debug, Clone)]
struct BindingRow {
    action_name: String,
    display_name: String,
    group: &'static str,
    shortcut_text: String,
}

pub struct SettingsWindow {
    config: TileConfig,
    scroll_handle: UniformListScrollHandle,
    recording_action: Option<String>,
}

impl SettingsWindow {
    pub fn new(config: TileConfig) -> Self {
        Self {
            config,
            scroll_handle: UniformListScrollHandle::new(),
            recording_action: None,
        }
    }

    fn save_config(&self) {
        match self.config.save() {
            Ok(()) => info!("Settings saved"),
            Err(e) => log::error!("Failed to save config: {}", e),
        }
    }

    fn reset_defaults(&mut self, _: &ClickEvent, _window: &mut Window, cx: &mut Context<Self>) {
        self.config = TileConfig::default();
        self.recording_action = None;
        self.save_config();
        info!("Reset keybindings to defaults");
        cx.notify();
    }

    fn save(&mut self, _: &ClickEvent, _window: &mut Window, _cx: &mut Context<Self>) {
        self.save_config();
    }

    fn set_mode_snap(&mut self, _: &ClickEvent, _window: &mut Window, cx: &mut Context<Self>) {
        self.config.tiling_mode = TilingModeConfig::Snap;
        self.save_config();
        cx.notify();
    }

    fn set_mode_bsp(&mut self, _: &ClickEvent, _window: &mut Window, cx: &mut Context<Self>) {
        self.config.tiling_mode = TilingModeConfig::Bsp;
        self.save_config();
        cx.notify();
    }

    fn toggle_recording(
        &mut self,
        action_name: String,
        _: &ClickEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.recording_action.as_deref() == Some(action_name.as_str()) {
            self.recording_action = None;
        } else {
            self.recording_action = Some(action_name);
        }
        cx.notify();
    }

    fn handle_key_down(
        &mut self,
        event: &KeyDownEvent,
        phase: DispatchPhase,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !phase.bubble() {
            return;
        }

        let Some(action_name) = self.recording_action.clone() else {
            return;
        };

        cx.stop_propagation();

        if event.is_held {
            return;
        }

        if event.keystroke.key.eq_ignore_ascii_case("escape") {
            self.recording_action = None;
            cx.notify();
            return;
        }

        let Some(binding) = binding_from_keystroke(&event.keystroke) else {
            log::warn!("Unsupported key while recording binding: {:?}", event.keystroke);
            return;
        };

        self.config.set_binding(&action_name, binding);
        self.recording_action = None;
        self.save_config();
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let rows = build_rows(&self.config);
        let row_count = rows.len();
        let outer_gap = format!("{:.0}px", self.config.gap_outer);
        let inner_gap = format!("{:.0}px", self.config.gap_inner);
        let is_snap = self.config.tiling_mode == TilingModeConfig::Snap;
        let is_bsp = self.config.tiling_mode == TilingModeConfig::Bsp;
        let recording_action = self.recording_action.clone();
        let recording_action_for_list = recording_action.clone();
        let recording_action_for_banner = recording_action.clone();
        let entity = cx.entity();
        let key_entity = entity.clone();
        let list_entity = entity.clone();

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
            .bg(if is_snap {
                rgb(0x89b4fa)
            } else {
                rgb(0x27272a)
            })
            .text_color(if is_snap {
                rgb(0x1e1e2e)
            } else {
                rgb(0xa1a1aa)
            })
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

        if recording_action.is_some() {
            window.on_key_event(move |event: &KeyDownEvent, phase, window, app| {
                key_entity.update(app, |this, cx| this.handle_key_down(event, phase, window, cx));
            });
        }

        let list = uniform_list("keybindings", row_count, move |range, _window, _cx| {
            range
                .map(|ix| {
                    let row = &rows[ix];
                    render_row(
                        ix,
                        row,
                        recording_action_for_list.as_deref(),
                        list_entity.clone(),
                    )
                })
                .collect()
        })
        .track_scroll(self.scroll_handle.clone());

        let recording_banner = if let Some(action) = recording_action_for_banner.as_ref() {
            div()
                .px(px(12.0))
                .py(px(8.0))
                .rounded(px(8.0))
                .bg(rgb(0x3f2e14))
                .border_1()
                .border_color(rgb(0xe0a94f))
                .text_color(rgb(0xf5d08a))
                .text_size(px(12.0))
                .child(format!("Recording {action}. Press a key or Escape to cancel."))
        } else {
            div()
        };

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
                        "Snap: use hotkeys to position windows. BSP: all windows are auto-tiled in a persistent grid, with draggable dividers and Ctrl+Cmd drag to snap beside."
                    {recording_banner}

                div px-5 py-2 border-b border-zinc-800 flex text-xs uppercase tracking-widest text-zinc-500
                    div w-[110px]
                        "Group"
                    div flex-1
                        "Action"
                    div w-[220px] text-right
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
                    action_name: name.clone(),
                    display_name: action_display_name(name),
                    group,
                    shortcut_text: format_binding(binding),
                });
            }
        }
    }
    rows
}

fn render_row(
    ix: usize,
    row: &BindingRow,
    recording_action: Option<&str>,
    entity: Entity<SettingsWindow>,
) -> impl IntoElement {
    let bg = if ix.is_multiple_of(2) {
        rgb(0x12161d)
    } else {
        rgb(0x171b23)
    };
    let is_recording = recording_action == Some(row.action_name.as_str());
    let shortcut_text = if is_recording {
        "Press a key or Escape".to_string()
    } else {
        row.shortcut_text.clone()
    };
    let button_bg = if is_recording {
        rgb(0xe0a94f)
    } else {
        rgb(0x27272a)
    };
    let button_fg = if is_recording {
        rgb(0x1e1e2e)
    } else {
        rgb(0xe4e4e7)
    };
    let action_name = row.action_name.clone();

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
                .w(px(220.0))
                .flex()
                .justify_end()
                .child(
                    div()
                        .id(("binding", ix))
                        .px(px(8.0))
                        .py(px(2.0))
                        .bg(button_bg)
                        .rounded(px(4.0))
                        .cursor_pointer()
                        .text_size(px(12.0))
                        .text_color(button_fg)
                        .hover(|s| s.opacity(0.82))
                        .on_click(move |event: &ClickEvent, window, app| {
                            entity.update(app, |this, cx| {
                                this.toggle_recording(action_name.clone(), event, window, cx)
                            });
                        })
                        .child(SharedString::from(shortcut_text)),
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
