//! Icon component for rendering Lucide SVG icons.
//!
//! Adapted from gpui-component (https://github.com/longbridge/gpui-component)
//! Licensed under Apache-2.0.
//!
//! Uses GPUI's built-in `svg()` element with icon paths loaded via AssetSource.

#![allow(dead_code)]

use gpui::{
    prelude::FluentBuilder as _, svg, AnyElement, App, Hsla, IntoElement, Pixels, RenderOnce,
    SharedString, StyleRefinement, Styled, Svg, Transformation, Window,
};

/// The name of a Lucide icon in the asset bundle.
///
/// Each variant maps to an SVG file in `assets/icons/`.
/// Based on the icons used in the original Secousse React app.
#[derive(IntoElement, Clone, Debug)]
pub enum IconName {
    // --- Icons from the original Secousse React version ---
    Search,
    User,
    LogIn,
    X,
    PanelLeft,
    PanelRight,
    Settings,
    Send,
    Heart,
    HeartOff,
    Video,
    Play,
    Pause,
    Volume1,
    Volume2,
    VolumeX,
    Maximize,
    Minimize,
    Loader,
    // --- Additional useful icons ---
    ChevronDown,
    ChevronUp,
    ChevronLeft,
    ChevronRight,
    Eye,
    EyeOff,
    Menu,
    Plus,
    Minus,
    Check,
    Close,
    Star,
    Globe,
    ArrowLeft,
    ArrowRight,
    ExternalLink,
    Info,
}

impl IconName {
    /// Returns the asset path for this icon name.
    pub fn path(&self) -> SharedString {
        match self {
            Self::Search => "icons/search.svg",
            Self::User => "icons/user.svg",
            Self::LogIn => "icons/log-in.svg",
            Self::X => "icons/x.svg",
            Self::PanelLeft => "icons/panel-left.svg",
            Self::PanelRight => "icons/panel-right.svg",
            Self::Settings => "icons/settings.svg",
            Self::Send => "icons/send.svg",
            Self::Heart => "icons/heart.svg",
            Self::HeartOff => "icons/heart-off.svg",
            Self::Video => "icons/video.svg",
            Self::Play => "icons/play.svg",
            Self::Pause => "icons/pause.svg",
            Self::Volume1 => "icons/volume-1.svg",
            Self::Volume2 => "icons/volume-2.svg",
            Self::VolumeX => "icons/volume-x.svg",
            Self::Maximize => "icons/maximize.svg",
            Self::Minimize => "icons/minimize.svg",
            Self::Loader => "icons/loader.svg",
            Self::ChevronDown => "icons/chevron-down.svg",
            Self::ChevronUp => "icons/chevron-up.svg",
            Self::ChevronLeft => "icons/chevron-left.svg",
            Self::ChevronRight => "icons/chevron-right.svg",
            Self::Eye => "icons/eye.svg",
            Self::EyeOff => "icons/eye-off.svg",
            Self::Menu => "icons/menu.svg",
            Self::Plus => "icons/plus.svg",
            Self::Minus => "icons/minus.svg",
            Self::Check => "icons/check.svg",
            Self::Close => "icons/x.svg", // Same as X
            Self::Star => "icons/star.svg",
            Self::Globe => "icons/globe.svg",
            Self::ArrowLeft => "icons/arrow-left.svg",
            Self::ArrowRight => "icons/arrow-right.svg",
            Self::ExternalLink => "icons/external-link.svg",
            Self::Info => "icons/info.svg",
        }
        .into()
    }
}

impl RenderOnce for IconName {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        Icon::new(self)
    }
}

impl From<IconName> for AnyElement {
    fn from(val: IconName) -> Self {
        Icon::new(val).into_any_element()
    }
}

/// An icon element that renders an SVG from the asset bundle.
///
/// Usage:
/// ```
/// Icon::new(IconName::Search).size_4().text_color(gpui::white())
/// ```
#[derive(IntoElement)]
pub struct Icon {
    base: Svg,
    style: StyleRefinement,
    path: SharedString,
    color: Option<Hsla>,
    icon_size: Option<IconSize>,
}

/// Predefined icon sizes
#[derive(Clone, Copy, Debug)]
pub enum IconSize {
    /// 12px
    XSmall,
    /// 14px
    Small,
    /// 16px (default)
    Medium,
    /// 24px
    Large,
    /// Custom pixel size
    Custom(Pixels),
}

impl Default for Icon {
    fn default() -> Self {
        Self {
            base: svg().flex_none(),
            style: StyleRefinement::default(),
            path: "".into(),
            color: None,
            icon_size: None,
        }
    }
}

impl Icon {
    /// Create a new icon from an IconName.
    pub fn new(name: IconName) -> Self {
        Self::default().path(name.path())
    }

    /// Create an icon from a custom SVG asset path.
    pub fn from_path(path: impl Into<SharedString>) -> Self {
        Self::default().path(path.into())
    }

    /// Set the SVG asset path.
    pub fn path(mut self, path: impl Into<SharedString>) -> Self {
        self.path = path.into();
        self
    }

    /// Set the icon color.
    pub fn color(mut self, color: impl Into<Hsla>) -> Self {
        self.color = Some(color.into());
        self
    }

    /// Set a predefined icon size.
    pub fn with_size(mut self, size: IconSize) -> Self {
        self.icon_size = Some(size);
        self
    }

    /// Rotate the icon.
    pub fn rotate(mut self, radians: impl Into<gpui::Radians>) -> Self {
        self.base = self
            .base
            .with_transformation(Transformation::rotate(radians));
        self
    }
}

impl Styled for Icon {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl RenderOnce for Icon {
    fn render(self, window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let text_color = self.color.unwrap_or_else(|| window.text_style().color);
        let has_explicit_size = self.style.size.width.is_some() || self.style.size.height.is_some();

        let mut base = self.base;
        *base.style() = self.style;

        base.flex_shrink_0()
            .text_color(text_color)
            .when(!has_explicit_size, |this: Svg| {
                if let Some(size) = self.icon_size {
                    match size {
                        IconSize::XSmall => this.size_3(),
                        IconSize::Small => this.size_3p5(),
                        IconSize::Medium => this.size_4(),
                        IconSize::Large => this.size_6(),
                        IconSize::Custom(px) => this.size(px),
                    }
                } else {
                    this.size_4() // default 16px
                }
            })
            .path(self.path)
    }
}

impl From<Icon> for AnyElement {
    fn from(val: Icon) -> Self {
        val.into_any_element()
    }
}
