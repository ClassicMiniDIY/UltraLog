//! Core application state types and constants.
//!
//! This module contains the fundamental data structures used throughout
//! the application, including loaded files, selected channels, and color palettes.

use std::path::PathBuf;

use crate::parsers::{Channel, EcuType, Log};

// ============================================================================
// Constants
// ============================================================================

/// Maximum number of channels that can be selected simultaneously
pub const MAX_CHANNELS: usize = 10;

/// Maximum points to render in chart (for performance via LTTB downsampling)
pub const MAX_CHART_POINTS: usize = 2000;

/// Supported log file extensions (used in file dialogs)
pub const SUPPORTED_EXTENSIONS: &[&str] = &[
    "csv", "log", "txt", "mlg", "llg", "llg5", "xrk", "drk", "lg1", "lg2",
];

/// Color palette for chart lines (matches original theme)
pub const CHART_COLORS: &[[u8; 3]] = &[
    [113, 120, 78],  // Olive green (primary)
    [191, 78, 48],   // Rust orange (accent)
    [71, 108, 155],  // Blue (info)
    [159, 166, 119], // Sage green (success)
    [253, 193, 73],  // Amber (warning)
    [135, 30, 28],   // Dark red (error)
    [246, 247, 235], // Cream
    [100, 149, 237], // Cornflower blue
    [255, 127, 80],  // Coral
    [144, 238, 144], // Light green
];

/// Colorblind-friendly palette (based on Wong's optimized palette)
/// Designed to be distinguishable for deuteranopia, protanopia, and tritanopia
pub const COLORBLIND_COLORS: &[[u8; 3]] = &[
    [0, 114, 178],   // Blue
    [230, 159, 0],   // Orange
    [0, 158, 115],   // Bluish green
    [204, 121, 167], // Reddish purple
    [86, 180, 233],  // Sky blue
    [213, 94, 0],    // Vermillion
    [240, 228, 66],  // Yellow
    [0, 0, 0],       // Black (for contrast on light backgrounds, shows as white on dark)
    [136, 204, 238], // Light blue
    [153, 153, 153], // Gray
];

// ============================================================================
// Core Types
// ============================================================================

/// Represents a loaded log file with its parsed data
#[derive(Clone)]
pub struct LoadedFile {
    /// Path to the original file
    pub path: PathBuf,
    /// Display name for the file
    pub name: String,
    /// Type of ECU that generated this log
    pub ecu_type: EcuType,
    /// Parsed log data
    pub log: Log,
    /// Cached flag for each channel: true if channel has non-zero data
    /// Computed once on load for UI performance
    pub channels_with_data: Vec<bool>,
}

impl LoadedFile {
    /// Create a new LoadedFile, computing channel data flags
    pub fn new(path: PathBuf, name: String, ecu_type: EcuType, log: Log) -> Self {
        // Pre-compute which channels have data (any non-zero values)
        let channels_with_data: Vec<bool> = (0..log.channels.len())
            .map(|idx| {
                let data = log.get_channel_data(idx);
                data.iter().any(|&v| v.abs() > 0.0001)
            })
            .collect();

        Self {
            path,
            name,
            ecu_type,
            log,
            channels_with_data,
        }
    }

    /// Check if a channel has meaningful data (cached)
    #[inline]
    pub fn channel_has_data(&self, channel_index: usize) -> bool {
        self.channels_with_data
            .get(channel_index)
            .copied()
            .unwrap_or(false)
    }
}

/// A channel selected for visualization on the chart
#[derive(Clone)]
pub struct SelectedChannel {
    /// Index of the file this channel belongs to
    pub file_index: usize,
    /// Index of the channel within the file
    pub channel_index: usize,
    /// The channel data itself
    pub channel: Channel,
    /// Index into the color palette for this channel's line
    pub color_index: usize,
}

/// Result from background file loading operation
pub enum LoadResult {
    Success(Box<LoadedFile>),
    Error(String),
}

/// Current state of file loading
pub enum LoadingState {
    /// No loading in progress
    Idle,
    /// Loading a file (contains filename being loaded)
    Loading(String),
}

/// Type of toast notification (determines color)
#[derive(Clone, Copy, Default)]
pub enum ToastType {
    /// Informational message (blue)
    #[default]
    Info,
    /// Success message (green)
    Success,
    /// Warning message (amber)
    Warning,
    /// Error message (red)
    Error,
}

impl ToastType {
    /// Get the background color for this toast type
    pub fn color(&self) -> [u8; 3] {
        match self {
            ToastType::Info => [71, 108, 155],    // Blue
            ToastType::Success => [113, 120, 78], // Olive green
            ToastType::Warning => [253, 193, 73], // Amber
            ToastType::Error => [135, 30, 28],    // Dark red
        }
    }

    /// Get the text color for this toast type
    pub fn text_color(&self) -> [u8; 3] {
        match self {
            ToastType::Warning => [30, 30, 30], // Dark text for amber background
            _ => [255, 255, 255],               // White text for other backgrounds
        }
    }
}

/// Cache key for downsampled data, uniquely identifying a channel's data
#[derive(Hash, Eq, PartialEq, Clone)]
pub struct CacheKey {
    pub file_index: usize,
    pub channel_index: usize,
}

// ============================================================================
// Tool/View Types
// ============================================================================

/// The currently active tool/view in the application
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum ActiveTool {
    /// Standard log viewer with time-series chart
    #[default]
    LogViewer,
    /// Scatter plot view for comparing two variables with color coding
    ScatterPlot,
    /// Histogram view for 2D distribution analysis
    Histogram,
}

impl ActiveTool {
    /// Get the display name for this tool
    pub fn name(&self) -> &'static str {
        match self {
            ActiveTool::LogViewer => "Log Viewer",
            ActiveTool::ScatterPlot => "Scatter Plots",
            ActiveTool::Histogram => "Histogram",
        }
    }
}

/// The currently active side panel in the activity bar
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum ActivePanel {
    /// Files panel - file management, loading, file list
    #[default]
    Files,
    /// Channels panel - channel selection and selected channels
    Channels,
    /// Tools panel - analysis tools, computed channels, export
    Tools,
    /// Settings panel - all preferences consolidated
    Settings,
}

impl ActivePanel {
    /// Get the display name for this panel
    pub fn name(&self) -> &'static str {
        match self {
            ActivePanel::Files => "Files",
            ActivePanel::Channels => "Channels",
            ActivePanel::Tools => "Tools",
            ActivePanel::Settings => "Settings",
        }
    }

    /// Get the icon character for this panel (using Unicode symbols)
    pub fn icon(&self) -> &'static str {
        match self {
            ActivePanel::Files => "\u{1F4C1}",    // Folder icon
            ActivePanel::Channels => "\u{1F4CA}", // Chart icon
            ActivePanel::Tools => "\u{1F527}",    // Wrench icon
            ActivePanel::Settings => "\u{2699}",  // Gear icon
        }
    }
}

/// Font scale preference for UI elements
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum FontScale {
    /// Smaller fonts (0.85x)
    Small,
    /// Default size (1.0x)
    #[default]
    Medium,
    /// Larger fonts (1.2x)
    Large,
    /// Extra large fonts (1.4x)
    ExtraLarge,
}

impl FontScale {
    /// Get the multiplier for this font scale
    pub fn multiplier(&self) -> f32 {
        match self {
            FontScale::Small => 0.85,
            FontScale::Medium => 1.0,
            FontScale::Large => 1.2,
            FontScale::ExtraLarge => 1.4,
        }
    }
}

/// A selected point on a heatmap
#[derive(Clone, Default)]
pub struct SelectedHeatmapPoint {
    /// X axis value
    pub x_value: f64,
    /// Y axis value
    pub y_value: f64,
    /// Hit count at this point
    pub hits: u32,
}

/// Configuration for a single scatter plot panel
#[derive(Clone, Default)]
pub struct ScatterPlotConfig {
    /// File index for the data source
    pub file_index: Option<usize>,
    /// Channel index for X axis
    pub x_channel: Option<usize>,
    /// Channel index for Y axis
    pub y_channel: Option<usize>,
    /// Channel index for Z axis (color coding)
    pub z_channel: Option<usize>,
    /// Currently selected point (persisted on click)
    pub selected_point: Option<SelectedHeatmapPoint>,
}

/// State for the scatter plot view (dual plots)
#[derive(Clone, Default)]
pub struct ScatterPlotState {
    /// Configuration for the left scatter plot
    pub left: ScatterPlotConfig,
    /// Configuration for the right scatter plot
    pub right: ScatterPlotConfig,
}

// ============================================================================
// Histogram Types
// ============================================================================

/// Display mode for histogram cell values
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum HistogramMode {
    /// Show average Z-channel value in cells
    #[default]
    AverageZ,
    /// Show hit count (number of data points) in cells
    HitCount,
}

/// Grid size options for histogram
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum HistogramGridSize {
    /// 16x16 grid
    Size16,
    /// 32x32 grid
    #[default]
    Size32,
    /// 64x64 grid
    Size64,
}

impl HistogramGridSize {
    /// Get the numeric size value
    pub fn size(&self) -> usize {
        match self {
            HistogramGridSize::Size16 => 16,
            HistogramGridSize::Size32 => 32,
            HistogramGridSize::Size64 => 64,
        }
    }

    /// Get display name
    pub fn name(&self) -> &'static str {
        match self {
            HistogramGridSize::Size16 => "16x16",
            HistogramGridSize::Size32 => "32x32",
            HistogramGridSize::Size64 => "64x64",
        }
    }
}

/// Statistics for a selected histogram cell
#[derive(Clone, Default)]
pub struct SelectedHistogramCell {
    /// X bin index
    pub x_bin: usize,
    /// Y bin index
    pub y_bin: usize,
    /// X axis value range (min, max) for this cell
    pub x_range: (f64, f64),
    /// Y axis value range (min, max) for this cell
    pub y_range: (f64, f64),
    /// Number of data points in cell
    pub hit_count: u32,
    /// Sum of weights (for weighted averaging)
    pub cell_weight: f64,
    /// Variance of Z values
    pub variance: f64,
    /// Standard deviation of Z values
    pub std_dev: f64,
    /// Minimum Z value in cell
    pub minimum: f64,
    /// Mean Z value in cell
    pub mean: f64,
    /// Maximum Z value in cell
    pub maximum: f64,
}

/// Configuration for the histogram view
#[derive(Clone, Default)]
pub struct HistogramConfig {
    /// Channel index for X axis
    pub x_channel: Option<usize>,
    /// Channel index for Y axis
    pub y_channel: Option<usize>,
    /// Channel index for Z axis (value to average)
    pub z_channel: Option<usize>,
    /// Display mode (average Z vs hit count)
    pub mode: HistogramMode,
    /// Grid size
    pub grid_size: HistogramGridSize,
    /// Currently selected cell (for statistics display)
    pub selected_cell: Option<SelectedHistogramCell>,
}

/// State for the histogram view
#[derive(Clone, Default)]
pub struct HistogramState {
    /// Histogram configuration
    pub config: HistogramConfig,
}

// ============================================================================
// Tab Types
// ============================================================================

/// A tab representing a single log file's view state
#[derive(Clone)]
pub struct Tab {
    /// Index of the file this tab displays
    pub file_index: usize,
    /// Display name for the tab (usually filename)
    pub name: String,
    /// Channels selected for visualization in this tab
    pub selected_channels: Vec<SelectedChannel>,
    /// Channel search/filter text for this tab
    pub channel_search: String,
    /// Current cursor position in seconds for this tab
    pub cursor_time: Option<f64>,
    /// Current data record index at cursor position
    pub cursor_record: Option<usize>,
    /// Whether user has interacted with chart zoom/pan
    pub chart_interacted: bool,
    /// Time range for this tab's log file (min, max)
    pub time_range: Option<(f64, f64)>,
    /// Scatter plot state for this tab (dual heatmaps)
    pub scatter_plot_state: ScatterPlotState,
    /// Histogram state for this tab
    pub histogram_state: HistogramState,
    /// Request to jump the view to a specific time (used for min/max jump buttons)
    pub jump_to_time: Option<f64>,
}

impl Tab {
    /// Create a new tab for a file
    pub fn new(file_index: usize, name: String) -> Self {
        // Initialize scatter plot state with this tab's file index
        let mut scatter_plot_state = ScatterPlotState::default();
        scatter_plot_state.left.file_index = Some(file_index);
        scatter_plot_state.right.file_index = Some(file_index);

        Self {
            file_index,
            name,
            selected_channels: Vec::new(),
            channel_search: String::new(),
            cursor_time: None,
            cursor_record: None,
            chart_interacted: false,
            time_range: None,
            scatter_plot_state,
            histogram_state: HistogramState::default(),
            jump_to_time: None,
        }
    }
}
