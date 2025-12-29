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
//! - `normalization_editor` - Field normalization customization window
//! - `tool_switcher` - Pill-style tab navigation between tools
//! - `scatter_plot` - Scatter plot visualization view
//! - `tab_bar` - Chrome-style tabs for managing multiple log files
//! - `update_dialog` - Auto-update dialog window

pub mod channels;
pub mod chart;
pub mod computed_channels_manager;
pub mod export;
pub mod formula_editor;
pub mod icons;
pub mod menu;
pub mod normalization_editor;
pub mod scatter_plot;
pub mod sidebar;
pub mod tab_bar;
pub mod timeline;
pub mod toast;
pub mod tool_switcher;
pub mod update_dialog;
