//! Core application state types and constants.
//!
//! This module contains the fundamental data structures used throughout
//! the application, including loaded files, selected channels, and color palettes.

use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::colormap::Colormap;
use crate::laps::LapInfo;
use crate::parsers::{Channel, EcuType, Log};

// ============================================================================
// Constants
// ============================================================================

/// Maximum number of channels that can be selected simultaneously (in single-plot mode)
pub const MAX_CHANNELS: usize = 10;

/// Maximum number of channels per plot area in stacked mode
pub const MAX_CHANNELS_PER_PLOT: usize = 10;

/// Maximum total channels across all plots in stacked mode (6 plots × 10 channels)
pub const MAX_TOTAL_CHANNELS: usize = 60;

/// Minimum height for a plot area in pixels (stacked mode)
pub const MIN_PLOT_HEIGHT: f32 = 100.0;

/// Height of plot area header (title, controls) in pixels (stacked mode)
pub const PLOT_AREA_HEADER_HEIGHT: f32 = 35.0;

/// Height of resize handle between plots in pixels (stacked mode)
pub const PLOT_RESIZE_HANDLE_HEIGHT: f32 = 8.0;

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
    /// Lazy column-major view of `log.data` as `Vec<Vec<f64>>`. Built on first
    /// access so the chart hot path can borrow `&[f64]` for a channel instead
    /// of re-collecting an owned `Vec<f64>` from the row-major store on every
    /// frame.
    channel_columns: OnceLock<Vec<Vec<f64>>>,
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
            channel_columns: OnceLock::new(),
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

    /// Borrow a regular channel's f64 data without copying. Lazily transposes
    /// `log.data` into column-major form on first call.
    pub fn get_channel_column(&self, channel_index: usize) -> Option<&[f64]> {
        let cols = self.channel_columns.get_or_init(|| {
            (0..self.log.channels.len())
                .map(|i| self.log.get_channel_data(i))
                .collect()
        });
        cols.get(channel_index).map(Vec::as_slice)
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
    /// Plot area ID (0 in single-plot mode, or actual ID in stacked mode)
    pub plot_area_id: usize,
}

/// Cached downsampled chart points per (file_index, channel_index,
/// plot_area_id), tagged with the viewport key they were computed for.
/// Keyed per plot area so a channel shown in two stacked plots with
/// different viewports doesn't evict its own entry every frame.
pub type DownsampleCache =
    std::collections::HashMap<(usize, usize, usize), (DownsampleViewKey, Vec<[f64; 2]>)>;

/// Precomputed 2D histogram for the scatter-plot heatmap. Channel data is
/// immutable once a file is loaded, so this only needs rebuilding when the
/// axis selection changes (cache is cleared on file removal).
pub struct ScatterHistogram {
    pub bins: Vec<Vec<u32>>,
    pub max_hits: u32,
    pub x_min: f64,
    pub x_max: f64,
    pub y_min: f64,
    pub y_max: f64,
}

/// Scatter heatmap cache keyed by (file_index, x_channel, y_channel)
pub type ScatterHistogramCache = std::collections::HashMap<(usize, usize, usize), ScatterHistogram>;

/// Identifies which viewport a cached set of downsampled chart points was
/// computed for. The bucketed downsampler anchors bucket boundaries at
/// multiples of `bucket_size` from t=0, so its output is fully determined
/// by the first bucket index and the bucket width.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DownsampleViewKey {
    /// Full-range LTTB downsample (no viewport bounds available)
    Full,
    /// Anchored min/max-per-bucket downsample over a padded viewport
    Bucketed {
        /// Index of the first bucket (floor of viewport start / bucket size)
        k_lo: i64,
        /// Bit pattern of the f64 bucket width (bitwise-comparable)
        bucket_bits: u64,
    },
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
    /// Tool Properties panel - dynamic panel showing controls for the current tool
    /// (channels for Log Viewer, histogram controls for Histogram, scatter plot controls for Scatter Plot)
    ToolProperties,
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
            ActivePanel::ToolProperties => "Properties",
            ActivePanel::Tools => "Tools",
            ActivePanel::Settings => "Settings",
        }
    }

    /// Get the icon character for this panel (using Unicode symbols)
    /// Note: Activity bar draws custom icons, this is kept for reference
    pub fn icon(&self) -> &'static str {
        match self {
            ActivePanel::Files => "\u{1F4C1}",          // Folder icon
            ActivePanel::ToolProperties => "\u{1F3DB}", // Sliders icon
            ActivePanel::Tools => "\u{1F527}",          // Wrench icon
            ActivePanel::Settings => "\u{2699}",        // Gear icon
        }
    }
}

/// Font scale preference for UI elements
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug, serde::Serialize, serde::Deserialize)]
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

/// Filter configuration for excluding samples based on channel value ranges
#[derive(Clone)]
pub struct SampleFilter {
    /// Channel index to filter on
    pub channel_idx: usize,
    /// Display name for the channel (cached for UI)
    pub channel_name: String,
    /// Minimum value (samples below this are excluded)
    pub min_value: Option<f64>,
    /// Maximum value (samples above this are excluded)
    pub max_value: Option<f64>,
    /// Whether this filter is currently enabled
    pub enabled: bool,
}

impl SampleFilter {
    /// Create a new sample filter
    pub fn new(channel_idx: usize, channel_name: String) -> Self {
        Self {
            channel_idx,
            channel_name,
            min_value: None,
            max_value: None,
            enabled: true,
        }
    }
}

/// Represents a pasted fuel/tune table for comparison operations
#[derive(Clone, Default)]
pub struct PastedTable {
    /// The table data (row-major, y_bin outer, x_bin inner)
    pub data: Vec<Vec<f64>>,
    /// X-axis breakpoints from the pasted table (optional)
    pub x_breakpoints: Vec<f64>,
    /// Y-axis breakpoints from the pasted table (optional)
    pub y_breakpoints: Vec<f64>,
    /// Original dimensions before resampling
    pub original_rows: usize,
    pub original_cols: usize,
    /// Whether the table has been resampled to match histogram grid
    pub is_resampled: bool,
}

/// Operation to apply between histogram and pasted table
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum TableOperation {
    #[default]
    Add,
    Subtract,
    Multiply,
    Divide,
}

impl TableOperation {
    /// Get the display symbol for this operation
    pub fn symbol(&self) -> &'static str {
        match self {
            TableOperation::Add => "+",
            TableOperation::Subtract => "-",
            TableOperation::Multiply => "×",
            TableOperation::Divide => "÷",
        }
    }

    /// Apply the operation to two values
    pub fn apply(&self, histogram_val: f64, table_val: f64) -> f64 {
        match self {
            TableOperation::Add => histogram_val + table_val,
            TableOperation::Subtract => histogram_val - table_val,
            TableOperation::Multiply => histogram_val * table_val,
            TableOperation::Divide => {
                if table_val.abs() < f64::EPSILON {
                    0.0
                } else {
                    histogram_val / table_val
                }
            }
        }
    }
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
    /// Grid size (legacy enum, use custom grid if set)
    pub grid_size: HistogramGridSize,
    /// Custom grid columns (X axis bins). 0 = use grid_size enum
    pub custom_grid_columns: usize,
    /// Custom grid rows (Y axis bins). 0 = use grid_size enum
    pub custom_grid_rows: usize,
    /// Currently selected cell (for statistics display)
    pub selected_cell: Option<SelectedHistogramCell>,
    /// Minimum hits filter - cells with fewer hits are grayed out
    pub min_hits_filter: u32,
    /// Custom X axis range. None = auto from data
    pub custom_x_range: Option<(f64, f64)>,
    /// Custom Y axis range. None = auto from data
    pub custom_y_range: Option<(f64, f64)>,
    /// Sample filters - all must pass for sample to be included (AND logic)
    pub sample_filters: Vec<SampleFilter>,
    /// Pasted table for comparison operations
    pub pasted_table: Option<PastedTable>,
    /// Operation to apply between histogram and pasted table
    pub table_operation: TableOperation,
    /// Whether to show the side-by-side comparison view
    pub show_comparison_view: bool,
}

impl HistogramConfig {
    /// Get the effective grid size as (columns, rows)
    /// Returns custom grid if set, otherwise uses the square grid_size enum for both dimensions
    pub fn effective_grid_size(&self) -> (usize, usize) {
        if self.custom_grid_columns > 0 && self.custom_grid_rows > 0 {
            (
                self.custom_grid_columns.clamp(4, 256),
                self.custom_grid_rows.clamp(4, 256),
            )
        } else {
            let size = self.grid_size.size();
            (size, size)
        }
    }
}

/// State for the histogram view
#[derive(Clone, Default)]
pub struct HistogramState {
    /// Histogram configuration
    pub config: HistogramConfig,
}

// ============================================================================
// Plot Area Types (for Stacked Plot Mode)
// ============================================================================

/// Represents a single plot area in stacked mode
#[derive(Clone)]
pub struct PlotArea {
    /// Unique identifier for this plot area
    pub id: usize,
    /// User-defined name for the plot area
    pub name: String,
    /// Indices into Tab::selected_channels that belong to this plot
    pub channel_indices: Vec<usize>,
    /// Absolute height in pixels for this plot
    pub height_pixels: f32,
    /// Whether this plot area is collapsed (minimized)
    pub collapsed: bool,
}

impl PlotArea {
    /// Create a new plot area with default settings
    pub fn new(id: usize, name: String) -> Self {
        Self {
            id,
            name,
            channel_indices: Vec::new(),
            height_pixels: 300.0, // Default to 300px height
            collapsed: false,
        }
    }

    /// Get the number of channels in this plot area
    pub fn channel_count(&self) -> usize {
        self.channel_indices.len()
    }

    /// Check if this plot area can accept more channels
    pub fn has_capacity(&self) -> bool {
        self.channel_indices.len() < MAX_CHANNELS_PER_PLOT
    }
}

// ============================================================================
// Data Panel + Track Map Types
// ============================================================================

/// Identifier for a satellite/map tile provider.
///
/// New variants slot in here without touching the trait wiring; see
/// `crate::tiles` for the runtime provider trait.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize,
)]
pub enum TileProviderId {
    #[default]
    EsriWorldImagery,
    OpenStreetMap,
}

/// Pan + zoom state for the track map. Independent of the chosen projection.
#[derive(Debug, Clone, Copy)]
pub struct MapView {
    /// User pan offset (screen pixels) on top of the auto-fit transform.
    pub pan_px: eframe::egui::Vec2,
    /// Multiplicative zoom relative to the auto-fit baseline. 1.0 = fit.
    pub zoom: f32,
}

impl Default for MapView {
    fn default() -> Self {
        Self {
            pan_px: eframe::egui::Vec2::ZERO,
            zoom: 1.0,
        }
    }
}

/// Cached projection of a single GPS track. Rebuilt when the source file
/// changes or the lat/lon channel binding changes; per-segment color cache
/// is invalidated on `(color_channel, color_min, color_max, colormap)` change.
#[derive(Debug, Clone)]
pub struct TrackCache {
    pub file_index: usize,
    pub lat_idx: usize,
    pub lon_idx: usize,
    /// Local meters projection (equirectangular) used in the no-tiles path.
    pub points_m: std::sync::Arc<[eframe::egui::Vec2]>,
    /// Web Mercator offsets at zoom 0 relative to the track bbox center.
    /// Keeping local offsets avoids precision loss at high tile zoom levels.
    pub mercator_offsets_z0: std::sync::Arc<[eframe::egui::Vec2]>,
    /// Record indices used for rendering. Large logs are reduced to a bounded
    /// number of points.
    pub render_indices: std::sync::Arc<[usize]>,
    /// Contiguous valid-track segment for each entry in `render_indices`.
    /// Different IDs prevent decimation from joining points across GPS gaps.
    pub render_segments: std::sync::Arc<[Option<u32>]>,
    pub bbox_min: eframe::egui::Vec2,
    pub bbox_max: eframe::egui::Vec2,
    /// (west, east, south, north) in continuous degrees. West/east may fall
    /// outside the conventional longitude range for antimeridian tracks.
    pub lonlat_bbox: (f64, f64, f64, f64),
    /// Per-segment color, lazily populated on first paint after coloring
    /// inputs change.
    pub color_cache: Option<std::sync::Arc<[eframe::egui::Color32]>>,
    /// Snapshot of the color inputs for the cached colors. When this no
    /// longer matches the active selection, the cache is invalidated.
    pub color_signature: Option<ColorSignature>,
    /// Effective min/max values used by `color_cache` and its legend.
    pub color_range: Option<(f64, f64)>,
}

/// Identity tuple for a `color_cache` so we can detect staleness without
/// recomputing the colors themselves.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColorSignature {
    pub channel: Option<(usize, usize)>,
    pub data_address: usize,
    pub data_length: usize,
    pub colormap: Colormap,
    pub requested_min: Option<f64>,
    pub requested_max: Option<f64>,
}

/// Per-tab state for the Track Map widget.
#[derive(Debug, Clone)]
pub struct TrackMapState {
    pub enabled: bool,
    /// Index into `Tab.selected_channels` used to color the polyline.
    /// `None` means a single solid color.
    pub color_channel: Option<usize>,
    pub colormap: Colormap,
    pub color_min: Option<f64>,
    pub color_max: Option<f64>,
    pub view: MapView,
    /// Cached GPS channel lookup for this tab. Interior mutability keeps
    /// availability checks cheap even though the widget registry uses `&self`.
    pub gps_channels: std::cell::Cell<Option<GpsChannelCache>>,
    pub cache: Option<TrackCache>,
    pub laps: Vec<LapInfo>,
    /// `None` means show all laps.
    pub selected_lap: Option<usize>,
    pub tiles_enabled: bool,
    pub tile_provider: TileProviderId,
    /// Tile alpha (0.0..=1.0). 1.0 = fully opaque; lower values let the
    /// background colour bleed through so the coloured polyline pops.
    pub tile_opacity: f32,
    /// When true, decoded tiles are converted to greyscale before upload -
    /// useful when the tile colours fight the data overlay (e.g. green map
    /// vs viridis polyline).
    pub tile_grayscale: bool,
}

impl Default for TrackMapState {
    fn default() -> Self {
        Self {
            enabled: true,
            color_channel: None,
            colormap: Colormap::default(),
            color_min: None,
            color_max: None,
            view: MapView::default(),
            gps_channels: std::cell::Cell::new(None),
            cache: None,
            laps: Vec::new(),
            selected_lap: None,
            tiles_enabled: false,
            tile_provider: TileProviderId::default(),
            tile_opacity: 1.0,
            tile_grayscale: false,
        }
    }
}

/// Identity of a cached GPS channel lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GpsChannelCache {
    pub file_index: usize,
    pub spec_generation: u64,
    pub channels: Option<(usize, usize)>,
}

impl TrackMapState {
    pub(crate) fn remove_color_channel_slot(&mut self, removed_index: usize) {
        match self.color_channel {
            Some(index) if index == removed_index => self.clear_color_channel(),
            Some(index) if index > removed_index => {
                self.color_channel = Some(index - 1);
            }
            _ => {}
        }
    }

    pub(crate) fn clear_color_channel(&mut self) {
        self.color_channel = None;
        self.color_min = None;
        self.color_max = None;
        if let Some(cache) = self.cache.as_mut() {
            cache.color_cache = None;
            cache.color_signature = None;
            cache.color_range = None;
        }
    }
}

/// Per-tab state for the right-side data panel that hosts widgets.
#[derive(Debug, Clone)]
pub struct DataPanelState {
    /// Master visibility toggle for the panel.
    pub visible: bool,
    /// Width of the panel as a fraction of the available central area.
    pub split_fraction: f32,
    pub track_map: TrackMapState,
    // future widgets go here: g_sensor, gauges, camera, ...
}

impl Default for DataPanelState {
    fn default() -> Self {
        Self {
            visible: true,
            split_fraction: 0.4,
            track_map: TrackMapState::default(),
        }
    }
}

// ============================================================================
// Tab Types
// ============================================================================

/// A tab representing a single log file's view state
#[derive(Clone)]
pub struct Tab {
    /// Stable identity used for persistent UI state.
    pub id: u64,
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
    /// Right-side data panel (track map + future widgets) state
    pub data_panel_state: DataPanelState,
    /// Request to jump the view to a specific time (used for min/max jump buttons)
    pub jump_to_time: Option<f64>,
    /// Plot areas for stacked mode (ordered top to bottom)
    pub plot_areas: Vec<PlotArea>,
    /// Whether stacked plot mode is enabled
    pub stacked_mode: bool,
    /// Next available plot area ID (for unique identification)
    pub next_plot_area_id: usize,
}

impl Tab {
    /// Create a new tab for a file
    pub fn new(file_index: usize, name: String) -> Self {
        // Initialize scatter plot state with this tab's file index
        let mut scatter_plot_state = ScatterPlotState::default();
        scatter_plot_state.left.file_index = Some(file_index);
        scatter_plot_state.right.file_index = Some(file_index);

        // Initialize with a single default plot area
        let default_plot = PlotArea::new(0, "Plot 1".to_string());

        Self {
            id: NEXT_TAB_ID.fetch_add(1, Ordering::Relaxed),
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
            data_panel_state: DataPanelState::default(),
            jump_to_time: None,
            plot_areas: vec![default_plot],
            stacked_mode: false,
            next_plot_area_id: 1,
        }
    }
}

static NEXT_TAB_ID: AtomicU64 = AtomicU64::new(1);

#[cfg(test)]
mod tests {
    use super::{Tab, TrackMapState};

    #[test]
    fn track_map_color_slot_follows_channel_removals() {
        let mut state = TrackMapState {
            color_channel: Some(3),
            color_min: Some(1.0),
            color_max: Some(2.0),
            ..TrackMapState::default()
        };

        state.remove_color_channel_slot(1);
        assert_eq!(state.color_channel, Some(2));
        assert_eq!(state.color_min, Some(1.0));
        assert_eq!(state.color_max, Some(2.0));

        state.remove_color_channel_slot(2);
        assert_eq!(state.color_channel, None);
        assert_eq!(state.color_min, None);
        assert_eq!(state.color_max, None);
    }

    #[test]
    fn tabs_receive_distinct_persistent_ids() {
        let first = Tab::new(0, "first".to_string());
        let second = Tab::new(0, "second".to_string());

        assert_ne!(first.id, second.id);
    }
}
