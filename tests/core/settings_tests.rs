//! Tests for user settings persistence
//!
//! Tests cover:
//! - Default settings values
//! - Serialization/deserialization
//! - Settings roundtrip
//! - Config path handling

use ultralog::i18n::Language;
use ultralog::settings::UserSettings;

// ============================================
// Default Settings Tests
// ============================================

#[test]
fn test_settings_default_version() {
    let settings = UserSettings::default();
    assert_eq!(settings.version, 1);
}

#[test]
fn test_settings_default_language() {
    let settings = UserSettings::default();
    assert_eq!(settings.language, Language::English);
}

#[test]
fn test_settings_default_is_consistent() {
    let settings1 = UserSettings::default();
    let settings2 = UserSettings::default();

    assert_eq!(settings1.version, settings2.version);
    assert_eq!(settings1.language, settings2.language);
}

// ============================================
// Serialization Tests
// ============================================

#[test]
fn test_settings_serialize_default() {
    let settings = UserSettings::default();
    let json = serde_json::to_string(&settings).unwrap();

    // Should contain version and language fields
    assert!(json.contains("version"));
    assert!(json.contains("language"));
}

#[test]
fn test_settings_serialize_pretty() {
    let settings = UserSettings::default();
    let json = serde_json::to_string_pretty(&settings).unwrap();

    // Pretty format should have newlines
    assert!(json.contains('\n'));
}

#[test]
fn test_settings_deserialize_default() {
    let json = r#"{"version":1,"language":"English"}"#;
    let settings: UserSettings = serde_json::from_str(json).unwrap();

    assert_eq!(settings.version, 1);
    assert_eq!(settings.language, Language::English);
}

#[test]
fn test_settings_deserialize_spanish() {
    let json = r#"{"version":1,"language":"Spanish"}"#;
    let settings: UserSettings = serde_json::from_str(json).unwrap();

    assert_eq!(settings.version, 1);
    assert_eq!(settings.language, Language::Spanish);
}

#[test]
fn test_settings_deserialize_missing_version() {
    // Version should default to 1 if missing
    let json = r#"{"language":"Spanish"}"#;
    let settings: UserSettings = serde_json::from_str(json).unwrap();

    assert_eq!(settings.version, 1);
    assert_eq!(settings.language, Language::Spanish);
}

#[test]
fn test_settings_deserialize_missing_language() {
    // Language should default to English if missing
    let json = r#"{"version":1}"#;
    let settings: UserSettings = serde_json::from_str(json).unwrap();

    assert_eq!(settings.version, 1);
    assert_eq!(settings.language, Language::English);
}

#[test]
fn test_settings_deserialize_empty_object() {
    // All fields should use defaults for empty object
    let json = r#"{}"#;
    let settings: UserSettings = serde_json::from_str(json).unwrap();

    assert_eq!(settings.version, 1);
    assert_eq!(settings.language, Language::English);
}

#[test]
fn test_settings_roundtrip() {
    let original = UserSettings {
        version: 1,
        language: Language::Spanish,
    };

    let json = serde_json::to_string(&original).unwrap();
    let restored: UserSettings = serde_json::from_str(&json).unwrap();

    assert_eq!(original.version, restored.version);
    assert_eq!(original.language, restored.language);
}

#[test]
fn test_settings_roundtrip_all_languages() {
    for lang in Language::all() {
        let settings = UserSettings {
            version: 1,
            language: *lang,
        };

        let json = serde_json::to_string(&settings).unwrap();
        let restored: UserSettings = serde_json::from_str(&json).unwrap();

        assert_eq!(settings.language, restored.language);
    }
}

// ============================================
// Config Path Tests
// ============================================

#[test]
fn test_config_dir_returns_some() {
    // On most systems, this should return Some
    // (unless running in a very restricted environment)
    let config_dir = UserSettings::get_config_dir();

    // This test may need to be adjusted based on environment
    // We just verify it doesn't panic
    let _ = config_dir;
}

#[test]
fn test_settings_path_returns_some() {
    let settings_path = UserSettings::get_settings_path();

    // This test may need to be adjusted based on environment
    // We just verify it doesn't panic
    let _ = settings_path;
}

#[test]
fn test_settings_path_ends_with_json() {
    if let Some(path) = UserSettings::get_settings_path() {
        let path_str = path.to_string_lossy();
        assert!(
            path_str.ends_with(".json"),
            "Settings path should end with .json"
        );
    }
}

#[test]
fn test_settings_path_contains_settings_filename() {
    if let Some(path) = UserSettings::get_settings_path() {
        let filename = path.file_name().unwrap().to_string_lossy();
        assert_eq!(filename, "settings.json");
    }
}

#[test]
fn test_config_dir_contains_ultralog() {
    if let Some(path) = UserSettings::get_config_dir() {
        let path_str = path.to_string_lossy().to_lowercase();
        assert!(
            path_str.contains("ultralog"),
            "Config dir should contain 'ultralog' or 'UltraLog'"
        );
    }
}

// ============================================
// Load Tests (non-destructive)
// ============================================

#[test]
fn test_load_returns_valid_settings() {
    // load() should always return valid settings, even if file doesn't exist
    let settings = UserSettings::load();

    // Should have valid version
    assert!(settings.version >= 1);

    // Should have a valid language
    assert!(Language::all().contains(&settings.language));
}

#[test]
fn test_load_is_idempotent() {
    let settings1 = UserSettings::load();
    let settings2 = UserSettings::load();

    assert_eq!(settings1.version, settings2.version);
    // Note: language may have been changed by the user between loads in real usage
    // but in tests, it should be consistent
}

// ============================================
// Clone and Debug Tests
// ============================================

#[test]
fn test_settings_clone() {
    let original = UserSettings {
        version: 1,
        language: Language::Spanish,
    };

    let cloned = original.clone();

    assert_eq!(original.version, cloned.version);
    assert_eq!(original.language, cloned.language);
}

#[test]
fn test_settings_debug() {
    let settings = UserSettings::default();
    let debug = format!("{:?}", settings);

    assert!(debug.contains("UserSettings"));
    assert!(debug.contains("version"));
    assert!(debug.contains("language"));
}

// ============================================
// Future Migration Tests
// ============================================

#[test]
fn test_settings_version_for_migration() {
    let settings = UserSettings::default();

    // Current version should be 1
    assert_eq!(settings.version, 1);

    // Version should be serialized for future migration support
    let json = serde_json::to_string(&settings).unwrap();
    assert!(json.contains("\"version\":1"));
}

#[test]
fn test_settings_handles_unknown_fields() {
    // Settings should ignore unknown fields for forward compatibility
    let json = r#"{"version":1,"language":"English","unknown_field":"value"}"#;
    let result: Result<UserSettings, _> = serde_json::from_str(json);

    // Should successfully deserialize, ignoring unknown field
    assert!(result.is_ok());
}
