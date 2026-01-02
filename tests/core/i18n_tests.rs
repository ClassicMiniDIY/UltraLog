//! Tests for internationalization (i18n) functionality
//!
//! Tests cover:
//! - Language enum methods (locale_code, display_name, all)
//! - Default language selection
//! - Serialization/deserialization
//! - Translation loading via rust-i18n

use ultralog::i18n::Language;

// ============================================
// Language Enum Basic Tests
// ============================================

#[test]
fn test_language_default_is_english() {
    let lang = Language::default();
    assert_eq!(lang, Language::English);
}

#[test]
fn test_language_english_locale_code() {
    assert_eq!(Language::English.locale_code(), "en");
}

#[test]
fn test_language_spanish_locale_code() {
    assert_eq!(Language::Spanish.locale_code(), "es");
}

#[test]
fn test_language_english_display_name() {
    assert_eq!(Language::English.display_name(), "English");
}

#[test]
fn test_language_spanish_display_name() {
    // Display name should be in the native language
    assert_eq!(Language::Spanish.display_name(), "Español");
}

#[test]
fn test_language_all_returns_all_languages() {
    let all = Language::all();
    assert_eq!(all.len(), 15);
    assert!(all.contains(&Language::English));
    assert!(all.contains(&Language::Spanish));
    assert!(all.contains(&Language::German));
    assert!(all.contains(&Language::French));
    assert!(all.contains(&Language::Italian));
    assert!(all.contains(&Language::PortugueseBrazil));
    assert!(all.contains(&Language::PortuguesePortugal));
    assert!(all.contains(&Language::ChineseSimplified));
    assert!(all.contains(&Language::Hindi));
    assert!(all.contains(&Language::Arabic));
    assert!(all.contains(&Language::Bengali));
    assert!(all.contains(&Language::Russian));
    assert!(all.contains(&Language::Urdu));
    assert!(all.contains(&Language::Indonesian));
    assert!(all.contains(&Language::Japanese));
}

#[test]
fn test_language_all_english_first() {
    // English should be first in the list as the default
    let all = Language::all();
    assert_eq!(all[0], Language::English);
}

// ============================================
// Language Equality Tests
// ============================================

#[test]
fn test_language_equality() {
    assert_eq!(Language::English, Language::English);
    assert_eq!(Language::Spanish, Language::Spanish);
    assert_ne!(Language::English, Language::Spanish);
}

#[test]
fn test_language_clone() {
    let lang = Language::Spanish;
    let cloned = lang;
    assert_eq!(lang, cloned);
}

#[test]
fn test_language_copy() {
    let lang = Language::English;
    let copied = lang;
    // Both should still be usable (Copy trait)
    assert_eq!(lang.locale_code(), "en");
    assert_eq!(copied.locale_code(), "en");
}

// ============================================
// Language Serialization Tests
// ============================================

#[test]
fn test_language_serialize_english() {
    let lang = Language::English;
    let json = serde_json::to_string(&lang).unwrap();
    assert_eq!(json, "\"English\"");
}

#[test]
fn test_language_serialize_spanish() {
    let lang = Language::Spanish;
    let json = serde_json::to_string(&lang).unwrap();
    assert_eq!(json, "\"Spanish\"");
}

#[test]
fn test_language_deserialize_english() {
    let json = "\"English\"";
    let lang: Language = serde_json::from_str(json).unwrap();
    assert_eq!(lang, Language::English);
}

#[test]
fn test_language_deserialize_spanish() {
    let json = "\"Spanish\"";
    let lang: Language = serde_json::from_str(json).unwrap();
    assert_eq!(lang, Language::Spanish);
}

#[test]
fn test_language_roundtrip_serialization() {
    for lang in Language::all() {
        let json = serde_json::to_string(lang).unwrap();
        let deserialized: Language = serde_json::from_str(&json).unwrap();
        assert_eq!(*lang, deserialized);
    }
}

// ============================================
// Language Debug Tests
// ============================================

#[test]
fn test_language_debug_english() {
    let debug = format!("{:?}", Language::English);
    assert_eq!(debug, "English");
}

#[test]
fn test_language_debug_spanish() {
    let debug = format!("{:?}", Language::Spanish);
    assert_eq!(debug, "Spanish");
}

// ============================================
// Locale Code Consistency Tests
// ============================================

#[test]
fn test_locale_codes_are_valid_bcp47() {
    // Locale codes follow BCP 47 format: language code (2 chars) optionally followed by
    // a hyphen and region code (e.g., "en", "pt-BR", "zh-CN")
    for lang in Language::all() {
        let code = lang.locale_code();
        assert!(!code.is_empty(), "Locale code should not be empty");

        // Split by hyphen to check format
        let parts: Vec<&str> = code.split('-').collect();
        assert!(
            parts.len() <= 2,
            "Locale code should have at most 2 parts: {:?}",
            code
        );

        // First part should be 2 lowercase letters (language code)
        assert_eq!(
            parts[0].len(),
            2,
            "Language code should be 2 characters: {:?}",
            code
        );
        assert!(
            parts[0].chars().all(|c| c.is_ascii_lowercase()),
            "Language code should be lowercase ASCII: {:?}",
            code
        );

        // If there's a second part, it should be a region code (2 uppercase or mixed case)
        if parts.len() == 2 {
            assert!(
                parts[1].len() >= 2,
                "Region code should be at least 2 characters: {:?}",
                code
            );
        }
    }
}

#[test]
fn test_all_languages_have_unique_locale_codes() {
    let all = Language::all();
    let codes: Vec<&str> = all.iter().map(|l| l.locale_code()).collect();

    // Check for duplicates
    let mut unique_codes = codes.clone();
    unique_codes.sort();
    unique_codes.dedup();

    assert_eq!(
        codes.len(),
        unique_codes.len(),
        "All locale codes should be unique"
    );
}

#[test]
fn test_all_languages_have_non_empty_display_names() {
    for lang in Language::all() {
        let name = lang.display_name();
        assert!(
            !name.is_empty(),
            "Display name should not be empty for {:?}",
            lang
        );
    }
}
