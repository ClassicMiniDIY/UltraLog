//! Internationalization support for UltraLog.
//!
//! This module provides language selection and locale management.

use serde::{Deserialize, Serialize};

/// Supported application languages
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    #[default]
    English,
    Spanish,
}

impl Language {
    /// Get the locale code for rust-i18n
    pub fn locale_code(&self) -> &'static str {
        match self {
            Language::English => "en",
            Language::Spanish => "es",
        }
    }

    /// Get the display name for the language (in its native language)
    pub fn display_name(&self) -> &'static str {
        match self {
            Language::English => "English",
            Language::Spanish => "Español",
        }
    }

    /// Get all available languages
    pub fn all() -> &'static [Language] {
        &[Language::English, Language::Spanish]
    }
}
