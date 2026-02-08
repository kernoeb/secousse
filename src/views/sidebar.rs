//! Left sidebar
//!
//! Contains the followed channels list and collapse toggle.

use crate::state::{AppState, FollowedChannel};
use crate::theme;
use gpui::prelude::{FluentBuilder, StyledImage};
use gpui::*;
use gpui_component::button::{Button, ButtonCustomVariant, ButtonVariants};
use gpui_component::{IconName, Sizable};

/// Events emitted by the sidebar
#[derive(Clone, Debug)]
pub enum SidebarEvent {
    /// User clicked on a channel
    ChannelSelected(String),
}

/// Sidebar view component
pub struct SidebarView {
    app_state: Entity<AppState>,
}

impl EventEmitter<SidebarEvent> for SidebarView {}

impl SidebarView {
    /// Create a new sidebar view
    pub fn new(app_state: Entity<AppState>) -> Self {
        Self { app_state }
    }
}

impl Render for SidebarView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Extract all values needed upfront to avoid borrow issues
        let is_open = self.app_state.read(cx).is_sidebar_open;
        let followed = self.app_state.read(cx).followed_channels.clone();
        let current_channel = self.app_state.read(cx).current_channel.clone();

        // Build channel items list (need to do this outside the closure to access cx)
        let channel_items: Vec<AnyElement> = followed
            .iter()
            .map(|channel| {
                self.render_channel_item(
                    channel,
                    is_open,
                    current_channel.as_deref() == Some(&channel.login),
                    cx,
                )
                .into_any_element()
            })
            .collect();

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
                    .when(!is_open, |el: Div| el.justify_center().px(px(0.0)))
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
            .child(div().id("channel-list").flex_1().overflow_y_scroll().child(
                if followed.is_empty() {
                    if is_open {
                        self.render_empty_state().into_any_element()
                    } else {
                        div().into_any_element()
                    }
                } else {
                    div()
                        .flex()
                        .flex_col()
                        .children(channel_items)
                        .into_any_element()
                },
            ))
    }
}

impl SidebarView {
    /// Render the collapse/expand button
    fn render_collapse_button(&self, is_open: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let app_state = self.app_state.clone();
        let hover_variant = ButtonCustomVariant::new(cx)
            .color(theme::TRANSPARENT.into())
            .foreground(theme::TEXT_SECONDARY.into())
            .border(theme::TRANSPARENT.into())
            .hover(theme::BG_TERTIARY.into())
            .active(theme::BG_ELEVATED.into());

        Button::new("sidebar-toggle")
            .custom(hover_variant)
            .xsmall()
            .rounded(px(2.0))
            .icon(if is_open {
                IconName::PanelLeft
            } else {
                IconName::PanelRight
            })
            .on_click(move |_event, _window, cx| {
                app_state.update(cx, |state, cx| {
                    state.toggle_sidebar();
                    cx.notify();
                });
            })
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
        let login = channel.login.clone();
        let display_name = channel.display_name.clone();
        let is_live = channel.is_live;
        let viewer_count = channel.viewer_count;
        let game_name = channel.game_name.clone();
        let first_char = display_name.chars().next().unwrap_or('?').to_string();
        let avatar_url = channel.profile_image_url.clone();

        let mut item = div()
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
                theme::TRANSPARENT
            })
            .hover(|style| style.bg(theme::BG_TERTIARY))
            .on_click(cx.listener(move |_this, _event, _window, cx| {
                cx.emit(SidebarEvent::ChannelSelected(login.clone()));
            }));

        // Avatar with optional live indicator
        let mut avatar = if let Some(url) = avatar_url {
            div()
                .size(px(30.0))
                .rounded_full()
                .overflow_hidden()
                .bg(theme::BG_TERTIARY)
                .flex_shrink_0()
                .child(
                    img(url)
                        .w_full()
                        .h_full()
                        .object_fit(ObjectFit::Cover)
                        .rounded_full(),
                )
        } else {
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
                .child(first_char)
        };

        if is_live {
            avatar = avatar.relative().child(
                div()
                    .absolute()
                    .bottom(px(-2.0))
                    .right(px(-2.0))
                    .size(px(10.0))
                    .rounded_full()
                    .bg(theme::LIVE_RED)
                    .border_2()
                    .border_color(theme::BG_SECONDARY),
            );
        }

        item = item.child(avatar);

        // Channel info (only when expanded)
        if is_open {
            let mut info = div()
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
                );

            // Game name
            if let Some(game) = game_name {
                info = info.child(
                    div()
                        .text_color(theme::TEXT_SECONDARY)
                        .text_size(px(12.0))
                        .text_ellipsis()
                        .child(game),
                );
            }

            item = item.child(info);

            // Viewer count (only if live)
            if is_live && let Some(count) = viewer_count {
                item = item.child(
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
                );
            }
        }

        item
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
