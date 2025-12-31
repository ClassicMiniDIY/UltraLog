//! UI rendering modules for the UltraLog application.
//!
//! This module organizes the various UI components into logical submodules:
//!
//! ## New Activity Bar Architecture
//! - `activity_bar` - VS Code-style vertical icon strip for panel navigation
//! - `side_panel` - Container that routes to the appropriate panel
//! - `files_panel` - File management, loading, and file list
//! - `channels_panel` - Channel selection (works in all modes)
//! - `tools_panel` - Analysis tools, computed channels, export
//! - `settings_panel` - Consolidated settings (display, units, normalization, updates)
//!
//! ## Core UI Components
//! - `sidebar` - Legacy files panel (being replaced by files_panel)
//! - `channels` - Legacy channel selection (being replaced by channels_panel)
//! - `chart` - Main chart rendering and legends
//! - `timeline` - Timeline scrubber and playback controls
//! - `menu` - Menu bar (File, Edit, View, Help)
//! - `toast` - Toast notification system
//! - `icons` - Custom icon drawing utilities
//! - `export` - Chart export functionality (PNG, PDF)
//! - `normalization_editor` - Field normalization customization window
//! - `tool_switcher` - Tool mode selection (Log Viewer, Scatter Plot, Histogram)
//! - `scatter_plot` - Scatter plot visualization view
//! - `histogram` - Histogram visualization view
//! - `tab_bar` - Chrome-style tabs for managing multiple log files
//! - `update_dialog` - Auto-update dialog window
//! - `analysis_panel` - Signal analysis tools window
//! - `computed_channels_manager` - Computed channels library manager
//! - `formula_editor` - Formula creation and editing

// New activity bar architecture
pub mod activity_bar;
pub mod channels_panel;
pub mod files_panel;
pub mod settings_panel;
pub mod side_panel;
pub mod tools_panel;

// Core UI components
pub mod analysis_panel;
pub mod channels;
pub mod chart;
pub mod computed_channels_manager;
pub mod export;
pub mod formula_editor;
pub mod histogram;
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
