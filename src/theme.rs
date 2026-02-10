//! Twitch-themed color palette and styling constants for Secousse
//!
//! These colors are based on Twitch's dark mode design system.

use gpui::Rgba;

/// Convert a hex color to GPUI Rgba
pub const fn rgb(hex: u32) -> Rgba {
    Rgba {
        r: ((hex >> 16) & 0xFF) as f32 / 255.0,
        g: ((hex >> 8) & 0xFF) as f32 / 255.0,
        b: (hex & 0xFF) as f32 / 255.0,
        a: 1.0,
    }
}

/// Convert a hex color with alpha to GPUI Rgba
pub const fn rgba(hex: u32, alpha: f32) -> Rgba {
    Rgba {
        r: ((hex >> 16) & 0xFF) as f32 / 255.0,
        g: ((hex >> 8) & 0xFF) as f32 / 255.0,
        b: (hex & 0xFF) as f32 / 255.0,
        a: alpha,
    }
}

// ============================================================================
// Background Colors
// ============================================================================

/// Main application background (darkest)
pub const BG_PRIMARY: Rgba = rgb(0x0e0e10);

/// Sidebar, cards, panels background
pub const BG_SECONDARY: Rgba = rgb(0x18181b);

/// Hover states, elevated surfaces
pub const BG_TERTIARY: Rgba = rgb(0x1f1f23);

/// Menus, dropdowns, modals
pub const BG_ELEVATED: Rgba = rgb(0x26262c);

// ============================================================================
// Text Colors
// ============================================================================

/// Primary text (white-ish)
pub const TEXT_PRIMARY: Rgba = rgb(0xefeff1);

/// Secondary/muted text
pub const TEXT_SECONDARY: Rgba = rgb(0xadadb8);

/// Disabled/placeholder text
pub const TEXT_DISABLED: Rgba = rgb(0x71717a);

// ============================================================================
// Brand Colors
// ============================================================================

/// Twitch Purple - primary brand color
pub const TWITCH_PURPLE: Rgba = rgb(0x9146ff);

// ============================================================================
// Status Colors
// ============================================================================

/// Live indicator red
pub const LIVE_RED: Rgba = rgb(0xeb0400);

/// Success/online green
pub const SUCCESS_GREEN: Rgba = rgb(0x00c853);

/// Offline/away gray
pub const OFFLINE_GRAY: Rgba = rgb(0x6b6b6b);

/// Subtle border for cards
pub const BORDER_SUBTLE: Rgba = rgb(0x27272a);

// ============================================================================
// Special Colors
// ============================================================================

/// Video player background (pure black)
pub const VIDEO_BG: Rgba = rgb(0x000000);

/// Selected/active item background
pub const SELECTED_BG: Rgba = rgba(0x9146ff, 0.2);

/// Transparent background (for unselected states)
pub const TRANSPARENT: Rgba = Rgba {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 0.0,
};

// ============================================================================
// Sizing Constants
// ============================================================================

/// Navbar height in pixels (38px — traffic lights sit near-center without
/// needing custom `traffic_light_position`, which breaks macOS hover tracking)
pub const NAVBAR_HEIGHT: f32 = 38.0;

/// macOS traffic light buttons inset (width needed to clear close/minimize/zoom buttons)
/// Traffic lights are positioned at (9, 9) and each button is ~14px with ~6px spacing
pub const TRAFFIC_LIGHT_INSET: f32 = 78.0;

/// Sidebar width when expanded
pub const SIDEBAR_WIDTH: f32 = 240.0;

/// Sidebar width when collapsed
pub const SIDEBAR_COLLAPSED_WIDTH: f32 = 50.0;

/// Chat panel default width
pub const CHAT_WIDTH: f32 = 340.0;
