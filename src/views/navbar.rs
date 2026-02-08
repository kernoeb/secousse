//! Top navigation bar
//!
//! Contains the logo, navigation tabs, search, and user profile.

use crate::state::{ActiveTab, AppState, AuthState};
use crate::theme;
use gpui::*;
use gpui_component::button::{Button, ButtonCustomVariant, ButtonVariants};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::{Icon, IconName, Sizable};

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
    search_input: Entity<InputState>,
}

impl NavbarView {
    /// Create a new navbar view
    pub fn new(
        app_state: Entity<AppState>,
        auth_state: Entity<AuthState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Search...")
                .clean_on_escape()
        });

        // Subscribe to input events
        cx.subscribe_in(
            &search_input,
            window,
            |_this, state, event: &InputEvent, _window, cx| match event {
                InputEvent::PressEnter { .. } => {
                    let text = state.read(cx).value().to_string();
                    if !text.is_empty() {
                        log::info!("Submitting search: {}", text);
                        cx.emit(NavbarEvent::SearchSubmitted(text));
                    }
                }
                InputEvent::Change => {
                    let text = state.read(cx).value();
                    if text.is_empty() {
                        cx.emit(NavbarEvent::SearchCleared);
                    }
                }
                InputEvent::Focus | InputEvent::Blur => {}
            },
        )
        .detach();

        Self {
            app_state,
            auth_state,
            search_input,
        }
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
        let is_following_selected = matches!(active_tab, ActiveTab::Following);
        let is_browse_selected = matches!(active_tab, ActiveTab::Browse);

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
            .child(self.render_tab_buttons(is_following_selected, is_browse_selected, cx))
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
    fn render_tab_buttons(
        &self,
        is_following_selected: bool,
        is_browse_selected: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let tab_variant = ButtonCustomVariant::new(cx)
            .color(theme::TRANSPARENT.into())
            .foreground(theme::TEXT_SECONDARY.into())
            .border(theme::TRANSPARENT.into())
            .hover(theme::BG_TERTIARY.into())
            .active(theme::BG_ELEVATED.into());
        let selected_variant = ButtonCustomVariant::new(cx)
            .color(theme::BG_TERTIARY.into())
            .foreground(theme::TEXT_PRIMARY.into())
            .border(theme::TRANSPARENT.into())
            .hover(theme::BG_ELEVATED.into())
            .active(theme::BG_ELEVATED.into());

        let following_variant = if is_following_selected {
            selected_variant
        } else {
            tab_variant
        };
        let browse_variant = if is_browse_selected {
            selected_variant
        } else {
            tab_variant
        };

        div()
            .flex()
            .items_center()
            .gap(px(6.0))
            .child(
                Button::new("tab-following")
                    .custom(following_variant)
                    .xsmall()
                    .rounded(px(2.0))
                    .label("Following")
                    .on_click(cx.listener(|this, _event, _window, cx| {
                        let tab = ActiveTab::Following;
                        this.app_state.update(cx, |state, cx| {
                            state.set_tab(tab);
                            state.set_channel(None);
                            cx.notify();
                        });
                        cx.emit(NavbarEvent::TabChanged(tab));
                    })),
            )
            .child(
                Button::new("tab-browse")
                    .custom(browse_variant)
                    .xsmall()
                    .rounded(px(2.0))
                    .label("Browse")
                    .on_click(cx.listener(|this, _event, _window, cx| {
                        let tab = ActiveTab::Browse;
                        this.app_state.update(cx, |state, cx| {
                            state.set_tab(tab);
                            state.set_channel(None);
                            cx.notify();
                        });
                        cx.emit(NavbarEvent::TabChanged(tab));
                    })),
            )
    }

    /// Render the search input
    fn render_search_input(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div().w(px(240.0)).flex().items_center().child(
            Input::new(&self.search_input)
                .small()
                .cleanable(true)
                .prefix(
                    Icon::new(IconName::Search)
                        .small()
                        .text_color(theme::TEXT_DISABLED),
                ),
        )
    }

    /// Render login button for unauthenticated users
    fn render_login_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        Button::new("login-button")
            .primary()
            .small()
            .rounded(px(2.0))
            .icon(Icon::empty().path("icons/log-in.svg").small())
            .label("Log In")
            .on_click(cx.listener(|_this, _event, _window, cx| {
                log::info!("Login button clicked, emitting LoginRequested event");
                cx.emit(NavbarEvent::LoginRequested);
            }))
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
                Button::new("user-button")
                    .ghost()
                    .compact()
                    .rounded(px(2.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .child(
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
                    ),
            )
            .child(
                Button::new("logout-button")
                    .text()
                    .xsmall()
                    .rounded(px(2.0))
                    .label("Logout")
                    .on_click(cx.listener(|_this, _event, _window, cx| {
                        log::info!("Logout button clicked, emitting LogoutRequested event");
                        cx.emit(NavbarEvent::LogoutRequested);
                    })),
            )
    }
}
