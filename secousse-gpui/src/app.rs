//! Main application view
//!
//! This is the root view that contains the entire application layout.

use gpui::*;
use log::info;

use crate::api::TwitchClient;
use crate::state::{ActiveTab, AppState, AuthState, Settings};
use crate::theme;
use crate::views::{NavbarView, SidebarView};

/// The root application view
pub struct SecousseApp {
    /// Main application state
    app_state: Entity<AppState>,
    /// Twitch API client
    twitch_client: TwitchClient,
    /// Navbar view
    navbar: Entity<NavbarView>,
    /// Sidebar view
    sidebar: Entity<SidebarView>,
}

impl SecousseApp {
    /// Create a new application instance
    pub fn new(settings: Settings, window: &mut Window, cx: &mut Context<Self>) -> Self {
        info!("Initializing SecousseApp...");

        // Get or create device ID
        let mut settings = settings;
        let device_id = settings.get_or_create_device_id();

        // Create auth state from settings
        let auth_state = cx.new(|_| {
            AuthState::from_settings(device_id.clone(), settings.access_token.clone())
        });

        // Create settings entity
        let settings_entity = cx.new(|_| settings.clone());

        // Create main app state
        let app_state = cx.new(|_| {
            AppState::new(
                auth_state.clone(),
                settings_entity.clone(),
                settings.sidebar_open,
                settings.chat_open,
            )
        });

        // Create Twitch client
        let twitch_client = TwitchClient::new(settings.access_token.clone(), Some(device_id));

        // Create navbar view
        let navbar = cx.new(|_| NavbarView::new(app_state.clone(), auth_state.clone()));

        // Create sidebar view
        let sidebar = cx.new(|_| SidebarView::new(app_state.clone()));

        // Validate token on startup if we have one
        if twitch_client.is_authenticated() {
            Self::validate_token_async(auth_state.clone(), settings_entity.clone(), cx);
        }

        Self {
            app_state,
            twitch_client,
            navbar,
            sidebar,
        }
    }

    /// Validate the stored token asynchronously
    fn validate_token_async(
        auth: Entity<AuthState>,
        settings: Entity<Settings>,
        cx: &mut Context<Self>,
    ) {
        auth.update(cx, |auth, _| {
            auth.is_validating = true;
        });

        let token = auth.read(cx).access_token.clone();
        let device_id = auth.read(cx).device_id.clone();

        cx.spawn(|_this, mut cx| async move {
            if let Some(token) = token {
                let client = TwitchClient::new(Some(token.clone()), Some(device_id));
                
                match client.get_self_info().await {
                    Ok(user_info) => {
                        info!("Token validated for user: {}", user_info.display_name);
                        
                        let _ = cx.update(|cx| {
                            auth.update(cx, |auth, _| {
                                auth.set_logged_in(
                                    token,
                                    crate::state::auth_state::SelfInfo {
                                        id: user_info.id,
                                        login: user_info.login,
                                        display_name: user_info.display_name,
                                        profile_image_url: user_info.profile_image_url,
                                    },
                                );
                            });
                        });
                    }
                    Err(e) => {
                        info!("Token validation failed: {}", e);
                        
                        let _ = cx.update(|cx| {
                            auth.update(cx, |auth, _| {
                                auth.logout();
                            });
                            
                            settings.update(cx, |settings, _| {
                                settings.set_access_token(None);
                            });
                        });
                    }
                }
            }
        })
        .detach();
    }

    /// Render the main content area based on current tab and channel
    fn render_main_content(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let app_state = self.app_state.read(cx);
        
        if let Some(channel) = &app_state.current_channel {
            // Channel view (video + chat)
            self.render_channel_view(channel.clone(), cx)
        } else {
            // Tab-based view (Following or Browse)
            match app_state.active_tab {
                ActiveTab::Following => self.render_following_tab(cx),
                ActiveTab::Browse => self.render_browse_tab(cx),
            }
        }
    }

    /// Render the channel view with video player and chat
    fn render_channel_view(&self, channel: String, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_1()
            .bg(theme::BG_PRIMARY)
            .child(
                // Video area
                div()
                    .flex()
                    .flex_1()
                    .flex_col()
                    .child(
                        // Video player placeholder
                        div()
                            .flex()
                            .flex_1()
                            .bg(theme::VIDEO_BG)
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .text_color(theme::TEXT_SECONDARY)
                                    .text_size(px(16.0))
                                    .child(format!("Video Player: {}", channel)),
                            ),
                    )
                    .child(
                        // Stream info bar
                        div()
                            .h(px(80.0))
                            .bg(theme::BG_SECONDARY)
                            .border_t_1()
                            .border_color(theme::BORDER_SUBTLE)
                            .px(px(16.0))
                            .py(px(12.0))
                            .child(
                                div()
                                    .text_color(theme::TEXT_PRIMARY)
                                    .text_size(px(14.0))
                                    .child("Stream Info Bar"),
                            ),
                    ),
            )
            .child(
                // Chat panel
                div()
                    .w(px(theme::CHAT_WIDTH))
                    .h_full()
                    .bg(theme::BG_SECONDARY)
                    .border_l_1()
                    .border_color(theme::BORDER_SUBTLE)
                    .flex()
                    .flex_col()
                    .child(
                        // Chat header
                        div()
                            .h(px(50.0))
                            .px(px(16.0))
                            .flex()
                            .items_center()
                            .border_b_1()
                            .border_color(theme::BORDER_SUBTLE)
                            .child(
                                div()
                                    .text_color(theme::TEXT_PRIMARY)
                                    .text_size(px(14.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("STREAM CHAT"),
                            ),
                    )
                    .child(
                        // Chat messages area
                        div()
                            .flex_1()
                            .overflow_y_scroll()
                            .px(px(12.0))
                            .py(px(8.0))
                            .child(
                                div()
                                    .text_color(theme::TEXT_SECONDARY)
                                    .text_size(px(13.0))
                                    .child("Chat messages will appear here..."),
                            ),
                    )
                    .child(
                        // Chat input
                        div()
                            .h(px(70.0))
                            .px(px(12.0))
                            .py(px(10.0))
                            .border_t_1()
                            .border_color(theme::BORDER_SUBTLE)
                            .child(
                                div()
                                    .w_full()
                                    .h_full()
                                    .bg(theme::BG_INPUT)
                                    .rounded(px(4.0))
                                    .px(px(12.0))
                                    .flex()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_color(theme::TEXT_DISABLED)
                                            .text_size(px(14.0))
                                            .child("Send a message"),
                                    ),
                            ),
                    ),
            )
    }

    /// Render the Following tab
    fn render_following_tab(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_1()
            .flex_col()
            .bg(theme::BG_PRIMARY)
            .p(px(24.0))
            .child(
                div()
                    .text_color(theme::TEXT_PRIMARY)
                    .text_size(px(24.0))
                    .font_weight(FontWeight::BOLD)
                    .mb(px(16.0))
                    .child("Following"),
            )
            .child(
                div()
                    .text_color(theme::TEXT_SECONDARY)
                    .text_size(px(14.0))
                    .child("Your followed channels will appear here when you're logged in."),
            )
    }

    /// Render the Browse tab
    fn render_browse_tab(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_1()
            .flex_col()
            .bg(theme::BG_PRIMARY)
            .p(px(24.0))
            .child(
                div()
                    .text_color(theme::TEXT_PRIMARY)
                    .text_size(px(24.0))
                    .font_weight(FontWeight::BOLD)
                    .mb(px(16.0))
                    .child("Browse"),
            )
            .child(
                div()
                    .text_color(theme::TEXT_SECONDARY)
                    .text_size(px(14.0))
                    .child("Top streams will appear here..."),
            )
    }
}

impl Render for SecousseApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let app_state = self.app_state.read(cx);
        let sidebar_width = if app_state.is_sidebar_open {
            theme::SIDEBAR_WIDTH
        } else {
            theme::SIDEBAR_COLLAPSED_WIDTH
        };

        div()
            .id("secousse-app")
            .size_full()
            .flex()
            .flex_col()
            .bg(theme::BG_PRIMARY)
            .text_color(theme::TEXT_PRIMARY)
            .child(
                // Navbar
                div()
                    .h(px(theme::NAVBAR_HEIGHT))
                    .w_full()
                    .bg(theme::BG_SECONDARY)
                    .border_b_1()
                    .border_color(theme::BORDER_SUBTLE)
                    .child(self.navbar.clone()),
            )
            .child(
                // Main content area (sidebar + content)
                div()
                    .flex()
                    .flex_1()
                    .overflow_hidden()
                    .child(
                        // Sidebar
                        div()
                            .w(px(sidebar_width))
                            .h_full()
                            .bg(theme::BG_SECONDARY)
                            .border_r_1()
                            .border_color(theme::BORDER_SUBTLE)
                            .flex_shrink_0()
                            .child(self.sidebar.clone()),
                    )
                    .child(
                        // Main content
                        self.render_main_content(cx),
                    ),
            )
    }
}
