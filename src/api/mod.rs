//! API client modules for Twitch and third-party services
//!
//! This module contains all HTTP and WebSocket clients for interacting with:
//! - Twitch GQL API (internal)
//! - Twitch Helix API (official)
//! - 7TV, BTTV, FFZ emote APIs
//! - Twitch IRC chat

pub mod chat;
pub mod emotes;
pub mod oauth;
pub mod twitch;

pub use oauth::start_oauth_flow;
pub use twitch::{StreamQuality, TwitchClient};
