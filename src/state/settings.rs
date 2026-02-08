//! Persistent settings storage
//!
//! Handles loading and saving user preferences to disk.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Application settings that persist across sessions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// Unique device identifier for Twitch API
    pub device_id: Option<String>,

    /// OAuth access token
    pub access_token: Option<String>,

    /// Whether sidebar is expanded
    #[serde(default = "default_true")]
    pub sidebar_open: bool,

    /// Whether chat panel is visible
    #[serde(default = "default_true")]
    pub chat_open: bool,
}

fn default_true() -> bool {
    true
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            device_id: None,
            access_token: None,
            sidebar_open: true,
            chat_open: true,
        }
    }
}

impl Settings {
    /// Load settings from disk, returning defaults if file doesn't exist
    pub fn load() -> Self {
        let path = Self::settings_path();

        if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(contents) => match serde_json::from_str(&contents) {
                    Ok(settings) => {
                        log::info!("Loaded settings from {:?}", path);
                        return settings;
                    }
                    Err(e) => {
                        log::warn!("Failed to parse settings: {}", e);
                    }
                },
                Err(e) => {
                    log::warn!("Failed to read settings file: {}", e);
                }
            }
        }

        log::info!("Using default settings");
        Self::default()
    }

    /// Save settings to disk
    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::settings_path();

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, json)?;

        log::info!("Saved settings to {:?}", path);
        Ok(())
    }

    /// Get the settings file path
    pub fn settings_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("secousse")
            .join("settings.json")
    }

    /// Get the application data directory
    pub fn data_dir() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("secousse")
    }

    /// Get or generate a device ID
    pub fn get_or_create_device_id(&mut self) -> String {
        if let Some(ref id) = self.device_id {
            id.clone()
        } else {
            let id = uuid::Uuid::new_v4()
                .to_string()
                .replace("-", "")
                .chars()
                .take(32)
                .collect::<String>();
            self.device_id = Some(id.clone());
            // Auto-save when generating new device ID
            let _ = self.save();
            id
        }
    }

    /// Update access token and save
    pub fn set_access_token(&mut self, token: Option<String>) {
        self.access_token = token;
        let _ = self.save();
    }
}
