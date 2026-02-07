//! Asset bundling for embedded SVG icons and other static assets.
//!
//! Uses rust-embed to compile SVG files from `assets/` into the binary.
//! Register with `Application::new().with_assets(Assets)`.

use anyhow::anyhow;
use gpui::{AssetSource, Result, SharedString};
use rust_embed::RustEmbed;
use std::borrow::Cow;

/// Embedded application assets (icons, etc.)
///
/// Includes all SVG files under `assets/icons/`.
#[derive(RustEmbed)]
#[folder = "assets"]
#[include = "icons/**/*.svg"]
#[include = "app-icon.png"]
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if path.is_empty() {
            return Ok(None);
        }

        Self::get(path)
            .map(|f| Some(f.data))
            .ok_or_else(|| anyhow!("could not find asset at path \"{path}\""))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(Self::iter()
            .filter_map(|p| {
                p.starts_with(path)
                    .then(|| SharedString::from(p.to_string()))
            })
            .collect())
    }
}
