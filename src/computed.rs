//! Computed channels functionality
//!
//! This module provides the ability to create virtual channels from mathematical
//! expressions over existing log data, with time-shifting capabilities and a
//! global reusable template library.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// A template for a computed channel stored in the global library
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ComputedChannelTemplate {
    /// Unique identifier (UUID)
    pub id: String,
    /// Display name for the computed channel
    pub name: String,
    /// The formula expression (e.g., "RPM * 0.5 + Boost")
    pub formula: String,
    /// Unit to display (user-specified)
    pub unit: String,
    /// Optional description for user reference
    #[serde(default)]
    pub description: String,
    /// Created timestamp (unix seconds)
    pub created_at: u64,
    /// Last modified timestamp (unix seconds)
    pub modified_at: u64,
}

impl ComputedChannelTemplate {
    /// Create a new template with generated ID and timestamps
    pub fn new(name: String, formula: String, unit: String, description: String) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            formula,
            unit,
            description,
            created_at: now,
            modified_at: now,
        }
    }

    /// Update the modified timestamp
    pub fn touch(&mut self) {
        self.modified_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
    }
}

/// A computed channel instantiated for a specific log file
#[derive(Clone, Debug)]
pub struct ComputedChannel {
    /// The template this channel was created from
    pub template: ComputedChannelTemplate,
    /// Resolved channel name -> index mappings for this file
    pub channel_bindings: HashMap<String, usize>,
    /// Cached computed values (computed on first access)
    pub cached_data: Option<Vec<f64>>,
    /// Any errors from binding or evaluation
    pub error: Option<String>,
}

impl ComputedChannel {
    /// Create a new computed channel from a template
    pub fn from_template(template: ComputedChannelTemplate) -> Self {
        Self {
            template,
            channel_bindings: HashMap::new(),
            cached_data: None,
            error: None,
        }
    }

    /// Get the display name
    pub fn name(&self) -> &str {
        &self.template.name
    }

    /// Get the formula
    pub fn formula(&self) -> &str {
        &self.template.formula
    }

    /// Get the unit
    pub fn unit(&self) -> &str {
        &self.template.unit
    }

    /// Check if this channel has been evaluated successfully
    pub fn is_valid(&self) -> bool {
        self.error.is_none() && self.cached_data.is_some()
    }

    /// Clear cached data (forces re-evaluation)
    pub fn invalidate_cache(&mut self) {
        self.cached_data = None;
    }
}

/// Time shift specification for channel references in formulas
#[derive(Clone, Debug, Default, PartialEq)]
pub enum TimeShift {
    /// No time shift - use current record value
    #[default]
    None,
    /// Index offset: e.g., RPM[-1] for previous sample, RPM[+2] for 2 samples ahead
    IndexOffset(i32),
    /// Time offset in seconds: e.g., RPM@-0.1s for value 100ms ago
    TimeOffset(f64),
}

/// A reference to a channel in a formula expression
#[derive(Clone, Debug)]
pub struct ChannelReference {
    /// Channel name as written in formula
    pub name: String,
    /// Time shift type
    pub time_shift: TimeShift,
    /// The full match string from the formula (for replacement)
    pub full_match: String,
}

/// The global library of computed channel templates
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ComputedChannelLibrary {
    /// Library format version (for future migrations)
    #[serde(default)]
    pub version: u32,
    /// The stored templates
    #[serde(default)]
    pub templates: Vec<ComputedChannelTemplate>,
}

impl ComputedChannelLibrary {
    /// Current library format version
    pub const CURRENT_VERSION: u32 = 1;

    /// Create a new empty library
    pub fn new() -> Self {
        Self {
            version: Self::CURRENT_VERSION,
            templates: Vec::new(),
        }
    }

    /// Add a template to the library
    pub fn add_template(&mut self, template: ComputedChannelTemplate) {
        self.templates.push(template);
    }

    /// Remove a template by ID
    pub fn remove_template(&mut self, id: &str) -> Option<ComputedChannelTemplate> {
        if let Some(pos) = self.templates.iter().position(|t| t.id == id) {
            Some(self.templates.remove(pos))
        } else {
            None
        }
    }

    /// Find a template by ID
    pub fn find_template(&self, id: &str) -> Option<&ComputedChannelTemplate> {
        self.templates.iter().find(|t| t.id == id)
    }

    /// Find a template by ID (mutable)
    pub fn find_template_mut(&mut self, id: &str) -> Option<&mut ComputedChannelTemplate> {
        self.templates.iter_mut().find(|t| t.id == id)
    }

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

    /// Get the path to the library JSON file
    pub fn get_library_path() -> Option<PathBuf> {
        Self::get_config_dir().map(|p| p.join("computed_channels.json"))
    }

    /// Load the library from disk
    pub fn load() -> Self {
        let path = match Self::get_library_path() {
            Some(p) => p,
            None => {
                tracing::warn!(
                    "Could not determine config directory for computed channels library"
                );
                return Self::new();
            }
        };

        if !path.exists() {
            tracing::info!("Computed channels library not found, using empty library");
            return Self::new();
        }

        match std::fs::read_to_string(&path) {
            Ok(content) => match serde_json::from_str(&content) {
                Ok(library) => {
                    tracing::info!("Loaded computed channels library from {:?}", path);
                    library
                }
                Err(e) => {
                    tracing::error!("Failed to parse computed channels library: {}", e);
                    Self::new()
                }
            },
            Err(e) => {
                tracing::error!("Failed to read computed channels library: {}", e);
                Self::new()
            }
        }
    }

    /// Save the library to disk
    pub fn save(&self) -> Result<(), String> {
        let path = Self::get_library_path()
            .ok_or_else(|| "Could not determine config directory".to_string())?;

        // Ensure the directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config directory: {}", e))?;
        }

        let content = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize library: {}", e))?;

        std::fs::write(&path, content)
            .map_err(|e| format!("Failed to write library file: {}", e))?;

        tracing::info!("Saved computed channels library to {:?}", path);
        Ok(())
    }
}

/// State for the formula editor dialog
#[derive(Clone, Debug, Default)]
pub struct FormulaEditorState {
    /// Template ID being edited (None = creating new)
    pub editing_template_id: Option<String>,
    /// Name input field
    pub name: String,
    /// Formula input field
    pub formula: String,
    /// Description input field
    pub description: String,
    /// Unit input field
    pub unit: String,
    /// Validation error message
    pub validation_error: Option<String>,
    /// Preview values (first few computed values)
    pub preview_values: Option<Vec<f64>>,
    /// Whether the dialog is open
    pub is_open: bool,
}

impl FormulaEditorState {
    /// Open the editor for creating a new template
    pub fn open_new(&mut self) {
        self.editing_template_id = None;
        self.name = String::new();
        self.formula = String::new();
        self.description = String::new();
        self.unit = String::new();
        self.validation_error = None;
        self.preview_values = None;
        self.is_open = true;
    }

    /// Open the editor for editing an existing template
    pub fn open_edit(&mut self, template: &ComputedChannelTemplate) {
        self.editing_template_id = Some(template.id.clone());
        self.name = template.name.clone();
        self.formula = template.formula.clone();
        self.description = template.description.clone();
        self.unit = template.unit.clone();
        self.validation_error = None;
        self.preview_values = None;
        self.is_open = true;
    }

    /// Close the editor
    pub fn close(&mut self) {
        self.is_open = false;
        self.editing_template_id = None;
        self.validation_error = None;
        self.preview_values = None;
    }

    /// Check if we're editing an existing template
    pub fn is_editing(&self) -> bool {
        self.editing_template_id.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_template_creation() {
        let template = ComputedChannelTemplate::new(
            "Test Channel".to_string(),
            "RPM * 2".to_string(),
            "RPM".to_string(),
            "A test channel".to_string(),
        );

        assert_eq!(template.name, "Test Channel");
        assert_eq!(template.formula, "RPM * 2");
        assert_eq!(template.unit, "RPM");
        assert!(!template.id.is_empty());
        assert!(template.created_at > 0);
    }

    #[test]
    fn test_library_operations() {
        let mut library = ComputedChannelLibrary::new();

        let template = ComputedChannelTemplate::new(
            "Test".to_string(),
            "A + B".to_string(),
            "unit".to_string(),
            String::new(),
        );
        let id = template.id.clone();

        library.add_template(template);
        assert_eq!(library.templates.len(), 1);

        assert!(library.find_template(&id).is_some());
        assert!(library.find_template("nonexistent").is_none());

        let removed = library.remove_template(&id);
        assert!(removed.is_some());
        assert!(library.templates.is_empty());
    }

    #[test]
    fn test_time_shift() {
        assert_eq!(TimeShift::default(), TimeShift::None);

        let index_shift = TimeShift::IndexOffset(-1);
        let time_shift = TimeShift::TimeOffset(-0.1);

        assert_ne!(index_shift, time_shift);
    }

    #[test]
    fn test_computed_channel() {
        let template = ComputedChannelTemplate::new(
            "Test".to_string(),
            "RPM".to_string(),
            "RPM".to_string(),
            String::new(),
        );

        let mut channel = ComputedChannel::from_template(template);
        assert!(!channel.is_valid());
        assert_eq!(channel.name(), "Test");

        channel.cached_data = Some(vec![1.0, 2.0, 3.0]);
        assert!(channel.is_valid());

        channel.invalidate_cache();
        assert!(!channel.is_valid());
    }
}
