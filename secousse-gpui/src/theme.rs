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

/// Input fields, search boxes
pub const BG_INPUT: Rgba = rgb(0x3f3f46);

// ============================================================================
// Text Colors
// ============================================================================

/// Primary text (white-ish)
pub const TEXT_PRIMARY: Rgba = rgb(0xefeff1);

/// Secondary/muted text
pub const TEXT_SECONDARY: Rgba = rgb(0xadadb8);

/// Disabled/placeholder text
pub const TEXT_DISABLED: Rgba = rgb(0x71717a);

/// Links and interactive text
pub const TEXT_LINK: Rgba = rgb(0xbf94ff);

// ============================================================================
// Brand Colors
// ============================================================================

/// Twitch Purple - primary brand color
pub const TWITCH_PURPLE: Rgba = rgb(0x9146ff);

/// Twitch Purple hover state
pub const TWITCH_PURPLE_HOVER: Rgba = rgb(0x772ce8);

/// Twitch Purple active/pressed state
pub const TWITCH_PURPLE_ACTIVE: Rgba = rgb(0x5c16c5);

// ============================================================================
// Status Colors
// ============================================================================

/// Live indicator red
pub const LIVE_RED: Rgba = rgb(0xeb0400);

/// Success/online green
pub const SUCCESS_GREEN: Rgba = rgb(0x00c853);

/// Warning yellow
pub const WARNING_YELLOW: Rgba = rgb(0xffca28);

/// Error red
pub const ERROR_RED: Rgba = rgb(0xff4444);

/// Offline/away gray
pub const OFFLINE_GRAY: Rgba = rgb(0x6b6b6b);

// ============================================================================
// Border Colors
// ============================================================================

/// Default border color
pub const BORDER_DEFAULT: Rgba = rgb(0x3f3f46);

/// Focus border (purple accent)
pub const BORDER_FOCUS: Rgba = rgb(0x9146ff);

/// Subtle border for cards
pub const BORDER_SUBTLE: Rgba = rgb(0x27272a);

// ============================================================================
// Special Colors
// ============================================================================

/// Video player background (pure black)
pub const VIDEO_BG: Rgba = rgb(0x000000);

/// Chat message hover background
pub const CHAT_HOVER: Rgba = rgba(0xffffff, 0.05);

/// Selected/active item background
pub const SELECTED_BG: Rgba = rgba(0x9146ff, 0.2);

// ============================================================================
// Sizing Constants
// ============================================================================

/// Navbar height in pixels
pub const NAVBAR_HEIGHT: f32 = 50.0;

/// Sidebar width when expanded
pub const SIDEBAR_WIDTH: f32 = 240.0;

/// Sidebar width when collapsed
pub const SIDEBAR_COLLAPSED_WIDTH: f32 = 50.0;

/// Chat panel default width
pub const CHAT_WIDTH: f32 = 340.0;

/// Standard border radius
pub const BORDER_RADIUS: f32 = 4.0;

/// Rounded border radius (buttons, badges)
pub const BORDER_RADIUS_ROUNDED: f32 = 8.0;

/// Full rounded (pills, avatars)
pub const BORDER_RADIUS_FULL: f32 = 9999.0;

// ============================================================================
// Spacing Constants (based on 4px grid)
// ============================================================================

pub const SPACING_1: f32 = 4.0;
pub const SPACING_2: f32 = 8.0;
pub const SPACING_3: f32 = 12.0;
pub const SPACING_4: f32 = 16.0;
pub const SPACING_5: f32 = 20.0;
pub const SPACING_6: f32 = 24.0;
pub const SPACING_8: f32 = 32.0;

// ============================================================================
// Font Sizes
// ============================================================================

pub const FONT_SIZE_XS: f32 = 11.0;
pub const FONT_SIZE_SM: f32 = 13.0;
pub const FONT_SIZE_BASE: f32 = 14.0;
pub const FONT_SIZE_LG: f32 = 16.0;
pub const FONT_SIZE_XL: f32 = 18.0;
pub const FONT_SIZE_2XL: f32 = 24.0;

// ============================================================================
// Animation Durations (in seconds)
// ============================================================================

pub const TRANSITION_FAST: f32 = 0.1;
pub const TRANSITION_NORMAL: f32 = 0.2;
pub const TRANSITION_SLOW: f32 = 0.3;
