use eframe::egui;
use egui_plot::{Line, Plot, PlotPoints};
use std::fs;
use std::path::PathBuf;

use crate::parsers::{Channel, EcuType, Haltech, Log, Parseable};

/// Color palette for chart lines (matches original theme)
const CHART_COLORS: &[[u8; 3]] = &[
    [113, 120, 78],   // Olive green (primary)
    [191, 78, 48],    // Rust orange (accent)
    [71, 108, 155],   // Blue (info)
    [159, 166, 119],  // Sage green (success)
    [253, 193, 73],   // Amber (warning)
    [135, 30, 28],    // Dark red (error)
    [246, 247, 235],  // Cream
    [100, 149, 237],  // Cornflower blue
    [255, 127, 80],   // Coral
    [144, 238, 144],  // Light green
];

/// Maximum number of channels that can be selected
const MAX_CHANNELS: usize = 10;

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
        }
    }
}

impl UltraLogApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self::default()
    }

    /// Load a log file from disk
    fn load_file(&mut self, path: PathBuf) -> Result<(), String> {
        // Check for duplicate
        if self.files.iter().any(|f| f.path == path) {
            return Err("File already loaded".to_string());
        }

        let contents = fs::read_to_string(&path).map_err(|e| e.to_string())?;

        // Try Haltech parser (can be extended for other formats)
        let parser = Haltech;
        let log = parser.parse(&contents).map_err(|e| e.to_string())?;

        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "Unknown".to_string());

        self.files.push(LoadedFile {
            path,
            name,
            ecu_type: EcuType::Haltech,
            log,
        });

        // Auto-select the newly loaded file
        self.selected_file = Some(self.files.len() - 1);

        Ok(())
    }

    /// Remove a loaded file
    fn remove_file(&mut self, index: usize) {
        if index < self.files.len() {
            // Remove any selected channels from this file
            self.selected_channels
                .retain(|c| c.file_index != index);

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
                    self.selected_file = if self.files.is_empty() {
                        None
                    } else {
                        Some(0)
                    };
                } else if selected > index {
                    self.selected_file = Some(selected - 1);
                }
            }
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
        let color_index = self.selected_channels.len() % CHART_COLORS.len();

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

    /// Render the file sidebar
    fn render_sidebar(&mut self, ui: &mut egui::Ui) {
        ui.heading("Files");
        ui.separator();

        // File open button
        if ui.button("Open File...").clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Log Files", &["csv", "log", "txt"])
                .pick_file()
            {
                if let Err(e) = self.load_file(path) {
                    self.show_toast(&format!("Error: {}", e));
                }
            }
        }

        ui.add_space(10.0);

        // File list
        let mut file_to_remove: Option<usize> = None;
        for (i, file) in self.files.iter().enumerate() {
            let is_selected = self.selected_file == Some(i);

            ui.horizontal(|ui| {
                let response = ui.selectable_label(is_selected, &file.name);
                if response.clicked() {
                    self.selected_file = Some(i);
                }

                // Delete button
                if ui.small_button("\u{2715}").clicked() {
                    file_to_remove = Some(i);
                }
            });

            // Show ECU type
            ui.indent(format!("file_indent_{}", i), |ui| {
                ui.label(
                    egui::RichText::new(file.ecu_type.name())
                        .small()
                        .color(egui::Color32::GRAY),
                );
            });
        }

        if let Some(index) = file_to_remove {
            self.remove_file(index);
        }

        if self.files.is_empty() {
            ui.label(
                egui::RichText::new("Drop files here or click 'Open File'")
                    .italics()
                    .color(egui::Color32::GRAY),
            );
        }
    }

    /// Render channel selection panel
    fn render_channel_selection(&mut self, ui: &mut egui::Ui) {
        ui.heading("Channels");
        ui.separator();

        if let Some(file_index) = self.selected_file {
            let file: &LoadedFile = &self.files[file_index];

            // Search box
            ui.horizontal(|ui| {
                ui.label("Search:");
                ui.text_edit_singleline(&mut self.channel_search);
            });

            ui.add_space(5.0);

            // Channel count
            ui.label(format!(
                "Selected: {} / {}",
                self.selected_channels.len(),
                MAX_CHANNELS
            ));

            ui.separator();

            // Channel list (scrollable)
            egui::ScrollArea::vertical()
                .max_height(200.0)
                .show(ui, |ui| {
                    let search_lower = self.channel_search.to_lowercase();

                    for (channel_index, channel) in file.log.channels.iter().enumerate() {
                        let name = channel.name();

                        // Filter by search
                        if !search_lower.is_empty()
                            && !name.to_lowercase().contains(&search_lower)
                        {
                            continue;
                        }

                        // Check if already selected
                        let is_selected = self
                            .selected_channels
                            .iter()
                            .any(|c| c.file_index == file_index && c.channel_index == channel_index);

                        ui.horizontal(|ui| {
                            if is_selected {
                                ui.label(
                                    egui::RichText::new("\u{2713}")
                                        .color(egui::Color32::from_rgb(113, 120, 78)),
                                );
                            }
                            ui.label(&name);
                        });
                    }
                });
        } else {
            ui.label(
                egui::RichText::new("Select a file to view channels")
                    .italics()
                    .color(egui::Color32::GRAY),
            );
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
                    let color = CHART_COLORS[selected.color_index % CHART_COLORS.len()];
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
                                        egui::RichText::new(&selected.channel.name())
                                            .strong()
                                            .color(color32),
                                    );
                                    if ui.small_button("\u{2715}").clicked() {
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

                                if let (Some(min), Some(max)) =
                                    (selected.channel.display_min(), selected.channel.display_max())
                                {
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

    /// Render the main chart
    fn render_chart(&self, ui: &mut egui::Ui) {
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

        Plot::new("log_chart")
            .legend(egui_plot::Legend::default())
            .show(ui, |plot_ui| {
                for selected in &self.selected_channels {
                    if selected.file_index >= self.files.len() {
                        continue;
                    }

                    let file = &self.files[selected.file_index];
                    let times = file.log.get_times_as_f64();
                    let data = file.log.get_channel_data(selected.channel_index);

                    if times.len() != data.len() || times.is_empty() {
                        continue;
                    }

                    let points: PlotPoints = times
                        .iter()
                        .zip(data.iter())
                        .map(|(t, v)| [*t, *v])
                        .collect();

                    let color = CHART_COLORS[selected.color_index % CHART_COLORS.len()];

                    plot_ui.line(
                        Line::new(points)
                            .name(&selected.channel.name())
                            .color(egui::Color32::from_rgb(color[0], color[1], color[2]))
                            .width(2.0),
                    );
                }
            });
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
                                ui.label(
                                    egui::RichText::new(message)
                                        .color(egui::Color32::WHITE),
                                );
                            });
                    });
            } else {
                self.toast_message = None;
            }
        }
    }

    /// Handle file drops
    fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        // Debounce file drops (5 second window)
        if let Some(last_drop) = self.last_drop_time {
            if last_drop.elapsed().as_secs() < 5 {
                return;
            }
        }

        let dropped_files: Vec<PathBuf> = ctx
            .input(|i| {
                i.raw.dropped_files
                    .iter()
                    .filter_map(|f| f.path.clone())
                    .collect()
            });

        if !dropped_files.is_empty() {
            self.last_drop_time = Some(std::time::Instant::now());

            for path in dropped_files {
                if let Err(e) = self.load_file(path) {
                    self.show_toast(&format!("Error: {}", e));
                }
            }
        }
    }
}

impl eframe::App for UltraLogApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Handle file drops
        self.handle_dropped_files(ctx);

        // Apply dark theme
        ctx.set_visuals(egui::Visuals::dark());

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
            .default_width(250.0)
            .resizable(true)
            .show(ctx, |ui| {
                // Channel selection at top
                self.render_channel_selection(ui);

                // Need to handle channel clicks with deferred action
                if let Some(file_index) = self.selected_file {
                    let file: &LoadedFile = &self.files[file_index];
                    let search_lower = self.channel_search.to_lowercase();

                    let mut channel_to_add: Option<(usize, usize)> = None;

                    egui::ScrollArea::vertical()
                        .id_salt("channel_scroll_clickable")
                        .max_height(200.0)
                        .show(ui, |ui| {
                            for (channel_index, channel) in file.log.channels.iter().enumerate() {
                                let name = channel.name();

                                if !search_lower.is_empty()
                                    && !name.to_lowercase().contains(&search_lower)
                                {
                                    continue;
                                }

                                let is_selected = self.selected_channels.iter().any(|c| {
                                    c.file_index == file_index && c.channel_index == channel_index
                                });

                                let response = ui.selectable_label(is_selected, &name);
                                if response.clicked() && !is_selected {
                                    channel_to_add = Some((file_index, channel_index));
                                }
                            }
                        });

                    if let Some((file_idx, channel_idx)) = channel_to_add {
                        self.add_channel(file_idx, channel_idx);
                    }
                }
            });

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
