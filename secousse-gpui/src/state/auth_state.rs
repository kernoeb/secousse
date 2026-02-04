//! Authentication state management
//!
//! Handles OAuth tokens, user info, and login status.

use serde::{Deserialize, Serialize};

/// Information about the currently logged-in user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelfInfo {
    pub id: String,
    pub login: String,
    pub display_name: String,
    pub profile_image_url: Option<String>,
}

/// Authentication state for the application
#[derive(Debug, Clone)]
pub struct AuthState {
    /// Whether the user is currently logged in
    pub is_logged_in: bool,

    /// OAuth access token
    pub access_token: Option<String>,

    /// Current user information (fetched after login)
    pub self_info: Option<SelfInfo>,

    /// Device ID for Twitch API
    pub device_id: String,

    /// Whether we're currently validating the token
    pub is_validating: bool,

    /// Whether we're currently in the OAuth flow
    pub is_authenticating: bool,
}

impl AuthState {
    /// Create a new auth state with the given device ID
    pub fn new(device_id: String) -> Self {
        Self {
            is_logged_in: false,
            access_token: None,
            self_info: None,
            device_id,
            is_validating: false,
            is_authenticating: false,
        }
    }

    /// Create auth state from saved settings
    pub fn from_settings(device_id: String, access_token: Option<String>) -> Self {
        Self {
            is_logged_in: access_token.is_some(),
            access_token,
            self_info: None,
            device_id,
            is_validating: false,
            is_authenticating: false,
        }
    }

    /// Set logged in state with token and user info
    pub fn set_logged_in(&mut self, token: String, info: SelfInfo) {
        self.is_logged_in = true;
        self.access_token = Some(token);
        self.self_info = Some(info);
        self.is_validating = false;
        self.is_authenticating = false;
    }

    /// Clear login state
    pub fn logout(&mut self) {
        self.is_logged_in = false;
        self.access_token = None;
        self.self_info = None;
        self.is_validating = false;
        self.is_authenticating = false;
    }

    /// Get the user's display name if logged in
    pub fn display_name(&self) -> Option<&str> {
        self.self_info.as_ref().map(|i| i.display_name.as_str())
    }

    /// Get the user's ID if logged in
    pub fn user_id(&self) -> Option<&str> {
        self.self_info.as_ref().map(|i| i.id.as_str())
    }

    /// Get the user's login (username) if logged in
    pub fn login(&self) -> Option<&str> {
        self.self_info.as_ref().map(|i| i.login.as_str())
    }
}

impl Default for AuthState {
    fn default() -> Self {
        Self::new(String::new())
    }
}
