//! Main application state
#![allow(dead_code)]
//!
//! Contains the root state that holds references to all sub-states.

use super::{AuthState, Settings};
use gpui::Entity;

/// Active navigation tab
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ActiveTab {
    #[default]
    Following,
    Browse,
}

/// Main application state container
pub struct AppState {
    /// Currently selected channel (login name)
    pub current_channel: Option<String>,

    /// Active navigation tab
    pub active_tab: ActiveTab,

    /// Whether sidebar is expanded
    pub is_sidebar_open: bool,

    /// Whether chat panel is visible
    pub is_chat_open: bool,

    /// Whether video is in fullscreen mode
    pub is_fullscreen: bool,

    /// Authentication state entity
    pub auth: Entity<AuthState>,

    /// Application settings entity
    pub settings: Entity<Settings>,

    /// List of followed channels (cached)
    pub followed_channels: Vec<FollowedChannel>,

    /// Whether we're currently loading followed channels
    pub is_loading_followed: bool,

    /// List of top streams for Browse tab
    pub top_streams: Vec<FollowedChannel>,

    /// Whether we're currently loading top streams
    pub is_loading_browse: bool,

    /// Whether top streams have been fetched at least once
    pub browse_loaded: bool,

    /// Current search query
    pub search_query: String,

    /// Search results
    pub search_results: Vec<FollowedChannel>,

    /// Whether we're currently searching
    pub is_searching: bool,

    /// Whether search is active (showing results)
    pub search_active: bool,
}

/// A followed channel with live status
#[derive(Debug, Clone)]
pub struct FollowedChannel {
    pub id: String,
    pub login: String,
    pub display_name: String,
    pub profile_image_url: Option<String>,
    pub is_live: bool,
    pub viewer_count: Option<u32>,
    pub game_name: Option<String>,
    pub stream_title: Option<String>,
    pub thumbnail_url: Option<String>,
}

impl AppState {
    /// Create a new app state with the given sub-entities
    pub fn new(
        auth: Entity<AuthState>,
        settings: Entity<Settings>,
        is_sidebar_open: bool,
        is_chat_open: bool,
    ) -> Self {
        Self {
            current_channel: None,
            active_tab: ActiveTab::default(),
            is_sidebar_open,
            is_chat_open,
            is_fullscreen: false,
            auth,
            settings,
            followed_channels: Vec::new(),
            is_loading_followed: false,
            top_streams: Vec::new(),
            is_loading_browse: false,
            browse_loaded: false,
            search_query: String::new(),
            search_results: Vec::new(),
            is_searching: false,
            search_active: false,
        }
    }

    /// Set the current channel
    pub fn set_channel(&mut self, channel: Option<String>) {
        self.current_channel = channel;
    }

    /// Switch to a specific tab
    pub fn set_tab(&mut self, tab: ActiveTab) {
        self.active_tab = tab;
    }

    /// Toggle sidebar visibility
    pub fn toggle_sidebar(&mut self) {
        self.is_sidebar_open = !self.is_sidebar_open;
    }

    /// Toggle chat panel visibility
    pub fn toggle_chat(&mut self) {
        self.is_chat_open = !self.is_chat_open;
    }

    /// Toggle fullscreen mode
    pub fn toggle_fullscreen(&mut self) {
        self.is_fullscreen = !self.is_fullscreen;
    }

    /// Update followed channels list
    pub fn set_followed_channels(&mut self, channels: Vec<FollowedChannel>) {
        self.followed_channels = channels;
        self.is_loading_followed = false;
    }

    /// Get online followed channels count
    pub fn online_followed_count(&self) -> usize {
        self.followed_channels.iter().filter(|c| c.is_live).count()
    }

    /// Update top streams list
    pub fn set_top_streams(&mut self, streams: Vec<FollowedChannel>) {
        self.top_streams = streams;
        self.is_loading_browse = false;
        self.browse_loaded = true;
    }

    /// Check if browse tab needs to fetch data
    pub fn needs_browse_fetch(&self) -> bool {
        (!self.browse_loaded || self.top_streams.is_empty()) && !self.is_loading_browse
    }

    /// Set search query
    pub fn set_search_query(&mut self, query: String) {
        self.search_query = query;
    }

    /// Set search results
    pub fn set_search_results(&mut self, results: Vec<FollowedChannel>) {
        let has_results = !results.is_empty();
        self.search_results = results;
        self.is_searching = false;
        self.search_active = has_results || !self.search_query.is_empty();
    }

    /// Clear search
    pub fn clear_search(&mut self) {
        self.search_query.clear();
        self.search_results.clear();
        self.is_searching = false;
        self.search_active = false;
    }
}
