//! UI Views
//!
//! This module contains all the main view components of the application.

pub mod chat;
pub mod navbar;
pub mod sidebar;

pub use chat::{ChatView, ChatViewEvent};
pub use navbar::{NavbarEvent, NavbarView};
pub use sidebar::{SidebarEvent, SidebarView};
