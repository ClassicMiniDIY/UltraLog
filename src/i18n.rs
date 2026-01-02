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
    German,
    French,
    Italian,
    #[serde(rename = "PortugueseBrazil")]
    PortugueseBrazil,
    #[serde(rename = "PortuguesePortugal")]
    PortuguesePortugal,
    #[serde(rename = "ChineseSimplified")]
    ChineseSimplified,
    Hindi,
    Arabic,
    Bengali,
    Russian,
    Urdu,
    Indonesian,
    Japanese,
}

impl Language {
    /// Get the locale code for rust-i18n
    pub fn locale_code(&self) -> &'static str {
        match self {
            Language::English => "en",
            Language::Spanish => "es",
            Language::German => "de",
            Language::French => "fr",
            Language::Italian => "it",
            Language::PortugueseBrazil => "pt-BR",
            Language::PortuguesePortugal => "pt-PT",
            Language::ChineseSimplified => "zh-CN",
            Language::Hindi => "hi",
            Language::Arabic => "ar",
            Language::Bengali => "bn",
            Language::Russian => "ru",
            Language::Urdu => "ur",
            Language::Indonesian => "id",
            Language::Japanese => "ja",
        }
    }

    /// Get the display name for the language (in its native language)
    pub fn display_name(&self) -> &'static str {
        match self {
            Language::English => "English",
            Language::Spanish => "Español",
            Language::German => "Deutsch",
            Language::French => "Français",
            Language::Italian => "Italiano",
            Language::PortugueseBrazil => "Português (Brasil)",
            Language::PortuguesePortugal => "Português (Portugal)",
            Language::ChineseSimplified => "简体中文",
            Language::Hindi => "हिन्दी",
            Language::Arabic => "العربية",
            Language::Bengali => "বাংলা",
            Language::Russian => "Русский",
            Language::Urdu => "اردو",
            Language::Indonesian => "Bahasa Indonesia",
            Language::Japanese => "日本語",
        }
    }

    /// Get all available languages
    pub fn all() -> &'static [Language] {
        &[
            Language::English,
            Language::Spanish,
            Language::German,
            Language::French,
            Language::Italian,
            Language::PortugueseBrazil,
            Language::PortuguesePortugal,
            Language::ChineseSimplified,
            Language::Hindi,
            Language::Arabic,
            Language::Bengali,
            Language::Russian,
            Language::Urdu,
            Language::Indonesian,
            Language::Japanese,
        ]
    }
}
