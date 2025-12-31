//! Analysis Panel UI.
//!
//! Provides a window for users to run analysis algorithms on the active log file,
//! including signal processing filters and statistical analyzers.

use eframe::egui;
use std::collections::HashMap;

use crate::analysis::{AnalysisResult, Analyzer, AnalyzerConfig, LogDataAccess};
use crate::app::UltraLogApp;
use crate::computed::{ComputedChannel, ComputedChannelTemplate};
use crate::normalize::sort_channels_by_priority;
use crate::parsers::types::ComputedChannelInfo;
use crate::parsers::Channel;
use crate::state::{SelectedChannel, CHART_COLORS};

/// Info about an analyzer for display (avoids borrow issues)
struct AnalyzerInfo {
    id: String,
    name: String,
    description: String,
    category: String,
    config: AnalyzerConfig,
}

/// Parameter definition for UI rendering
#[derive(Clone)]
struct ParamDef {
    key: String,
    label: String,
    param_type: ParamType,
    /// Tooltip with helpful information about expected channel types
    tooltip: Option<String>,
}

#[derive(Clone)]
enum ParamType {
    Channel, // Channel selector dropdown
    Integer { min: i32, max: i32 },
    Float { min: f64, max: f64 },
    Boolean,
}

impl UltraLogApp {
    /// Render the analysis panel window
    pub fn render_analysis_panel(&mut self, ctx: &egui::Context) {
        if !self.show_analysis_panel {
            return;
        }

        let mut open = true;

        egui::Window::new("Analysis Tools")
            .open(&mut open)
            .resizable(true)
            .default_width(550.0)
            .default_height(500.0)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                // Header
                ui.heading("Signal Analysis");
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(
                        "Run signal processing and statistical analysis on log data.",
                    )
                    .color(egui::Color32::GRAY),
                );
                ui.add_space(8.0);

                ui.separator();
                ui.add_space(4.0);

                // Check if we have a file loaded
                let has_file = self.selected_file.is_some() && !self.files.is_empty();

                if !has_file {
                    ui.vertical_centered(|ui| {
                        ui.add_space(40.0);
                        ui.label(
                            egui::RichText::new("No log file loaded")
                                .color(egui::Color32::GRAY)
                                .size(16.0),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new("Load a log file to access analysis tools.")
                                .color(egui::Color32::GRAY)
                                .small(),
                        );
                        ui.add_space(40.0);
                    });
                } else {
                    // Show available analyzers grouped by category
                    self.render_analyzer_list(ui);
                }
            });

        if !open {
            self.show_analysis_panel = false;
        }
    }

    /// Render the list of available analyzers
    fn render_analyzer_list(&mut self, ui: &mut egui::Ui) {
        // Get channel names from the currently selected file, with normalization and sorting
        let (channel_names, channel_display_names): (Vec<String>, Vec<String>) =
            if let Some(file_idx) = self.selected_file {
                if let Some(file) = self.files.get(file_idx) {
                    let raw_names = file.log.channel_names();

                    // Sort channels like the main sidebar: normalized first, then alphabetically
                    let sorted = sort_channels_by_priority(
                        raw_names.len(),
                        |idx| raw_names.get(idx).cloned().unwrap_or_default(),
                        self.field_normalization,
                        Some(&self.custom_normalizations),
                    );

                    // Build parallel vectors: raw names for matching, display names for UI
                    let mut raw_sorted = Vec::with_capacity(sorted.len());
                    let mut display_sorted = Vec::with_capacity(sorted.len());

                    for (idx, display_name, _is_normalized) in sorted {
                        if let Some(raw_name) = raw_names.get(idx) {
                            raw_sorted.push(raw_name.clone());
                            display_sorted.push(display_name);
                        }
                    }

                    (raw_sorted, display_sorted)
                } else {
                    (vec![], vec![])
                }
            } else {
                (vec![], vec![])
            };

        // Collect analyzer info upfront to avoid borrow issues
        let analyzer_infos: Vec<AnalyzerInfo> = self
            .analyzer_registry
            .all()
            .iter()
            .map(|a| AnalyzerInfo {
                id: a.id().to_string(),
                name: a.name().to_string(),
                description: a.description().to_string(),
                category: a.category().to_string(),
                config: a.get_config(),
            })
            .collect();

        // Group by category
        let mut categories: HashMap<String, Vec<&AnalyzerInfo>> = HashMap::new();
        for info in &analyzer_infos {
            categories
                .entry(info.category.clone())
                .or_default()
                .push(info);
        }

        let mut sorted_categories: Vec<_> = categories.keys().cloned().collect();
        sorted_categories.sort();

        // Track actions to perform after rendering
        let mut analyzer_to_run: Option<String> = None;
        let mut analyzer_to_run_and_chart: Option<String> = None;
        let mut config_updates: Vec<(String, AnalyzerConfig)> = Vec::new();
        let mut result_to_add: Option<usize> = None;
        let mut result_to_remove: Option<usize> = None;

        egui::ScrollArea::vertical()
            .id_salt("analysis_panel_scroll")
            .show(ui, |ui| {
                // Show analysis results at the TOP if any exist
                if let Some(file_idx) = self.selected_file {
                    if let Some(results) = self.analysis_results.get(&file_idx) {
                        if !results.is_empty() {
                            egui::CollapsingHeader::new(
                                egui::RichText::new(format!("Results ({})", results.len()))
                                    .strong()
                                    .size(14.0),
                            )
                            .default_open(true)
                            .show(ui, |ui| {
                                for (i, result) in results.iter().enumerate() {
                                    if let Some(action) =
                                        Self::render_analysis_result_with_actions(ui, result, i)
                                    {
                                        match action {
                                            ResultAction::AddToChart => result_to_add = Some(i),
                                            ResultAction::Remove => result_to_remove = Some(i),
                                        }
                                    }
                                }
                            });

                            ui.add_space(4.0);
                            ui.separator();
                            ui.add_space(4.0);
                        }
                    }
                }

                // Show available analyzers grouped by category
                for category in &sorted_categories {
                    if let Some(analyzers) = categories.get(category) {
                        ui.add_space(4.0);

                        egui::CollapsingHeader::new(
                            egui::RichText::new(category).strong().size(14.0),
                        )
                        .default_open(true)
                        .show(ui, |ui| {
                            for info in analyzers {
                                if let Some((id, action)) = Self::render_analyzer_card_with_config(
                                    ui,
                                    info,
                                    &channel_names,
                                    &channel_display_names,
                                ) {
                                    match action {
                                        AnalyzerAction::Run => {
                                            analyzer_to_run = Some(id);
                                        }
                                        AnalyzerAction::RunAndChart => {
                                            analyzer_to_run_and_chart = Some(id);
                                        }
                                        AnalyzerAction::UpdateConfig(config) => {
                                            config_updates.push((id, config));
                                        }
                                    }
                                }
                            }
                        });

                        ui.add_space(4.0);
                    }
                }
            });

        // Handle deferred actions

        // Apply config updates
        for (id, config) in config_updates {
            if let Some(analyzer) = self.analyzer_registry.find_by_id_mut(&id) {
                analyzer.set_config(&config);
            }
        }

        // Run analyzer (just run, don't add to chart)
        if let Some(id) = analyzer_to_run {
            self.run_analyzer(&id);
        }

        // Run analyzer AND add to chart immediately
        if let Some(id) = analyzer_to_run_and_chart {
            self.run_analyzer_and_chart(&id);
        }

        // Add result to chart
        if let Some(idx) = result_to_add {
            self.add_analysis_result_to_chart(idx);
        }

        // Remove result
        if let Some(idx) = result_to_remove {
            if let Some(file_idx) = self.selected_file {
                if let Some(results) = self.analysis_results.get_mut(&file_idx) {
                    if idx < results.len() {
                        results.remove(idx);
                    }
                }
            }
        }
    }

    /// Render a single analyzer card with configuration options
    /// Returns Some((id, action)) if an action was triggered
    ///
    /// `channel_names` - raw channel names (for config storage and matching)
    /// `channel_display_names` - display names (normalized if enabled)
    fn render_analyzer_card_with_config(
        ui: &mut egui::Ui,
        info: &AnalyzerInfo,
        channel_names: &[String],
        channel_display_names: &[String],
    ) -> Option<(String, AnalyzerAction)> {
        let mut action: Option<AnalyzerAction> = None;
        let mut new_config = info.config.clone();

        // Get parameter definitions for this analyzer
        let param_defs = get_analyzer_params(&info.id);

        // Check if required channels are available
        let channels_available = check_channels_available(
            &new_config,
            &param_defs,
            channel_names,
            channel_display_names,
        );

        let card_bg = if channels_available {
            egui::Color32::from_rgb(40, 45, 40)
        } else {
            egui::Color32::from_rgb(45, 45, 45)
        };

        egui::Frame::NONE
            .fill(card_bg)
            .corner_radius(6)
            .inner_margin(10.0)
            .outer_margin(egui::Margin::symmetric(0, 2))
            .show(ui, |ui| {
                // Header row with name and Run button
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new(&info.name).strong());
                        ui.label(
                            egui::RichText::new(&info.description)
                                .color(egui::Color32::GRAY)
                                .small(),
                        );
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // "Run & Chart" button (primary action)
                        let run_chart_btn = egui::Button::new("Run & Chart")
                            .fill(egui::Color32::from_rgb(60, 100, 60));
                        let run_chart_response = ui.add_enabled(channels_available, run_chart_btn);

                        if run_chart_response.clicked() {
                            action = Some(AnalyzerAction::RunAndChart);
                        }

                        if !channels_available {
                            run_chart_response.on_hover_text("Select valid channels first");
                        } else {
                            run_chart_response
                                .on_hover_text("Run analysis and add result to chart");
                        }

                        ui.add_space(4.0);

                        // "Run" button (secondary - just adds to results)
                        let run_btn = egui::Button::new("Run");
                        let run_response = ui.add_enabled(channels_available, run_btn);

                        if run_response.clicked() {
                            action = Some(AnalyzerAction::Run);
                        }

                        if !channels_available {
                            run_response.on_hover_text("Select valid channels first");
                        } else {
                            run_response.on_hover_text("Run analysis (add to chart later)");
                        }
                    });
                });

                ui.add_space(6.0);

                // Parameter configuration
                let mut config_changed = false;

                for param in &param_defs {
                    ui.horizontal(|ui| {
                        let label_response = ui.label(
                            egui::RichText::new(&param.label)
                                .color(egui::Color32::GRAY)
                                .small(),
                        );

                        // Show tooltip if available
                        if let Some(tooltip) = &param.tooltip {
                            label_response.on_hover_text(tooltip);
                        }

                        ui.add_space(4.0);

                        match &param.param_type {
                            ParamType::Channel => {
                                let config_value = new_config
                                    .parameters
                                    .get(&param.key)
                                    .cloned()
                                    .unwrap_or_default();

                                // Resolve config value to raw channel name
                                // Config might have a normalized name (e.g., "AFR") that needs
                                // to be resolved to the actual raw name (e.g., "Wideband O2 Overall")
                                let (current_raw, current_display) = if let Some(idx) =
                                    channel_names
                                        .iter()
                                        .position(|n| n.eq_ignore_ascii_case(&config_value))
                                {
                                    // Config value matches a raw name directly
                                    (
                                        channel_names[idx].clone(),
                                        channel_display_names
                                            .get(idx)
                                            .cloned()
                                            .unwrap_or(config_value.clone()),
                                    )
                                } else if let Some(idx) = channel_display_names
                                    .iter()
                                    .position(|n| n.eq_ignore_ascii_case(&config_value))
                                {
                                    // Config value matches a display/normalized name - resolve to raw
                                    let raw = channel_names
                                        .get(idx)
                                        .cloned()
                                        .unwrap_or(config_value.clone());
                                    let display = channel_display_names[idx].clone();
                                    // Auto-update config to use the raw name
                                    new_config.parameters.insert(param.key.clone(), raw.clone());
                                    config_changed = true;
                                    (raw, display)
                                } else {
                                    // No match found, keep as-is
                                    (config_value.clone(), config_value)
                                };

                                let combo_response = egui::ComboBox::from_id_salt(format!(
                                    "{}_{}_combo",
                                    info.id, param.key
                                ))
                                .width(180.0)
                                .selected_text(&current_display)
                                .show_ui(ui, |ui| {
                                    // Show display names but store raw names
                                    for (raw_name, display_name) in
                                        channel_names.iter().zip(channel_display_names.iter())
                                    {
                                        if ui
                                            .selectable_label(
                                                current_raw == *raw_name,
                                                display_name,
                                            )
                                            .clicked()
                                        {
                                            new_config
                                                .parameters
                                                .insert(param.key.clone(), raw_name.clone());
                                            config_changed = true;
                                        }
                                    }
                                });

                                // Show tooltip on combo box too
                                if let Some(tooltip) = &param.tooltip {
                                    combo_response.response.on_hover_text(tooltip);
                                }
                            }
                            ParamType::Integer { min, max } => {
                                let current: i32 = new_config
                                    .parameters
                                    .get(&param.key)
                                    .and_then(|s| s.parse().ok())
                                    .unwrap_or(*min);

                                let mut value = current;
                                if ui
                                    .add(
                                        egui::DragValue::new(&mut value)
                                            .range(*min..=*max)
                                            .speed(1),
                                    )
                                    .changed()
                                {
                                    new_config
                                        .parameters
                                        .insert(param.key.clone(), value.to_string());
                                    config_changed = true;
                                }
                            }
                            ParamType::Float { min, max } => {
                                let current: f64 = new_config
                                    .parameters
                                    .get(&param.key)
                                    .and_then(|s| s.parse().ok())
                                    .unwrap_or(*min);

                                let mut value = current;
                                if ui
                                    .add(
                                        egui::DragValue::new(&mut value)
                                            .range(*min..=*max)
                                            .speed(0.01)
                                            .fixed_decimals(2),
                                    )
                                    .changed()
                                {
                                    new_config
                                        .parameters
                                        .insert(param.key.clone(), value.to_string());
                                    config_changed = true;
                                }
                            }
                            ParamType::Boolean => {
                                let current: bool = new_config
                                    .parameters
                                    .get(&param.key)
                                    .and_then(|s| s.parse().ok())
                                    .unwrap_or(false);

                                let mut value = current;
                                if ui.checkbox(&mut value, "").changed() {
                                    new_config
                                        .parameters
                                        .insert(param.key.clone(), value.to_string());
                                    config_changed = true;
                                }
                            }
                        }
                    });
                }

                if config_changed && action.is_none() {
                    action = Some(AnalyzerAction::UpdateConfig(new_config.clone()));
                }
            });

        action.map(|a| (info.id.clone(), a))
    }

    /// Render an analysis result with Add to Chart and Remove buttons
    fn render_analysis_result_with_actions(
        ui: &mut egui::Ui,
        result: &AnalysisResult,
        _index: usize,
    ) -> Option<ResultAction> {
        let mut action: Option<ResultAction> = None;

        egui::Frame::NONE
            .fill(egui::Color32::from_rgb(35, 40, 45))
            .corner_radius(6)
            .inner_margin(8.0)
            .outer_margin(egui::Margin::symmetric(0, 2))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(&result.name).strong());
                            if !result.unit.is_empty() {
                                ui.label(
                                    egui::RichText::new(format!("({})", result.unit))
                                        .color(egui::Color32::GRAY)
                                        .small(),
                                );
                            }
                        });

                        // Show basic stats about the result
                        if !result.values.is_empty() {
                            let min = result.values.iter().cloned().fold(f64::INFINITY, f64::min);
                            let max = result
                                .values
                                .iter()
                                .cloned()
                                .fold(f64::NEG_INFINITY, f64::max);
                            let mean: f64 =
                                result.values.iter().sum::<f64>() / result.values.len() as f64;

                            ui.label(
                                egui::RichText::new(format!(
                                    "Min: {:.2}  Max: {:.2}  Mean: {:.2}  ({} pts)",
                                    min,
                                    max,
                                    mean,
                                    result.values.len()
                                ))
                                .color(egui::Color32::GRAY)
                                .small(),
                            );
                        }
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Remove button
                        if ui
                            .small_button("x")
                            .on_hover_text("Remove result")
                            .clicked()
                        {
                            action = Some(ResultAction::Remove);
                        }

                        ui.add_space(4.0);

                        // Add to chart button
                        if ui
                            .button("+ Chart")
                            .on_hover_text("Add to chart as a channel")
                            .clicked()
                        {
                            action = Some(ResultAction::AddToChart);
                        }
                    });
                });
            });

        action
    }

    /// Add an analysis result to the chart as a computed channel
    fn add_analysis_result_to_chart(&mut self, result_index: usize) {
        let file_idx = match self.selected_file {
            Some(idx) => idx,
            None => return,
        };

        let tab_idx = match self.active_tab {
            Some(idx) => idx,
            None => return,
        };

        // Get the result
        let result = match self.analysis_results.get(&file_idx) {
            Some(results) => match results.get(result_index) {
                Some(r) => r.clone(),
                None => return,
            },
            None => return,
        };

        // Create a computed channel from the result
        let template = ComputedChannelTemplate::new(
            result.name.clone(),
            format!("_analysis_result_{}", result_index), // Placeholder formula
            result.unit.clone(),
            format!("Analysis result: {}", result.metadata.algorithm),
        );

        let mut computed = ComputedChannel::from_template(template);
        computed.cached_data = Some(result.values.clone());

        // Add to file's computed channels
        let computed_channels = self.file_computed_channels.entry(file_idx).or_default();
        computed_channels.push(computed.clone());

        // Get the virtual channel index
        let file = match self.files.get(file_idx) {
            Some(f) => f,
            None => return,
        };
        let base_channel_count = file.log.channels.len();
        let virtual_channel_index = base_channel_count + computed_channels.len() - 1;

        // Find next available color
        let used_colors: std::collections::HashSet<usize> = self.tabs[tab_idx]
            .selected_channels
            .iter()
            .map(|c| c.color_index)
            .collect();
        let color_index = (0..CHART_COLORS.len())
            .find(|&i| !used_colors.contains(&i))
            .unwrap_or(0);

        // Create the channel enum variant
        let channel = Channel::Computed(ComputedChannelInfo {
            name: computed.name().to_string(),
            formula: computed.formula().to_string(),
            unit: computed.unit().to_string(),
        });

        // Add to selected channels
        self.tabs[tab_idx].selected_channels.push(SelectedChannel {
            file_index: file_idx,
            channel_index: virtual_channel_index,
            channel,
            color_index,
        });

        self.show_toast_success(&format!("Added '{}' to chart", result.name));
    }

    /// Run an analyzer by its ID
    fn run_analyzer(&mut self, analyzer_id: &str) {
        let file_idx = match self.selected_file {
            Some(idx) => idx,
            None => {
                self.show_toast_error("No file selected");
                return;
            }
        };

        // Get the log data
        let log = match self.files.get(file_idx) {
            Some(file) => &file.log,
            None => {
                self.show_toast_error("File not found");
                return;
            }
        };

        // Find and run the analyzer
        // We need to clone the analyzer to avoid borrow issues
        let analyzer_clone: Option<Box<dyn Analyzer>> = self
            .analyzer_registry
            .find_by_id(analyzer_id)
            .map(|a| a.clone_box());

        if let Some(analyzer) = analyzer_clone {
            match analyzer.analyze(log) {
                Ok(result) => {
                    let result_name = result.name.clone();
                    self.analysis_results
                        .entry(file_idx)
                        .or_default()
                        .push(result);
                    self.show_toast_success(&format!("Analysis complete: {}", result_name));
                }
                Err(e) => {
                    self.show_toast_error(&format!("Analysis failed: {}", e));
                }
            }
        } else {
            self.show_toast_error(&format!("Analyzer not found: {}", analyzer_id));
        }
    }

    /// Run an analyzer and immediately add the result to the chart
    fn run_analyzer_and_chart(&mut self, analyzer_id: &str) {
        let file_idx = match self.selected_file {
            Some(idx) => idx,
            None => {
                self.show_toast_error("No file selected");
                return;
            }
        };

        // Get the log data
        let log = match self.files.get(file_idx) {
            Some(file) => &file.log,
            None => {
                self.show_toast_error("File not found");
                return;
            }
        };

        // Find and run the analyzer
        let analyzer_clone: Option<Box<dyn Analyzer>> = self
            .analyzer_registry
            .find_by_id(analyzer_id)
            .map(|a| a.clone_box());

        if let Some(analyzer) = analyzer_clone {
            match analyzer.analyze(log) {
                Ok(result) => {
                    let result_name = result.name.clone();

                    // Add to results
                    self.analysis_results
                        .entry(file_idx)
                        .or_default()
                        .push(result);

                    // Get the index of the result we just added
                    let result_idx = self
                        .analysis_results
                        .get(&file_idx)
                        .map(|r| r.len().saturating_sub(1))
                        .unwrap_or(0);

                    // Immediately add to chart
                    self.add_analysis_result_to_chart(result_idx);

                    self.show_toast_success(&format!("'{}' added to chart", result_name));
                }
                Err(e) => {
                    self.show_toast_error(&format!("Analysis failed: {}", e));
                }
            }
        } else {
            self.show_toast_error(&format!("Analyzer not found: {}", analyzer_id));
        }
    }
}

/// Actions that can be triggered by analyzer card
enum AnalyzerAction {
    Run,
    RunAndChart,
    UpdateConfig(AnalyzerConfig),
}

/// Actions for analysis results
enum ResultAction {
    AddToChart,
    Remove,
}

/// Get parameter definitions for a specific analyzer
fn get_analyzer_params(analyzer_id: &str) -> Vec<ParamDef> {
    match analyzer_id {
        // ============== Filters ==============
        "moving_average" => vec![
            ParamDef {
                key: "channel".to_string(),
                label: "Channel:".to_string(),
                param_type: ParamType::Channel,
                tooltip: Some("Any numeric channel to smooth".to_string()),
            },
            ParamDef {
                key: "window_size".to_string(),
                label: "Window:".to_string(),
                param_type: ParamType::Integer { min: 2, max: 100 },
                tooltip: Some("Number of samples to average (larger = smoother)".to_string()),
            },
        ],
        "exponential_moving_average" => vec![
            ParamDef {
                key: "channel".to_string(),
                label: "Channel:".to_string(),
                param_type: ParamType::Channel,
                tooltip: Some("Any numeric channel to smooth".to_string()),
            },
            ParamDef {
                key: "alpha".to_string(),
                label: "Alpha:".to_string(),
                param_type: ParamType::Float { min: 0.01, max: 1.0 },
                tooltip: Some("Smoothing factor: 0.1 = very smooth, 0.5 = moderate, 0.9 = responsive".to_string()),
            },
        ],
        "median_filter" => vec![
            ParamDef {
                key: "channel".to_string(),
                label: "Channel:".to_string(),
                param_type: ParamType::Channel,
                tooltip: Some("Channel with spike noise to remove".to_string()),
            },
            ParamDef {
                key: "window_size".to_string(),
                label: "Window:".to_string(),
                param_type: ParamType::Integer { min: 3, max: 51 },
                tooltip: Some("Must be odd number. Larger = removes wider spikes".to_string()),
            },
        ],
        "butterworth_lowpass" => vec![
            ParamDef {
                key: "channel".to_string(),
                label: "Channel:".to_string(),
                param_type: ParamType::Channel,
                tooltip: Some("Channel to filter. Common: RPM, MAP, TPS, AFR, Knock".to_string()),
            },
            ParamDef {
                key: "cutoff_normalized".to_string(),
                label: "Cutoff:".to_string(),
                param_type: ParamType::Float { min: 0.01, max: 0.49 },
                tooltip: Some("Normalized cutoff (0-0.5). 0.1 = 10% of sample rate. Lower = more smoothing".to_string()),
            },
            ParamDef {
                key: "order".to_string(),
                label: "Order:".to_string(),
                param_type: ParamType::Integer { min: 1, max: 8 },
                tooltip: Some("Filter order (1-8). Higher = sharper cutoff but more ringing. 2-4 recommended".to_string()),
            },
        ],
        "butterworth_highpass" => vec![
            ParamDef {
                key: "channel".to_string(),
                label: "Channel:".to_string(),
                param_type: ParamType::Channel,
                tooltip: Some("Channel to remove DC/drift from. Common: Knock sensor, vibration data".to_string()),
            },
            ParamDef {
                key: "cutoff_normalized".to_string(),
                label: "Cutoff:".to_string(),
                param_type: ParamType::Float { min: 0.01, max: 0.49 },
                tooltip: Some("Normalized cutoff (0-0.5). Frequencies below this are removed".to_string()),
            },
            ParamDef {
                key: "order".to_string(),
                label: "Order:".to_string(),
                param_type: ParamType::Integer { min: 1, max: 8 },
                tooltip: Some("Filter order (1-8). Higher = sharper cutoff. 2-4 recommended".to_string()),
            },
        ],

        // ============== Statistics ==============
        "descriptive_stats" => vec![ParamDef {
            key: "channel".to_string(),
            label: "Channel:".to_string(),
            param_type: ParamType::Channel,
            tooltip: Some("Any channel to compute min/max/mean/stdev statistics".to_string()),
        }],
        "correlation" => vec![
            ParamDef {
                key: "channel_x".to_string(),
                label: "Channel X:".to_string(),
                param_type: ParamType::Channel,
                tooltip: Some("First channel (e.g., RPM, TPS, MAP)".to_string()),
            },
            ParamDef {
                key: "channel_y".to_string(),
                label: "Channel Y:".to_string(),
                param_type: ParamType::Channel,
                tooltip: Some("Second channel to correlate (e.g., AFR, Fuel Trim)".to_string()),
            },
        ],
        "rate_of_change" => vec![
            ParamDef {
                key: "channel".to_string(),
                label: "Channel:".to_string(),
                param_type: ParamType::Channel,
                tooltip: Some("Channel to differentiate. Common: RPM (acceleration), TPS (throttle rate)".to_string()),
            },
            ParamDef {
                key: "time_based".to_string(),
                label: "Per second:".to_string(),
                param_type: ParamType::Boolean,
                tooltip: Some("If checked, rate is per second. Otherwise, per sample".to_string()),
            },
        ],

        // ============== AFR Analysis ==============
        "fuel_trim_drift" => vec![
            ParamDef {
                key: "channel".to_string(),
                label: "Fuel Trim:".to_string(),
                param_type: ParamType::Channel,
                tooltip: Some("Fuel trim channel. Common names: LTFT, STFT, Long Term FT, Short Term FT, Fuel Trim, FT Bank1".to_string()),
            },
            ParamDef {
                key: "k".to_string(),
                label: "Sensitivity (k):".to_string(),
                param_type: ParamType::Float { min: 0.1, max: 10.0 },
                tooltip: Some("CUSUM slack parameter. Lower = more sensitive to small drifts. Default 2.5".to_string()),
            },
            ParamDef {
                key: "h".to_string(),
                label: "Threshold (h):".to_string(),
                param_type: ParamType::Float { min: 1.0, max: 100.0 },
                tooltip: Some("Detection threshold. Lower = faster detection, more false alarms. Default 20".to_string()),
            },
            ParamDef {
                key: "baseline_pct".to_string(),
                label: "Baseline %:".to_string(),
                param_type: ParamType::Float { min: 1.0, max: 50.0 },
                tooltip: Some("% of data from start to use as baseline. Default 10%".to_string()),
            },
        ],
        "rich_lean_zone" => vec![
            ParamDef {
                key: "channel".to_string(),
                label: "AFR/Lambda:".to_string(),
                param_type: ParamType::Channel,
                tooltip: Some("AFR or Lambda channel. Auto-detects unit type. Common: AFR, Lambda, Wideband O2, O2".to_string()),
            },
            ParamDef {
                key: "target".to_string(),
                label: "Target:".to_string(),
                param_type: ParamType::Float { min: 0.0, max: 20.0 },
                tooltip: Some("Target value. Set to 0 for auto-detect (AFR: 14.7, Lambda: 1.0). Or set manually.".to_string()),
            },
            ParamDef {
                key: "rich_threshold".to_string(),
                label: "Rich threshold:".to_string(),
                param_type: ParamType::Float { min: 0.0, max: 3.0 },
                tooltip: Some("Set to 0 for auto-detect (AFR: 0.5, Lambda: 0.03). Rich = below (target - threshold)".to_string()),
            },
            ParamDef {
                key: "lean_threshold".to_string(),
                label: "Lean threshold:".to_string(),
                param_type: ParamType::Float { min: 0.0, max: 3.0 },
                tooltip: Some("Set to 0 for auto-detect (AFR: 0.5, Lambda: 0.03). Lean = above (target + threshold)".to_string()),
            },
        ],
        "afr_deviation" => vec![
            ParamDef {
                key: "channel".to_string(),
                label: "AFR/Lambda:".to_string(),
                param_type: ParamType::Channel,
                tooltip: Some("AFR or Lambda channel. Auto-detects unit type. Common: AFR, Lambda, Wideband O2, O2".to_string()),
            },
            ParamDef {
                key: "target".to_string(),
                label: "Target:".to_string(),
                param_type: ParamType::Float { min: 0.0, max: 20.0 },
                tooltip: Some("Target value. Set to 0 for auto-detect (AFR: 14.7, Lambda: 1.0). Or set manually.".to_string()),
            },
        ],

        // ============== Derived Calculations ==============
        "volumetric_efficiency" => vec![
            ParamDef {
                key: "rpm_channel".to_string(),
                label: "RPM:".to_string(),
                param_type: ParamType::Channel,
                tooltip: Some("Engine RPM channel. Common names: RPM, Engine Speed, Engine RPM".to_string()),
            },
            ParamDef {
                key: "map_channel".to_string(),
                label: "MAP (kPa):".to_string(),
                param_type: ParamType::Channel,
                tooltip: Some("Manifold Absolute Pressure in kPa. Common names: MAP, Manifold Pressure, Boost".to_string()),
            },
            ParamDef {
                key: "iat_channel".to_string(),
                label: "IAT:".to_string(),
                param_type: ParamType::Channel,
                tooltip: Some("Intake Air Temp in °C. Common names: IAT, Intake Temp, Air Temp, ACT".to_string()),
            },
            ParamDef {
                key: "displacement_l".to_string(),
                label: "Displacement (L):".to_string(),
                param_type: ParamType::Float { min: 0.1, max: 10.0 },
                tooltip: Some("Engine displacement in liters. E.g., 2.0, 3.5, 5.7".to_string()),
            },
            ParamDef {
                key: "is_iat_kelvin".to_string(),
                label: "IAT in Kelvin:".to_string(),
                param_type: ParamType::Boolean,
                tooltip: Some("Check if your IAT channel is already in Kelvin (rare). Usually °C.".to_string()),
            },
        ],
        "injector_duty_cycle" => vec![
            ParamDef {
                key: "pulse_width_channel".to_string(),
                label: "Pulse Width (ms):".to_string(),
                param_type: ParamType::Channel,
                tooltip: Some("Injector pulse width in milliseconds. Common names: IPW, Inj PW, Pulse Width, Fuel PW, Inj DC".to_string()),
            },
            ParamDef {
                key: "rpm_channel".to_string(),
                label: "RPM:".to_string(),
                param_type: ParamType::Channel,
                tooltip: Some("Engine RPM channel. Common names: RPM, Engine Speed, Engine RPM".to_string()),
            },
        ],
        "lambda_calculator" => vec![
            ParamDef {
                key: "afr_channel".to_string(),
                label: "AFR Channel:".to_string(),
                param_type: ParamType::Channel,
                tooltip: Some("Air-Fuel Ratio channel. Common names: AFR, O2, Wideband, A/F Ratio".to_string()),
            },
            ParamDef {
                key: "stoich_afr".to_string(),
                label: "Stoich AFR:".to_string(),
                param_type: ParamType::Float { min: 5.0, max: 20.0 },
                tooltip: Some("Stoichiometric AFR for your fuel. Gasoline: 14.7, E85: 9.8, E10: 14.1, Methanol: 6.4".to_string()),
            },
        ],

        _ => vec![],
    }
}

/// Check if required channels are available in the log
///
/// Checks both raw channel names and normalized display names to handle
/// cases where analyzer defaults (e.g., "AFR") need to match normalized names
/// (e.g., "Wideband O2 Overall" -> "AFR")
fn check_channels_available(
    config: &AnalyzerConfig,
    param_defs: &[ParamDef],
    channel_names: &[String],
    channel_display_names: &[String],
) -> bool {
    for param in param_defs {
        if matches!(param.param_type, ParamType::Channel) {
            if let Some(ch) = config.parameters.get(&param.key) {
                if !ch.is_empty() {
                    // Check if configured channel matches raw name OR display name
                    let found_in_raw = channel_names
                        .iter()
                        .any(|name| name.eq_ignore_ascii_case(ch));
                    let found_in_display = channel_display_names
                        .iter()
                        .any(|name| name.eq_ignore_ascii_case(ch));

                    if !found_in_raw && !found_in_display {
                        return false;
                    }
                }
            }
        }
    }
    true
}
