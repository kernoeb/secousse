//! Left sidebar
//!
//! Contains the followed channels list and collapse toggle.

use gpui::*;

use crate::state::{AppState, FollowedChannel};
use crate::theme;

/// Sidebar view component
pub struct SidebarView {
    app_state: Entity<AppState>,
}

impl SidebarView {
    /// Create a new sidebar view
    pub fn new(app_state: Entity<AppState>) -> Self {
        Self { app_state }
    }
}

impl Render for SidebarView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let app = self.app_state.read(cx);
        let is_open = app.is_sidebar_open;
        let followed = &app.followed_channels;
        let current_channel = &app.current_channel;

        div()
            .id("sidebar")
            .w_full()
            .h_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            // Header with toggle
            .child(
                div()
                    .h(px(42.0))
                    .px(px(if is_open { 10.0 } else { 8.0 }))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(if is_open {
                        div()
                            .text_color(theme::TEXT_PRIMARY)
                            .text_size(px(14.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("FOLLOWED CHANNELS")
                            .into_any_element()
                    } else {
                        div().into_any_element()
                    })
                    .child(self.render_collapse_button(is_open, cx)),
            )
            // Channel list
            .child(
                div()
                    .flex_1()
                    .overflow_y_scroll()
                    .child(if followed.is_empty() {
                        if is_open {
                            self.render_empty_state().into_any_element()
                        } else {
                            div().into_any_element()
                        }
                    } else {
                        div()
                            .flex()
                            .flex_col()
                            .children(followed.iter().map(|channel| {
                                self.render_channel_item(
                                    channel,
                                    is_open,
                                    current_channel.as_deref() == Some(&channel.login),
                                    cx,
                                )
                            }))
                            .into_any_element()
                    }),
            )
    }
}

impl SidebarView {
    /// Render the collapse/expand button
    fn render_collapse_button(&self, is_open: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let app_state = self.app_state.clone();

        div()
            .id("sidebar-toggle")
            .size(px(30.0))
            .rounded(px(4.0))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .hover(|style| style.bg(theme::BG_TERTIARY))
            .text_color(theme::TEXT_SECONDARY)
            .text_size(px(16.0))
            .on_click(move |_event, window, cx| {
                app_state.update(cx, |state, cx| {
                    state.toggle_sidebar();
                    cx.notify();
                });
            })
            .child(if is_open { "<" } else { ">" })
    }

    /// Render empty state when no followed channels
    fn render_empty_state(&self) -> impl IntoElement {
        div().px(px(10.0)).py(px(16.0)).child(
            div()
                .text_color(theme::TEXT_SECONDARY)
                .text_size(px(13.0))
                .child("Log in to see your followed channels"),
        )
    }

    /// Render a single channel item
    fn render_channel_item(
        &self,
        channel: &FollowedChannel,
        is_open: bool,
        is_selected: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let app_state = self.app_state.clone();
        let login = channel.login.clone();
        let display_name = channel.display_name.clone();
        let is_live = channel.is_live;
        let viewer_count = channel.viewer_count;
        let game_name = channel.game_name.clone();

        div()
            .id(SharedString::from(format!("channel-{}", login)))
            .w_full()
            .px(px(if is_open { 10.0 } else { 8.0 }))
            .py(px(5.0))
            .flex()
            .items_center()
            .gap(px(10.0))
            .cursor_pointer()
            .bg(if is_selected {
                theme::SELECTED_BG
            } else {
                gpui::transparent_black()
            })
            .hover(|style| style.bg(theme::BG_TERTIARY))
            .on_click(move |_event, window, cx| {
                let login = login.clone();
                app_state.update(cx, |state, cx| {
                    state.set_channel(Some(login));
                    cx.notify();
                });
            })
            // Avatar
            .child(
                div()
                    .size(px(30.0))
                    .rounded_full()
                    .bg(theme::TWITCH_PURPLE)
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(theme::TEXT_PRIMARY)
                    .text_size(px(12.0))
                    .font_weight(FontWeight::BOLD)
                    .child(display_name.chars().next().unwrap_or('?').to_string())
                    // Live indicator
                    .when(is_live, |this| {
                        this.relative().child(
                            div()
                                .absolute()
                                .bottom(px(-2.0))
                                .right(px(-2.0))
                                .size(px(10.0))
                                .rounded_full()
                                .bg(theme::LIVE_RED)
                                .border_2()
                                .border_color(theme::BG_SECONDARY),
                        )
                    }),
            )
            // Channel info (only when expanded)
            .when(is_open, |this| {
                this.child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .overflow_hidden()
                        // Channel name
                        .child(
                            div()
                                .text_color(theme::TEXT_PRIMARY)
                                .text_size(px(14.0))
                                .font_weight(FontWeight::MEDIUM)
                                .text_ellipsis()
                                .child(display_name),
                        )
                        // Game name
                        .when_some(game_name, |this, game| {
                            this.child(
                                div()
                                    .text_color(theme::TEXT_SECONDARY)
                                    .text_size(px(12.0))
                                    .text_ellipsis()
                                    .child(game),
                            )
                        }),
                )
                // Viewer count
                .when_some(viewer_count.filter(|_| is_live), |this, count| {
                    this.child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .child(div().size(px(8.0)).rounded_full().bg(theme::LIVE_RED))
                            .child(
                                div()
                                    .text_color(theme::TEXT_SECONDARY)
                                    .text_size(px(12.0))
                                    .child(format_viewer_count(count)),
                            ),
                    )
                })
            })
    }
}

/// Format viewer count for display (e.g., 1.2K, 45.3K)
fn format_viewer_count(count: u32) -> String {
    if count >= 1_000_000 {
        format!("{:.1}M", count as f64 / 1_000_000.0)
    } else if count >= 1_000 {
        format!("{:.1}K", count as f64 / 1_000.0)
    } else {
        count.to_string()
    }
}
