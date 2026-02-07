//! Persistent settings storage
//!
//! Handles loading and saving user preferences to disk.
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Application settings that persist across sessions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// Unique device identifier for Twitch API
    pub device_id: Option<String>,

    /// OAuth access token
    pub access_token: Option<String>,

    /// Last watched channel
    pub last_channel: Option<String>,

    /// Volume level (0.0 - 1.0)
    #[serde(default = "default_volume")]
    pub volume: f64,

    /// Whether sidebar is expanded
    #[serde(default = "default_true")]
    pub sidebar_open: bool,

    /// Whether chat panel is visible
    #[serde(default = "default_true")]
    pub chat_open: bool,

    /// Preferred video quality (e.g., "auto", "1080p60", "720p60")
    #[serde(default = "default_quality")]
    pub video_quality: String,

    /// Whether to auto-play streams when selecting a channel
    #[serde(default = "default_true")]
    pub auto_play: bool,

    /// Chat message buffer size
    #[serde(default = "default_chat_buffer")]
    pub chat_buffer_size: usize,
}

fn default_volume() -> f64 {
    0.5
}
fn default_true() -> bool {
    true
}
fn default_quality() -> String {
    "auto".to_string()
}
fn default_chat_buffer() -> usize {
    500
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            device_id: None,
            access_token: None,
            last_channel: None,
            volume: default_volume(),
            sidebar_open: true,
            chat_open: true,
            video_quality: default_quality(),
            auto_play: true,
            chat_buffer_size: default_chat_buffer(),
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

    /// Update last channel and save
    pub fn set_last_channel(&mut self, channel: Option<String>) {
        self.last_channel = channel;
        let _ = self.save();
    }

    /// Update volume and save
    pub fn set_volume(&mut self, volume: f64) {
        self.volume = volume.clamp(0.0, 1.0);
        let _ = self.save();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_settings() {
        let settings = Settings::default();
        assert_eq!(settings.volume, 0.5);
        assert!(settings.sidebar_open);
        assert!(settings.chat_open);
    }

    #[test]
    fn test_device_id_generation() {
        let mut settings = Settings::default();
        let id = settings.get_or_create_device_id();
        assert_eq!(id.len(), 32);

        // Should return same ID on subsequent calls
        let id2 = settings.get_or_create_device_id();
        assert_eq!(id, id2);
    }
}
