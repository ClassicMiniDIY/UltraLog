//! UltraLog - A high-performance ECU log viewer written in Rust
//!
//! UltraLog is a desktop application for viewing and analyzing ECU (Engine Control Unit)
//! log files from automotive performance tuning systems. It supports multiple ECU formats
//! including Haltech, MegaSquirt, AEM, and more.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui::IconData;
use std::sync::Arc;
use ultralog::app::UltraLogApp;

/// Set up Linux-specific scaling configuration before window creation.
/// This handles common DPI/scaling issues on X11, especially with KDE Plasma.
#[cfg(target_os = "linux")]
fn setup_linux_scaling() {
    // If no X11 scale factor is set, try to detect from common environment variables
    // This helps on systems where the scale factor isn't properly detected
    if std::env::var("WINIT_X11_SCALE_FACTOR").is_err() {
        // Check if GDK_SCALE is set (common on GTK-based systems)
        if let Ok(gdk_scale) = std::env::var("GDK_SCALE") {
            std::env::set_var("WINIT_X11_SCALE_FACTOR", &gdk_scale);
        }
        // Check QT_SCALE_FACTOR (common on KDE/Qt systems like Kubuntu)
        else if let Ok(qt_scale) = std::env::var("QT_SCALE_FACTOR") {
            std::env::set_var("WINIT_X11_SCALE_FACTOR", &qt_scale);
        }
        // Check QT_AUTO_SCREEN_SCALE_FACTOR
        else if std::env::var("QT_AUTO_SCREEN_SCALE_FACTOR").is_ok() {
            // Let winit auto-detect, but ensure it uses randr for X11
            std::env::set_var("WINIT_X11_SCALE_FACTOR", "randr");
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn setup_linux_scaling() {
    // No-op on non-Linux platforms
}

/// Load the platform-specific application icon
fn load_app_icon() -> Option<Arc<IconData>> {
    // Select the appropriate icon based on platform
    #[cfg(target_os = "windows")]
    let icon_bytes = include_bytes!("../assets/icons/windows.png");

    #[cfg(target_os = "macos")]
    let icon_bytes = include_bytes!("../assets/icons/mac.png");

    #[cfg(target_os = "linux")]
    let icon_bytes = include_bytes!("../assets/icons/linux.png");

    // Fallback for other platforms
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    let icon_bytes = include_bytes!("../assets/icons/linux.png");

    // Decode the PNG image
    match image::load_from_memory(icon_bytes) {
        Ok(img) => {
            let rgba = img.to_rgba8();
            let (width, height) = rgba.dimensions();
            Some(Arc::new(IconData {
                rgba: rgba.into_raw(),
                width,
                height,
            }))
        }
        Err(e) => {
            eprintln!("Failed to load app icon: {}", e);
            None
        }
    }
}

/// Set the macOS application name for the dock
#[cfg(target_os = "macos")]
fn set_macos_app_name() {
    use objc2::{class, msg_send};
    use objc2_foundation::NSString;

    unsafe {
        let app_name = NSString::from_str("UltraLog");
        let process_info_class = class!(NSProcessInfo);
        let process_info: *mut objc2::runtime::AnyObject =
            msg_send![process_info_class, processInfo];
        let _: () = msg_send![process_info, setProcessName: &*app_name];
    }
}

#[cfg(not(target_os = "macos"))]
fn set_macos_app_name() {}

fn main() -> eframe::Result<()> {
    // Set up platform-specific configuration before anything else
    set_macos_app_name();
    setup_linux_scaling();

    // Initialize logging
    tracing_subscriber::fmt::init();

    // Track app startup for analytics
    ultralog::analytics::track_app_started();

    // Load platform-specific app icon
    let icon = load_app_icon();

    // Configure native options
    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_inner_size([1920.0, 1080.0])
        .with_min_inner_size([1000.0, 900.0])
        .with_title("UltraLog - ECU Log Viewer")
        .with_app_id("UltraLog")
        .with_drag_and_drop(true);

    // On Linux, start maximized to avoid sizing/scaling issues across different
    // desktop environments and display configurations
    #[cfg(target_os = "linux")]
    {
        viewport = viewport.with_maximized(true);
    }

    // Set icon if loaded successfully
    if let Some(icon_data) = icon {
        viewport = viewport.with_icon(icon_data);
    }

    let native_options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    // Run the application
    eframe::run_native(
        "UltraLog",
        native_options,
        Box::new(|cc| Ok(Box::new(UltraLogApp::new(cc)))),
    )
}
