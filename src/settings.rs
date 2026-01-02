//! User settings persistence.
//!
//! This module handles loading and saving user preferences across sessions.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::i18n::Language;

/// User settings that persist across sessions
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserSettings {
    /// Settings file version for migration support
    #[serde(default = "default_version")]
    pub version: u32,
    /// Selected language
    #[serde(default)]
    pub language: Language,
}

fn default_version() -> u32 {
    1
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            version: 1,
            language: Language::default(),
        }
    }
}

impl UserSettings {
    /// Get the config directory path for UltraLog
    pub fn get_config_dir() -> Option<PathBuf> {
        #[cfg(target_os = "macos")]
        {
            dirs::data_dir().map(|p| p.join("UltraLog"))
        }
        #[cfg(target_os = "windows")]
        {
            dirs::config_dir().map(|p| p.join("UltraLog"))
        }
        #[cfg(target_os = "linux")]
        {
            dirs::config_dir().map(|p| p.join("ultralog"))
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            dirs::config_dir().map(|p| p.join("ultralog"))
        }
    }

    /// Get the path to the settings JSON file
    pub fn get_settings_path() -> Option<PathBuf> {
        Self::get_config_dir().map(|p| p.join("settings.json"))
    }

    /// Load settings from disk
    pub fn load() -> Self {
        let path = match Self::get_settings_path() {
            Some(p) => p,
            None => return Self::default(),
        };

        if !path.exists() {
            return Self::default();
        }

        match std::fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Save settings to disk
    pub fn save(&self) -> Result<(), String> {
        let path = Self::get_settings_path()
            .ok_or_else(|| "Could not determine config directory".to_string())?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config directory: {}", e))?;
        }

        let content = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize settings: {}", e))?;

        std::fs::write(&path, content)
            .map_err(|e| format!("Failed to write settings file: {}", e))?;

        Ok(())
    }
}
