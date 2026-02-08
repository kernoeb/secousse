//! Secousse - A Twitch streaming client built with GPUI
//!
//! This is the main entry point for the application.

mod actions;
mod api;
mod app;
mod assets;
mod http;
mod state;
mod theme;
mod video;
mod views;

use app::SecousseApp;
use gpui::*;
use gpui_component::{Root, Theme, ThemeRegistry};
use log::info;
use state::Settings;
use std::sync::OnceLock;

/// Set the macOS Dock icon from an embedded PNG asset.
/// This is needed because `cargo run` doesn't produce a `.app` bundle
/// with an Info.plist, so macOS shows a generic executable icon.
#[cfg(target_os = "macos")]
fn set_dock_icon() {
    use cocoa::appkit::{NSApp, NSApplication, NSImage};
    use cocoa::base::nil;
    use cocoa::foundation::NSData;
    let Some(icon_data) = assets::Assets::get("app-icon.png") else {
        log::warn!("app-icon.png not found in embedded assets");
        return;
    };

    unsafe {
        let data = NSData::dataWithBytes_length_(
            nil,
            icon_data.data.as_ptr() as *const std::ffi::c_void,
            icon_data.data.len() as u64,
        );
        let icon = NSImage::initWithData_(NSImage::alloc(nil), data);
        if icon != nil {
            NSApplication::setApplicationIconImage_(NSApp(), icon);
        }
    }
}



/// Global tokio runtime for async operations
/// This is needed because GPUI uses smol but our HTTP client (reqwest) requires tokio.
/// The runtime stays alive for the entire application lifetime.
static TOKIO_RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

/// Get or create the global tokio runtime
pub fn tokio_runtime() -> &'static tokio::runtime::Runtime {
    TOKIO_RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to create tokio runtime")
    })
}

fn main() {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    info!("Starting Secousse GPUI...");

    // Initialize the global tokio runtime early
    // This ensures it's available for all async operations
    let _runtime = tokio_runtime();
    info!("Tokio runtime initialized");

    // Warm up GStreamer in the background to reduce first-play latency
    video::warmup_gstreamer();

    // Load settings
    let settings = Settings::load();
    info!("Loaded settings from {:?}", Settings::settings_path());

    // Initialize GPUI application with embedded assets (icons, etc.)
    let http_client =
        http::create_http_client("secousse").expect("Failed to create HTTP client");

    Application::new()
        .with_assets(assets::Assets)
        .with_http_client(http_client)
        .run(|cx: &mut App| {

        // Set the Dock icon (macOS only — needed for cargo run without .app bundle)
        #[cfg(target_os = "macos")]
        set_dock_icon();

        // Initialize GPUI Component
        gpui_component::init(cx);

        // Force a dark theme if available
        let theme_name = SharedString::from("Ayu Dark");
        let registry = ThemeRegistry::global(cx);
        let theme = registry
            .themes()
            .get(&theme_name)
            .cloned()
            .or_else(|| {
                registry
                    .themes()
                    .iter()
                    .find(|(name, _)| name.as_ref().to_ascii_lowercase().contains("dark"))
                    .map(|(_, theme)| theme.clone())
            });
        if let Some(theme) = theme {
            Theme::global_mut(cx).apply_config(&theme);
        } else {
            log::warn!("No dark theme found in registry");
        }

        // Register keybindings
        actions::register_keybindings(cx);

        // Handle Quit action (Cmd+Q)
        cx.on_action::<actions::Quit>(|_action, cx| {
            cx.quit();
        });

        // Create the main window
        let bounds = Bounds::centered(None, size(px(1400.0), px(900.0)), cx);

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Secousse".into()),
                    appears_transparent: true,
                    // Nudge traffic lights down slightly so they're more centered
                    // in the 38px navbar. Keep y ≤ 12 to stay within the default
                    // ~28px container view (avoids broken hover tracking).
                    traffic_light_position: Some(point(px(9.0), px(12.0))),
                }),
                window_background: WindowBackgroundAppearance::Blurred,
                focus: true,
                show: true,
                kind: WindowKind::Normal,
                is_movable: true,
                display_id: None,
                window_min_size: Some(size(px(800.0), px(600.0))),
                window_decorations: Some(WindowDecorations::Server),
                app_id: Some("com.secousse.app".to_string()),
                is_minimizable: true,
                is_resizable: true,
                tabbing_identifier: None,
            },
            |window: &mut Window, cx: &mut App| {
                // Quit the entire app when the window is closed (red traffic light)
                window.on_window_should_close(cx, |_window, cx| {
                    cx.quit();
                    true
                });

                // Create the app with settings
                let app = cx.new(|cx| SecousseApp::new(settings, window, cx));
                cx.new(|cx| Root::new(app, window, cx))
            },
        )
        .expect("Failed to open window");
    });
}
