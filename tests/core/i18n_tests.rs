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
fn test_language_all_returns_both_languages() {
    let all = Language::all();
    assert_eq!(all.len(), 2);
    assert!(all.contains(&Language::English));
    assert!(all.contains(&Language::Spanish));
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
fn test_locale_codes_are_valid_iso_639_1() {
    // ISO 639-1 codes are exactly 2 lowercase letters
    for lang in Language::all() {
        let code = lang.locale_code();
        assert_eq!(code.len(), 2, "Locale code should be 2 characters");
        assert!(
            code.chars().all(|c| c.is_ascii_lowercase()),
            "Locale code should be lowercase ASCII"
        );
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
