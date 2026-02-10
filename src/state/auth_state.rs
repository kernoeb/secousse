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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_auth_state() {
        let auth = AuthState::new("test-device-id".to_string());
        assert!(!auth.is_logged_in);
        assert!(auth.access_token.is_none());
        assert!(auth.self_info.is_none());
        assert_eq!(auth.device_id, "test-device-id");
        assert!(!auth.is_validating);
        assert!(!auth.is_authenticating);
    }

    #[test]
    fn test_from_settings_with_token() {
        let auth =
            AuthState::from_settings("device-123".to_string(), Some("token-abc".to_string()));
        assert!(auth.is_logged_in);
        assert_eq!(auth.access_token, Some("token-abc".to_string()));
        assert!(auth.self_info.is_none()); // User info not fetched yet
    }

    #[test]
    fn test_from_settings_without_token() {
        let auth = AuthState::from_settings("device-123".to_string(), None);
        assert!(!auth.is_logged_in);
        assert!(auth.access_token.is_none());
    }

    #[test]
    fn test_set_logged_in() {
        let mut auth = AuthState::new("device".to_string());
        auth.is_validating = true;
        auth.is_authenticating = true;

        auth.set_logged_in(
            "new-token".to_string(),
            SelfInfo {
                id: "12345".to_string(),
                login: "testuser".to_string(),
                display_name: "TestUser".to_string(),
                profile_image_url: Some("https://example.com/avatar.png".to_string()),
            },
        );

        assert!(auth.is_logged_in);
        assert_eq!(auth.access_token, Some("new-token".to_string()));
        assert!(!auth.is_validating);
        assert!(!auth.is_authenticating);

        let info = auth.self_info.as_ref().unwrap();
        assert_eq!(info.id, "12345");
        assert_eq!(info.login, "testuser");
        assert_eq!(info.display_name, "TestUser");
    }

    #[test]
    fn test_logout() {
        let mut auth = AuthState::from_settings("device".to_string(), Some("token".to_string()));
        auth.self_info = Some(SelfInfo {
            id: "123".to_string(),
            login: "user".to_string(),
            display_name: "User".to_string(),
            profile_image_url: None,
        });
        auth.is_validating = true;

        auth.logout();

        assert!(!auth.is_logged_in);
        assert!(auth.access_token.is_none());
        assert!(auth.self_info.is_none());
        assert!(!auth.is_validating);
    }

    #[test]
    fn test_display_name() {
        let mut auth = AuthState::new("device".to_string());
        assert!(auth.display_name().is_none());

        auth.self_info = Some(SelfInfo {
            id: "123".to_string(),
            login: "mylogin".to_string(),
            display_name: "MyDisplayName".to_string(),
            profile_image_url: None,
        });

        assert_eq!(auth.display_name(), Some("MyDisplayName"));
    }

    #[test]
    fn test_user_id_and_login() {
        let mut auth = AuthState::new("device".to_string());
        assert!(auth.user_id().is_none());
        assert!(auth.login().is_none());

        auth.self_info = Some(SelfInfo {
            id: "user-id-123".to_string(),
            login: "username".to_string(),
            display_name: "DisplayName".to_string(),
            profile_image_url: None,
        });

        assert_eq!(auth.user_id(), Some("user-id-123"));
        assert_eq!(auth.login(), Some("username"));
    }
}
