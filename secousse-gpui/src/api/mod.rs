//! API client modules for Twitch and third-party services
//!
//! This module contains all HTTP and WebSocket clients for interacting with:
//! - Twitch GQL API (internal)
//! - Twitch Helix API (official)
//! - 7TV, BTTV, FFZ emote APIs
//! - Twitch IRC chat

pub mod twitch;
pub mod emotes;
pub mod chat;

pub use twitch::TwitchClient;
pub use emotes::{Emote, fetch_global_emotes, fetch_channel_emotes};
pub use chat::{ChatConnection, ChatMessage, connect_chat};
