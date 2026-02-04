//! Top navigation bar
//!
//! Contains the logo, navigation tabs, search, and user profile.

use gpui::*;

use crate::state::{ActiveTab, AppState, AuthState};
use crate::theme;

/// Navbar view component
pub struct NavbarView {
    app_state: Entity<AppState>,
    auth_state: Entity<AuthState>,
}

impl NavbarView {
    /// Create a new navbar view
    pub fn new(app_state: Entity<AppState>, auth_state: Entity<AuthState>) -> Self {
        Self {
            app_state,
            auth_state,
        }
    }
}

impl Render for NavbarView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let auth = self.auth_state.read(cx);
        let app = self.app_state.read(cx);

        div()
            .id("navbar")
            .w_full()
            .h_full()
            .flex()
            .items_center()
            .px(px(16.0))
            .gap(px(16.0))
            // Logo
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        // Twitch-style logo placeholder
                        div()
                            .size(px(28.0))
                            .bg(theme::TWITCH_PURPLE)
                            .rounded(px(6.0)),
                    )
                    .child(
                        div()
                            .text_color(theme::TEXT_PRIMARY)
                            .text_size(px(18.0))
                            .font_weight(FontWeight::BOLD)
                            .child("Secousse"),
                    ),
            )
            // Navigation tabs
            .child(
                div()
                    .flex()
                    .gap(px(4.0))
                    .child(self.render_tab("Following", ActiveTab::Following, app.active_tab, cx))
                    .child(self.render_tab("Browse", ActiveTab::Browse, app.active_tab, cx)),
            )
            // Spacer
            .child(div().flex_1())
            // Search box placeholder
            .child(
                div()
                    .w(px(300.0))
                    .h(px(36.0))
                    .bg(theme::BG_INPUT)
                    .rounded(px(6.0))
                    .px(px(12.0))
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .text_color(theme::TEXT_DISABLED)
                            .text_size(px(14.0))
                            .child("Search"),
                    ),
            )
            // User section
            .child(if auth.is_logged_in {
                self.render_user_profile(auth, cx)
            } else {
                self.render_login_button(cx)
            })
    }
}

impl NavbarView {
    /// Render a navigation tab button
    fn render_tab(
        &self,
        label: &str,
        tab: ActiveTab,
        current_tab: ActiveTab,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_active = tab == current_tab;
        let app_state = self.app_state.clone();

        div()
            .id(SharedString::from(format!("tab-{:?}", tab)))
            .px(px(16.0))
            .py(px(8.0))
            .rounded(px(4.0))
            .cursor_pointer()
            .bg(if is_active {
                theme::BG_TERTIARY
            } else {
                gpui::transparent_black()
            })
            .hover(|style| style.bg(theme::BG_TERTIARY))
            .text_color(if is_active {
                theme::TEXT_PRIMARY
            } else {
                theme::TEXT_SECONDARY
            })
            .text_size(px(14.0))
            .font_weight(if is_active {
                FontWeight::SEMIBOLD
            } else {
                FontWeight::NORMAL
            })
            .on_click(move |_event, window, cx| {
                app_state.update(cx, |state, cx| {
                    state.set_tab(tab);
                    state.set_channel(None); // Clear channel when switching tabs
                    cx.notify();
                });
            })
            .child(label)
    }

    /// Render login button for unauthenticated users
    fn render_login_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let auth_state = self.auth_state.clone();

        div()
            .id("login-button")
            .px(px(16.0))
            .py(px(8.0))
            .bg(theme::TWITCH_PURPLE)
            .hover(|style| style.bg(theme::TWITCH_PURPLE_HOVER))
            .rounded(px(4.0))
            .cursor_pointer()
            .text_color(theme::TEXT_PRIMARY)
            .text_size(px(14.0))
            .font_weight(FontWeight::SEMIBOLD)
            .on_click(move |_event, window, cx| {
                // TODO: Trigger OAuth flow
                log::info!("Login button clicked - OAuth flow not implemented yet");
            })
            .child("Log In")
    }

    /// Render user profile for authenticated users
    fn render_user_profile(&self, auth: &AuthState, cx: &mut Context<Self>) -> impl IntoElement {
        let display_name = auth.display_name().unwrap_or("User").to_string();

        div()
            .id("user-profile")
            .flex()
            .items_center()
            .gap(px(8.0))
            .px(px(8.0))
            .py(px(4.0))
            .rounded(px(4.0))
            .cursor_pointer()
            .hover(|style| style.bg(theme::BG_TERTIARY))
            .child(
                // Avatar placeholder
                div()
                    .size(px(30.0))
                    .bg(theme::TWITCH_PURPLE)
                    .rounded_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(theme::TEXT_PRIMARY)
                    .text_size(px(12.0))
                    .font_weight(FontWeight::BOLD)
                    .child(display_name.chars().next().unwrap_or('U').to_string()),
            )
            .child(
                div()
                    .text_color(theme::TEXT_PRIMARY)
                    .text_size(px(14.0))
                    .child(display_name),
            )
    }
}
