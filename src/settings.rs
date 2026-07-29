//! User settings persistence.
//!
//! This module handles loading and saving user preferences across sessions.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::i18n::Language;
use crate::state::{FontScale, TileProviderId};
use crate::units::UnitPreferences;

/// User settings that persist across sessions
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UserSettings {
    /// Settings file version for migration support
    #[serde(default = "default_version")]
    pub version: u32,
    /// Selected language
    #[serde(default)]
    pub language: Language,
    /// When true, scroll wheel zooms chart directly instead of panning
    #[serde(default)]
    pub scroll_to_zoom: bool,
    /// When true, draw the chart background grid
    #[serde(default = "default_show_grid")]
    pub show_grid: bool,
    /// Grid line opacity, 0..=255. Modulates the base grid color's alpha
    /// before egui_plot's distance-based fade
    #[serde(default = "default_grid_opacity")]
    pub grid_opacity: u8,
    /// Display unit selections (temperature, pressure, speed, ...)
    #[serde(default)]
    pub unit_preferences: UnitPreferences,
    /// UI font scale
    #[serde(default)]
    pub font_scale: FontScale,
    /// Use the colorblind-friendly chart palette
    #[serde(default)]
    pub color_blind_mode: bool,
    /// Normalize ECU-specific channel names to standardized names
    #[serde(default = "default_true")]
    pub field_normalization: bool,
    /// Keep the chart view locked to the cursor during playback
    #[serde(default = "default_true")]
    pub cursor_tracking: bool,
    /// Check for updates on startup
    #[serde(default = "default_true")]
    pub auto_check_updates: bool,
    /// User-defined channel-name normalization mappings (source -> display)
    #[serde(default)]
    pub custom_normalizations: HashMap<String, String>,
    /// Default tile provider for the Track Map widget. Persisted so users
    /// don't have to re-pick after every restart.
    #[serde(default)]
    pub tile_provider: TileProviderId,
    /// Soft cap for the on-disk tile cache, in MB.
    #[serde(default = "default_tile_cache_max_mb")]
    pub tile_cache_max_mb: u32,
    /// Default for `TrackMapState::tiles_enabled`: remember whether the
    /// satellite background was last shown so the next session starts the
    /// same way.
    #[serde(default)]
    pub tiles_enabled: bool,
    /// Default for `TrackMapState::tile_opacity` (0.1..=1.0).
    #[serde(default = "default_tile_opacity")]
    pub tile_opacity: f32,
    /// Default for `TrackMapState::tile_grayscale`.
    #[serde(default)]
    pub tile_grayscale: bool,
    /// Set of widget IDs the user has explicitly hidden from the data
    /// panel. The user choice wins over `is_available()`: a hidden
    /// widget stays hidden even after a file with matching data is
    /// opened. The user re-adds it via the panel header's "+" button.
    #[serde(default)]
    pub hidden_widgets: HashSet<String>,
}

fn default_version() -> u32 {
    1
}

fn default_show_grid() -> bool {
    true
}

fn default_grid_opacity() -> u8 {
    255
}

fn default_true() -> bool {
    true
}

fn default_tile_cache_max_mb() -> u32 {
    256
}

fn default_tile_opacity() -> f32 {
    1.0
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            version: 1,
            language: Language::default(),
            scroll_to_zoom: false,
            show_grid: default_show_grid(),
            grid_opacity: default_grid_opacity(),
            unit_preferences: UnitPreferences::default(),
            font_scale: FontScale::default(),
            color_blind_mode: false,
            field_normalization: default_true(),
            cursor_tracking: default_true(),
            auto_check_updates: default_true(),
            custom_normalizations: HashMap::new(),
            tile_provider: TileProviderId::default(),
            tile_cache_max_mb: default_tile_cache_max_mb(),
            tiles_enabled: false,
            tile_opacity: default_tile_opacity(),
            tile_grayscale: false,
            hidden_widgets: HashSet::new(),
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
