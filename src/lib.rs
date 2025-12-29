//! UltraLog - A high-performance ECU log viewer written in Rust
//!
//! This library provides parsing functionality for various ECU log formats
//! and a graphical user interface for visualizing log data.
//!
//! ## Module Structure
//!
//! - [`app`] - Main application state and eframe::App implementation
//! - [`parsers`] - ECU log file parsers (Haltech, etc.)
//! - [`state`] - Core data types and constants
//! - [`units`] - Unit preference types and conversion utilities
//! - [`normalize`] - Field name normalization for standardizing channel names
//! - [`updater`] - Auto-update functionality for checking and downloading updates
//! - [`analytics`] - Anonymous usage analytics via PostHog
//! - [`ui`] - User interface components
//!   - `sidebar` - File list and view options
//!   - `channels` - Channel selection and display
//!   - `chart` - Main chart rendering and legends
//!   - `timeline` - Timeline scrubber and playback controls
//!   - `menu` - Menu bar (Units, Help)
//!   - `toast` - Toast notification system
//!   - `icons` - Custom icon drawing utilities

pub mod analytics;
pub mod app;
pub mod computed;
pub mod expression;
pub mod normalize;
pub mod parsers;
pub mod state;
pub mod ui;
pub mod units;
pub mod updater;
