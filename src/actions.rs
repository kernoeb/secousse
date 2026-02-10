//! Keyboard actions and shortcuts
//!
//! Defines all keyboard shortcuts and actions for the application.

use gpui::*;

// Navigation actions
actions!(
    secousse,
    [
        // Tab navigation
        GoToFollowing,
        GoToBrowse,
        // UI toggles
        ToggleSidebar,
        ToggleChat,
        ToggleFullscreen,
        // Video controls
        TogglePlayPause,
        ToggleMute,
        VolumeUp,
        VolumeDown,
        // Chat
        FocusChatInput,
        ScrollToBottom,
        // Search
        FocusSearch,
        ClearSearch,
        // App
        Quit,
        Refresh,
        ExitFullscreen,
        // Authentication
        StartLogin,
        Logout,
    ]
);

/// Register default keybindings
pub fn register_keybindings(cx: &mut App) {
    cx.bind_keys([
        // Tab navigation
        KeyBinding::new("cmd-1", GoToFollowing, None),
        KeyBinding::new("cmd-2", GoToBrowse, None),
        // UI toggles
        KeyBinding::new("cmd-b", ToggleSidebar, None),
        KeyBinding::new("cmd-shift-c", ToggleChat, None),
        KeyBinding::new("cmd-shift-f", ToggleFullscreen, None),
        // Video controls (when video player is focused)
        KeyBinding::new("space", TogglePlayPause, Some("video-player")),
        KeyBinding::new("m", ToggleMute, Some("video-player")),
        KeyBinding::new("f", ToggleFullscreen, Some("video-player")),
        KeyBinding::new("up", VolumeUp, Some("video-player")),
        KeyBinding::new("down", VolumeDown, Some("video-player")),
        // Search
        KeyBinding::new("cmd-k", FocusSearch, None),
        KeyBinding::new("escape", ClearSearch, Some("search")),
        // Fullscreen
        KeyBinding::new("escape", ExitFullscreen, None),
        // App
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("cmd-r", Refresh, None),
    ]);
}
