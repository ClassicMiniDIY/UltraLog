//! UI rendering modules for the UltraLog application.
//!
//! This module organizes the various UI components into logical submodules:
//! - `sidebar` - Files panel and view options
//! - `channels` - Channel selection and display
//! - `chart` - Main chart rendering and legends
//! - `timeline` - Timeline scrubber and playback controls
//! - `menu` - Menu bar (File, Units, Help)
//! - `toast` - Toast notification system
//! - `icons` - Custom icon drawing utilities
//! - `export` - Chart export functionality (PNG, PDF)

pub mod channels;
pub mod chart;
pub mod export;
pub mod icons;
pub mod menu;
pub mod sidebar;
pub mod timeline;
pub mod toast;
