//! Analytics module for UltraLog using PostHog.
//!
//! This module provides anonymous usage analytics to help improve UltraLog.
//! All data is anonymous - we only track feature usage, not personal information.

use serde::Serialize;
use std::collections::HashMap;
use std::sync::{mpsc, OnceLock};
use uuid::Uuid;

/// PostHog API key for UltraLog analytics
const POSTHOG_API_KEY: &str = "phc_jrZkZhkhoHXknLz7djnuBR8s4tl9mZnR00UAWVl2GHO";

/// PostHog API endpoint
const POSTHOG_API_URL: &str = "https://us.i.posthog.com/capture/";

/// Application version for tracking
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Global distinct ID for this session
static DISTINCT_ID: OnceLock<String> = OnceLock::new();

/// Channel sender for the analytics background thread
static EVENT_SENDER: OnceLock<mpsc::Sender<AnalyticsEvent>> = OnceLock::new();

/// Event structure for PostHog API
#[derive(Serialize, Clone)]
struct AnalyticsEvent {
    api_key: &'static str,
    event: String,
    distinct_id: String,
    properties: HashMap<String, serde_json::Value>,
}

/// Get or generate the session's distinct ID
fn get_distinct_id() -> String {
    DISTINCT_ID
        .get_or_init(|| Uuid::new_v4().to_string())
        .clone()
}

/// Initialize the analytics background thread
fn get_sender() -> &'static mpsc::Sender<AnalyticsEvent> {
    EVENT_SENDER.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<AnalyticsEvent>();

        // Spawn a long-lived background thread that processes events
        std::thread::spawn(move || {
            // Process events as they arrive
            while let Ok(event) = rx.recv() {
                // Send to PostHog via HTTP POST - ignore errors
                let _ = ureq::post(POSTHOG_API_URL)
                    .header("Content-Type", "application/json")
                    .send_json(&event);
            }
        });

        tx
    })
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
fn create_event(event_name: &str) -> AnalyticsEvent {
    let mut properties = HashMap::new();
    properties.insert(
        "app_version".to_string(),
        serde_json::Value::String(APP_VERSION.to_string()),
    );
    properties.insert(
        "platform".to_string(),
        serde_json::Value::String(get_platform().to_string()),
    );

    AnalyticsEvent {
        api_key: POSTHOG_API_KEY,
        event: event_name.to_string(),
        distinct_id: get_distinct_id(),
        properties,
    }
}

/// Capture an event (fire and forget - errors are silently ignored)
fn capture_event(event: AnalyticsEvent) {
    // Send event to background thread for processing
    if let Some(sender) = EVENT_SENDER.get() {
        let _ = sender.send(event);
    } else {
        // Initialize on first use and send
        let _ = get_sender().send(event);
    }
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

    event.properties.insert(
        "ecu_type".to_string(),
        serde_json::Value::String(ecu_type.to_string()),
    );
    event.properties.insert(
        "file_size_kb".to_string(),
        serde_json::json!(file_size_bytes / 1024),
    );

    capture_event(event);
}

/// Track chart export (PNG or PDF)
pub fn track_export(format: &str) {
    let mut event = create_event("chart_exported");

    event.properties.insert(
        "format".to_string(),
        serde_json::Value::String(format.to_string()),
    );

    capture_event(event);
}

/// Track tool/view switch
pub fn track_tool_switched(tool_name: &str) {
    let mut event = create_event("tool_switched");

    event.properties.insert(
        "tool".to_string(),
        serde_json::Value::String(tool_name.to_string()),
    );

    capture_event(event);
}

/// Track playback usage
pub fn track_playback_started(speed: f64) {
    let mut event = create_event("playback_started");

    event
        .properties
        .insert("speed".to_string(), serde_json::json!(speed));

    capture_event(event);
}

/// Track update check
pub fn track_update_checked(update_available: bool) {
    let mut event = create_event("update_checked");

    event.properties.insert(
        "update_available".to_string(),
        serde_json::json!(update_available),
    );

    capture_event(event);
}

/// Track colorblind mode toggle
pub fn track_colorblind_mode_toggled(enabled: bool) {
    let mut event = create_event("colorblind_mode_toggled");

    event
        .properties
        .insert("enabled".to_string(), serde_json::json!(enabled));

    capture_event(event);
}
