//! Top navigation bar
//!
//! Contains the logo, navigation tabs, search, and user profile.

use gpui::*;

use crate::components::icon::{Icon, IconName};
use crate::components::text_input::{TextInput, TextInputEvent, TextInputState};
use crate::state::{ActiveTab, AppState, AuthState};
use crate::theme;

/// Events emitted by the navbar
#[derive(Clone)]
pub enum NavbarEvent {
    LoginRequested,
    LogoutRequested,
    TabChanged(ActiveTab),
    SearchSubmitted(String),
    SearchCleared,
}

impl EventEmitter<NavbarEvent> for NavbarView {}

/// Navbar view component
pub struct NavbarView {
    app_state: Entity<AppState>,
    auth_state: Entity<AuthState>,
    search_input: Entity<TextInputState>,
}

impl NavbarView {
    /// Create a new navbar view
    pub fn new(
        app_state: Entity<AppState>,
        auth_state: Entity<AuthState>,
        cx: &mut Context<Self>,
    ) -> Self {
        let search_input = cx.new(|cx| TextInputState::new(cx));

        // Subscribe to text input events
        cx.subscribe(&search_input, |this, _input, event: &TextInputEvent, cx| {
            match event {
                TextInputEvent::Submit(text) => {
                    if !text.is_empty() {
                        log::info!("Submitting search: {}", text);
                        cx.emit(NavbarEvent::SearchSubmitted(text.clone()));
                    }
                }
                TextInputEvent::Escape => {
                    // Clear search on escape
                    this.search_input.update(cx, |state, cx| {
                        state.set_text("", cx);
                    });
                    cx.emit(NavbarEvent::SearchCleared);
                }
                TextInputEvent::Change(text) => {
                    if text.is_empty() {
                        cx.emit(NavbarEvent::SearchCleared);
                    }
                }
            }
        })
        .detach();

        Self {
            app_state,
            auth_state,
            search_input,
        }
    }

    /// Clear search
    fn clear_search(&mut self, cx: &mut Context<Self>) {
        self.search_input.update(cx, |state, cx| {
            state.set_text("", cx);
        });
        cx.emit(NavbarEvent::SearchCleared);
        cx.notify();
    }
}

impl Render for NavbarView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Extract all values needed upfront to avoid borrow issues
        let is_logged_in = self.auth_state.read(cx).is_logged_in;
        let display_name = self
            .auth_state
            .read(cx)
            .display_name()
            .map(|s| s.to_string());
        let active_tab = self.app_state.read(cx).active_tab;

        div()
            .id("navbar")
            .w_full()
            .h_full()
            .flex()
            .items_center()
            // Left padding accounts for macOS traffic lights (close/minimize/zoom buttons)
            .pl(px(theme::TRAFFIC_LIGHT_INSET))
            .pr(px(12.0))
            .gap(px(12.0))
            // Double-click on navbar = macOS titlebar double-click (zoom/minimize per system pref)
            .on_click(|event, window, _cx| {
                if event.click_count() == 2 {
                    window.titlebar_double_click();
                }
            })
            // Logo
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .child(
                        svg()
                            .path("icons/app-icon.svg")
                            .size(px(22.0))
                            .flex_none()
                            .text_color(theme::TWITCH_PURPLE),
                    )
                    .child(
                        div()
                            .text_color(theme::TEXT_PRIMARY)
                            .text_size(px(15.0))
                            .font_weight(FontWeight::BOLD)
                            .child("Secousse"),
                    ),
            )
            // Navigation tabs
            .child(
                div()
                    .flex()
                    .gap(px(4.0))
                    .child(self.render_tab("Following", ActiveTab::Following, active_tab, cx))
                    .child(self.render_tab("Browse", ActiveTab::Browse, active_tab, cx)),
            )
            // Spacer
            .child(div().flex_1())
            // Search box
            .child(self.render_search_input(window, cx))
            // User section
            .child(if is_logged_in {
                self.render_user_menu(display_name, cx).into_any_element()
            } else {
                self.render_login_button(cx).into_any_element()
            })
    }
}

impl NavbarView {
    /// Render a navigation tab button
    fn render_tab(
        &self,
        label: &'static str,
        tab: ActiveTab,
        current_tab: ActiveTab,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_active = tab == current_tab;
        let _app_state = self.app_state.clone();

        div()
            .id(SharedString::from(format!("tab-{:?}", tab)))
            .px(px(8.0))
            .py(px(3.0))
            .rounded(px(4.0))
            .cursor_pointer()
            .bg(if is_active {
                theme::BG_TERTIARY
            } else {
                theme::TRANSPARENT
            })
            .hover(|style| style.bg(theme::BG_TERTIARY))
            .text_color(if is_active {
                theme::TEXT_PRIMARY
            } else {
                theme::TEXT_SECONDARY
            })
            .text_size(px(12.0))
            .font_weight(if is_active {
                FontWeight::SEMIBOLD
            } else {
                FontWeight::NORMAL
            })
            .on_click(cx.listener(move |this, _event, _window, cx| {
                this.app_state.update(cx, |state, cx| {
                    state.set_tab(tab);
                    state.set_channel(None); // Clear channel when switching tabs
                    cx.notify();
                });
                // Emit tab changed event so the app can fetch data if needed
                cx.emit(NavbarEvent::TabChanged(tab));
            }))
            .child(label)
    }

    /// Render the search input
    fn render_search_input(
        &self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let has_text = !self.search_input.read(cx).text().is_empty();

        div().w(px(240.0)).flex().items_center().gap(px(0.0)).child(
            TextInput::new(&self.search_input)
                .placeholder("Search...")
                .prefix(
                    Icon::new(IconName::Search)
                        .size_3p5()
                        .color(theme::TEXT_DISABLED),
                )
                .suffix(if has_text {
                    div()
                        .id("search-clear")
                        .size(px(16.0))
                        .rounded_full()
                        .bg(theme::BG_TERTIARY)
                        .hover(|style| style.bg(theme::BG_ELEVATED))
                        .cursor_pointer()
                        .flex()
                        .items_center()
                        .justify_center()
                        .on_click(cx.listener(|this, _event, _window, cx| {
                            this.clear_search(cx);
                        }))
                        .child(Icon::new(IconName::X).size_3().color(theme::TEXT_SECONDARY))
                        .into_any_element()
                } else {
                    div().into_any_element()
                }),
        )
    }

    /// Render login button for unauthenticated users
    fn render_login_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("login-button")
            .px(px(12.0))
            .py(px(4.0))
            .bg(theme::TWITCH_PURPLE)
            .hover(|style| style.bg(theme::TWITCH_PURPLE_HOVER))
            .rounded(px(4.0))
            .cursor_pointer()
            .text_color(theme::TEXT_PRIMARY)
            .text_size(px(12.0))
            .font_weight(FontWeight::SEMIBOLD)
            .on_click(cx.listener(|_this, _event, _window, cx| {
                log::info!("Login button clicked, emitting LoginRequested event");
                cx.emit(NavbarEvent::LoginRequested);
            }))
            .flex()
            .items_center()
            .gap(px(6.0))
            .child(
                Icon::new(IconName::LogIn)
                    .size_4()
                    .color(theme::TEXT_PRIMARY),
            )
            .child("Log In")
    }

    /// Render user menu for authenticated users
    fn render_user_menu(
        &self,
        display_name: Option<String>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let name = display_name.unwrap_or_else(|| "User".to_string());
        let first_char = name.chars().next().unwrap_or('U').to_string();

        div()
            .id("user-profile")
            .flex()
            .items_center()
            .gap(px(6.0))
            .child(
                // User profile button
                div()
                    .id("user-button")
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .px(px(6.0))
                    .py(px(2.0))
                    .rounded(px(4.0))
                    .cursor_pointer()
                    .hover(|style| style.bg(theme::BG_TERTIARY))
                    .child(
                        // Avatar
                        div()
                            .size(px(22.0))
                            .bg(theme::TWITCH_PURPLE)
                            .rounded_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(theme::TEXT_PRIMARY)
                            .text_size(px(10.0))
                            .font_weight(FontWeight::BOLD)
                            .child(first_char),
                    )
                    .child(
                        div()
                            .text_color(theme::TEXT_PRIMARY)
                            .text_size(px(12.0))
                            .child(name),
                    ),
            )
            .child(
                // Logout button
                div()
                    .id("logout-button")
                    .px(px(10.0))
                    .py(px(3.0))
                    .bg(theme::BG_TERTIARY)
                    .hover(|style| style.bg(theme::BG_ELEVATED))
                    .rounded(px(4.0))
                    .cursor_pointer()
                    .text_color(theme::TEXT_SECONDARY)
                    .text_size(px(11.0))
                    .on_click(cx.listener(|_this, _event, _window, cx| {
                        log::info!("Logout button clicked, emitting LogoutRequested event");
                        cx.emit(NavbarEvent::LogoutRequested);
                    }))
                    .child("Logout"),
            )
    }
}
