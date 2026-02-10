//! Application state management
//!
//! This module contains all the state entities used throughout the application.

pub mod app_state;
pub mod auth_state;
pub mod settings;

pub use app_state::{ActiveTab, AppState, FollowedChannel};
pub use auth_state::AuthState;
pub use settings::Settings;
