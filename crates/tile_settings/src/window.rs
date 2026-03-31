//! GPUI-based settings window for keybind customization.

use gpui::prelude::*;
use gpui::{
    div, point, px, rgb, size, uniform_list, App, Bounds, SharedString,
    UniformListScrollHandle, Window, WindowBounds, WindowKind, WindowOptions,
    TitlebarOptions,
};
use log::info;

use crate::config::{
    action_display_name, action_group, format_binding, TileConfig,
};

/// A row in the keybinding table.
#[derive(Debug, Clone)]
struct BindingRow {
    display_name: String,
    group: &'static str,
    shortcut_text: String,
}

/// The settings window state.
pub struct SettingsWindow {
    config: TileConfig,
    rows: Vec<BindingRow>,
    scroll_handle: UniformListScrollHandle,
    selected_row: Option<usize>,
}

impl SettingsWindow {
    pub fn new(config: TileConfig) -> Self {
        let rows = build_rows(&config);
        Self {
            config,
            rows,
            scroll_handle: UniformListScrollHandle::new(),
            selected_row: None,
        }
    }

    fn rebuild_rows(&mut self) {
        self.rows = build_rows(&self.config);
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

impl Render for SettingsWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let row_count = self.rows.len();
        let selected = self.selected_row;
        let rows = self.rows.clone();
        let entity = cx.entity().clone();

        div()
            .flex()
            .flex_col()
            .size_full()
            .bg(rgb(0x1e1e2e))
            .text_color(rgb(0xcdd6f4))
            .text_size(px(13.0))
            .child(
                // Title bar
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px(px(16.0))
                    .py(px(12.0))
                    .border_b_1()
                    .border_color(rgb(0x313244))
                    .child(
                        div()
                            .text_size(px(16.0))
                            .font_weight(gpui::FontWeight::BOLD)
                            .child("Tile Settings"),
                    )
                    .child(
                        div()
                            .flex()
                            .gap(px(8.0))
                            .child({
                                let entity = entity.clone();
                                div()
                                    .id("reset-btn")
                                    .px(px(12.0))
                                    .py(px(4.0))
                                    .bg(rgb(0x45475a))
                                    .rounded(px(6.0))
                                    .cursor_pointer()
                                    .text_size(px(12.0))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .hover(|s| s.opacity(0.8))
                                    .on_click(move |_event, _window, cx| {
                                        entity.update(cx, |this, cx| {
                                            this.config = TileConfig::default();
                                            this.rebuild_rows();
                                            this.selected_row = None;
                                            info!("Reset keybindings to defaults");
                                            cx.notify();
                                        });
                                    })
                                    .child("Reset Defaults")
                            })
                            .child({
                                let entity = entity.clone();
                                div()
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
                                    .on_click(move |_event, _window, cx| {
                                        entity.update(cx, |this, _cx| {
                                            match this.config.save() {
                                                Ok(()) => info!("Settings saved"),
                                                Err(e) => log::error!("Failed to save: {}", e),
                                            }
                                        });
                                    })
                                    .child("Save")
                            }),
                    ),
            )
            .child(
                // Hint
                div()
                    .px(px(16.0))
                    .py(px(8.0))
                    .text_size(px(12.0))
                    .text_color(rgb(0x6c7086))
                    .child("Click a shortcut to change it. Changes require restart to take effect."),
            )
            .child(
                // Table header
                div()
                    .flex()
                    .px(px(16.0))
                    .py(px(6.0))
                    .border_b_1()
                    .border_color(rgb(0x313244))
                    .text_size(px(11.0))
                    .text_color(rgb(0x6c7086))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(div().w(px(100.0)).child("Group"))
                    .child(div().flex_grow().child("Action"))
                    .child(div().w(px(200.0)).child("Shortcut")),
            )
            .child(
                // Scrollable list
                div().flex_grow().child(
                    uniform_list(
                        "keybindings",
                        row_count,
                        move |range, _window, _cx| {
                            range
                                .map(|ix| {
                                    let row = &rows[ix];
                                    let is_selected = selected == Some(ix);
                                    render_row(ix, row, is_selected)
                                })
                                .collect()
                        },
                    )
                    .track_scroll(self.scroll_handle.clone()),
                ),
            )
            .child(
                // Gap settings
                div()
                    .flex()
                    .items_center()
                    .gap(px(16.0))
                    .px(px(16.0))
                    .py(px(12.0))
                    .border_t_1()
                    .border_color(rgb(0x313244))
                    .text_size(px(12.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .child("Outer Gap:")
                            .child(
                                div()
                                    .px(px(6.0))
                                    .py(px(2.0))
                                    .bg(rgb(0x313244))
                                    .rounded(px(4.0))
                                    .child(SharedString::from(format!("{:.0}px", self.config.gap_outer))),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .child("Inner Gap:")
                            .child(
                                div()
                                    .px(px(6.0))
                                    .py(px(2.0))
                                    .bg(rgb(0x313244))
                                    .rounded(px(4.0))
                                    .child(SharedString::from(format!("{:.0}px", self.config.gap_inner))),
                            ),
                    ),
            )
    }
}

fn render_row(ix: usize, row: &BindingRow, is_selected: bool) -> impl IntoElement {
    let bg = if is_selected {
        rgb(0x45475a)
    } else if ix.is_multiple_of(2) {
        rgb(0x1e1e2e)
    } else {
        rgb(0x181825)
    };

    let shortcut_bg = if is_selected {
        rgb(0x585b70)
    } else {
        rgb(0x313244)
    };

    div()
        .id(("row", ix))
        .flex()
        .items_center()
        .px(px(16.0))
        .py(px(4.0))
        .bg(bg)
        .hover(|s| s.bg(rgb(0x313244)))
        .child(
            div()
                .w(px(100.0))
                .text_size(px(11.0))
                .text_color(rgb(0x6c7086))
                .child(SharedString::from(row.group.to_string())),
        )
        .child(
            div()
                .flex_grow()
                .child(SharedString::from(row.display_name.clone())),
        )
        .child(
            div()
                .w(px(200.0))
                .child(
                    div()
                        .px(px(8.0))
                        .py(px(2.0))
                        .bg(shortcut_bg)
                        .rounded(px(4.0))
                        .text_size(px(12.0))
                        .cursor_pointer()
                        .child(SharedString::from(row.shortcut_text.clone())),
                ),
        )
}

/// Open the settings window. Call from the GPUI App::run closure.
pub fn open_settings_window(cx: &mut App) {
    let config = TileConfig::load();
    let bounds = Bounds::centered(None, size(px(600.0), px(500.0)), cx);

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
