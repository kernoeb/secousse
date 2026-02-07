//! Main application view
//!
//! This is the root view that contains the entire application layout.

use gpui::prelude::{FluentBuilder, StyledImage};
use gpui::*;
use log::{error, info};
use std::sync::Arc;
use std::time::Duration;

use crate::actions;
use crate::api::{StreamQuality, TwitchClient, start_oauth_flow};
use crate::components::icon::{Icon, IconName};
use crate::state::{ActiveTab, AppState, AuthState, FollowedChannel, Settings};
use crate::theme;
use crate::video::{Video, VideoOptions};
use crate::views::{ChatView, ChatViewEvent, NavbarEvent, NavbarView, SidebarEvent, SidebarView};
use async_compat::Compat;

/// The root application view
pub struct SecousseApp {
    /// Main application state
    app_state: Entity<AppState>,
    /// Auth state
    auth_state: Entity<AuthState>,
    /// Settings
    settings: Entity<Settings>,
    /// Twitch API client (wrapped in Arc for sharing)
    twitch_client: Arc<std::sync::Mutex<TwitchClient>>,
    /// Navbar view
    navbar: Entity<NavbarView>,
    /// Sidebar view
    sidebar: Entity<SidebarView>,
    /// Video player (GStreamer-based)
    video: Option<Video>,
    /// Current chat view (created when entering a channel)
    chat_view: Option<Entity<ChatView>>,
    /// Available stream qualities
    stream_qualities: Vec<StreamQuality>,
    /// Currently selected quality index
    selected_quality_index: usize,
    /// Whether quality menu is open
    quality_menu_open: bool,
    /// Master playlist URL (for quality switching)
    master_playlist_url: Option<String>,
    /// Current volume level (0.0 to 1.0)
    volume: f64,
    /// Whether audio is muted
    is_muted: bool,
    /// Whether volume slider is being dragged
    volume_dragging: bool,

    /// Current channel ID (used for follow actions)
    current_channel_id: Option<String>,
    /// Whether current channel is followed by the user
    is_following_current: Option<bool>,
    /// Whether follow action is in-flight
    follow_in_flight: bool,
    /// Stream error message (e.g. "Channel is offline")
    stream_error: Option<String>,
    /// Root focus handle — clicking on the app background transfers focus here,
    /// effectively blurring any focused text input.
    root_focus: FocusHandle,
}

impl SecousseApp {
    /// Create a new application instance
    pub fn new(settings: Settings, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        info!("Initializing SecousseApp...");

        // Get or create device ID
        let mut settings = settings;
        let device_id = settings.get_or_create_device_id();

        // Create auth state from settings
        let auth_state =
            cx.new(|_| AuthState::from_settings(device_id.clone(), settings.access_token.clone()));

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
        let twitch_client = Arc::new(std::sync::Mutex::new(TwitchClient::new(
            settings.access_token.clone(),
            Some(device_id),
        )));

        // Create navbar view
        let navbar =
            cx.new(|cx| NavbarView::new(app_state.clone(), auth_state.clone(), cx));

        // Create sidebar view
        let sidebar = cx.new(|_| SidebarView::new(app_state.clone()));

        // Subscribe to navbar events
        cx.subscribe(
            &navbar,
            |this, _navbar, event: &NavbarEvent, cx| match event {
                NavbarEvent::LoginRequested => {
                    info!("Received LoginRequested event from navbar");
                    this.start_login(cx);
                }
                NavbarEvent::LogoutRequested => {
                    info!("Received LogoutRequested event from navbar");
                    this.logout(cx);
                }
                NavbarEvent::TabChanged(tab) => {
                    info!("Tab changed to {:?}", tab);
                    // Stop any active stream when switching tabs
                    if this.video.is_some() {
                        this.leave_channel(cx);
                    }
                    if *tab == crate::state::ActiveTab::Browse {
                        this.fetch_browse_if_needed(cx);
                    }
                }
                NavbarEvent::SearchSubmitted(query) => {
                    info!("Search submitted: {}", query);
                    // Stop any active stream when searching
                    if this.video.is_some() {
                        this.leave_channel(cx);
                    }
                    this.perform_search(query.clone(), cx);
                }
                NavbarEvent::SearchCleared => {
                    info!("Search cleared");
                    this.app_state.update(cx, |state, cx| {
                        state.clear_search();
                        cx.notify();
                    });
                }
            },
        )
        .detach();

        // Subscribe to sidebar events
        cx.subscribe(
            &sidebar,
            |this, _sidebar, event: &SidebarEvent, cx| match event {
                SidebarEvent::ChannelSelected(channel) => {
                    info!("Channel selected from sidebar: {}", channel);
                    this.enter_channel(channel.clone(), cx);
                }
            },
        )
        .detach();

        let app = Self {
            app_state,
            auth_state,
            settings: settings_entity,
            twitch_client,
            navbar,
            sidebar,
            video: None,
            chat_view: None,
            stream_qualities: Vec::new(),
            selected_quality_index: 0,
            quality_menu_open: false,
            master_playlist_url: None,
            volume: 1.0,
            is_muted: false,
            volume_dragging: false,

            current_channel_id: None,
            is_following_current: None,
            follow_in_flight: false,
            stream_error: None,
            root_focus: cx.focus_handle(),
        };

        // Validate token on startup if we have one
        if settings.access_token.is_some() {
            app.validate_and_fetch_data(cx);
        }

        // Start auto-refresh loops
        app.start_followed_refresh_loop(cx);
        app.start_browse_refresh_loop(cx);

        app
    }

    /// Validate token and fetch user data
    fn validate_and_fetch_data(&self, cx: &mut Context<Self>) {
        let auth = self.auth_state.clone();
        let settings = self.settings.clone();
        let app_state = self.app_state.clone();
        let client = self.twitch_client.clone();

        auth.update(cx, |auth, _| {
            auth.is_validating = true;
        });

        cx.spawn(async move |_this: gpui::WeakEntity<SecousseApp>, cx: &mut gpui::AsyncApp| {
            let token = cx
                .update(|cx: &mut App| auth.read(cx).access_token.clone())
                .ok()
                .flatten();

            let device_id = cx
                .update(|cx: &mut App| auth.read(cx).device_id.clone())
                .unwrap_or_default();

            if let Some(token) = token {
                // Validate token by getting self info
                let api_client = TwitchClient::new(Some(token.clone()), Some(device_id.clone()));

                let user_info_result = Compat::new(api_client.get_self_info()).await;

                match user_info_result {
                    Ok(user_info) => {
                        info!("Token validated for user: {}", user_info.display_name);
                        let user_id = user_info.id.clone();

                        // Update auth state
                        let _ = cx.update(|cx: &mut App| {
                            auth.update(cx, |auth, _| {
                                auth.set_logged_in(
                                    token.clone(),
                                    crate::state::auth_state::SelfInfo {
                                        id: user_info.id.clone(),
                                        login: user_info.login.clone(),
                                        display_name: user_info.display_name.clone(),
                                        profile_image_url: user_info.profile_image_url.clone(),
                                    },
                                );
                            });
                        });

                        // Update twitch client
                        if let Ok(mut c) = client.lock() {
                            c.set_access_token(Some(token.clone()));
                        }

                        // Mark as loading before fetching
                        let _ = cx.update(|cx: &mut App| {
                            app_state.update(cx, |state, _| {
                                state.is_loading_followed = true;
                            });
                        });

                        // Fetch followed channels
                        info!("Fetching followed channels for user {}", user_id);
                        let api_client = TwitchClient::new(Some(token), Some(device_id));
                        let followed_result =
                            Compat::new(api_client.get_followed_live_streams(&user_id)).await;

                        match followed_result {
                            Ok(channels) => {
                                info!("Found {} followed channels", channels.len());
                                let followed: Vec<FollowedChannel> = channels
                                    .into_iter()
                                    .map(|c| FollowedChannel {
                                        id: c.user.id,
                                        login: c.user.login,
                                        display_name: c.user.display_name,
                                        profile_image_url: c.user.profile_image_url,
                                        is_live: c.stream.is_some(),
                                        viewer_count: c.stream.as_ref().map(|s| s.viewer_count),
                                        game_name: c
                                            .stream
                                            .as_ref()
                                            .and_then(|s| s.game_name.clone()),
                                        stream_title: c.stream.as_ref().and_then(|s| {
                                            if s.title.is_empty() {
                                                None
                                            } else {
                                                Some(s.title.clone())
                                            }
                                        }),
                                        thumbnail_url: c
                                            .stream
                                            .as_ref()
                                            .and_then(|s| s.thumbnail_url.clone()),
                                    })
                                    .collect();

                                let _ = cx.update(|cx: &mut App| {
                                    app_state.update(cx, |state, cx| {
                                        state.set_followed_channels(followed);
                                        cx.notify();
                                    });
                                });
                            }
                            Err(e) => {
                                error!("Failed to fetch followed channels: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Token validation failed: {}", e);

                        let _ = cx.update(|cx: &mut App| {
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

    /// Start OAuth login flow
    fn start_login(&self, cx: &mut Context<Self>) {
        let auth = self.auth_state.clone();
        let settings = self.settings.clone();
        let app_state = self.app_state.clone();
        let client = self.twitch_client.clone();

        info!("Starting OAuth login flow...");

        cx.spawn(async move |_this: gpui::WeakEntity<SecousseApp>, cx: &mut gpui::AsyncApp| {
            // Run OAuth flow in blocking thread (it opens browser and waits)
            let result = smol::unblock(start_oauth_flow).await;

            match result {
                Ok(token) => {
                    info!("OAuth login successful!");

                    // Get device ID
                    let device_id = cx
                        .update(|cx: &mut App| auth.read(cx).device_id.clone())
                        .unwrap_or_default();

                    // Validate token and get user info
                    let api_client =
                        TwitchClient::new(Some(token.clone()), Some(device_id.clone()));

                    let user_info_result = Compat::new(api_client.get_self_info()).await;

                    match user_info_result {
                        Ok(user_info) => {
                            info!("Logged in as: {}", user_info.display_name);
                            let user_id = user_info.id.clone();

                            // Update auth state
                            let _ = cx.update(|cx: &mut App| {
                                auth.update(cx, |auth, _| {
                                    auth.set_logged_in(
                                        token.clone(),
                                        crate::state::auth_state::SelfInfo {
                                            id: user_info.id.clone(),
                                            login: user_info.login.clone(),
                                            display_name: user_info.display_name.clone(),
                                            profile_image_url: user_info.profile_image_url.clone(),
                                        },
                                    );
                                });

                                // Save token to settings
                                settings.update(cx, |settings, _| {
                                    settings.set_access_token(Some(token.clone()));
                                });
                            });

                            // Update twitch client
                            if let Ok(mut c) = client.lock() {
                                c.set_access_token(Some(token.clone()));
                            }

                            // Mark as loading before fetching
                            let _ = cx.update(|cx: &mut App| {
                                app_state.update(cx, |state, _| {
                                    state.is_loading_followed = true;
                                });
                            });

                            // Fetch followed channels
                            let api_client = TwitchClient::new(Some(token), Some(device_id));
                            let followed_result =
                                Compat::new(api_client.get_followed_live_streams(&user_id)).await;

                            match followed_result {
                                Ok(channels) => {
                                    info!("Found {} followed channels", channels.len());
                                    let followed: Vec<FollowedChannel> = channels
                                        .into_iter()
                                        .map(|c| FollowedChannel {
                                            id: c.user.id,
                                            login: c.user.login,
                                            display_name: c.user.display_name,
                                            profile_image_url: c.user.profile_image_url,
                                            is_live: c.stream.is_some(),
                                            viewer_count: c.stream.as_ref().map(|s| s.viewer_count),
                                            game_name: c
                                                .stream
                                                .as_ref()
                                                .and_then(|s| s.game_name.clone()),
                                            stream_title: c.stream.as_ref().and_then(|s| {
                                                if s.title.is_empty() {
                                                    None
                                                } else {
                                                    Some(s.title.clone())
                                                }
                                            }),
                                            thumbnail_url: c
                                                .stream
                                                .as_ref()
                                                .and_then(|s| s.thumbnail_url.clone()),
                                        })
                                        .collect();

                                    let _ = cx.update(|cx: &mut App| {
                                        app_state.update(cx, |state, cx| {
                                            state.set_followed_channels(followed);
                                            cx.notify();
                                        });
                                    });
                                }
                                Err(e) => {
                                    error!("Failed to fetch followed channels: {}", e);
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to get user info after login: {}", e);
                        }
                    }
                }
                Err(e) => {
                    error!("OAuth login failed: {}", e);
                }
            }
        })
        .detach();
    }

    /// Perform a search query
    fn perform_search(&self, query: String, cx: &mut Context<Self>) {
        if query.is_empty() {
            return;
        }

        info!("Performing search for: {}", query);

        let app_state = self.app_state.clone();
        let client = self.twitch_client.clone();

        // Mark as searching and store query
        app_state.update(cx, |state, _| {
            state.search_query = query.clone();
            state.is_searching = true;
            state.search_active = true;
        });

        cx.spawn(async move |_this: gpui::WeakEntity<SecousseApp>, cx: &mut gpui::AsyncApp| {
            // Get the client config
            let (access_token, device_id) = {
                let guard = client.lock().ok();
                guard
                    .map(|c| (c.access_token.clone(), c.device_id().to_string()))
                    .unwrap_or((None, String::new()))
            };

            // Search
            let api_client = TwitchClient::new(access_token, Some(device_id));
            let search_result = Compat::new(api_client.search_channels(&query)).await;

            match search_result {
                Ok(channels) => {
                    info!("Search returned {} results", channels.len());
                    let results: Vec<FollowedChannel> = channels
                        .into_iter()
                        .map(|c| FollowedChannel {
                            id: c.user.id,
                            login: c.user.login,
                            display_name: c.user.display_name,
                            profile_image_url: c.user.profile_image_url,
                            is_live: c.stream.is_some(),
                            viewer_count: c.stream.as_ref().map(|s| s.viewer_count),
                            game_name: c.stream.as_ref().and_then(|s| s.game_name.clone()),
                            stream_title: c.stream.as_ref().and_then(|s| {
                                if s.title.is_empty() {
                                    None
                                } else {
                                    Some(s.title.clone())
                                }
                            }),
                            thumbnail_url: c.stream.as_ref().and_then(|s| s.thumbnail_url.clone()),
                        })
                        .collect();

                    let _ = cx.update(|cx: &mut App| {
                        app_state.update(cx, |state, cx| {
                            state.set_search_results(results);
                            cx.notify();
                        });
                    });
                }
                Err(e) => {
                    error!("Search failed: {}", e);
                    let _ = cx.update(|cx: &mut App| {
                        app_state.update(cx, |state, cx| {
                            state.is_searching = false;
                            cx.notify();
                        });
                    });
                }
            }
        })
        .detach();
    }

    /// Fetch top streams for Browse tab if not already loaded
    fn fetch_browse_if_needed(&self, cx: &mut Context<Self>) {
        let needs_fetch = self.app_state.read(cx).needs_browse_fetch();
        if !needs_fetch {
            info!("Browse data already loaded, skipping fetch");
            return;
        }

        info!("Fetching top streams for Browse tab...");

        let app_state = self.app_state.clone();
        let client = self.twitch_client.clone();

        // Mark as loading
        app_state.update(cx, |state, _| {
            state.is_loading_browse = true;
        });

        cx.spawn(async move |_this: gpui::WeakEntity<SecousseApp>, cx: &mut gpui::AsyncApp| {
            // Get the client config
            let (access_token, device_id) = {
                let guard = client.lock().ok();
                guard
                    .map(|c| (c.access_token.clone(), c.device_id().to_string()))
                    .unwrap_or((None, String::new()))
            };

            // Fetch top streams
            let api_client = TwitchClient::new(access_token, Some(device_id));
            let streams_result = Compat::new(api_client.get_top_streams(50)).await;

            match streams_result {
                Ok(streams) => {
                    info!("Fetched {} top streams", streams.len());
                    let top_streams: Vec<FollowedChannel> = streams
                        .into_iter()
                        .map(|c| FollowedChannel {
                            id: c.user.id,
                            login: c.user.login,
                            display_name: c.user.display_name,
                            profile_image_url: c.user.profile_image_url,
                            is_live: c.stream.is_some(),
                            viewer_count: c.stream.as_ref().map(|s| s.viewer_count),
                            game_name: c.stream.as_ref().and_then(|s| s.game_name.clone()),
                            stream_title: c.stream.as_ref().and_then(|s| {
                                if s.title.is_empty() {
                                    None
                                } else {
                                    Some(s.title.clone())
                                }
                            }),
                            thumbnail_url: c.stream.as_ref().and_then(|s| s.thumbnail_url.clone()),
                        })
                        .collect();

                    let _ = cx.update(|cx: &mut App| {
                        app_state.update(cx, |state, cx| {
                            state.set_top_streams(top_streams);
                            cx.notify();
                        });
                    });
                }
                Err(e) => {
                    error!("Failed to fetch top streams: {}", e);
                    let _ = cx.update(|cx: &mut App| {
                        app_state.update(cx, |state, _| {
                            state.is_loading_browse = false;
                        });
                    });
                }
            }
        })
        .detach();
    }

    /// Start auto-refresh loop for followed channels
    fn start_followed_refresh_loop(&self, cx: &mut Context<Self>) {
        let app_state = self.app_state.clone();
        let auth_state = self.auth_state.clone();

        cx.spawn(async move |_this: gpui::WeakEntity<SecousseApp>, cx: &mut gpui::AsyncApp| {
            loop {
                smol::Timer::after(Duration::from_secs(60)).await;

                let state = cx
                    .update(|cx: &mut App| {
                        let auth = auth_state.read(cx);
                        let is_logged_in = auth.is_logged_in;
                        let user_id = auth.user_id().map(str::to_string);
                        let device_id = auth.device_id.clone();
                        let access_token = auth.access_token.clone();
                        let is_loading = app_state.read(cx).is_loading_followed;
                        (is_logged_in, user_id, device_id, access_token, is_loading)
                    })
                    .ok();

                let Some((is_logged_in, user_id, device_id, access_token, is_loading)) = state
                else {
                    break;
                };

                if !is_logged_in || user_id.is_none() || is_loading {
                    continue;
                }

                let user_id = user_id.unwrap();

                // Silent refresh: don't set is_loading_followed so existing data stays visible

                let api_client = TwitchClient::new(access_token, Some(device_id));
                let followed_result =
                    Compat::new(api_client.get_followed_live_streams(&user_id)).await;

                match followed_result {
                    Ok(channels) => {
                        info!("Auto-refreshed {} followed channels", channels.len());
                        let followed: Vec<FollowedChannel> = channels
                            .into_iter()
                            .map(|c| FollowedChannel {
                                id: c.user.id,
                                login: c.user.login,
                                display_name: c.user.display_name,
                                profile_image_url: c.user.profile_image_url,
                                is_live: c.stream.is_some(),
                                viewer_count: c.stream.as_ref().map(|s| s.viewer_count),
                                game_name: c.stream.as_ref().and_then(|s| s.game_name.clone()),
                                stream_title: c.stream.as_ref().and_then(|s| {
                                    if s.title.is_empty() {
                                        None
                                    } else {
                                        Some(s.title.clone())
                                    }
                                }),
                                thumbnail_url: c
                                    .stream
                                    .as_ref()
                                    .and_then(|s| s.thumbnail_url.clone()),
                            })
                            .collect();

                        let _ = cx.update(|cx: &mut App| {
                            app_state.update(cx, |state, cx| {
                                state.set_followed_channels(followed);
                                cx.notify();
                            });
                        });
                    }
                    Err(e) => {
                        error!("Failed to auto-refresh followed channels: {}", e);
                    }
                }
            }
        })
        .detach();
    }

    /// Start auto-refresh loop for Browse tab
    fn start_browse_refresh_loop(&self, cx: &mut Context<Self>) {
        let app_state = self.app_state.clone();
        let client = self.twitch_client.clone();

        cx.spawn(async move |_this: gpui::WeakEntity<SecousseApp>, cx: &mut gpui::AsyncApp| {
            loop {
                smol::Timer::after(Duration::from_secs(60)).await;

                let state = cx
                    .update(|cx: &mut App| {
                        let app = app_state.read(cx);
                        let should_refresh =
                            app.active_tab == ActiveTab::Browse && !app.is_loading_browse;
                        let search_active = app.search_active;
                        (should_refresh, search_active)
                    })
                    .ok();

                let Some((should_refresh, search_active)) = state else {
                    break;
                };

                if !should_refresh || search_active {
                    continue;
                }

                // Silent refresh: don't set is_loading_browse so existing data stays visible

                let (access_token, device_id) = {
                    let guard = client.lock().ok();
                    guard
                        .map(|c| (c.access_token.clone(), c.device_id().to_string()))
                        .unwrap_or((None, String::new()))
                };

                let api_client = TwitchClient::new(access_token, Some(device_id));
                let streams_result = Compat::new(api_client.get_top_streams(50)).await;

                match streams_result {
                    Ok(streams) => {
                        info!("Auto-refreshed {} top streams", streams.len());
                        let top_streams: Vec<FollowedChannel> = streams
                            .into_iter()
                            .map(|c| FollowedChannel {
                                id: c.user.id,
                                login: c.user.login,
                                display_name: c.user.display_name,
                                profile_image_url: c.user.profile_image_url,
                                is_live: c.stream.is_some(),
                                viewer_count: c.stream.as_ref().map(|s| s.viewer_count),
                                game_name: c.stream.as_ref().and_then(|s| s.game_name.clone()),
                                stream_title: c.stream.as_ref().and_then(|s| {
                                    if s.title.is_empty() {
                                        None
                                    } else {
                                        Some(s.title.clone())
                                    }
                                }),
                                thumbnail_url: c
                                    .stream
                                    .as_ref()
                                    .and_then(|s| s.thumbnail_url.clone()),
                            })
                            .collect();

                        let _ = cx.update(|cx: &mut App| {
                            app_state.update(cx, |state, cx| {
                                state.set_top_streams(top_streams);
                                cx.notify();
                            });
                        });
                    }
                    Err(e) => {
                        error!("Failed to auto-refresh top streams: {}", e);
                    }
                }
            }
        })
        .detach();
    }

    /// Logout user
    fn logout(&mut self, cx: &mut Context<Self>) {
        info!("Logging out...");

        self.auth_state.update(cx, |auth, _| {
            auth.logout();
        });

        self.settings.update(cx, |settings, _| {
            settings.set_access_token(None);
        });

        self.app_state.update(cx, |state, cx| {
            state.set_followed_channels(vec![]);
            cx.notify();
        });

        if let Ok(mut client) = self.twitch_client.lock() {
            client.set_access_token(None);
        }

        // Stop any playing video by dropping it
        self.video = None;
    }

    /// Start playing a stream
    fn play_stream(&mut self, login: &str, cx: &mut Context<Self>) {
        info!("Starting playback for channel: {}", login);

        let client = self.twitch_client.clone();
        let login = login.to_string();

        cx.spawn(async move |this: gpui::WeakEntity<SecousseApp>, cx: &mut gpui::AsyncApp| {
            // Get the client config
            let (access_token, device_id) = {
                let guard = client.lock().ok();
                guard
                    .map(|c| (c.access_token.clone(), c.device_id().to_string()))
                    .unwrap_or((None, String::new()))
            };

            // Get playback access token
            let api_client = TwitchClient::new(access_token.clone(), Some(device_id.clone()));
            let token_result = Compat::new(api_client.get_playback_access_token(&login)).await;

            match token_result {
                Ok(token) => {
                    // Construct the master HLS URL
                    let api_client =
                        TwitchClient::new(access_token.clone(), Some(device_id.clone()));
                    let master_url = api_client.get_usher_url(&login, &token);
                    info!("Got master HLS URL: {}", master_url);

                    // Fetch available qualities
                    let qualities_result =
                        Compat::new(api_client.get_stream_qualities(&master_url)).await;

                    let qualities = match qualities_result {
                        Ok(q) if !q.is_empty() => {
                            info!("Found {} quality options", q.len());
                            for (i, quality) in q.iter().enumerate() {
                                info!(
                                    "  Quality {}: {} ({})",
                                    i,
                                    quality.name,
                                    quality.display_name()
                                );
                            }
                            q
                        }
                        Ok(_) => {
                            info!("No qualities found, using master playlist directly");
                            Vec::new()
                        }
                        Err(e) => {
                            error!("Failed to fetch qualities: {}", e);
                            Vec::new()
                        }
                    };

                    // Determine which URL to use
                    let stream_url = if !qualities.is_empty() {
                        // Use the first (highest) quality by default
                        qualities[0].url.clone()
                    } else {
                        // Fallback to master playlist
                        master_url.clone()
                    };

                    info!("Using stream URL: {}", stream_url);

                    // Parse the URL first
                    let uri = match url::Url::parse(&stream_url) {
                        Ok(uri) => uri,
                        Err(e) => {
                            error!("Failed to parse stream URL: {}", e);
                            return;
                        }
                    };

                    // Store qualities in state
                    let _ = cx.update(|cx: &mut App| {
                        this.update(cx, |this: &mut SecousseApp, cx| {
                            this.stream_qualities = qualities;
                            this.selected_quality_index = 0;
                            this.master_playlist_url = Some(master_url);
                            cx.notify();
                        })
                        .ok();
                    });

                    // Create video player in background thread to avoid blocking main thread
                    // Video::new_with_options() blocks while waiting for GStreamer pipeline
                    info!("Creating video player in background thread...");
                    let video_result = smol::unblock(move || {
                        info!("Background thread: starting GStreamer pipeline creation");
                        let options = VideoOptions {
                            frame_buffer_capacity: Some(0),
                            looping: Some(false),
                            speed: Some(1.0),
                        };
                        let result = Video::new_with_options(&uri, options);
                        info!("Background thread: GStreamer pipeline creation finished");
                        result
                    })
                    .await;

                    // Store the video on main thread
                    match video_result {
                        Ok(video) => {
                            info!("Video player created successfully");
                            let _ = cx.update(|cx: &mut App| {
                                this.update(cx, |this: &mut SecousseApp, cx| {
                                    // Apply current volume settings to new video
                                    video.set_volume(this.volume);
                                    video.set_muted(this.is_muted);
                                    this.video = Some(video);
                                    this.stream_error = None;
                                    cx.notify();
                                })
                                .ok();
                            });
                        }
                        Err(e) => {
                            error!("Failed to create video player: {:?}", e);
                            let _ = cx.update(|cx: &mut App| {
                                this.update(cx, |this: &mut SecousseApp, cx| {
                                    this.stream_error =
                                        Some(format!("Failed to load stream: {}", e));
                                    cx.notify();
                                })
                                .ok();
                            });
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to get playback token: {}", e);
                    let error_msg = if e.to_string().contains("GQL Error")
                        || e.to_string().contains("null")
                    {
                        "Channel is offline".to_string()
                    } else {
                        format!("Failed to load stream: {}", e)
                    };
                    let _ = cx.update(|cx: &mut App| {
                        this.update(cx, |this: &mut SecousseApp, cx| {
                            this.stream_error = Some(error_msg);
                            cx.notify();
                        })
                        .ok();
                    });
                }
            }
        })
        .detach();
    }

    /// Stop playing
    fn stop_stream(&mut self) {
        self.video = None;
        self.stream_qualities.clear();
        self.selected_quality_index = 0;
        self.quality_menu_open = false;
        self.master_playlist_url = None;
    }

    /// Switch to a different quality
    fn switch_quality(&mut self, quality_index: usize, cx: &mut Context<Self>) {
        if quality_index >= self.stream_qualities.len() {
            return;
        }

        if quality_index == self.selected_quality_index {
            self.quality_menu_open = false;
            cx.notify();
            return;
        }

        info!(
            "Switching to quality {}: {}",
            quality_index, self.stream_qualities[quality_index].name
        );

        let stream_url = self.stream_qualities[quality_index].url.clone();
        self.selected_quality_index = quality_index;
        self.quality_menu_open = false;

        // Drop current video
        self.video = None;
        cx.notify();

        // Create new video with selected quality
        cx.spawn(async move |this: gpui::WeakEntity<SecousseApp>, cx: &mut gpui::AsyncApp| {
            let uri = match url::Url::parse(&stream_url) {
                Ok(uri) => uri,
                Err(e) => {
                    error!("Failed to parse stream URL: {}", e);
                    return;
                }
            };

            info!("Creating video player for quality switch...");
            let video_result = smol::unblock(move || {
                let options = VideoOptions {
                    frame_buffer_capacity: Some(0),
                    looping: Some(false),
                    speed: Some(1.0),
                };
                Video::new_with_options(&uri, options)
            })
            .await;

            match video_result {
                Ok(video) => {
                    info!("Quality switch successful");
                    let _ = cx.update(|cx: &mut App| {
                        this.update(cx, |this: &mut SecousseApp, cx| {
                            // Apply current volume settings to new video
                            video.set_volume(this.volume);
                            video.set_muted(this.is_muted);
                            this.video = Some(video);
                            cx.notify();
                        })
                        .ok();
                    });
                }
                Err(e) => {
                    error!("Failed to switch quality: {:?}", e);
                }
            }
        })
        .detach();
    }

    /// Enter a channel - sets up video and chat
    fn enter_channel(&mut self, channel: String, cx: &mut Context<Self>) {
        info!("Entering channel: {}", channel);

        // Stop any currently active stream before starting the new one
        self.stop_stream();
        self.chat_view = None;

        // Get auth info for chat
        let (access_token, username) = {
            let auth = self.auth_state.read(cx);
            (auth.access_token.clone(), auth.login().map(String::from))
        };

        // Resolve channel ID for emote fetching (from followed/top/search lists)
        let channel_id = self
            .current_channel_info(&channel, cx)
            .map(|c| c.id);

        // Create chat view with emote support
        let chat_channel = channel.clone();
        let chat_view = cx.new(|cx| {
            ChatView::new(chat_channel, channel_id, access_token, username, cx)
        });
        cx.subscribe(&chat_view, |this, _chat, event, cx| {
            if let ChatViewEvent::CloseRequested = event {
                this.app_state.update(cx, |state, cx| {
                    state.toggle_chat();
                    cx.notify();
                });
            }
        })
        .detach();
        self.chat_view = Some(chat_view);

        // Clear any previous error and start video playback
        self.stream_error = None;
        self.play_stream(&channel, cx);

        // Update app state
        let channel_for_state = channel.clone();
        self.app_state.update(cx, |state, cx| {
            state.set_channel(Some(channel_for_state));
            cx.notify();
        });

        self.update_current_channel_context(&channel, cx);
        self.check_follow_status(&channel, cx);
    }

    /// Leave the current channel - cleans up video and chat
    fn leave_channel(&mut self, cx: &mut Context<Self>) {
        info!("Leaving channel");

        // Stop video playback by dropping
        self.video = None;
        self.stream_error = None;

        // Clear quality state
        self.stream_qualities.clear();
        self.selected_quality_index = 0;
        self.quality_menu_open = false;
        self.master_playlist_url = None;

        // Remove chat view (will disconnect automatically when dropped)
        self.chat_view = None;

        // Clear channel state
        self.app_state.update(cx, |state, cx| {
            state.set_channel(None);
            cx.notify();
        });

        self.current_channel_id = None;
        self.is_following_current = None;
        self.follow_in_flight = false;
    }

    /// Find current channel info by login
    fn current_channel_info(&self, login: &str, cx: &mut Context<Self>) -> Option<FollowedChannel> {
        let state = self.app_state.read(cx);
        state
            .followed_channels
            .iter()
            .chain(state.top_streams.iter())
            .chain(state.search_results.iter())
            .find(|c| c.login == login)
            .cloned()
    }

    /// Update current channel context (ID and cached info)
    fn update_current_channel_context(&mut self, login: &str, cx: &mut Context<Self>) {
        let channel = self.current_channel_info(login, cx);
        self.current_channel_id = channel.as_ref().map(|c| c.id.clone());
    }

    /// Check follow status for the current channel
    fn check_follow_status(&mut self, login: &str, cx: &mut Context<Self>) {
        let auth = self.auth_state.read(cx);
        let Some(access_token) = auth.access_token.clone() else {
            self.is_following_current = None;
            return;
        };
        let Some(user_id) = auth.user_id().map(str::to_string) else {
            self.is_following_current = None;
            return;
        };

        let device_id = auth.device_id.clone();

        if self.current_channel_id.is_none() {
            self.update_current_channel_context(login, cx);
        }

        let Some(channel_id) = self.current_channel_id.clone() else {
            self.is_following_current = None;
            return;
        };

        self.follow_in_flight = true;

        cx.spawn(async move |this: gpui::WeakEntity<SecousseApp>, cx: &mut gpui::AsyncApp| {
            let api_client = TwitchClient::new(Some(access_token), Some(device_id));
            let result = Compat::new(api_client.check_follow_status(&user_id, &channel_id)).await;

            let _ = cx.update(|cx: &mut App| {
                this.update(cx, |this: &mut SecousseApp, cx| {
                    this.follow_in_flight = false;
                    match result {
                        Ok(is_following) => {
                            this.is_following_current = Some(is_following);
                        }
                        Err(e) => {
                            error!("Failed to check follow status: {}", e);
                            this.is_following_current = None;
                        }
                    }
                    cx.notify();
                })
                .ok();
            });
        })
        .detach();
    }

    /// Toggle follow status for the current channel
    fn toggle_follow(&mut self, cx: &mut Context<Self>) {
        if self.follow_in_flight {
            return;
        }

        let auth = self.auth_state.read(cx);
        let Some(access_token) = auth.access_token.clone() else {
            return;
        };

        let device_id = auth.device_id.clone();
        let Some(channel_id) = self.current_channel_id.clone() else {
            return;
        };

        let currently_following = self.is_following_current.unwrap_or(false);
        self.follow_in_flight = true;

        cx.spawn(async move |this: gpui::WeakEntity<SecousseApp>, cx: &mut gpui::AsyncApp| {
            let api_client = TwitchClient::new(Some(access_token), Some(device_id));
            let result = if currently_following {
                Compat::new(api_client.unfollow_user(&channel_id)).await
            } else {
                Compat::new(api_client.follow_user(&channel_id)).await
            };

            let _ = cx.update(|cx: &mut App| {
                this.update(cx, |this: &mut SecousseApp, cx| {
                    this.follow_in_flight = false;
                    match result {
                        Ok(_) => {
                            this.is_following_current = Some(!currently_following);
                        }
                        Err(e) => {
                            error!("Failed to update follow status: {}", e);
                        }
                    }
                    cx.notify();
                })
                .ok();
            });
        })
        .detach();
    }

    /// Render the main content area based on current tab and channel
    fn render_main_content(&self, cx: &mut Context<Self>) -> AnyElement {
        let app_state = self.app_state.read(cx);

        if let Some(channel) = &app_state.current_channel {
            // Channel view (video + chat)
            self.render_channel_view(channel.clone(), cx)
                .into_any_element()
        } else if app_state.search_active {
            // Search results view
            self.render_search_results(cx).into_any_element()
        } else {
            // Tab-based view (Following or Browse)
            match app_state.active_tab {
                ActiveTab::Following => self.render_following_tab(cx).into_any_element(),
                ActiveTab::Browse => self.render_browse_tab(cx).into_any_element(),
            }
        }
    }

    /// Render the channel view with video player and chat
    fn render_channel_view(&self, channel: String, cx: &mut Context<Self>) -> impl IntoElement {
        // Get video instance and stream error
        let video_instance = self.video.clone();
        let stream_error = self.stream_error.clone();
        let is_playing = video_instance
            .as_ref()
            .map(|v| !v.paused())
            .unwrap_or(false);
        let channel_for_back = channel.clone();
        let channel_info = self.current_channel_info(&channel, cx);
        let stream_title = truncate_with_ellipsis(
            &channel_info
                .as_ref()
                .and_then(|c| c.stream_title.as_deref())
                .unwrap_or("Live stream"),
            100,
        );
        let game_name = truncate_with_ellipsis(
            &channel_info
                .as_ref()
                .and_then(|c| c.game_name.as_deref())
                .unwrap_or("Just Chatting"),
            60,
        );
        let viewer_count = channel_info
            .as_ref()
            .and_then(|c| c.viewer_count)
            .unwrap_or(0);
        let avatar_url = channel_info
            .as_ref()
            .and_then(|c| c.profile_image_url.clone());
        let is_logged_in = self.auth_state.read(cx).is_logged_in;
        let follow_label = if !is_logged_in {
            "Log in to follow"
        } else if self.follow_in_flight {
            "Updating..."
        } else if self.is_following_current == Some(true) {
            "Following"
        } else {
            "Follow"
        };
        let follow_enabled =
            is_logged_in && !self.follow_in_flight && self.current_channel_id.is_some();
        let is_chat_open = self.app_state.read(cx).is_chat_open;
        let is_fullscreen = self.app_state.read(cx).is_fullscreen;

        div()
            .flex()
            .flex_1()
            .h_full()
            .overflow_hidden()
            .bg(theme::BG_PRIMARY)
            .child(
                // Video area
                div()
                    .flex()
                    .flex_1()
                    .h_full()
                    .flex_col()
                    .overflow_hidden()
                    .child(
                        // Video player area
                        div()
                            .flex()
                            .flex_1()
                            .w_full()
                            .bg(theme::VIDEO_BG)
                            .relative()
                            // Render video if available, otherwise show loading/error state
                            .when_some(video_instance.clone(), |this, vid| {
                                this.child(
                                    crate::video::video(vid)
                                        .id("stream-video"),
                                )
                            })
                            .when(
                                video_instance.is_none() && stream_error.is_some(),
                                {
                                    let err = stream_error.clone().unwrap_or_default();
                                    move |this| {
                                        this.flex_col().items_center().justify_center().child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .items_center()
                                                .gap(px(8.0))
                                                .child(
                                                    div()
                                                        .text_color(theme::TEXT_PRIMARY)
                                                        .text_size(px(18.0))
                                                        .child(err),
                                                )
                                                .child(
                                                    div()
                                                        .text_color(theme::TEXT_SECONDARY)
                                                        .text_size(px(14.0))
                                                        .child(
                                                            "The stream may be offline or unavailable.",
                                                        ),
                                                ),
                                        )
                                    }
                                },
                            )
                            .when(
                                video_instance.is_none() && stream_error.is_none(),
                                |this| {
                                    this.flex_col().items_center().justify_center().child(
                                        div()
                                            .text_color(theme::TEXT_SECONDARY)
                                            .text_size(px(16.0))
                                            .child("Loading stream..."),
                                    )
                                },
                            )
                            // Overlay controls at the bottom
                            .child(
                                div()
                                    .absolute()
                                    .bottom(px(16.0))
                                    .left(px(16.0))
                                    .right(px(16.0))
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .flex()
                                            .gap(px(8.0))
                                            .child({
                                                let video = video_instance.clone();
                                                div()
                                                    .id("pause-btn")
                                                    .px(px(16.0))
                                                    .py(px(8.0))
                                                    .bg(rgba(0x000000b3)) // 0.7 alpha = 0xb3
                                                    .hover(|style| style.bg(rgba(0x000000e6))) // 0.9 alpha = 0xe6
                                                    .rounded(px(4.0))
                                                    .cursor_pointer()
                                                    .text_color(theme::TEXT_PRIMARY)
                                                    .text_size(px(13.0))
                                                    .on_click(move |_event, _window, _cx| {
                                                        if let Some(video) = &video {
                                                            let paused = video.paused();
                                                            video.set_paused(!paused);
                                                        }
                                                    })
                                                    .child(if is_playing {
                                                        Icon::new(IconName::Pause).size_4().color(theme::TEXT_PRIMARY).into_any_element()
                                                    } else {
                                                        Icon::new(IconName::Play).size_4().color(theme::TEXT_PRIMARY).into_any_element()
                                                    })
                                            })
                                            // Quality selector
                                            .child(self.render_quality_selector(cx))
                                            // Volume control
                                            .child(self.render_volume_control(cx)),
                                    )
                                    // Fullscreen toggle button (right side)
                                    .child(
                                        div()
                                            .id("fullscreen-btn")
                                            .px(px(16.0))
                                            .py(px(8.0))
                                            .bg(rgba(0x000000b3))
                                            .hover(|style| style.bg(rgba(0x000000e6)))
                                            .rounded(px(4.0))
                                            .cursor_pointer()
                                            .on_click(cx.listener(|this, _event, _window, cx| {
                                                this.app_state.update(cx, |state, cx| {
                                                    state.toggle_fullscreen();
                                                    cx.notify();
                                                });
                                            }))
                                            .child(
                                                Icon::new(if is_fullscreen {
                                                    IconName::Minimize
                                                } else {
                                                    IconName::Maximize
                                                })
                                                .size_4()
                                                .color(theme::TEXT_PRIMARY),
                                            ),
                                    ),
                            )
                            // LIVE badge + viewer count overlay at top-left
                            .child(
                                div()
                                    .absolute()
                                    .top(px(16.0))
                                    .left(px(16.0))
                                    .flex()
                                    .items_center()
                                    .gap(px(6.0))
                                    .child(
                                        div()
                                            .px(px(6.0))
                                            .py(px(2.0))
                                            .bg(theme::LIVE_RED)
                                            .rounded(px(4.0))
                                            .text_color(gpui::white())
                                            .text_size(px(12.0))
                                            .font_weight(FontWeight::BOLD)
                                            .child("LIVE"),
                                    )
                                    .child(
                                        div()
                                            .px(px(8.0))
                                            .py(px(2.0))
                                            .bg(rgba(0x000000b3))
                                            .rounded(px(4.0))
                                            .text_color(theme::TEXT_PRIMARY)
                                            .text_size(px(12.0))
                                            .child(format_viewer_count(viewer_count)),
                                    ),
                            )
                            // Floating "open chat" button at top-right (only when chat is hidden, not in fullscreen)
                            .when(!is_chat_open && !is_fullscreen, |el: Div| {
                                el.child(
                                    div()
                                        .id("floating-chat-toggle")
                                        .absolute()
                                        .top(px(16.0))
                                        .right(px(16.0))
                                        .p(px(8.0))
                                        .bg(rgba(0x000000b3))
                                        .hover(|s| s.bg(rgba(0x000000e6)))
                                        .rounded(px(4.0))
                                        .cursor_pointer()
                                        .on_click(cx.listener(|this, _event, _window, cx| {
                                            this.app_state.update(cx, |state, cx| {
                                                state.toggle_chat();
                                                cx.notify();
                                            });
                                        }))
                                        .child(
                                            Icon::new(IconName::PanelRight)
                                                .size_4()
                                                .color(theme::TEXT_PRIMARY),
                                        ),
                                )
                            }),
                    )
                    // Stream info bar (hidden in fullscreen)
                    .when(!is_fullscreen, |el: Div| {
                        el.child(
                            div()
                                .h(px(96.0))
                                .w_full()
                                .flex_shrink_0()
                                .overflow_hidden()
                                .bg(theme::BG_SECONDARY)
                                .border_t_1()
                                .border_color(theme::BORDER_SUBTLE)
                                .px(px(16.0))
                                .py(px(12.0))
                                .flex()
                                .items_center()
                                .justify_between()
                                .gap(px(16.0))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(12.0))
                                    .child(if let Some(url) = avatar_url {
                                        div()
                                            .size(px(48.0))
                                            .rounded_full()
                                            .overflow_hidden()
                                            .bg(theme::BG_TERTIARY)
                                            .child(
                                                img(url)
                                                    .w_full()
                                                    .h_full()
                                                    .object_fit(ObjectFit::Cover),
                                            )
                                            .into_any_element()
                                    } else {
                                        div()
                                            .size(px(48.0))
                                            .rounded_full()
                                            .bg(theme::TWITCH_PURPLE)
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .text_color(theme::TEXT_PRIMARY)
                                            .text_size(px(16.0))
                                            .font_weight(FontWeight::BOLD)
                                            .child(
                                                channel_for_back
                                                    .chars()
                                                    .next()
                                                    .unwrap_or('?')
                                                    .to_string(),
                                            )
                                            .into_any_element()
                                    })
                                    .child(
                                        div()
                                            .flex_col()
                                            .gap(px(4.0))
                                            .child(
                                                div()
                                                    .text_color(theme::TEXT_PRIMARY)
                                                    .text_size(px(16.0))
                                                    .font_weight(FontWeight::BOLD)
                                                    .child(channel_for_back.clone()),
                                            )
                                            .child(
                                                div()
                                                    .text_color(theme::TEXT_PRIMARY)
                                                    .text_size(px(14.0))
                                                    .overflow_hidden()
                                                    .whitespace_nowrap()
                                                    .child(stream_title),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .items_center()
                                                    .gap(px(8.0))
                                                     .child(
                                                        div()
                                                            .text_color(theme::TEXT_SECONDARY)
                                                            .text_size(px(12.0))
                                                            .child(game_name),
                                                    ),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.0))
                                    // Follow button
                                    .child(
                                        div()
                                            .id("follow-btn")
                                            .px(px(14.0))
                                            .py(px(8.0))
                                            .bg(if follow_enabled {
                                                if self.is_following_current == Some(true) {
                                                    theme::BG_TERTIARY
                                                } else {
                                                    theme::TWITCH_PURPLE
                                                }
                                            } else {
                                                theme::BG_TERTIARY
                                            })
                                            .hover(|s| {
                                                if follow_enabled {
                                                    s.bg(theme::TWITCH_PURPLE_HOVER)
                                                } else {
                                                    s
                                                }
                                            })
                                            .rounded(px(6.0))
                                            .cursor_pointer()
                                            .text_color(if follow_enabled {
                                                theme::TEXT_PRIMARY
                                            } else {
                                                theme::TEXT_DISABLED
                                            })
                                            .text_size(px(12.0))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .on_click(cx.listener(|this, _event, _window, cx| {
                                                if this.follow_in_flight
                                                    || this.current_channel_id.is_none()
                                                {
                                                    return;
                                                }
                                                if this.auth_state.read(cx).is_logged_in {
                                                    this.toggle_follow(cx);
                                                }
                                            }))
                                            .flex()
                                            .items_center()
                                            .gap(px(6.0))
                                            .child(if self.is_following_current == Some(true) {
                                                Icon::new(IconName::Heart).size_4().color(theme::LIVE_RED).into_any_element()
                                            } else {
                                                Icon::new(IconName::Heart).size_4().color(theme::TEXT_PRIMARY).into_any_element()
                                            })
                                            .child(follow_label),
                                    ),
                            ),
                        )
                    })
            )
            .when(is_chat_open && !is_fullscreen, |el: Div| {
                el.child(
                    // Chat panel
                    div()
                        .w(px(theme::CHAT_WIDTH))
                        .h_full()
                        .border_l_1()
                        .border_color(theme::BORDER_SUBTLE)
                        .child(if let Some(chat_view) = &self.chat_view {
                            chat_view.clone().into_any_element()
                        } else {
                            div()
                                .w_full()
                                .h_full()
                                .bg(theme::BG_SECONDARY)
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(
                                    div()
                                        .text_color(theme::TEXT_SECONDARY)
                                        .text_size(px(14.0))
                                        .child("Chat loading..."),
                                )
                                .into_any_element()
                        }),
                )
            })
    }

    /// Render the quality selector dropdown
    fn render_quality_selector(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let qualities: Vec<StreamQuality> = self.stream_qualities.clone();
        let selected_index = self.selected_quality_index;
        let menu_open = self.quality_menu_open;

        if qualities.is_empty() {
            // No qualities available yet
            return div()
                .id("quality-loading")
                .px(px(16.0))
                .py(px(8.0))
                .bg(rgba(0x000000b3))
                .rounded(px(4.0))
                .text_color(theme::TEXT_SECONDARY)
                .text_size(px(13.0))
                .child("Auto")
                .into_any_element();
        }

        let current_quality = &qualities[selected_index];
        let display_name = current_quality.display_name();

        div()
            .id("quality-selector")
            .relative()
            .child(
                // Quality button
                div()
                    .id("quality-btn")
                    .px(px(16.0))
                    .py(px(8.0))
                    .bg(if menu_open {
                        rgba(0x000000e6)
                    } else {
                        rgba(0x000000b3)
                    })
                    .hover(|style| style.bg(rgba(0x000000e6)))
                    .rounded(px(4.0))
                    .cursor_pointer()
                    .text_color(theme::TEXT_PRIMARY)
                    .text_size(px(13.0))
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .on_click(cx.listener(|this, _event, _window, cx| {
                        this.quality_menu_open = !this.quality_menu_open;
                        cx.notify();
                    }))
                    .child(display_name)
                    .child(if menu_open {
                        Icon::new(IconName::ChevronDown).size_3p5().color(theme::TEXT_PRIMARY).into_any_element()
                    } else {
                        Icon::new(IconName::ChevronUp).size_3p5().color(theme::TEXT_PRIMARY).into_any_element()
                    }),
            )
            // Dropdown menu (shown above the button)
            .when(menu_open, |container| {
                let mut menu = div()
                    .absolute()
                    .bottom(px(44.0)) // Position above the button
                    .left_0()
                    .min_w(px(150.0))
                    .bg(rgba(0x000000e6))
                    .rounded(px(4.0))
                    .border_1()
                    .border_color(theme::BORDER_SUBTLE)
                    .py(px(4.0))
                    .flex()
                    .flex_col();

                for (i, quality) in qualities.iter().enumerate() {
                    let is_selected = i == selected_index;
                    let quality_name = quality.display_name();

                    menu = menu.child(
                        div()
                            .id(SharedString::from(format!("quality-option-{}", i)))
                            .px(px(12.0))
                            .py(px(8.0))
                            .cursor_pointer()
                            .bg(if is_selected {
                                theme::SELECTED_BG
                            } else {
                                theme::TRANSPARENT
                            })
                            .hover(|style| style.bg(theme::BG_TERTIARY))
                            .text_color(if is_selected {
                                theme::TWITCH_PURPLE
                            } else {
                                theme::TEXT_PRIMARY
                            })
                            .text_size(px(13.0))
                            .on_click(cx.listener(move |this, _event, _window, cx| {
                                this.switch_quality(i, cx);
                            }))
                            .child(quality_name),
                    );
                }

                container.child(menu)
            })
            .into_any_element()
    }

    /// Render a modern compact volume control (icon + short slider).
    ///
    /// Design:
    /// - 80px track, 4px visible height, 20px click-target height (easy to grab)
    /// - Snaps to 100 % when within 5 % of max so you never struggle to hit full volume
    /// - 3-tier icon: VolumeX / Volume1 / Volume2
    /// - No knob, no percentage label — clean filled-track style (Twitch / YouTube)
    fn render_volume_control(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let current_volume = self.volume;
        let is_muted = self.is_muted;
        let effective = if is_muted { 0.0 } else { current_volume };

        let volume_icon = if is_muted || current_volume == 0.0 {
            IconName::VolumeX
        } else if current_volume < 0.5 {
            IconName::Volume1
        } else {
            IconName::Volume2
        };

        // Track geometry
        let track_w: f32 = 80.0;
        let track_h: f32 = 4.0;
        let hit_h: f32 = 20.0; // invisible click area height
        let fill_w = (track_w * effective as f32).clamp(0.0, track_w);

        // Shared mutable left-edge captured by the canvas prepaint.
        let slider_left = std::rc::Rc::new(std::cell::Cell::new(0.0f32));
        let sl_canvas = slider_left.clone();
        let sl_down = slider_left.clone();
        let sl_move = slider_left.clone();

        div()
            .id("volume-control")
            .flex()
            .items_center()
            .gap(px(6.0))
            .px(px(8.0))
            .py(px(6.0))
            .bg(rgba(0x000000b3))
            .hover(|s| s.bg(rgba(0x000000e6)))
            .rounded(px(4.0))
            // — Mute / unmute icon —
            .child(
                div()
                    .id("mute-btn")
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _event, _window, cx| {
                        this.is_muted = !this.is_muted;
                        if let Some(video) = &this.video {
                            video.set_muted(this.is_muted);
                        }
                        cx.notify();
                    }))
                    .child(Icon::new(volume_icon).size_4().color(theme::TEXT_PRIMARY)),
            )
            // — Slider —
            .child(
                div()
                    .id("volume-slider")
                    .w(px(track_w))
                    .h(px(hit_h))
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .relative()
                    // Canvas overlay to capture left-edge position in window coords
                    .child(
                        gpui::canvas(
                            move |bounds: Bounds<Pixels>, _window: &mut Window, _cx: &mut App| {
                                sl_canvas.set(f32::from(bounds.origin.x));
                            },
                            |_: Bounds<Pixels>, _: (), _: &mut Window, _: &mut App| {},
                        )
                        .absolute()
                        .size_full(),
                    )
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, event: &MouseDownEvent, _window, cx| {
                            this.volume_dragging = true;
                            let x = f32::from(event.position.x);
                            let ratio = ((x - sl_down.get()) / track_w).clamp(0.0, 1.0);
                            this.set_volume_snapped(ratio as f64, cx);
                        }),
                    )
                    .on_mouse_move(
                        cx.listener(move |this, event: &MouseMoveEvent, _window, cx| {
                            if !this.volume_dragging {
                                return;
                            }
                            let x = f32::from(event.position.x);
                            let ratio = ((x - sl_move.get()) / track_w).clamp(0.0, 1.0);
                            this.set_volume_snapped(ratio as f64, cx);
                        }),
                    )
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _event, _window, cx| {
                            this.volume_dragging = false;
                            cx.notify();
                        }),
                    )
                    .on_mouse_up_out(
                        MouseButton::Left,
                        cx.listener(|this, _event, _window, cx| {
                            this.volume_dragging = false;
                            cx.notify();
                        }),
                    )
                    // Visible track (background)
                    .child(
                        div()
                            .w(px(track_w))
                            .h(px(track_h))
                            .rounded(px(999.0))
                            .bg(rgba(0xffffff33)) // subtle white 20 %
                            .relative()
                            // Filled portion
                            .child(
                                div()
                                    .absolute()
                                    .left_0()
                                    .top_0()
                                    .h_full()
                                    .w(px(fill_w))
                                    .bg(theme::TEXT_PRIMARY)
                                    .rounded(px(999.0)),
                            ),
                    ),
            )
    }

    /// Set volume with snap-to-100% — if the raw ratio is >= 0.95 we treat it as 1.0.
    fn set_volume_snapped(&mut self, ratio: f64, cx: &mut Context<Self>) {
        let snapped = if ratio >= 0.95 { 1.0 } else { ratio };
        self.set_volume(snapped, cx);
    }

    /// Set volume and update video player
    fn set_volume(&mut self, volume: f64, cx: &mut Context<Self>) {
        self.volume = volume.clamp(0.0, 1.0);
        self.is_muted = false;
        if let Some(video) = &self.video {
            video.set_volume(self.volume);
            video.set_muted(false);
        }
        cx.notify();
    }

    /// Render search results
    fn render_search_results(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let app_state = self.app_state.read(cx);
        let is_searching = app_state.is_searching;
        let query = app_state.search_query.clone();
        let results = app_state.search_results.clone();

        div()
            .id("search-tab")
            .flex_1()
            .flex()
            .flex_col()
            .h_full()
            .bg(theme::BG_PRIMARY)
            .overflow_hidden()
            .child(
                // Fixed header
                div()
                    .px(px(24.0))
                    .pt(px(24.0))
                    .pb(px(16.0))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap(px(16.0))
                    .child(
                        div()
                            .text_color(theme::TEXT_PRIMARY)
                            .text_size(px(24.0))
                            .font_weight(FontWeight::BOLD)
                            .child(format!("Search: \"{}\"", query)),
                    )
                    .child(
                        // Back button to clear search
                        {
                            let app_state = self.app_state.clone();
                            div()
                                .id("clear-search-btn")
                                .px(px(12.0))
                                .py(px(6.0))
                                .bg(theme::BG_TERTIARY)
                                .hover(|style| style.bg(theme::BG_ELEVATED))
                                .rounded(px(4.0))
                                .cursor_pointer()
                                .text_color(theme::TEXT_SECONDARY)
                                .text_size(px(12.0))
                                .on_click(move |_event, _window, cx| {
                                    app_state.update(cx, |state, cx| {
                                        state.clear_search();
                                        cx.notify();
                                    });
                                })
                                .child("Clear Search")
                        },
                    ),
            )
            .child(if is_searching {
                div()
                    .px(px(24.0))
                    .text_color(theme::TEXT_SECONDARY)
                    .text_size(px(14.0))
                    .child("Searching...")
                    .into_any_element()
            } else if results.is_empty() {
                div()
                    .px(px(24.0))
                    .text_color(theme::TEXT_SECONDARY)
                    .text_size(px(14.0))
                    .child("No results found.")
                    .into_any_element()
            } else {
                // Scrollable grid of search results
                let container = div()
                    .id("search-results-grid")
                    .flex_1()
                    .overflow_y_scroll()
                    .px(px(24.0))
                    .pb(px(24.0))
                    .child({
                        let mut grid = div()
                            .w_full()
                            .flex()
                            .flex_wrap()
                            .gap(px(16.0));
                        for channel in results.iter() {
                            grid = grid.child(self.render_stream_card(channel, cx));
                        }
                        grid
                    });
                container.into_any_element()
            })
    }

    /// Render the Following tab
    fn render_following_tab(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let is_logged_in = self.auth_state.read(cx).is_logged_in;
        let app = self.app_state.read(cx);
        let followed = app.followed_channels.clone();
        let is_loading = app.is_loading_followed;

        div()
            .id("following-tab")
            .flex_1()
            .flex()
            .flex_col()
            .h_full()
            .bg(theme::BG_PRIMARY)
            .overflow_hidden()
            .child(
                // Fixed header
                div()
                    .px(px(24.0))
                    .pt(px(24.0))
                    .pb(px(16.0))
                    .flex_shrink_0()
                    .child(
                        div()
                            .text_color(theme::TEXT_PRIMARY)
                            .text_size(px(24.0))
                            .font_weight(FontWeight::BOLD)
                            .child("Following"),
                    ),
            )
            .child(if !is_logged_in {
                div()
                    .px(px(24.0))
                    .text_color(theme::TEXT_SECONDARY)
                    .text_size(px(14.0))
                    .child("Log in to see your followed channels.")
                    .into_any_element()
            } else if is_loading && followed.is_empty() {
                // Only show loading when we have no data yet (initial load)
                div()
                    .px(px(24.0))
                    .text_color(theme::TEXT_SECONDARY)
                    .text_size(px(14.0))
                    .child("Loading followed channels...")
                    .into_any_element()
            } else if followed.is_empty() {
                div()
                    .px(px(24.0))
                    .text_color(theme::TEXT_SECONDARY)
                    .text_size(px(14.0))
                    .child("No channels are live right now.")
                    .into_any_element()
            } else {
                // Scrollable grid of followed channels
                let container = div()
                    .id("following-streams-grid")
                    .flex_1()
                    .overflow_y_scroll()
                    .px(px(24.0))
                    .pb(px(24.0))
                    .child({
                        let mut grid = div()
                            .w_full()
                            .flex()
                            .flex_wrap()
                            .gap(px(16.0));
                        for channel in followed.iter().filter(|c| c.is_live) {
                            grid = grid.child(self.render_stream_card(channel, cx));
                        }
                        grid
                    });
                container.into_any_element()
            })
    }

    /// Render a stream card
    fn render_stream_card(
        &self,
        channel: &FollowedChannel,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let login = channel.login.clone();
        let thumbnail_url = channel
            .thumbnail_url
            .as_ref()
            .map(|url| Self::format_thumbnail_url(url, 320, 180));
        let avatar_url = channel.profile_image_url.clone();
        let stream_title = truncate_with_ellipsis(
            &channel
                .stream_title
                .as_deref()
                .unwrap_or("Live stream"),
            50,
        );

        div()
            .id(SharedString::from(format!("stream-card-{}", channel.login)))
            .flex_basis(px(280.0))
            .flex_grow()
            .flex_shrink()
            .min_w(px(250.0))
            .max_w(px(400.0))
            .bg(theme::BG_SECONDARY)
            .rounded(px(8.0))
            .overflow_hidden()
            .cursor_pointer()
            .hover(|style| style.bg(theme::BG_TERTIARY))
            .on_click(cx.listener(move |this, _event, _window, cx| {
                this.enter_channel(login.clone(), cx);
            }))
            .child(
                // Thumbnail
                if let Some(url) = thumbnail_url {
                    img(url)
                        .w_full()
                        .h(px(170.0))
                        .object_fit(ObjectFit::Cover)
                        .with_loading(|| {
                            div()
                                .w_full()
                                .h(px(170.0))
                                .bg(theme::VIDEO_BG)
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(
                                    div()
                                        .text_color(theme::TEXT_SECONDARY)
                                        .text_size(px(12.0))
                                        .child("Loading..."),
                                )
                                .into_any_element()
                        })
                        .with_fallback(|| {
                            div()
                                .w_full()
                                .h(px(170.0))
                                .bg(theme::VIDEO_BG)
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(
                                    div()
                                        .text_color(theme::TEXT_SECONDARY)
                                        .text_size(px(12.0))
                                        .child("LIVE"),
                                )
                                .into_any_element()
                        })
                        .into_any_element()
                } else {
                    div()
                        .w_full()
                        .h(px(170.0))
                        .bg(theme::VIDEO_BG)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .text_color(theme::TEXT_SECONDARY)
                                .text_size(px(12.0))
                                .child("LIVE"),
                        )
                        .into_any_element()
                },
            )
            .child(
                // Stream info
                div()
                    .p(px(12.0))
                    .flex()
                    .gap(px(10.0))
                    .child(
                        // Avatar
                        if let Some(url) = avatar_url {
                            div()
                                .size(px(40.0))
                                .rounded_full()
                                .overflow_hidden()
                                .bg(theme::BG_TERTIARY)
                                .flex_shrink_0()
                                .child(img(url).w_full().h_full().object_fit(ObjectFit::Cover))
                                .into_any_element()
                        } else {
                            div()
                                .size(px(40.0))
                                .rounded_full()
                                .bg(theme::TWITCH_PURPLE)
                                .flex()
                                .items_center()
                                .justify_center()
                                .flex_shrink_0()
                                .text_color(theme::TEXT_PRIMARY)
                                .text_size(px(14.0))
                                .font_weight(FontWeight::BOLD)
                                .child(
                                    channel
                                        .display_name
                                        .chars()
                                        .next()
                                        .unwrap_or('?')
                                        .to_string(),
                                )
                                .into_any_element()
                        },
                    )
                    .child(
                        div()
                            .flex_1()
                            .overflow_hidden()
                            .child(
                                div()
                                    .text_color(theme::TEXT_PRIMARY)
                                    .text_size(px(14.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .child(truncate_with_ellipsis(&channel.display_name, 30)),
                            )
                            .child(
                                div()
                                    .text_color(theme::TEXT_PRIMARY)
                                    .text_size(px(13.0))
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .child(stream_title),
                            )
                            .child(
                                div()
                                    .text_color(theme::TEXT_SECONDARY)
                                    .text_size(px(12.0))
                                    .overflow_hidden()
                                    .whitespace_nowrap()
                                    .child(truncate_with_ellipsis(
                                        &channel
                                            .game_name
                                            .as_deref()
                                            .unwrap_or("Just Chatting"),
                                        40,
                                    )),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(4.0))
                                    .child(div().size(px(8.0)).rounded_full().bg(theme::LIVE_RED))
                                    .child(
                                        div()
                                            .text_color(theme::TEXT_SECONDARY)
                                            .text_size(px(12.0))
                                            .child(format_viewer_count(
                                                channel.viewer_count.unwrap_or(0),
                                            )),
                                    ),
                            ),
                    ),
            )
    }

    /// Format a Twitch thumbnail URL to a specific size
    fn format_thumbnail_url(url: &str, width: u32, height: u32) -> String {
        url.replace("{width}", &width.to_string())
            .replace("{height}", &height.to_string())
            .replace("%{width}", &width.to_string())
            .replace("%{height}", &height.to_string())
    }

    /// Render the Browse tab
    fn render_browse_tab(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let app_state = self.app_state.read(cx);
        let is_loading = app_state.is_loading_browse;
        let top_streams = app_state.top_streams.clone();
        let browse_loaded = app_state.browse_loaded;

        div()
            .id("browse-tab")
            .flex_1()
            .flex()
            .flex_col()
            .h_full()
            .bg(theme::BG_PRIMARY)
            .overflow_hidden()
            .child(
                // Fixed header
                div()
                    .px(px(24.0))
                    .pt(px(24.0))
                    .pb(px(16.0))
                    .flex_shrink_0()
                    .child(
                        div()
                            .text_color(theme::TEXT_PRIMARY)
                            .text_size(px(24.0))
                            .font_weight(FontWeight::BOLD)
                            .child("Browse"),
                    ),
            )
            .child(if is_loading && top_streams.is_empty() {
                // Only show loading when we have no data yet (initial load)
                div()
                    .px(px(24.0))
                    .text_color(theme::TEXT_SECONDARY)
                    .text_size(px(14.0))
                    .child("Loading top streams...")
                    .into_any_element()
            } else if !browse_loaded && top_streams.is_empty() {
                div()
                    .px(px(24.0))
                    .text_color(theme::TEXT_SECONDARY)
                    .text_size(px(14.0))
                    .child("Click to load top streams")
                    .into_any_element()
            } else if top_streams.is_empty() {
                div()
                    .px(px(24.0))
                    .text_color(theme::TEXT_SECONDARY)
                    .text_size(px(14.0))
                    .child("No streams found.")
                    .into_any_element()
            } else {
                // Scrollable grid of top streams
                let container = div()
                    .id("browse-streams-grid")
                    .flex_1()
                    .overflow_y_scroll()
                    .px(px(24.0))
                    .pb(px(24.0))
                    .child({
                        let mut grid = div()
                            .w_full()
                            .flex()
                            .flex_wrap()
                            .gap(px(16.0));
                        for stream in top_streams.iter() {
                            grid = grid.child(self.render_stream_card(stream, cx));
                        }
                        grid
                    });
                container.into_any_element()
            })
    }
}

impl Render for SecousseApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let app_state = self.app_state.read(cx);
        let is_fullscreen = app_state.is_fullscreen;
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
            .track_focus(&self.root_focus)
            .on_action(cx.listener(|this, _action: &actions::ToggleFullscreen, _window, cx| {
                this.app_state.update(cx, |state, cx| {
                    state.toggle_fullscreen();
                    cx.notify();
                });
            }))
            .on_action(cx.listener(|this, _action: &actions::ExitFullscreen, _window, cx| {
                let is_fullscreen = this.app_state.read(cx).is_fullscreen;
                if is_fullscreen {
                    this.app_state.update(cx, |state, cx| {
                        state.toggle_fullscreen();
                        cx.notify();
                    });
                }
            }))
            // Navbar (hidden in fullscreen)
            .when(!is_fullscreen, |el| {
                el.child(
                    div()
                        .h(px(theme::NAVBAR_HEIGHT))
                        .w_full()
                        .bg(theme::BG_SECONDARY)
                        .border_b_1()
                        .border_color(theme::BORDER_SUBTLE)
                        .child(self.navbar.clone()),
                )
            })
            .child(
                // Main content area (sidebar + content)
                div()
                    .flex()
                    .flex_1()
                    .overflow_hidden()
                    // Sidebar (hidden in fullscreen)
                    .when(!is_fullscreen, |el: Div| {
                        el.child(
                            div()
                                .w(px(sidebar_width))
                                .h_full()
                                .bg(theme::BG_SECONDARY)
                                .border_r_1()
                                .border_color(theme::BORDER_SUBTLE)
                                .flex_shrink_0()
                                .child(self.sidebar.clone()),
                        )
                    })
                    .child(
                        // Main content
                        div()
                            .flex_1()
                            .h_full()
                            .overflow_hidden()
                            .child(self.render_main_content(cx)),
                    ),
            )
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

/// Truncate a string to at most `max_chars` characters, appending "…" if truncated.
/// Always respects char boundaries (safe for emoji/multi-byte).
fn truncate_with_ellipsis(text: &str, max_chars: usize) -> String {
    let char_count = text.chars().count();
    if char_count <= max_chars {
        text.to_string()
    } else {
        let truncated: String = text.chars().take(max_chars).collect();
        format!("{}…", truncated)
    }
}
