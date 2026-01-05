//! UltraLog - A high-performance ECU log viewer written in Rust
//!
//! This library provides parsing functionality for various ECU log formats
//! and a graphical user interface for visualizing log data.
//!
//! ## Module Structure
//!
//! - [`adapters`] - OpenECU Alliance adapter specs for channel normalization
//! - [`app`] - Main application state and eframe::App implementation
//! - [`parsers`] - ECU log file parsers (Haltech, etc.)
//! - [`state`] - Core data types and constants
//! - [`units`] - Unit preference types and conversion utilities
//! - [`normalize`] - Field name normalization for standardizing channel names
//! - [`updater`] - Auto-update functionality for checking and downloading updates
//! - [`analytics`] - Anonymous usage analytics via PostHog
//! - [`analysis`] - Signal processing and statistical analysis algorithms
//! - [`mod@i18n`] - Internationalization support
//! - [`settings`] - User settings persistence
//! - [`ui`] - User interface components
//!   - `sidebar` - File list and view options
//!   - `channels` - Channel selection and display
//!   - `chart` - Main chart rendering and legends
//!   - `timeline` - Timeline scrubber and playback controls
//!   - `menu` - Menu bar (Units, Help)
//!   - `toast` - Toast notification system
//!   - `icons` - Custom icon drawing utilities

#[macro_use]
extern crate rust_i18n;

// Initialize i18n with translation files from the i18n directory
// Fallback to English if a translation is missing
i18n!("i18n", fallback = "en");

pub mod adapters;
pub mod analysis;
pub mod analytics;
pub mod app;
pub mod computed;
pub mod expression;
pub mod i18n;
pub mod normalize;
pub mod parsers;
pub mod settings;
pub mod state;
pub mod ui;
pub mod units;
pub mod updater;
