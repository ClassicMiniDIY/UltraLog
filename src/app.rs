use eframe::egui;
use egui_plot::{Line, Plot, PlotBounds, PlotPoints, VLine};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

use crate::parsers::{Channel, EcuType, Haltech, Log, Parseable};

/// Color palette for chart lines (matches original theme)
const CHART_COLORS: &[[u8; 3]] = &[
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
const COLORBLIND_COLORS: &[[u8; 3]] = &[
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

/// Maximum number of channels that can be selected
const MAX_CHANNELS: usize = 10;

/// Maximum points to render in chart (for performance)
const MAX_CHART_POINTS: usize = 2000;

/// Represents a loaded log file
#[derive(Clone)]
pub struct LoadedFile {
    pub path: PathBuf,
    pub name: String,
    pub ecu_type: EcuType,
    pub log: Log,
}

/// Selected channel for visualization
#[derive(Clone)]
pub struct SelectedChannel {
    pub file_index: usize,
    pub channel_index: usize,
    pub channel: Channel,
    pub color_index: usize,
}

/// Result from background file loading
enum LoadResult {
    Success(Box<LoadedFile>),
    Error(String),
}

/// Loading state
enum LoadingState {
    Idle,
    Loading(String), // filename being loaded
}

/// Cache key for downsampled data
#[derive(Hash, Eq, PartialEq, Clone)]
struct CacheKey {
    file_index: usize,
    channel_index: usize,
}

/// Main application state
pub struct UltraLogApp {
    /// List of loaded log files
    files: Vec<LoadedFile>,
    /// Currently selected file index
    selected_file: Option<usize>,
    /// Channels selected for visualization
    selected_channels: Vec<SelectedChannel>,
    /// Channel search/filter text
    channel_search: String,
    /// Toast messages for user feedback
    toast_message: Option<(String, std::time::Instant)>,
    /// Track dropped files to prevent duplicates
    last_drop_time: Option<std::time::Instant>,
    /// Channel for receiving loaded files from background thread
    load_receiver: Option<Receiver<LoadResult>>,
    /// Current loading state
    loading_state: LoadingState,
    /// Cache for downsampled chart data
    downsample_cache: HashMap<CacheKey, Vec<[f64; 2]>>,
    /// Current cursor position in seconds (timeline feature)
    cursor_time: Option<f64>,
    /// Total time range across all loaded files (min, max)
    time_range: Option<(f64, f64)>,
    /// Current data record index at cursor position
    cursor_record: Option<usize>,
    // === View Options ===
    /// When true, keep cursor centered and pan graph during scrubbing
    cursor_tracking: bool,
    /// Visible time window width in seconds (for cursor tracking mode)
    view_window_seconds: f64,
    // === Playback ===
    /// Whether playback is active
    is_playing: bool,
    /// Last frame time for calculating delta
    last_frame_time: Option<std::time::Instant>,
    /// Playback speed multiplier (1.0 = real-time)
    playback_speed: f64,
    // === Accessibility ===
    /// When true, use colorblind-friendly color palette
    color_blind_mode: bool,
    // === Chart View State ===
    /// Whether user has interacted with chart zoom/pan (false = use initial zoomed view)
    chart_interacted: bool,
    /// Initial view window in seconds (shown before user interacts with chart)
    initial_view_seconds: f64,
}

impl Default for UltraLogApp {
    fn default() -> Self {
        Self {
            files: Vec::new(),
            selected_file: None,
            selected_channels: Vec::new(),
            channel_search: String::new(),
            toast_message: None,
            last_drop_time: None,
            load_receiver: None,
            loading_state: LoadingState::Idle,
            downsample_cache: HashMap::new(),
            cursor_time: None,
            time_range: None,
            cursor_record: None,
            cursor_tracking: false,
            view_window_seconds: 30.0, // Default 30 second window
            is_playing: false,
            last_frame_time: None,
            playback_speed: 1.0,
            color_blind_mode: false,
            chart_interacted: false,
            initial_view_seconds: 60.0, // Start with 60 second view
        }
    }
}

impl UltraLogApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Load custom Outfit font
        let mut fonts = egui::FontDefinitions::default();

        // Load Outfit Regular
        fonts.font_data.insert(
            "Outfit-Regular".to_owned(),
            egui::FontData::from_static(include_bytes!("../assets/Outfit-Regular.ttf")),
        );

        // Load Outfit Bold
        fonts.font_data.insert(
            "Outfit-Bold".to_owned(),
            egui::FontData::from_static(include_bytes!("../assets/Outfit-Bold.ttf")),
        );

        // Set Outfit as the primary proportional font
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, "Outfit-Regular".to_owned());

        // Add bold variant for strong text
        fonts
            .families
            .entry(egui::FontFamily::Name("Bold".into()))
            .or_default()
            .insert(0, "Outfit-Bold".to_owned());

        // Apply fonts
        cc.egui_ctx.set_fonts(fonts);

        Self::default()
    }

    /// Get color for a channel based on color blind mode setting
    fn get_channel_color(&self, color_index: usize) -> [u8; 3] {
        let palette = if self.color_blind_mode {
            COLORBLIND_COLORS
        } else {
            CHART_COLORS
        };
        palette[color_index % palette.len()]
    }

    /// Start loading a file in the background
    fn start_loading_file(&mut self, path: PathBuf) {
        // Check for duplicate
        if self.files.iter().any(|f| f.path == path) {
            self.show_toast("File already loaded");
            return;
        }

        let filename = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "Unknown".to_string());

        self.loading_state = LoadingState::Loading(filename.clone());

        let (sender, receiver): (Sender<LoadResult>, Receiver<LoadResult>) = channel();
        self.load_receiver = Some(receiver);

        // Spawn background thread for loading
        thread::spawn(move || {
            let result = Self::load_file_sync(path);
            let _ = sender.send(result);
        });
    }

    /// Synchronously load a file (runs in background thread)
    fn load_file_sync(path: PathBuf) -> LoadResult {
        let contents = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => return LoadResult::Error(format!("Failed to read file: {}", e)),
        };

        let parser = Haltech;
        let log = match parser.parse(&contents) {
            Ok(l) => l,
            Err(e) => return LoadResult::Error(format!("Failed to parse file: {}", e)),
        };

        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "Unknown".to_string());

        LoadResult::Success(Box::new(LoadedFile {
            path,
            name,
            ecu_type: EcuType::Haltech,
            log,
        }))
    }

    /// Check for completed background loads
    fn check_loading_complete(&mut self) {
        if let Some(receiver) = &self.load_receiver {
            if let Ok(result) = receiver.try_recv() {
                match result {
                    LoadResult::Success(file) => {
                        self.files.push(*file);
                        self.selected_file = Some(self.files.len() - 1);
                        self.update_time_range();
                        // Reset chart interaction so new file shows initial zoomed view
                        self.chart_interacted = false;
                        self.show_toast("File loaded successfully");
                    }
                    LoadResult::Error(e) => {
                        self.show_toast(&format!("Error: {}", e));
                    }
                }
                self.load_receiver = None;
                self.loading_state = LoadingState::Idle;
            }
        }
    }

    /// Update the total time range based on all loaded files
    fn update_time_range(&mut self) {
        let mut min_time = f64::MAX;
        let mut max_time = f64::MIN;

        for file in &self.files {
            let times = file.log.get_times_as_f64();
            if let (Some(&first), Some(&last)) = (times.first(), times.last()) {
                min_time = min_time.min(first);
                max_time = max_time.max(last);
            }
        }

        if min_time <= max_time {
            self.time_range = Some((min_time, max_time));
            // Set cursor to start if not already set
            if self.cursor_time.is_none() {
                self.cursor_time = Some(min_time);
                self.cursor_record = Some(0);
            }
        } else {
            self.time_range = None;
            self.cursor_time = None;
            self.cursor_record = None;
        }
    }

    /// Find the record index closest to the given time
    fn find_record_at_time(&self, time: f64) -> Option<usize> {
        // Use the first file with data for record indexing
        if let Some(file) = self.files.first() {
            let times = file.log.get_times_as_f64();
            if times.is_empty() {
                return None;
            }
            // Binary search for closest time
            let mut low = 0;
            let mut high = times.len() - 1;
            while low < high {
                let mid = (low + high) / 2;
                if times[mid] < time {
                    low = mid + 1;
                } else {
                    high = mid;
                }
            }
            // Check if low or low-1 is closer
            if low > 0 && (times[low] - time).abs() > (times[low - 1] - time).abs() {
                Some(low - 1)
            } else {
                Some(low)
            }
        } else {
            None
        }
    }

    /// Get value at a specific record index for a channel
    fn get_value_at_record(
        &self,
        file_index: usize,
        channel_index: usize,
        record: usize,
    ) -> Option<f64> {
        if file_index < self.files.len() {
            let file = &self.files[file_index];
            if record < file.log.data.len() && channel_index < file.log.data[record].len() {
                return Some(file.log.data[record][channel_index].as_f64());
            }
        }
        None
    }

    /// Remove a loaded file
    fn remove_file(&mut self, index: usize) {
        if index < self.files.len() {
            // Remove any selected channels from this file
            self.selected_channels.retain(|c| c.file_index != index);

            // Clear cache entries for this file and update indices
            let mut new_cache = HashMap::new();
            for (key, value) in self.downsample_cache.drain() {
                if key.file_index == index {
                    // Skip entries for removed file
                    continue;
                } else if key.file_index > index {
                    // Update indices for files after the removed one
                    new_cache.insert(
                        CacheKey {
                            file_index: key.file_index - 1,
                            channel_index: key.channel_index,
                        },
                        value,
                    );
                } else {
                    new_cache.insert(key, value);
                }
            }
            self.downsample_cache = new_cache;

            // Update file indices for remaining channels
            for channel in &mut self.selected_channels {
                if channel.file_index > index {
                    channel.file_index -= 1;
                }
            }

            self.files.remove(index);

            // Update selected file
            if let Some(selected) = self.selected_file {
                if selected == index {
                    self.selected_file = if self.files.is_empty() { None } else { Some(0) };
                } else if selected > index {
                    self.selected_file = Some(selected - 1);
                }
            }

            // Update time range after file removal
            self.update_time_range();
        }
    }

    /// Add a channel to the selection
    fn add_channel(&mut self, file_index: usize, channel_index: usize) {
        if self.selected_channels.len() >= MAX_CHANNELS {
            self.show_toast("Maximum 10 channels reached");
            return;
        }

        // Check for duplicate
        if self
            .selected_channels
            .iter()
            .any(|c| c.file_index == file_index && c.channel_index == channel_index)
        {
            self.show_toast("Channel already selected");
            return;
        }

        let file = &self.files[file_index];
        let channel = file.log.channels[channel_index].clone();

        // Find the first unused color index
        let used_colors: std::collections::HashSet<usize> = self
            .selected_channels
            .iter()
            .map(|c| c.color_index)
            .collect();

        let color_index = (0..CHART_COLORS.len())
            .find(|i| !used_colors.contains(i))
            .unwrap_or(0);

        self.selected_channels.push(SelectedChannel {
            file_index,
            channel_index,
            channel,
            color_index,
        });
    }

    /// Remove a channel from the selection
    fn remove_channel(&mut self, index: usize) {
        if index < self.selected_channels.len() {
            self.selected_channels.remove(index);
        }
    }

    /// Show a toast message
    fn show_toast(&mut self, message: &str) {
        self.toast_message = Some((message.to_string(), std::time::Instant::now()));
    }

    /// LTTB (Largest Triangle Three Buckets) downsampling algorithm
    /// Reduces data points while preserving visual shape
    fn downsample_lttb(times: &[f64], values: &[f64], target_points: usize) -> Vec<[f64; 2]> {
        let n = times.len();

        if n <= target_points || target_points < 3 {
            // No downsampling needed
            return times
                .iter()
                .zip(values.iter())
                .map(|(t, v)| [*t, *v])
                .collect();
        }

        let mut result = Vec::with_capacity(target_points);

        // Always include first point
        result.push([times[0], values[0]]);

        // Bucket size
        let bucket_size = (n - 2) as f64 / (target_points - 2) as f64;

        let mut a_index = 0usize;

        for i in 0..(target_points - 2) {
            // Calculate bucket range
            let bucket_start = ((i as f64 + 1.0) * bucket_size).floor() as usize + 1;
            let bucket_end = (((i + 2) as f64) * bucket_size).floor() as usize + 1;
            let bucket_end = bucket_end.min(n - 1);

            // Calculate average point for next bucket (for triangle calculation)
            let next_bucket_start = bucket_end;
            let next_bucket_end = (((i + 3) as f64) * bucket_size).floor() as usize + 1;
            let next_bucket_end = next_bucket_end.min(n);

            let (avg_x, avg_y) = if next_bucket_start < next_bucket_end {
                let count = (next_bucket_end - next_bucket_start) as f64;
                let sum_x: f64 = times[next_bucket_start..next_bucket_end].iter().sum();
                let sum_y: f64 = values[next_bucket_start..next_bucket_end].iter().sum();
                (sum_x / count, sum_y / count)
            } else {
                (times[n - 1], values[n - 1])
            };

            // Find point in current bucket with largest triangle area
            let mut max_area = -1.0f64;
            let mut max_index = bucket_start;

            let a_x = times[a_index];
            let a_y = values[a_index];

            for j in bucket_start..bucket_end {
                // Calculate triangle area
                let area =
                    ((a_x - avg_x) * (values[j] - a_y) - (a_x - times[j]) * (avg_y - a_y)).abs();

                if area > max_area {
                    max_area = area;
                    max_index = j;
                }
            }

            result.push([times[max_index], values[max_index]]);
            a_index = max_index;
        }

        // Always include last point
        result.push([times[n - 1], values[n - 1]]);

        result
    }

    /// Render the file sidebar
    fn render_sidebar(&mut self, ui: &mut egui::Ui) {
        ui.heading("Files");
        ui.separator();

        // Show loading indicator
        if let LoadingState::Loading(filename) = &self.loading_state {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(format!("Loading {}...", filename));
            });
            ui.separator();
        }

        let is_loading = matches!(self.loading_state, LoadingState::Loading(_));

        // File list (if any files loaded)
        if !self.files.is_empty() {
            let mut file_to_remove: Option<usize> = None;
            for (i, file) in self.files.iter().enumerate() {
                let is_selected = self.selected_file == Some(i);

                ui.horizontal(|ui| {
                    let response = ui.selectable_label(is_selected, &file.name);
                    if response.clicked() {
                        self.selected_file = Some(i);
                    }

                    // Delete button
                    if ui.small_button("x").clicked() {
                        file_to_remove = Some(i);
                    }
                });

                // Show ECU type and data info
                ui.indent(format!("file_indent_{}", i), |ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "{} | {} channels | {} points",
                            file.ecu_type.name(),
                            file.log.channels.len(),
                            file.log.data.len()
                        ))
                        .small()
                        .color(egui::Color32::GRAY),
                    );
                });
            }

            if let Some(index) = file_to_remove {
                self.remove_file(index);
            }

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(5.0);

            // Add more files button (compact when files exist)
            ui.add_enabled_ui(!is_loading, |ui| {
                if ui.button("+ Add File").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Log Files", &["csv", "log", "txt"])
                        .pick_file()
                    {
                        self.start_loading_file(path);
                    }
                }
            });
        } else if !is_loading {
            // Nice drop zone when no files loaded
            let primary_color = egui::Color32::from_rgb(113, 120, 78); // Olive green
            let card_bg = egui::Color32::from_rgb(45, 45, 45); // Dark card for dark theme
            let text_gray = egui::Color32::from_rgb(150, 150, 150);

            ui.add_space(20.0);

            // Drop zone card
            egui::Frame::none()
                .fill(card_bg)
                .rounding(12.0)
                .inner_margin(20.0)
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(70, 70, 70)))
                .show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        // Upload icon
                        let icon_size = 32.0;
                        let (icon_rect, _) = ui.allocate_exact_size(
                            egui::vec2(icon_size, icon_size),
                            egui::Sense::hover(),
                        );
                        Self::draw_upload_icon(ui, icon_rect.center(), icon_size, primary_color);

                        ui.add_space(12.0);

                        // Select file button
                        let button_response = egui::Frame::none()
                            .fill(primary_color)
                            .rounding(6.0)
                            .inner_margin(egui::vec2(16.0, 8.0))
                            .show(ui, |ui| {
                                ui.label(
                                    egui::RichText::new("Select a file")
                                        .color(egui::Color32::WHITE)
                                        .size(14.0),
                                );
                            });

                        if button_response
                            .response
                            .interact(egui::Sense::click())
                            .clicked()
                        {
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("Log Files", &["csv", "log", "txt"])
                                .pick_file()
                            {
                                self.start_loading_file(path);
                            }
                        }

                        if button_response.response.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }

                        ui.add_space(12.0);

                        ui.label(egui::RichText::new("or").color(text_gray).size(12.0));

                        ui.add_space(8.0);

                        ui.label(
                            egui::RichText::new("Drop file here")
                                .color(egui::Color32::LIGHT_GRAY)
                                .size(13.0),
                        );

                        ui.add_space(12.0);

                        ui.label(
                            egui::RichText::new("CSV, LOG, TXT")
                                .color(text_gray)
                                .size(11.0),
                        );
                    });
                });
        }

        // View Options section at bottom
        ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
            // Reverse order since we're bottom-up
            ui.add_space(10.0);

            // Only show options when we have data to view
            if !self.files.is_empty() && !self.selected_channels.is_empty() {
                egui::Frame::none()
                    .fill(egui::Color32::from_rgb(35, 35, 35))
                    .rounding(8.0)
                    .inner_margin(10.0)
                    .show(ui, |ui| {
                        // Cursor tracking checkbox
                        ui.checkbox(&mut self.cursor_tracking, "Cursor Tracking");
                        ui.label(
                            egui::RichText::new("Keep cursor centered while scrubbing")
                                .small()
                                .color(egui::Color32::GRAY),
                        );

                        // Window size slider (only show when cursor tracking is enabled)
                        if self.cursor_tracking {
                            ui.add_space(8.0);
                            ui.label("View Window:");
                            ui.add(
                                egui::Slider::new(&mut self.view_window_seconds, 5.0..=120.0)
                                    .suffix("s")
                                    .logarithmic(true),
                            );
                        }

                        ui.add_space(8.0);
                        ui.separator();
                        ui.add_space(4.0);

                        // Color blind mode checkbox
                        ui.checkbox(&mut self.color_blind_mode, "Color Blind Mode");
                        ui.label(
                            egui::RichText::new("Use accessible color palette")
                                .small()
                                .color(egui::Color32::GRAY),
                        );
                    });

                ui.add_space(5.0);
                ui.separator();
                ui.heading("View Options");
            }
        });
    }

    /// Render channel selection panel - fills available space
    fn render_channel_selection(&mut self, ui: &mut egui::Ui) {
        ui.heading("Channels");
        ui.separator();

        if let Some(file_index) = self.selected_file {
            let file: &LoadedFile = &self.files[file_index];

            // Search box
            ui.horizontal(|ui| {
                ui.label("Search:");
                ui.add(
                    egui::TextEdit::singleline(&mut self.channel_search)
                        .desired_width(f32::INFINITY),
                );
            });

            ui.add_space(5.0);

            // Channel count
            ui.label(format!(
                "Selected: {} / {} | Total: {}",
                self.selected_channels.len(),
                MAX_CHANNELS,
                file.log.channels.len()
            ));

            ui.separator();

            // Channel list - use all remaining vertical space
            let search_lower = self.channel_search.to_lowercase();
            let mut channel_to_add: Option<(usize, usize)> = None;

            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());

                    for (channel_index, channel) in file.log.channels.iter().enumerate() {
                        let name = channel.name();

                        // Filter by search
                        if !search_lower.is_empty() && !name.to_lowercase().contains(&search_lower)
                        {
                            continue;
                        }

                        // Check if already selected
                        let is_selected = self.selected_channels.iter().any(|c| {
                            c.file_index == file_index && c.channel_index == channel_index
                        });

                        // Build the label with checkmark prefix if selected
                        let label_text = if is_selected {
                            format!("[*] {}", name)
                        } else {
                            format!("[ ] {}", name)
                        };

                        let response = ui.selectable_label(is_selected, label_text);

                        if response.clicked() && !is_selected {
                            channel_to_add = Some((file_index, channel_index));
                        }
                    }
                });

            // Handle deferred channel addition
            if let Some((file_idx, channel_idx)) = channel_to_add {
                self.add_channel(file_idx, channel_idx);
            }
        } else {
            ui.centered_and_justified(|ui| {
                ui.label(
                    egui::RichText::new("Select a file to view channels")
                        .italics()
                        .color(egui::Color32::GRAY),
                );
            });
        }
    }

    /// Render selected channel cards
    fn render_selected_channels(&mut self, ui: &mut egui::Ui) {
        ui.heading("Selected Channels");
        ui.separator();

        let mut channel_to_remove: Option<usize> = None;

        egui::ScrollArea::horizontal().show(ui, |ui| {
            ui.horizontal(|ui| {
                for (i, selected) in self.selected_channels.iter().enumerate() {
                    let color = self.get_channel_color(selected.color_index);
                    let color32 = egui::Color32::from_rgb(color[0], color[1], color[2]);

                    egui::Frame::none()
                        .fill(egui::Color32::from_rgb(40, 40, 40))
                        .stroke(egui::Stroke::new(2.0, color32))
                        .rounding(5.0)
                        .inner_margin(10.0)
                        .show(ui, |ui| {
                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new(selected.channel.name())
                                            .strong()
                                            .color(color32),
                                    );
                                    if ui.small_button("x").clicked() {
                                        channel_to_remove = Some(i);
                                    }
                                });

                                ui.label(
                                    egui::RichText::new(format!(
                                        "Type: {}",
                                        selected.channel.type_name()
                                    ))
                                    .small()
                                    .color(egui::Color32::GRAY),
                                );

                                if let (Some(min), Some(max)) = (
                                    selected.channel.display_min(),
                                    selected.channel.display_max(),
                                ) {
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "Range: {:.0} - {:.0}",
                                            min, max
                                        ))
                                        .small()
                                        .color(egui::Color32::GRAY),
                                    );
                                }
                            });
                        });

                    ui.add_space(5.0);
                }
            });
        });

        if let Some(index) = channel_to_remove {
            self.remove_channel(index);
        }

        if self.selected_channels.is_empty() {
            ui.label(
                egui::RichText::new("Click channels to add them to the chart")
                    .italics()
                    .color(egui::Color32::GRAY),
            );
        }
    }

    /// Format time in seconds to a human-readable string (h:mm:ss.xxx or m:ss.xxx or s.xxx)
    fn format_time(seconds: f64) -> String {
        let total_seconds = seconds.abs();
        let hours = (total_seconds / 3600.0).floor() as u32;
        let minutes = ((total_seconds % 3600.0) / 60.0).floor() as u32;
        let secs = total_seconds % 60.0;

        let sign = if seconds < 0.0 { "-" } else { "" };

        if hours > 0 {
            // h:mm:ss.xxx format
            format!("{}{}:{:02}:{:06.3}", sign, hours, minutes, secs)
        } else if minutes > 0 {
            // m:ss.xxx format
            format!("{}{}:{:06.3}", sign, minutes, secs)
        } else {
            // s.xxxs format
            format!("{}{:.3}s", sign, secs)
        }
    }

    /// Normalize values to 0-1 range
    fn normalize_points(points: &[[f64; 2]]) -> Vec<[f64; 2]> {
        if points.is_empty() {
            return Vec::new();
        }

        // Find min and max Y values
        let mut min_y = f64::MAX;
        let mut max_y = f64::MIN;
        for point in points {
            min_y = min_y.min(point[1]);
            max_y = max_y.max(point[1]);
        }

        // Handle case where all values are the same
        let range = max_y - min_y;
        if range.abs() < f64::EPSILON {
            // All values are the same, put at 0.5
            return points.iter().map(|p| [p[0], 0.5]).collect();
        }

        // Normalize to 0-1 range
        points
            .iter()
            .map(|p| [p[0], (p[1] - min_y) / range])
            .collect()
    }

    /// Render the main chart with cached downsampled data
    fn render_chart(&mut self, ui: &mut egui::Ui) {
        if self.selected_channels.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label(
                    egui::RichText::new("Select channels to display chart")
                        .size(20.0)
                        .color(egui::Color32::GRAY),
                );
            });
            return;
        }

        // Pre-compute and cache downsampled + normalized data for all selected channels
        for selected in &self.selected_channels {
            if selected.file_index >= self.files.len() {
                continue;
            }

            let cache_key = CacheKey {
                file_index: selected.file_index,
                channel_index: selected.channel_index,
            };

            if !self.downsample_cache.contains_key(&cache_key) {
                let file = &self.files[selected.file_index];
                let times = file.log.get_times_as_f64();
                let data = file.log.get_channel_data(selected.channel_index);

                if times.len() == data.len() && !times.is_empty() {
                    let downsampled = Self::downsample_lttb(&times, &data, MAX_CHART_POINTS);
                    // Normalize Y values to 0-1 range so all channels overlay
                    let normalized = Self::normalize_points(&downsampled);
                    self.downsample_cache.insert(cache_key, normalized);
                }
            }
        }

        // Pre-compute legend names with current values at cursor position
        let legend_names: Vec<String> = self
            .selected_channels
            .iter()
            .map(|selected| {
                let base_name = selected.channel.name();
                if let Some(record) = self.cursor_record {
                    if let Some(value) = self.get_value_at_record(
                        selected.file_index,
                        selected.channel_index,
                        record,
                    ) {
                        let unit = selected.channel.unit();
                        if unit.is_empty() {
                            format!("{}: {:.2}", base_name, value)
                        } else {
                            format!("{}: {:.2} {}", base_name, value, unit)
                        }
                    } else {
                        base_name.to_string()
                    }
                } else {
                    base_name.to_string()
                }
            })
            .collect();

        // Prepare data for the plot closure (can't borrow self mutably inside)
        let cache = &self.downsample_cache;
        let files = &self.files;
        let selected_channels = &self.selected_channels;
        let cursor_time = self.cursor_time;
        let cursor_tracking = self.cursor_tracking;
        let view_window = self.view_window_seconds;
        let time_range = self.time_range;
        let color_blind_mode = self.color_blind_mode;
        let chart_interacted = self.chart_interacted;
        let initial_view_seconds = self.initial_view_seconds;

        // Fixed Y bounds for normalized data (0-1 with small padding)
        const Y_MIN: f64 = -0.05;
        const Y_MAX: f64 = 1.05;

        // Build the plot - X-axis zoom only, Y fixed
        let plot = Plot::new("log_chart")
            .legend(egui_plot::Legend::default())
            .y_axis_label("") // Hide Y axis label since values are normalized
            .show_axes([true, false]) // Show X axis (time), hide Y axis (normalized 0-1)
            .allow_zoom([true, false]) // Only allow X-axis zoom
            .allow_drag([!cursor_tracking, false]) // Only allow X-axis drag, never Y
            .allow_scroll([!cursor_tracking, false]); // Only allow X-axis scroll, never Y

        let response = plot.show(ui, |plot_ui| {
            // Get current bounds
            let current_bounds = plot_ui.plot_bounds();
            let mut x_min = current_bounds.min()[0];
            let mut x_max = current_bounds.max()[0];

            // In cursor tracking mode, center on cursor
            if cursor_tracking {
                if let (Some(cursor), Some((min_t, max_t))) = (cursor_time, time_range) {
                    let half_window = view_window / 2.0;
                    x_min = (cursor - half_window).max(min_t);
                    x_max = (cursor + half_window).min(max_t);
                }
            } else if let Some((min_t, max_t)) = time_range {
                let data_width = max_t - min_t;

                // If chart hasn't been interacted with yet, use initial zoomed view
                if !chart_interacted && data_width > initial_view_seconds {
                    // Show initial view window starting from the beginning
                    x_min = min_t;
                    x_max = min_t + initial_view_seconds;
                } else {
                    // Clamp X bounds to data range - prevent zooming out beyond data
                    let current_width = x_max - x_min;

                    // Don't allow view wider than data range
                    if current_width > data_width {
                        x_min = min_t;
                        x_max = max_t;
                    } else {
                        // Keep view within data bounds
                        if x_min < min_t {
                            x_min = min_t;
                            x_max = min_t + current_width;
                        }
                        if x_max > max_t {
                            x_max = max_t;
                            x_min = max_t - current_width;
                        }
                    }
                }
            }

            // Always enforce bounds: X clamped to data, Y fixed to normalized range
            let new_bounds = PlotBounds::from_min_max([x_min, Y_MIN], [x_max, Y_MAX]);
            plot_ui.set_plot_bounds(new_bounds);

            // Draw channel data lines with values in legend
            for (i, selected) in selected_channels.iter().enumerate() {
                if selected.file_index >= files.len() {
                    continue;
                }

                let cache_key = CacheKey {
                    file_index: selected.file_index,
                    channel_index: selected.channel_index,
                };

                if let Some(points) = cache.get(&cache_key) {
                    let plot_points: PlotPoints = points.iter().copied().collect();
                    let palette = if color_blind_mode {
                        COLORBLIND_COLORS
                    } else {
                        CHART_COLORS
                    };
                    let color = palette[selected.color_index % palette.len()];

                    // Use legend name with value if available
                    let name = &legend_names[i];

                    plot_ui.line(
                        Line::new(plot_points)
                            .name(name)
                            .color(egui::Color32::from_rgb(color[0], color[1], color[2]))
                            .width(1.5),
                    );
                }
            }

            // Draw vertical cursor line
            if let Some(time) = cursor_time {
                plot_ui.vline(
                    VLine::new(time)
                        .color(egui::Color32::from_rgb(0, 255, 255)) // Cyan cursor
                        .width(2.0)
                        .name("Cursor"),
                );
            }

            // Return pointer position if hovering for click detection
            plot_ui.pointer_coordinate()
        });

        // Detect user interaction with chart (drag, zoom, scroll)
        // This marks the chart as "interacted" so we stop using the initial zoomed view
        if response.response.dragged()
            || response.response.drag_started()
            || ui.input(|i| i.zoom_delta() != 1.0)
            || ui.input(|i| i.smooth_scroll_delta.x != 0.0)
        {
            self.chart_interacted = true;
        }

        // Handle click on chart to set cursor position
        if response.response.clicked() {
            if let Some(pos) = response.inner {
                let clicked_time = pos.x;
                // Clamp to time range
                if let Some((min, max)) = self.time_range {
                    // Stop playback when user clicks on chart
                    self.is_playing = false;
                    self.last_frame_time = None;

                    let clamped_time = clicked_time.clamp(min, max);
                    self.cursor_time = Some(clamped_time);
                    self.cursor_record = self.find_record_at_time(clamped_time);
                    // Force repaint to update legend values immediately
                    ui.ctx().request_repaint();
                }
            }
        }
    }

    /// Render the timeline scrubber bar
    fn render_timeline_scrubber(&mut self, ui: &mut egui::Ui) {
        let Some((min_time, max_time)) = self.time_range else {
            return;
        };

        let total_duration = max_time - min_time;
        if total_duration <= 0.0 {
            return;
        }

        // Time labels row
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(Self::format_time(min_time))
                    .small()
                    .color(egui::Color32::LIGHT_GRAY),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(Self::format_time(max_time))
                        .small()
                        .color(egui::Color32::LIGHT_GRAY),
                );
            });
        });

        // Full-width slider - set slider_width to use available space
        let current_time = self.cursor_time.unwrap_or(min_time);
        let mut slider_value = current_time;
        let available_width = ui.available_width();

        // Temporarily set slider width to fill available space
        let old_slider_width = ui.spacing().slider_width;
        ui.spacing_mut().slider_width = available_width - 10.0; // Small margin for aesthetics

        let slider = egui::Slider::new(&mut slider_value, min_time..=max_time)
            .show_value(false)
            .clamping(egui::SliderClamping::Always);

        let slider_response = ui.add(slider);

        // Restore original slider width
        ui.spacing_mut().slider_width = old_slider_width;

        if slider_response.changed() {
            // Stop playback when user manually scrubs
            self.is_playing = false;
            self.last_frame_time = None;

            self.cursor_time = Some(slider_value);
            self.cursor_record = self.find_record_at_time(slider_value);
            // Force repaint to update legend values
            ui.ctx().request_repaint();
        }
    }

    /// Render the record/time indicator bar with playback controls
    fn render_record_indicator(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            // Playback controls
            let button_size = egui::vec2(28.0, 28.0);

            // Play/Pause button
            let play_text = if self.is_playing { "⏸" } else { "▶" };
            let play_button = egui::Button::new(egui::RichText::new(play_text).size(16.0).color(
                if self.is_playing {
                    egui::Color32::from_rgb(253, 193, 73) // Amber when playing
                } else {
                    egui::Color32::from_rgb(144, 238, 144) // Light green when paused
                },
            ))
            .min_size(button_size);

            if ui.add(play_button).clicked() {
                self.is_playing = !self.is_playing;
                if self.is_playing {
                    // Reset frame time when starting playback
                    self.last_frame_time = Some(std::time::Instant::now());
                    // Initialize cursor if not set
                    if self.cursor_time.is_none() {
                        if let Some((min, _)) = self.time_range {
                            self.cursor_time = Some(min);
                            self.cursor_record = self.find_record_at_time(min);
                        }
                    }
                }
            }

            // Stop button (resets to beginning)
            let stop_button = egui::Button::new(
                egui::RichText::new("⏹")
                    .size(16.0)
                    .color(egui::Color32::from_rgb(191, 78, 48)), // Rust orange
            )
            .min_size(button_size);

            if ui.add(stop_button).clicked() {
                self.is_playing = false;
                self.last_frame_time = None;
                // Reset cursor to beginning
                if let Some((min, _)) = self.time_range {
                    self.cursor_time = Some(min);
                    self.cursor_record = self.find_record_at_time(min);
                }
            }

            ui.separator();

            // Playback speed selector
            ui.label(
                egui::RichText::new("Speed:")
                    .small()
                    .color(egui::Color32::GRAY),
            );

            let speed_options = [0.25, 0.5, 1.0, 2.0, 4.0, 8.0];
            egui::ComboBox::from_id_salt("playback_speed")
                .selected_text(format!("{}x", self.playback_speed))
                .width(60.0)
                .show_ui(ui, |ui| {
                    for speed in speed_options {
                        ui.selectable_value(&mut self.playback_speed, speed, format!("{}x", speed));
                    }
                });

            ui.separator();

            // Current time display
            if let Some(time) = self.cursor_time {
                ui.label(
                    egui::RichText::new(format!("Time: {}", Self::format_time(time)))
                        .strong()
                        .color(egui::Color32::from_rgb(0, 255, 255)), // Cyan to match cursor
                );
            }

            ui.separator();

            // Record indicator
            if let Some(record) = self.cursor_record {
                if let Some(file) = self.files.first() {
                    let total_records = file.log.data.len();
                    ui.label(
                        egui::RichText::new(format!("Record {} of {}", record + 1, total_records))
                            .color(egui::Color32::LIGHT_GRAY),
                    );
                }
            }
        });
    }

    /// Update playback state - advances cursor based on elapsed time
    fn update_playback(&mut self, ctx: &egui::Context) {
        if !self.is_playing {
            return;
        }

        let Some((min_time, max_time)) = self.time_range else {
            self.is_playing = false;
            return;
        };

        let now = std::time::Instant::now();
        let delta = if let Some(last) = self.last_frame_time {
            now.duration_since(last).as_secs_f64()
        } else {
            0.0
        };
        self.last_frame_time = Some(now);

        // Advance cursor by delta * playback_speed
        if let Some(current_time) = self.cursor_time {
            let new_time = current_time + (delta * self.playback_speed);

            if new_time >= max_time {
                // Reached end - stop playback
                self.cursor_time = Some(max_time);
                self.cursor_record = self.find_record_at_time(max_time);
                self.is_playing = false;
                self.last_frame_time = None;
            } else {
                self.cursor_time = Some(new_time);
                self.cursor_record = self.find_record_at_time(new_time);
            }
        } else {
            // No cursor set, start from beginning
            self.cursor_time = Some(min_time);
            self.cursor_record = self.find_record_at_time(min_time);
        }

        // Request continuous repaint during playback
        ctx.request_repaint();
    }

    /// Draw an upload icon (circle with arrow pointing up)
    fn draw_upload_icon(ui: &mut egui::Ui, center: egui::Pos2, size: f32, color: egui::Color32) {
        let painter = ui.painter();
        let radius = size / 2.0;

        // Draw circle outline
        painter.circle_stroke(center, radius, egui::Stroke::new(2.0, color));

        // Draw arrow pointing up
        let arrow_size = size * 0.35;
        let arrow_top = egui::pos2(center.x, center.y - arrow_size * 0.6);
        let arrow_bottom = egui::pos2(center.x, center.y + arrow_size * 0.4);

        // Arrow shaft
        painter.line_segment([arrow_bottom, arrow_top], egui::Stroke::new(2.0, color));

        // Arrow head
        let head_size = arrow_size * 0.4;
        painter.line_segment(
            [
                arrow_top,
                egui::pos2(arrow_top.x - head_size, arrow_top.y + head_size),
            ],
            egui::Stroke::new(2.0, color),
        );
        painter.line_segment(
            [
                arrow_top,
                egui::pos2(arrow_top.x + head_size, arrow_top.y + head_size),
            ],
            egui::Stroke::new(2.0, color),
        );
    }

    /// Render toast notifications
    fn render_toast(&mut self, ctx: &egui::Context) {
        if let Some((message, time)) = &self.toast_message {
            if time.elapsed().as_secs() < 3 {
                egui::Area::new(egui::Id::new("toast"))
                    .fixed_pos(egui::pos2(10.0, 10.0))
                    .show(ctx, |ui| {
                        egui::Frame::none()
                            .fill(egui::Color32::from_rgb(191, 78, 48))
                            .rounding(5.0)
                            .inner_margin(10.0)
                            .show(ui, |ui| {
                                ui.label(egui::RichText::new(message).color(egui::Color32::WHITE));
                            });
                    });
            } else {
                self.toast_message = None;
            }
        }
    }

    /// Handle file drops
    fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        // Don't accept drops while loading
        if matches!(self.loading_state, LoadingState::Loading(_)) {
            return;
        }

        // Debounce file drops (5 second window)
        if let Some(last_drop) = self.last_drop_time {
            if last_drop.elapsed().as_secs() < 5 {
                return;
            }
        }

        let dropped_files: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect()
        });

        if !dropped_files.is_empty() {
            self.last_drop_time = Some(std::time::Instant::now());

            // Only load first file for now (could queue multiple)
            if let Some(path) = dropped_files.into_iter().next() {
                self.start_loading_file(path);
            }
        }
    }
}

impl eframe::App for UltraLogApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Check for completed background loads
        self.check_loading_complete();

        // Handle file drops
        self.handle_dropped_files(ctx);

        // Update playback (advances cursor if playing)
        self.update_playback(ctx);

        // Apply dark theme
        ctx.set_visuals(egui::Visuals::dark());

        // Request repaint while loading (for spinner animation)
        if matches!(self.loading_state, LoadingState::Loading(_)) {
            ctx.request_repaint();
        }

        // Toast notifications
        self.render_toast(ctx);

        // Left sidebar panel
        egui::SidePanel::left("files_panel")
            .default_width(200.0)
            .resizable(true)
            .show(ctx, |ui| {
                self.render_sidebar(ui);
            });

        // Right panel for channel selection
        egui::SidePanel::right("channels_panel")
            .default_width(300.0)
            .min_width(200.0)
            .resizable(true)
            .show(ctx, |ui| {
                self.render_channel_selection(ui);
            });

        // Bottom panel for timeline scrubber (render before central to claim space)
        if self.time_range.is_some() && !self.selected_channels.is_empty() {
            egui::TopBottomPanel::bottom("timeline_panel")
                .resizable(false)
                .min_height(60.0)
                .show(ctx, |ui| {
                    ui.add_space(5.0);
                    self.render_record_indicator(ui);
                    ui.separator();
                    self.render_timeline_scrubber(ui);
                    ui.add_space(5.0);
                });
        }

        // Main content area
        egui::CentralPanel::default().show(ctx, |ui| {
            // Selected channels at top
            ui.add_space(10.0);
            self.render_selected_channels(ui);

            ui.add_space(10.0);
            ui.separator();

            // Chart takes remaining space
            self.render_chart(ui);
        });
    }
}
