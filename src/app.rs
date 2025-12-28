//! Main application module for UltraLog.
//!
//! This module contains the core application state and the main eframe::App
//! implementation. UI rendering is delegated to the `ui` submodules.

use eframe::egui;
use memmap2::Mmap;
use std::collections::HashMap;
use std::fs::{self, File};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

use crate::parsers::{EcuMaster, EcuType, Haltech, Parseable, RomRaider, Speeduino};
use crate::state::{
    ActiveTool, CacheKey, LoadResult, LoadedFile, LoadingState, ScatterPlotConfig,
    ScatterPlotState, SelectedChannel, Tab, ToastType, CHART_COLORS, COLORBLIND_COLORS,
    MAX_CHANNELS,
};
use crate::units::UnitPreferences;
use crate::updater::{DownloadResult, UpdateCheckResult, UpdateState};

// ============================================================================
// Main Application State
// ============================================================================

/// Main application state for UltraLog
pub struct UltraLogApp {
    /// List of loaded log files
    pub(crate) files: Vec<LoadedFile>,
    /// Currently selected file index (matches active tab's file)
    pub(crate) selected_file: Option<usize>,
    /// Toast messages for user feedback (message, time, type)
    pub(crate) toast_message: Option<(String, std::time::Instant, ToastType)>,
    /// Track dropped files to prevent duplicates
    last_drop_time: Option<std::time::Instant>,
    /// Channel for receiving loaded files from background thread
    load_receiver: Option<Receiver<LoadResult>>,
    /// Current loading state
    pub(crate) loading_state: LoadingState,
    /// Cache for downsampled chart data
    pub(crate) downsample_cache: HashMap<CacheKey, Vec<[f64; 2]>>,
    /// Cache for channel min/max values (avoids O(n) scans)
    pub(crate) minmax_cache: HashMap<CacheKey, (f64, f64)>,
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
    /// When true, normalize field names to standard names
    pub(crate) field_normalization: bool,
    // === Chart View State ===
    /// Initial view window in seconds (shown before user interacts with chart)
    pub(crate) initial_view_seconds: f64,
    // === Unit Preferences ===
    /// User preferences for display units
    pub(crate) unit_preferences: UnitPreferences,
    // === Custom Field Normalization ===
    /// Custom user-defined field name mappings (source name -> normalized name)
    pub(crate) custom_normalizations: HashMap<String, String>,
    /// Whether to show the normalization editor window
    pub(crate) show_normalization_editor: bool,
    /// Input field for source name in "Extend Built-in" section
    pub(crate) norm_editor_extend_source: String,
    /// Selected built-in target in the extend dropdown
    pub(crate) norm_editor_selected_target: Option<String>,
    /// Input field for source name in "Create New Mapping" section
    pub(crate) norm_editor_custom_source: String,
    /// Input field for new normalized name in "Create New Mapping" section
    pub(crate) norm_editor_custom_target: String,
    // === Tool/View Selection ===
    /// Currently active tool/view
    pub(crate) active_tool: ActiveTool,
    // === Tab Management ===
    /// Open tabs (one per log file being viewed)
    pub(crate) tabs: Vec<Tab>,
    /// Index of the currently active tab
    pub(crate) active_tab: Option<usize>,
    // === Auto-Update ===
    /// Current state of the update checker
    pub(crate) update_state: UpdateState,
    /// Receiver for update check results from background thread
    update_check_receiver: Option<Receiver<UpdateCheckResult>>,
    /// Receiver for download results from background thread
    update_download_receiver: Option<Receiver<DownloadResult>>,
    /// Whether to show the update available dialog
    pub(crate) show_update_dialog: bool,
    /// User preference: check for updates on startup
    pub(crate) auto_check_updates: bool,
    /// Whether the startup check has been performed
    startup_check_done: bool,
    /// Set to true when the app should exit for an update to be applied
    pub(crate) should_exit_for_update: bool,
}

impl Default for UltraLogApp {
    fn default() -> Self {
        Self {
            files: Vec::new(),
            selected_file: None,
            toast_message: None,
            last_drop_time: None,
            load_receiver: None,
            loading_state: LoadingState::Idle,
            downsample_cache: HashMap::new(),
            minmax_cache: HashMap::new(),
            cursor_time: None,
            time_range: None,
            cursor_record: None,
            cursor_tracking: false,
            view_window_seconds: 30.0, // Default 30 second window
            is_playing: false,
            last_frame_time: None,
            playback_speed: 1.0,
            color_blind_mode: false,
            field_normalization: true, // Enabled by default for better readability
            initial_view_seconds: 60.0, // Start with 60 second view
            unit_preferences: UnitPreferences::default(),
            custom_normalizations: HashMap::new(),
            show_normalization_editor: false,
            norm_editor_extend_source: String::new(),
            norm_editor_selected_target: None,
            norm_editor_custom_source: String::new(),
            norm_editor_custom_target: String::new(),
            active_tool: ActiveTool::default(),
            tabs: Vec::new(),
            active_tab: None,
            update_state: UpdateState::default(),
            update_check_receiver: None,
            update_download_receiver: None,
            show_update_dialog: false,
            auto_check_updates: true, // Enabled by default
            startup_check_done: false,
            should_exit_for_update: false,
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
            egui::FontData::from_static(include_bytes!("../assets/Outfit-Regular.ttf")).into(),
        );

        // Load Outfit Bold
        fonts.font_data.insert(
            "Outfit-Bold".to_owned(),
            egui::FontData::from_static(include_bytes!("../assets/Outfit-Bold.ttf")).into(),
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
            self.show_toast_warning("File already loaded");
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
    /// Uses memory-mapped files for large files (>10MB) for better performance.
    fn load_file_sync(path: PathBuf) -> LoadResult {
        // Use memory mapping for large files (>10MB) to reduce memory pressure
        const MMAP_THRESHOLD: u64 = 10 * 1024 * 1024;

        // Get file metadata to check size
        let file_size = match fs::metadata(&path) {
            Ok(meta) => meta.len(),
            Err(e) => return LoadResult::Error(format!("Failed to read file metadata: {}", e)),
        };

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

        // Load file data - use mmap for large files, regular read for small files
        let (log, ecu_type) = if file_size > MMAP_THRESHOLD {
            // Use memory-mapped file for large files
            match Self::load_with_mmap(&path) {
                Ok(result) => result,
                Err(e) => return e,
            }
        } else {
            // Use regular file read for small files
            match Self::load_with_read(&path) {
                Ok(result) => result,
                Err(e) => return e,
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

    /// Load file using memory-mapped I/O for better performance with large files
    fn load_with_mmap(path: &PathBuf) -> Result<(crate::parsers::Log, EcuType), LoadResult> {
        let file = match File::open(path) {
            Ok(f) => f,
            Err(e) => return Err(LoadResult::Error(format!("Failed to open file: {}", e))),
        };

        // SAFETY: The file is opened read-only and we don't modify it.
        // The mapping is dropped after parsing completes.
        let mmap = match unsafe { Mmap::map(&file) } {
            Ok(m) => m,
            Err(e) => {
                return Err(LoadResult::Error(format!(
                    "Failed to memory-map file: {}",
                    e
                )))
            }
        };

        Self::parse_binary_data(&mmap, path)
    }

    /// Load file using regular file read (for smaller files)
    fn load_with_read(path: &PathBuf) -> Result<(crate::parsers::Log, EcuType), LoadResult> {
        let binary_data = match fs::read(path) {
            Ok(d) => d,
            Err(e) => return Err(LoadResult::Error(format!("Failed to read file: {}", e))),
        };

        Self::parse_binary_data(&binary_data, path)
    }

    /// Parse binary data and detect file format
    fn parse_binary_data(
        binary_data: &[u8],
        path: &PathBuf,
    ) -> Result<(crate::parsers::Log, EcuType), LoadResult> {
        // Check for Haltech HEPS format (.hlgzip) - proprietary compressed format
        if binary_data.len() >= 4 && &binary_data[0..4] == b"HEPS" {
            return Err(LoadResult::Error(
                "This is a Haltech .hlgzip file which uses proprietary compression.\n\n\
                To use this log in UltraLog, please export it as CSV from Haltech's ESP or NSP software:\n\
                1. Open the .hlgzip file in Haltech ESP/NSP\n\
                2. Go to File → Export → CSV\n\
                3. Load the exported .csv file in UltraLog"
                    .to_string(),
            ));
        }

        // Check for AEM .daq format - proprietary format (starts with "EMERALD")
        if binary_data.len() >= 7 && &binary_data[0..7] == b"EMERALD" {
            return Err(LoadResult::Error(
                "This is an AEM .daq file which uses a proprietary format.\n\n\
                To use this log in UltraLog, please export it as CSV from AEM's software:\n\
                1. Open the .daq file in AEMdata or AEM Pro\n\
                2. Go to File → Export → CSV\n\
                3. Load the exported .csv file in UltraLog"
                    .to_string(),
            ));
        }

        // Auto-detect file format and parse
        if Speeduino::detect(binary_data) {
            // Speeduino/rusEFI MLG format detected (binary)
            match Speeduino::parse_binary(binary_data) {
                Ok(l) => Ok((l, EcuType::Speeduino)),
                Err(e) => Err(LoadResult::Error(format!(
                    "Failed to parse Speeduino/rusEFI MLG file: {}",
                    e
                ))),
            }
        } else {
            // Try parsing as text-based formats
            // For mmap, we use from_utf8 which doesn't copy the data
            let contents = match std::str::from_utf8(binary_data) {
                Ok(c) => c,
                Err(_) => {
                    // Fall back to lossy conversion for files with encoding issues
                    return Self::parse_text_lossy(binary_data, path);
                }
            };

            Self::parse_text_content(contents)
        }
    }

    /// Parse text content after UTF-8 validation
    fn parse_text_content(contents: &str) -> Result<(crate::parsers::Log, EcuType), LoadResult> {
        if EcuMaster::detect(contents) {
            // ECUMaster format detected
            let parser = EcuMaster;
            match parser.parse(contents) {
                Ok(l) => Ok((l, EcuType::EcuMaster)),
                Err(e) => Err(LoadResult::Error(format!(
                    "Failed to parse ECUMaster file: {}",
                    e
                ))),
            }
        } else if RomRaider::detect(contents) {
            // RomRaider format detected
            let parser = RomRaider;
            match parser.parse(contents) {
                Ok(l) => Ok((l, EcuType::RomRaider)),
                Err(e) => Err(LoadResult::Error(format!(
                    "Failed to parse RomRaider file: {}",
                    e
                ))),
            }
        } else {
            // Default to Haltech format
            let parser = Haltech;
            match parser.parse(contents) {
                Ok(l) => Ok((l, EcuType::Haltech)),
                Err(e) => Err(LoadResult::Error(format!("Failed to parse file: {}", e))),
            }
        }
    }

    /// Parse text with lossy UTF-8 conversion for files with encoding issues
    fn parse_text_lossy(
        binary_data: &[u8],
        _path: &PathBuf,
    ) -> Result<(crate::parsers::Log, EcuType), LoadResult> {
        let contents = String::from_utf8_lossy(binary_data);
        Self::parse_text_content(&contents)
    }

    /// Check for completed background loads
    fn check_loading_complete(&mut self) {
        if let Some(receiver) = &self.load_receiver {
            if let Ok(result) = receiver.try_recv() {
                match result {
                    LoadResult::Success(file) => {
                        let file_index = self.files.len();
                        let file_name = file.name.clone();

                        // Compute time range for this file
                        let times = file.log.get_times_as_f64();
                        let file_time_range =
                            if let (Some(&first), Some(&last)) = (times.first(), times.last()) {
                                Some((first, last))
                            } else {
                                None
                            };

                        self.files.push(*file);
                        self.selected_file = Some(file_index);
                        self.update_time_range();

                        // Create a new tab for this file with its time range
                        let mut tab = Tab::new(file_index, file_name);
                        tab.time_range = file_time_range;
                        // Initialize cursor to start of file
                        if let Some((min_time, _)) = file_time_range {
                            tab.cursor_time = Some(min_time);
                            tab.cursor_record = Some(0);
                        }
                        self.tabs.push(tab);
                        self.active_tab = Some(self.tabs.len() - 1);

                        self.show_toast_success("File loaded successfully");
                    }
                    LoadResult::Error(e) => {
                        self.show_toast_error(&format!("Error: {}", e));
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

    /// Get min and max values for a channel across all records (cached)
    pub fn get_channel_min_max(
        &mut self,
        file_index: usize,
        channel_index: usize,
    ) -> Option<(f64, f64)> {
        if file_index >= self.files.len() {
            return None;
        }

        // Check cache first
        let cache_key = CacheKey {
            file_index,
            channel_index,
        };
        if let Some(&cached) = self.minmax_cache.get(&cache_key) {
            return Some(cached);
        }

        // Compute min/max
        let file = &self.files[file_index];
        let data = file.log.get_channel_data(channel_index);

        if data.is_empty() {
            return None;
        }

        let (min_val, max_val) = data.iter().fold((f64::MAX, f64::MIN), |(min, max), &v| {
            (min.min(v), max.max(v))
        });

        // Cache the result
        self.minmax_cache.insert(cache_key, (min_val, max_val));
        Some((min_val, max_val))
    }

    // ========================================================================
    // File and Channel Management
    // ========================================================================

    /// Remove a loaded file
    pub fn remove_file(&mut self, index: usize) {
        if index < self.files.len() {
            // Find and close the tab for this file
            if let Some(tab_idx) = self.tabs.iter().position(|t| t.file_index == index) {
                self.close_tab(tab_idx);
            }

            // Clear downsample cache entries for this file and update indices
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

            // Clear minmax cache entries for this file and update indices
            let mut new_minmax_cache = HashMap::new();
            for (key, value) in self.minmax_cache.drain() {
                if key.file_index == index {
                    continue;
                } else if key.file_index > index {
                    new_minmax_cache.insert(
                        CacheKey {
                            file_index: key.file_index - 1,
                            channel_index: key.channel_index,
                        },
                        value,
                    );
                } else {
                    new_minmax_cache.insert(key, value);
                }
            }
            self.minmax_cache = new_minmax_cache;

            // Update file indices for remaining tabs and their channels
            for tab in &mut self.tabs {
                if tab.file_index > index {
                    tab.file_index -= 1;
                    // Update file indices in selected channels
                    for channel in &mut tab.selected_channels {
                        if channel.file_index > index {
                            channel.file_index -= 1;
                        }
                    }
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

    /// Add a channel to the active tab's selection
    pub fn add_channel(&mut self, file_index: usize, channel_index: usize) {
        let Some(tab_idx) = self.active_tab else {
            self.show_toast_warning("No active tab");
            return;
        };

        let tab = &self.tabs[tab_idx];

        // Only allow adding channels from the tab's file
        if file_index != tab.file_index {
            self.show_toast_warning("Channel must be from the active tab's file");
            return;
        }

        if tab.selected_channels.len() >= MAX_CHANNELS {
            self.show_toast_warning("Maximum 10 channels reached");
            return;
        }

        // Check for duplicate
        if tab
            .selected_channels
            .iter()
            .any(|c| c.file_index == file_index && c.channel_index == channel_index)
        {
            self.show_toast_warning("Channel already selected");
            return;
        }

        let file = &self.files[file_index];
        let channel = file.log.channels[channel_index].clone();

        // Find the first unused color index
        let used_colors: std::collections::HashSet<usize> = tab
            .selected_channels
            .iter()
            .map(|c| c.color_index)
            .collect();

        let color_index = (0..CHART_COLORS.len())
            .find(|i| !used_colors.contains(i))
            .unwrap_or(0);

        self.tabs[tab_idx].selected_channels.push(SelectedChannel {
            file_index,
            channel_index,
            channel,
            color_index,
        });
    }

    /// Remove a channel from the active tab's selection
    pub fn remove_channel(&mut self, index: usize) {
        let Some(tab_idx) = self.active_tab else {
            return;
        };

        if index < self.tabs[tab_idx].selected_channels.len() {
            self.tabs[tab_idx].selected_channels.remove(index);
        }
    }

    /// Get the selected channels for the active tab
    pub fn get_selected_channels(&self) -> &[SelectedChannel] {
        if let Some(tab_idx) = self.active_tab {
            &self.tabs[tab_idx].selected_channels
        } else {
            &[]
        }
    }

    /// Get the channel search string for the active tab
    pub fn get_channel_search(&self) -> &str {
        if let Some(tab_idx) = self.active_tab {
            &self.tabs[tab_idx].channel_search
        } else {
            ""
        }
    }

    /// Set the channel search string for the active tab
    pub fn set_channel_search(&mut self, search: String) {
        if let Some(tab_idx) = self.active_tab {
            self.tabs[tab_idx].channel_search = search;
        }
    }

    /// Switch to a tab for the given file, creating one if it doesn't exist
    pub fn switch_to_file_tab(&mut self, file_index: usize) {
        // Check if a tab already exists for this file
        if let Some(tab_idx) = self.tabs.iter().position(|t| t.file_index == file_index) {
            self.active_tab = Some(tab_idx);
            self.selected_file = Some(file_index);
        } else {
            // Create a new tab for this file
            let file_name = self.files[file_index].name.clone();
            let tab = Tab::new(file_index, file_name);
            self.tabs.push(tab);
            self.active_tab = Some(self.tabs.len() - 1);
            self.selected_file = Some(file_index);
        }
    }

    /// Close a tab by index
    pub fn close_tab(&mut self, tab_index: usize) {
        if tab_index >= self.tabs.len() {
            return;
        }

        self.tabs.remove(tab_index);

        // Adjust active_tab if needed
        if self.tabs.is_empty() {
            self.active_tab = None;
            self.selected_file = None;
        } else if let Some(active) = self.active_tab {
            if active >= self.tabs.len() {
                self.active_tab = Some(self.tabs.len() - 1);
            } else if active > tab_index {
                self.active_tab = Some(active - 1);
            }
            // Update selected_file to match the new active tab
            if let Some(tab_idx) = self.active_tab {
                self.selected_file = Some(self.tabs[tab_idx].file_index);
            }
        }
    }

    /// Get the cursor time for the active tab
    pub fn get_cursor_time(&self) -> Option<f64> {
        self.active_tab.and_then(|idx| self.tabs[idx].cursor_time)
    }

    /// Set the cursor time for the active tab
    pub fn set_cursor_time(&mut self, time: Option<f64>) {
        if let Some(tab_idx) = self.active_tab {
            self.tabs[tab_idx].cursor_time = time;
        }
    }

    /// Get the cursor record for the active tab
    pub fn get_cursor_record(&self) -> Option<usize> {
        self.active_tab.and_then(|idx| self.tabs[idx].cursor_record)
    }

    /// Set the cursor record for the active tab
    pub fn set_cursor_record(&mut self, record: Option<usize>) {
        if let Some(tab_idx) = self.active_tab {
            self.tabs[tab_idx].cursor_record = record;
        }
    }

    /// Get the time range for the active tab
    pub fn get_time_range(&self) -> Option<(f64, f64)> {
        self.active_tab.and_then(|idx| self.tabs[idx].time_range)
    }

    /// Set the time range for the active tab
    pub fn set_time_range(&mut self, range: Option<(f64, f64)>) {
        if let Some(tab_idx) = self.active_tab {
            self.tabs[tab_idx].time_range = range;
        }
    }

    /// Get whether the chart has been interacted with for the active tab
    pub fn get_chart_interacted(&self) -> bool {
        self.active_tab
            .map(|idx| self.tabs[idx].chart_interacted)
            .unwrap_or(false)
    }

    /// Set the chart interacted state for the active tab
    pub fn set_chart_interacted(&mut self, interacted: bool) {
        if let Some(tab_idx) = self.active_tab {
            self.tabs[tab_idx].chart_interacted = interacted;
        }
    }

    /// Get the pending jump-to-time request for the active tab
    pub fn get_jump_to_time(&self) -> Option<f64> {
        self.active_tab.and_then(|idx| self.tabs[idx].jump_to_time)
    }

    /// Set a jump-to-time request for the active tab (chart will center on this time)
    pub fn set_jump_to_time(&mut self, time: Option<f64>) {
        if let Some(tab_idx) = self.active_tab {
            self.tabs[tab_idx].jump_to_time = time;
        }
    }

    /// Clear the jump-to-time request for the active tab
    pub fn clear_jump_to_time(&mut self) {
        if let Some(tab_idx) = self.active_tab {
            self.tabs[tab_idx].jump_to_time = None;
        }
    }

    /// Get the scatter plot state for the active tab
    pub fn get_scatter_plot_state(&self) -> Option<&ScatterPlotState> {
        self.active_tab
            .map(|idx| &self.tabs[idx].scatter_plot_state)
    }

    /// Get mutable scatter plot state for the active tab
    pub fn get_scatter_plot_state_mut(&mut self) -> Option<&mut ScatterPlotState> {
        self.active_tab
            .map(|idx| &mut self.tabs[idx].scatter_plot_state)
    }

    /// Get the left scatter plot config for the active tab
    pub fn get_scatter_left(&self) -> Option<&ScatterPlotConfig> {
        self.active_tab
            .map(|idx| &self.tabs[idx].scatter_plot_state.left)
    }

    /// Get mutable left scatter plot config for the active tab
    pub fn get_scatter_left_mut(&mut self) -> Option<&mut ScatterPlotConfig> {
        self.active_tab
            .map(|idx| &mut self.tabs[idx].scatter_plot_state.left)
    }

    /// Get the right scatter plot config for the active tab
    pub fn get_scatter_right(&self) -> Option<&ScatterPlotConfig> {
        self.active_tab
            .map(|idx| &self.tabs[idx].scatter_plot_state.right)
    }

    /// Get mutable right scatter plot config for the active tab
    pub fn get_scatter_right_mut(&mut self) -> Option<&mut ScatterPlotConfig> {
        self.active_tab
            .map(|idx| &mut self.tabs[idx].scatter_plot_state.right)
    }

    // ========================================================================
    // Auto-Update System
    // ========================================================================

    /// Start checking for updates in background
    pub fn start_update_check(&mut self) {
        // Don't start if already checking or downloading
        if matches!(
            self.update_state,
            UpdateState::Checking | UpdateState::Downloading
        ) {
            return;
        }

        self.update_state = UpdateState::Checking;

        let (sender, receiver) = channel();
        self.update_check_receiver = Some(receiver);

        thread::spawn(move || {
            let result = crate::updater::check_for_updates();
            let _ = sender.send(result);
        });
    }

    /// Start downloading update in background
    pub fn start_update_download(&mut self, url: String) {
        self.update_state = UpdateState::Downloading;

        let (sender, receiver) = channel();
        self.update_download_receiver = Some(receiver);

        thread::spawn(move || {
            let result = crate::updater::download_update(&url);
            let _ = sender.send(result);
        });
    }

    /// Check for completed update operations
    fn check_update_complete(&mut self) {
        // Check for update check completion
        if let Some(receiver) = &self.update_check_receiver {
            if let Ok(result) = receiver.try_recv() {
                match result {
                    UpdateCheckResult::UpdateAvailable(info) => {
                        self.update_state = UpdateState::UpdateAvailable(info);
                        self.show_update_dialog = true;
                    }
                    UpdateCheckResult::UpToDate => {
                        self.update_state = UpdateState::Idle;
                        // Only show toast for manual checks (not startup)
                        if self.startup_check_done {
                            self.show_toast_success("You're running the latest version");
                        }
                    }
                    UpdateCheckResult::Error(e) => {
                        self.update_state = UpdateState::Error(e.clone());
                        // Only show error toast for manual checks
                        if self.startup_check_done {
                            self.show_toast_error(&format!("Update check failed: {}", e));
                        }
                    }
                }
                self.update_check_receiver = None;
                self.startup_check_done = true;
            }
        }

        // Check for download completion
        if let Some(receiver) = &self.update_download_receiver {
            if let Ok(result) = receiver.try_recv() {
                match result {
                    DownloadResult::Success(path) => {
                        self.update_state = UpdateState::ReadyToInstall(path);
                        self.show_toast_success("Update downloaded successfully");
                    }
                    DownloadResult::Error(e) => {
                        self.update_state = UpdateState::Error(e.clone());
                        self.show_toast_error(&format!("Download failed: {}", e));
                    }
                }
                self.update_download_receiver = None;
            }
        }
    }

    /// Check for updates on startup (runs once)
    fn check_startup_update(&mut self) {
        if !self.startup_check_done && self.auto_check_updates {
            self.start_update_check();
        }
    }

    // ========================================================================
    // Toast Notifications
    // ========================================================================

    /// Show a toast message with a specific type
    pub fn show_toast_with_type(&mut self, message: &str, toast_type: ToastType) {
        self.toast_message = Some((message.to_string(), std::time::Instant::now(), toast_type));
    }

    /// Show an info toast (blue) - default for general messages
    pub fn show_toast(&mut self, message: &str) {
        self.show_toast_with_type(message, ToastType::Info);
    }

    /// Show a success toast (green)
    pub fn show_toast_success(&mut self, message: &str) {
        self.show_toast_with_type(message, ToastType::Success);
    }

    /// Show a warning toast (amber)
    pub fn show_toast_warning(&mut self, message: &str) {
        self.show_toast_with_type(message, ToastType::Warning);
    }

    /// Show an error toast (red)
    pub fn show_toast_error(&mut self, message: &str) {
        self.show_toast_with_type(message, ToastType::Error);
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
        if self.files.is_empty() || self.get_selected_channels().is_empty() {
            return;
        }

        // Don't handle shortcuts when a text field or other widget has keyboard focus
        if ctx.memory(|m| m.focused().is_some()) {
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
                    if self.get_cursor_time().is_none() {
                        if let Some((min, _)) = self.get_time_range() {
                            self.set_cursor_time(Some(min));
                            let record = self.find_record_at_time(min);
                            self.set_cursor_record(record);
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
        // Exit if update installation requires it (updater script is waiting)
        if self.should_exit_for_update {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        // Check for updates on startup (runs once)
        self.check_startup_update();

        // Check for completed update operations
        self.check_update_complete();

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

        // Request repaint while loading or updating (for spinner animation)
        if matches!(self.loading_state, LoadingState::Loading(_))
            || matches!(
                self.update_state,
                UpdateState::Checking | UpdateState::Downloading
            )
        {
            ctx.request_repaint();
        }

        // Toast notifications
        self.render_toast(ctx);

        // Modal windows
        self.render_normalization_editor(ctx);
        self.render_update_dialog(ctx);

        // Menu bar at top with padding
        let menu_frame = egui::Frame::NONE.inner_margin(egui::Margin {
            left: 10,
            right: 10,
            top: 8,
            bottom: 8,
        });

        egui::TopBottomPanel::top("menu_bar")
            .frame(menu_frame)
            .show(ctx, |ui| {
                self.render_menu_bar(ui);
            });

        // Tool switcher panel (pill tabs)
        let tool_switcher_frame = egui::Frame::NONE
            .fill(egui::Color32::from_rgb(35, 35, 35))
            .inner_margin(egui::Margin {
                left: 10,
                right: 10,
                top: 8,
                bottom: 8,
            });

        egui::TopBottomPanel::top("tool_switcher")
            .frame(tool_switcher_frame)
            .show(ctx, |ui| {
                self.render_tool_switcher(ui);
            });

        // Panel background color (matches drop zone card)
        let panel_bg = egui::Color32::from_rgb(45, 45, 45);
        let panel_frame = egui::Frame::NONE
            .fill(panel_bg)
            .inner_margin(egui::Margin::symmetric(10, 10));

        // Left sidebar panel (always visible)
        egui::SidePanel::left("files_panel")
            .default_width(200.0)
            .resizable(true)
            .frame(panel_frame.clone())
            .show(ctx, |ui| {
                self.render_sidebar(ui);
            });

        // Right panel for channel selection (only in Log Viewer mode)
        if self.active_tool == ActiveTool::LogViewer {
            egui::SidePanel::right("channels_panel")
                .default_width(300.0)
                .min_width(200.0)
                .resizable(true)
                .frame(panel_frame)
                .show(ctx, |ui| {
                    self.render_channel_selection(ui);
                });

            // Bottom panel for timeline scrubber (only in Log Viewer mode)
            if self.get_time_range().is_some() && !self.get_selected_channels().is_empty() {
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
        }

        // Main content area - render based on active tool
        egui::CentralPanel::default().show(ctx, |ui| {
            match self.active_tool {
                ActiveTool::LogViewer => {
                    // Tab bar at top (Chrome-style tabs for log files)
                    self.render_tab_bar(ui);

                    // Selected channels below tabs
                    ui.add_space(10.0);
                    self.render_selected_channels(ui);

                    ui.add_space(10.0);
                    ui.separator();

                    // Chart takes remaining space
                    self.render_chart(ui);
                }
                ActiveTool::ScatterPlot => {
                    ui.add_space(10.0);
                    self.render_scatter_plot_view(ui);
                }
            }
        });
    }
}
