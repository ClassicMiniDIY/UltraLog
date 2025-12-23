//! Main application module for UltraLog.
//!
//! This module contains the core application state and the main eframe::App
//! implementation. UI rendering is delegated to the `ui` submodules.

use eframe::egui;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

use crate::parsers::{EcuMaster, EcuType, Haltech, Parseable, Speeduino};
use crate::state::{
    CacheKey, LoadResult, LoadedFile, LoadingState, SelectedChannel, CHART_COLORS,
    COLORBLIND_COLORS, MAX_CHANNELS,
};
use crate::units::UnitPreferences;

// ============================================================================
// Main Application State
// ============================================================================

/// Main application state for UltraLog
pub struct UltraLogApp {
    /// List of loaded log files
    pub(crate) files: Vec<LoadedFile>,
    /// Currently selected file index
    pub(crate) selected_file: Option<usize>,
    /// Channels selected for visualization
    pub(crate) selected_channels: Vec<SelectedChannel>,
    /// Channel search/filter text
    pub(crate) channel_search: String,
    /// Toast messages for user feedback
    pub(crate) toast_message: Option<(String, std::time::Instant)>,
    /// Track dropped files to prevent duplicates
    last_drop_time: Option<std::time::Instant>,
    /// Channel for receiving loaded files from background thread
    load_receiver: Option<Receiver<LoadResult>>,
    /// Current loading state
    pub(crate) loading_state: LoadingState,
    /// Cache for downsampled chart data
    pub(crate) downsample_cache: HashMap<CacheKey, Vec<[f64; 2]>>,
    /// Current cursor position in seconds (timeline feature)
    pub(crate) cursor_time: Option<f64>,
    /// Total time range across all loaded files (min, max)
    pub(crate) time_range: Option<(f64, f64)>,
    /// Current data record index at cursor position
    pub(crate) cursor_record: Option<usize>,
    // === View Options ===
    /// When true, keep cursor centered and pan graph during scrubbing
    pub(crate) cursor_tracking: bool,
    /// Visible time window width in seconds (for cursor tracking mode)
    pub(crate) view_window_seconds: f64,
    // === Playback ===
    /// Whether playback is active
    pub(crate) is_playing: bool,
    /// Last frame time for calculating delta
    pub(crate) last_frame_time: Option<std::time::Instant>,
    /// Playback speed multiplier (1.0 = real-time)
    pub(crate) playback_speed: f64,
    // === Accessibility ===
    /// When true, use colorblind-friendly color palette
    pub(crate) color_blind_mode: bool,
    // === Chart View State ===
    /// Whether user has interacted with chart zoom/pan (false = use initial zoomed view)
    pub(crate) chart_interacted: bool,
    /// Initial view window in seconds (shown before user interacts with chart)
    pub(crate) initial_view_seconds: f64,
    // === Unit Preferences ===
    /// User preferences for display units
    pub(crate) unit_preferences: UnitPreferences,
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
            unit_preferences: UnitPreferences::default(),
        }
    }
}

impl UltraLogApp {
    /// Create a new UltraLogApp instance with custom fonts
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

    // ========================================================================
    // Color and Unit Helpers
    // ========================================================================

    /// Get color for a channel based on color blind mode setting
    pub fn get_channel_color(&self, color_index: usize) -> [u8; 3] {
        let palette = if self.color_blind_mode {
            COLORBLIND_COLORS
        } else {
            CHART_COLORS
        };
        palette[color_index % palette.len()]
    }

    // ========================================================================
    // File Loading
    // ========================================================================

    /// Start loading a file in the background
    pub fn start_loading_file(&mut self, path: PathBuf) {
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
        // First try reading as binary to detect Speeduino/rusEFI MLG format
        let binary_data = match fs::read(&path) {
            Ok(d) => d,
            Err(e) => return LoadResult::Error(format!("Failed to read file: {}", e)),
        };

        // Check for Haltech HEPS format (.hlgzip) - proprietary compressed format
        if binary_data.len() >= 4 && &binary_data[0..4] == b"HEPS" {
            return LoadResult::Error(
                "This is a Haltech .hlgzip file which uses proprietary compression.\n\n\
                To use this log in UltraLog, please export it as CSV from Haltech's ESP or NSP software:\n\
                1. Open the .hlgzip file in Haltech ESP/NSP\n\
                2. Go to File → Export → CSV\n\
                3. Load the exported .csv file in UltraLog".to_string()
            );
        }

        // Check for AEM .daq format - proprietary format (starts with "EMERALD")
        if binary_data.len() >= 7 && &binary_data[0..7] == b"EMERALD" {
            return LoadResult::Error(
                "This is an AEM .daq file which uses a proprietary format.\n\n\
                To use this log in UltraLog, please export it as CSV from AEM's software:\n\
                1. Open the .daq file in AEMdata or AEM Pro\n\
                2. Go to File → Export → CSV\n\
                3. Load the exported .csv file in UltraLog"
                    .to_string(),
            );
        }

        // Check for Link .llg format - proprietary format (check by extension since header varies)
        if let Some(ext) = path.extension() {
            if ext.to_string_lossy().to_lowercase() == "llg" {
                return LoadResult::Error(
                    "This is a Link .llg file which uses a proprietary format.\n\n\
                    To use this log in UltraLog, please export it as CSV from Link's software:\n\
                    1. Open the .llg file in PCLink or G4+ software\n\
                    2. Go to File → Export → CSV\n\
                    3. Load the exported .csv file in UltraLog"
                        .to_string(),
                );
            }
        }

        // Auto-detect file format and parse
        let (log, ecu_type) = if Speeduino::detect(&binary_data) {
            // Speeduino/rusEFI MLG format detected (binary)
            match Speeduino::parse_binary(&binary_data) {
                Ok(l) => (l, EcuType::Speeduino),
                Err(e) => {
                    return LoadResult::Error(format!(
                        "Failed to parse Speeduino/rusEFI MLG file: {}",
                        e
                    ))
                }
            }
        } else {
            // Try parsing as text-based formats
            let contents = match String::from_utf8(binary_data) {
                Ok(c) => c,
                Err(_) => return LoadResult::Error("File is not valid UTF-8 text".into()),
            };

            if EcuMaster::detect(&contents) {
                // ECUMaster format detected
                let parser = EcuMaster;
                match parser.parse(&contents) {
                    Ok(l) => (l, EcuType::EcuMaster),
                    Err(e) => {
                        return LoadResult::Error(format!("Failed to parse ECUMaster file: {}", e))
                    }
                }
            } else {
                // Default to Haltech format
                let parser = Haltech;
                match parser.parse(&contents) {
                    Ok(l) => (l, EcuType::Haltech),
                    Err(e) => return LoadResult::Error(format!("Failed to parse file: {}", e)),
                }
            }
        };

        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "Unknown".to_string());

        LoadResult::Success(Box::new(LoadedFile {
            path,
            name,
            ecu_type,
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

    // ========================================================================
    // Time Range and Cursor
    // ========================================================================

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
    pub fn find_record_at_time(&self, time: f64) -> Option<usize> {
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
    pub fn get_value_at_record(
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

    /// Get min and max values for a channel across all records
    pub fn get_channel_min_max(
        &self,
        file_index: usize,
        channel_index: usize,
    ) -> Option<(f64, f64)> {
        if file_index >= self.files.len() {
            return None;
        }

        let file = &self.files[file_index];
        let data = file.log.get_channel_data(channel_index);

        if data.is_empty() {
            return None;
        }

        let mut min_val = f64::MAX;
        let mut max_val = f64::MIN;

        for &value in &data {
            min_val = min_val.min(value);
            max_val = max_val.max(value);
        }

        Some((min_val, max_val))
    }

    // ========================================================================
    // File and Channel Management
    // ========================================================================

    /// Remove a loaded file
    pub fn remove_file(&mut self, index: usize) {
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
    pub fn add_channel(&mut self, file_index: usize, channel_index: usize) {
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

        eprintln!(
            "DEBUG: Adding channel - file={} channel={} name={} (total selected: {} -> {})",
            file_index,
            channel_index,
            channel.name(),
            self.selected_channels.len(),
            self.selected_channels.len() + 1
        );

        self.selected_channels.push(SelectedChannel {
            file_index,
            channel_index,
            channel,
            color_index,
        });
    }

    /// Remove a channel from the selection
    pub fn remove_channel(&mut self, index: usize) {
        if index < self.selected_channels.len() {
            self.selected_channels.remove(index);
        }
    }

    // ========================================================================
    // Toast Notifications
    // ========================================================================

    /// Show a toast message
    pub fn show_toast(&mut self, message: &str) {
        self.toast_message = Some((message.to_string(), std::time::Instant::now()));
    }

    // ========================================================================
    // Drag and Drop
    // ========================================================================

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

    // ========================================================================
    // Keyboard Shortcuts
    // ========================================================================

    /// Handle keyboard shortcuts
    fn handle_keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        // Only handle shortcuts when we have data loaded
        if self.files.is_empty() || self.selected_channels.is_empty() {
            return;
        }

        // Spacebar to toggle play/pause
        ctx.input(|i| {
            if i.key_pressed(egui::Key::Space) {
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
        });
    }
}

// ============================================================================
// eframe::App Implementation
// ============================================================================

impl eframe::App for UltraLogApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Check for completed background loads
        self.check_loading_complete();

        // Handle file drops
        self.handle_dropped_files(ctx);

        // Update playback (advances cursor if playing)
        self.update_playback(ctx);

        // Handle keyboard shortcuts
        self.handle_keyboard_shortcuts(ctx);

        // Apply dark theme
        ctx.set_visuals(egui::Visuals::dark());

        // Request repaint while loading (for spinner animation)
        if matches!(self.loading_state, LoadingState::Loading(_)) {
            ctx.request_repaint();
        }

        // Toast notifications
        self.render_toast(ctx);

        // Menu bar at top with padding
        let menu_frame = egui::Frame::none().inner_margin(egui::Margin {
            left: 10.0,
            right: 10.0,
            top: 8.0,
            bottom: 8.0,
        });

        egui::TopBottomPanel::top("menu_bar")
            .frame(menu_frame)
            .show(ctx, |ui| {
                self.render_menu_bar(ui);
            });

        // Panel background color (matches drop zone card)
        let panel_bg = egui::Color32::from_rgb(45, 45, 45);
        let panel_frame = egui::Frame::none()
            .fill(panel_bg)
            .inner_margin(egui::Margin::symmetric(10.0, 10.0));

        // Left sidebar panel
        egui::SidePanel::left("files_panel")
            .default_width(200.0)
            .resizable(true)
            .frame(panel_frame)
            .show(ctx, |ui| {
                self.render_sidebar(ui);
            });

        // Right panel for channel selection
        egui::SidePanel::right("channels_panel")
            .default_width(300.0)
            .min_width(200.0)
            .resizable(true)
            .frame(panel_frame)
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
