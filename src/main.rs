//! UltraLog - A high-performance ECU log viewer written in Rust
//!
//! UltraLog is a desktop application for viewing and analyzing ECU (Engine Control Unit)
//! log files from automotive performance tuning systems. It supports multiple ECU formats
//! including Haltech, MegaSquirt, AEM, and more.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod parsers;

use app::UltraLogApp;

fn main() -> eframe::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Configure native options
    let native_options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1920.0, 1080.0])
            .with_min_inner_size([1000.0, 900.0])
            .with_title("UltraLog - ECU Log Viewer")
            .with_drag_and_drop(true),
        ..Default::default()
    };

    // Run the application
    eframe::run_native(
        "UltraLog",
        native_options,
        Box::new(|cc| Ok(Box::new(UltraLogApp::new(cc)))),
    )
}
