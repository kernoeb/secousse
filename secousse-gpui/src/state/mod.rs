//! Application state management
//!
//! This module contains all the state entities used throughout the application.

pub mod settings;
pub mod app_state;
pub mod auth_state;

pub use settings::Settings;
pub use app_state::{AppState, ActiveTab};
pub use auth_state::AuthState;
