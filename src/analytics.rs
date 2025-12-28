//! Analytics module for UltraLog using PostHog.
//!
//! This module provides anonymous usage analytics to help improve UltraLog.
//! All data is anonymous - we only track feature usage, not personal information.

use posthog_rs::Event;
use std::sync::OnceLock;
use uuid::Uuid;

/// PostHog API key for UltraLog analytics
const POSTHOG_API_KEY: &str = "phc_jrZkZhkhoHXknLz7djnuBR8s4tl9mZnR00UAWVl2GHO";

/// Application version for tracking
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Global distinct ID for this session
static DISTINCT_ID: OnceLock<String> = OnceLock::new();

/// Flag to track if global client is initialized
static INITIALIZED: OnceLock<bool> = OnceLock::new();

/// Get or generate the session's distinct ID
fn get_distinct_id() -> &'static str {
    DISTINCT_ID.get_or_init(|| Uuid::new_v4().to_string())
}

/// Initialize the global PostHog client (call once at startup)
fn ensure_initialized() {
    INITIALIZED.get_or_init(|| {
        // Initialize the global PostHog client using builder pattern
        if let Ok(options) = posthog_rs::ClientOptionsBuilder::default()
            .api_key(POSTHOG_API_KEY.to_string())
            .build()
        {
            let _ = posthog_rs::init_global(options);
        }
        true
    });
}

/// Get the current platform as a string
fn get_platform() -> &'static str {
    #[cfg(target_os = "windows")]
    return "windows";

    #[cfg(target_os = "macos")]
    return "macos";

    #[cfg(target_os = "linux")]
    return "linux";

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    return "unknown";
}

/// Create a base event with common properties
fn create_event(event_name: &str) -> Event {
    let mut event = Event::new(event_name, get_distinct_id());

    // Add common properties
    let _ = event.insert_prop("app_version", APP_VERSION);
    let _ = event.insert_prop("platform", get_platform());

    event
}

/// Capture an event using the global client (fire and forget - errors are silently ignored)
fn capture_event(event: Event) {
    ensure_initialized();

    // Spawn in background thread to avoid blocking UI
    std::thread::spawn(move || {
        let _ = posthog_rs::capture(event);
    });
}

// ============================================================================
// Public Analytics Functions
// ============================================================================

/// Track application startup
pub fn track_app_started() {
    let event = create_event("app_started");
    capture_event(event);
}

/// Track when a log file is loaded
pub fn track_file_loaded(ecu_type: &str, file_size_bytes: u64) {
    let mut event = create_event("file_loaded");

    let _ = event.insert_prop("ecu_type", ecu_type);
    let _ = event.insert_prop("file_size_kb", file_size_bytes / 1024);

    capture_event(event);
}

/// Track when a channel is selected
pub fn track_channel_selected(channel_count: usize) {
    let mut event = create_event("channel_selected");

    let _ = event.insert_prop("total_channels", channel_count);

    capture_event(event);
}

/// Track chart export (PNG or PDF)
pub fn track_export(format: &str) {
    let mut event = create_event("chart_exported");

    let _ = event.insert_prop("format", format);

    capture_event(event);
}

/// Track tool/view switch
pub fn track_tool_switched(tool_name: &str) {
    let mut event = create_event("tool_switched");

    let _ = event.insert_prop("tool", tool_name);

    capture_event(event);
}

/// Track playback usage
pub fn track_playback_started(speed: f64) {
    let mut event = create_event("playback_started");

    let _ = event.insert_prop("speed", speed);

    capture_event(event);
}

/// Track unit preference changes
#[allow(dead_code)]
pub fn track_unit_changed(unit_category: &str, new_unit: &str) {
    let mut event = create_event("unit_changed");

    let _ = event.insert_prop("category", unit_category);
    let _ = event.insert_prop("new_unit", new_unit);

    capture_event(event);
}

/// Track update check
pub fn track_update_checked(update_available: bool) {
    let mut event = create_event("update_checked");

    let _ = event.insert_prop("update_available", update_available);

    capture_event(event);
}

/// Track colorblind mode toggle
pub fn track_colorblind_mode_toggled(enabled: bool) {
    let mut event = create_event("colorblind_mode_toggled");

    let _ = event.insert_prop("enabled", enabled);

    capture_event(event);
}

/// Track file format detection errors (helps prioritize new format support)
#[allow(dead_code)]
pub fn track_file_format_error(error_type: &str) {
    let mut event = create_event("file_format_error");

    let _ = event.insert_prop("error_type", error_type);

    capture_event(event);
}
