//! Secousse - A Twitch streaming client built with GPUI
//!
//! This is the main entry point for the application.

mod api;
mod app;
mod state;
mod theme;
mod views;
mod components;
mod actions;

use app::SecousseApp;
use gpui::*;
use log::info;
use state::Settings;

fn main() {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    info!("Starting Secousse GPUI...");

    // Load settings
    let settings = Settings::load();
    info!("Loaded settings from {:?}", Settings::settings_path());

    // Initialize GPUI application
    App::new().run(|cx: &mut AppContext| {
        // Create the main window
        let bounds = Bounds::centered(None, size(px(1400.0), px(900.0)), cx);
        
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Secousse".into()),
                    appears_transparent: true,
                    traffic_light_position: Some(point(px(9.0), px(9.0))),
                }),
                window_background: WindowBackgroundAppearance::Blurred,
                focus: true,
                show: true,
                kind: WindowKind::Normal,
                is_movable: true,
                display_id: None,
                window_min_size: Some(size(px(800.0), px(600.0))),
                window_decorations: Some(WindowDecorations::Client),
                app_id: Some("com.secousse.app".to_string()),
            },
            |window, cx| {
                // Create the app with settings
                cx.new(|cx| SecousseApp::new(settings, window, cx))
            },
        )
        .expect("Failed to open window");
    });
}
