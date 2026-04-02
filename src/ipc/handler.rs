//! IPC command handler - processes commands from the MCP server
//!
//! This module contains the logic for handling IPC commands and generating responses.

use std::path::PathBuf;

use crate::app::UltraLogApp;
use crate::computed::{ComputedChannel, ComputedChannelTemplate};
use crate::expression;
use crate::state::ActiveTool;

use super::commands::*;

impl UltraLogApp {
    /// Handle an incoming IPC command and return a response
    pub fn handle_ipc_command(&mut self, command: IpcCommand) -> IpcResponse {
        match command {
            IpcCommand::Ping => IpcResponse::ok_with_data(ResponseData::Pong),

            IpcCommand::GetState => self.handle_get_state(),

            IpcCommand::LoadFile { path } => self.handle_load_file(path),

            IpcCommand::CloseFile { file_id } => self.handle_close_file(&file_id),

            IpcCommand::ListChannels { file_id } => self.handle_list_channels(&file_id),

            IpcCommand::GetChannelData {
                file_id,
                channel_name,
                time_range,
            } => self.handle_get_channel_data(&file_id, &channel_name, time_range),

            IpcCommand::GetChannelStats {
                file_id,
                channel_name,
                time_range,
            } => self.handle_get_channel_stats(&file_id, &channel_name, time_range),

            IpcCommand::SelectChannel {
                file_id,
                channel_name,
            } => self.handle_select_channel(&file_id, &channel_name),

            IpcCommand::DeselectChannel {
                file_id,
                channel_name,
            } => self.handle_deselect_channel(&file_id, &channel_name),

            IpcCommand::DeselectAllChannels => self.handle_deselect_all_channels(),

            IpcCommand::CreateComputedChannel {
                name,
                formula,
                unit,
                description,
            } => self.handle_create_computed_channel(name, formula, unit, description),

            IpcCommand::DeleteComputedChannel { name } => {
                self.handle_delete_computed_channel(&name)
            }

            IpcCommand::ListComputedChannels => self.handle_list_computed_channels(),

            IpcCommand::EvaluateFormula {
                file_id,
                formula,
                time_range,
            } => self.handle_evaluate_formula(&file_id, &formula, time_range),

            IpcCommand::SetTimeRange { start, end } => self.handle_set_time_range(start, end),

            IpcCommand::SetCursor { time } => self.handle_set_cursor(time),

            IpcCommand::Play { speed } => self.handle_play(speed),

            IpcCommand::Pause => self.handle_pause(),

            IpcCommand::Stop => self.handle_stop(),

            IpcCommand::GetCursorValues { file_id } => self.handle_get_cursor_values(&file_id),

            IpcCommand::FindPeaks {
                file_id,
                channel_name,
                min_prominence,
            } => self.handle_find_peaks(&file_id, &channel_name, min_prominence),

            IpcCommand::CorrelateChannels {
                file_id,
                channel_a,
                channel_b,
            } => self.handle_correlate_channels(&file_id, &channel_a, &channel_b),

            IpcCommand::ShowScatterPlot {
                file_id,
                x_channel,
                y_channel,
            } => self.handle_show_scatter_plot(&file_id, &x_channel, &y_channel),

            IpcCommand::ShowChart => self.handle_show_chart(),
        }
    }

    // ========================================================================
    // Command Handlers
    // ========================================================================

    fn handle_get_state(&self) -> IpcResponse {
        let files: Vec<FileInfo> = self
            .files
            .iter()
            .enumerate()
            .map(|(idx, f)| self.file_to_info(idx, f))
            .collect();

        let active_file = self.active_tab.map(|t| self.tabs[t].file_index.to_string());

        let selected_channels: Vec<SelectedChannelInfo> = self
            .get_selected_channels()
            .iter()
            .map(|c| SelectedChannelInfo {
                file_id: c.file_index.to_string(),
                channel_name: c.channel.name(),
                color: format!(
                    "#{:02x}{:02x}{:02x}",
                    self.get_channel_color(c.color_index)[0],
                    self.get_channel_color(c.color_index)[1],
                    self.get_channel_color(c.color_index)[2]
                ),
            })
            .collect();

        let state = AppState {
            files,
            active_file,
            selected_channels,
            cursor_time: self.get_cursor_time(),
            visible_time_range: self.get_time_range(),
            is_playing: self.is_playing,
            view_mode: match self.active_tool {
                ActiveTool::LogViewer => "chart".to_string(),
                ActiveTool::ScatterPlot => "scatter".to_string(),
                ActiveTool::Histogram => "histogram".to_string(),
            },
        };

        IpcResponse::ok_with_data(ResponseData::State(state))
    }

    fn handle_load_file(&mut self, path: String) -> IpcResponse {
        let path_buf = PathBuf::from(&path);

        if !path_buf.exists() {
            return IpcResponse::error(format!("File not found: {}", path));
        }

        // Check if already loaded
        if let Some(idx) = self.files.iter().position(|f| f.path == path_buf) {
            let info = self.file_to_info(idx, &self.files[idx]);
            return IpcResponse::ok_with_data(ResponseData::FileLoaded(info));
        }

        // Start loading - this is async, so we need to return immediately
        // The file will be available on the next GetState call
        self.start_loading_file(path_buf);

        IpcResponse::ok_with_data(ResponseData::Ack)
    }

    fn handle_close_file(&mut self, file_id: &str) -> IpcResponse {
        match file_id.parse::<usize>() {
            Ok(idx) if idx < self.files.len() => {
                self.remove_file(idx);
                IpcResponse::ok()
            }
            _ => IpcResponse::error(format!("Invalid file ID: {}", file_id)),
        }
    }

    fn handle_list_channels(&self, file_id: &str) -> IpcResponse {
        let file_idx = match file_id.parse::<usize>() {
            Ok(idx) if idx < self.files.len() => idx,
            _ => return IpcResponse::error(format!("Invalid file ID: {}", file_id)),
        };

        let file = &self.files[file_idx];
        let mut channels: Vec<ChannelInfo> = file
            .log
            .channels
            .iter()
            .enumerate()
            .map(|(idx, c)| {
                let data = file.log.get_channel_data(idx);
                let (min_val, max_val) = if data.is_empty() {
                    (None, None)
                } else {
                    let min = data.iter().cloned().fold(f64::INFINITY, f64::min);
                    let max = data.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                    (Some(min), Some(max))
                };

                ChannelInfo {
                    name: c.name(),
                    unit: c.unit().to_string(),
                    channel_type: c.type_name(),
                    is_computed: false,
                    min_value: min_val,
                    max_value: max_val,
                }
            })
            .collect();

        // Add computed channels
        if let Some(computed) = self.file_computed_channels.get(&file_idx) {
            for c in computed {
                let (min_val, max_val) = if let Some(data) = &c.cached_data {
                    if data.is_empty() {
                        (None, None)
                    } else {
                        let min = data.iter().cloned().fold(f64::INFINITY, f64::min);
                        let max = data.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                        (Some(min), Some(max))
                    }
                } else {
                    (None, None)
                };

                channels.push(ChannelInfo {
                    name: c.name().to_string(),
                    unit: c.unit().to_string(),
                    channel_type: "Computed".to_string(),
                    is_computed: true,
                    min_value: min_val,
                    max_value: max_val,
                });
            }
        }

        IpcResponse::ok_with_data(ResponseData::Channels(channels))
    }

    fn handle_get_channel_data(
        &self,
        file_id: &str,
        channel_name: &str,
        time_range: Option<(f64, f64)>,
    ) -> IpcResponse {
        let file_idx = match file_id.parse::<usize>() {
            Ok(idx) if idx < self.files.len() => idx,
            _ => return IpcResponse::error(format!("Invalid file ID: {}", file_id)),
        };

        let file = &self.files[file_idx];

        // Find channel by name
        let channel_idx = file
            .log
            .channels
            .iter()
            .position(|c| c.name().eq_ignore_ascii_case(channel_name));

        let (times, values) = if let Some(idx) = channel_idx {
            let all_times = file.log.get_times_as_f64().to_vec();
            let all_values = file.log.get_channel_data(idx);
            self.filter_by_time_range(all_times, all_values, time_range)
        } else {
            // Check computed channels
            if let Some(computed) = self.file_computed_channels.get(&file_idx) {
                if let Some(c) = computed
                    .iter()
                    .find(|c| c.name().eq_ignore_ascii_case(channel_name))
                {
                    if let Some(data) = &c.cached_data {
                        let all_times = file.log.get_times_as_f64().to_vec();
                        self.filter_by_time_range(all_times, data.clone(), time_range)
                    } else {
                        return IpcResponse::error("Computed channel not evaluated yet");
                    }
                } else {
                    return IpcResponse::error(format!("Channel not found: {}", channel_name));
                }
            } else {
                return IpcResponse::error(format!("Channel not found: {}", channel_name));
            }
        };

        IpcResponse::ok_with_data(ResponseData::ChannelData { times, values })
    }

    fn handle_get_channel_stats(
        &self,
        file_id: &str,
        channel_name: &str,
        time_range: Option<(f64, f64)>,
    ) -> IpcResponse {
        // First get the data
        let data_response = self.handle_get_channel_data(file_id, channel_name, time_range);

        match data_response {
            IpcResponse::Ok(Some(ResponseData::ChannelData { times, values })) => {
                if values.is_empty() {
                    return IpcResponse::error("No data in range");
                }

                let stats = self.compute_stats(&times, &values);
                IpcResponse::ok_with_data(ResponseData::Stats(stats))
            }
            IpcResponse::Error { message } => IpcResponse::error(message),
            _ => IpcResponse::error("Unexpected response"),
        }
    }

    fn handle_select_channel(&mut self, file_id: &str, channel_name: &str) -> IpcResponse {
        let file_idx = match file_id.parse::<usize>() {
            Ok(idx) if idx < self.files.len() => idx,
            _ => return IpcResponse::error(format!("Invalid file ID: {}", file_id)),
        };

        // Ensure we have a tab for this file
        if self.tabs.iter().all(|t| t.file_index != file_idx) {
            self.switch_to_file_tab(file_idx);
        } else {
            // Switch to the existing tab
            if let Some(tab_idx) = self.tabs.iter().position(|t| t.file_index == file_idx) {
                self.active_tab = Some(tab_idx);
                self.selected_file = Some(file_idx);
            }
        }

        let file = &self.files[file_idx];

        // Find channel by name
        if let Some(idx) = file
            .log
            .channels
            .iter()
            .position(|c| c.name().eq_ignore_ascii_case(channel_name))
        {
            self.add_channel(file_idx, idx);
            IpcResponse::ok()
        } else {
            // Check computed channels
            if let Some(computed) = self.file_computed_channels.get(&file_idx) {
                if let Some(comp_idx) = computed
                    .iter()
                    .position(|c| c.name().eq_ignore_ascii_case(channel_name))
                {
                    let channel_idx = file.log.channels.len() + comp_idx;
                    self.add_channel(file_idx, channel_idx);
                    IpcResponse::ok()
                } else {
                    IpcResponse::error(format!("Channel not found: {}", channel_name))
                }
            } else {
                IpcResponse::error(format!("Channel not found: {}", channel_name))
            }
        }
    }

    fn handle_deselect_channel(&mut self, file_id: &str, channel_name: &str) -> IpcResponse {
        let file_idx = match file_id.parse::<usize>() {
            Ok(idx) if idx < self.files.len() => idx,
            _ => return IpcResponse::error(format!("Invalid file ID: {}", file_id)),
        };

        // Find the channel in selected channels
        if let Some(tab_idx) = self.active_tab {
            let tab = &self.tabs[tab_idx];
            if let Some(idx) = tab.selected_channels.iter().position(|c| {
                c.file_index == file_idx && c.channel.name().eq_ignore_ascii_case(channel_name)
            }) {
                self.remove_channel(idx);
                return IpcResponse::ok();
            }
        }

        IpcResponse::error(format!("Channel not selected: {}", channel_name))
    }

    fn handle_deselect_all_channels(&mut self) -> IpcResponse {
        if let Some(tab_idx) = self.active_tab {
            self.tabs[tab_idx].selected_channels.clear();
        }
        IpcResponse::ok()
    }

    fn handle_create_computed_channel(
        &mut self,
        name: String,
        formula: String,
        unit: String,
        description: Option<String>,
    ) -> IpcResponse {
        // Validate the formula
        let available_channels = self.get_available_channel_names();
        if let Err(e) = expression::validate_formula(&formula, &available_channels) {
            return IpcResponse::error(format!("Invalid formula: {}", e));
        }

        // Create the template
        let template = ComputedChannelTemplate::new(
            name.clone(),
            formula.clone(),
            unit,
            description.unwrap_or_default(),
        );

        // Add to library
        self.computed_library.add_template(template.clone());
        let _ = self.computed_library.save();

        // Create and add computed channel to active file
        let mut computed = ComputedChannel::from_template(template);

        // Evaluate it for the active file
        if let Some(tab_idx) = self.active_tab {
            let file_idx = self.tabs[tab_idx].file_index;
            if file_idx < self.files.len() {
                let file = &self.files[file_idx];

                // Build bindings
                let refs = expression::extract_channel_references(&formula);
                match expression::build_channel_bindings(&refs, &available_channels) {
                    Ok(bindings) => {
                        computed.channel_bindings = bindings.clone();

                        // Evaluate
                        match expression::evaluate_all_records(
                            &formula,
                            &bindings,
                            &file.log.data,
                            file.log.get_times_as_f64(),
                        ) {
                            Ok(values) => {
                                computed.cached_data = Some(values);
                            }
                            Err(e) => {
                                computed.error = Some(e);
                            }
                        }
                    }
                    Err(e) => {
                        computed.error = Some(e);
                    }
                }

                self.add_computed_channel(computed);
            }
        }

        IpcResponse::ok()
    }

    fn handle_delete_computed_channel(&mut self, name: &str) -> IpcResponse {
        // Remove from library
        if let Some(pos) = self
            .computed_library
            .templates
            .iter()
            .position(|t| t.name.eq_ignore_ascii_case(name))
        {
            self.computed_library.templates.remove(pos);
            let _ = self.computed_library.save();
        }

        // Remove from active file's computed channels
        if let Some(tab_idx) = self.active_tab {
            let file_idx = self.tabs[tab_idx].file_index;
            if let Some(computed) = self.file_computed_channels.get_mut(&file_idx) {
                if let Some(pos) = computed
                    .iter()
                    .position(|c| c.name().eq_ignore_ascii_case(name))
                {
                    computed.remove(pos);
                }
            }
        }

        IpcResponse::ok()
    }

    fn handle_list_computed_channels(&self) -> IpcResponse {
        let channels: Vec<ComputedChannelInfo> = self
            .computed_library
            .templates
            .iter()
            .map(|t| ComputedChannelInfo {
                id: t.id.clone(),
                name: t.name.clone(),
                formula: t.formula.clone(),
                unit: t.unit.clone(),
                description: t.description.clone(),
            })
            .collect();

        IpcResponse::ok_with_data(ResponseData::ComputedChannels(channels))
    }

    fn handle_evaluate_formula(
        &self,
        file_id: &str,
        formula: &str,
        time_range: Option<(f64, f64)>,
    ) -> IpcResponse {
        let file_idx = match file_id.parse::<usize>() {
            Ok(idx) if idx < self.files.len() => idx,
            _ => return IpcResponse::error(format!("Invalid file ID: {}", file_id)),
        };

        let file = &self.files[file_idx];
        let available_channels: Vec<String> = file.log.channels.iter().map(|c| c.name()).collect();

        // Validate formula
        if let Err(e) = expression::validate_formula(formula, &available_channels) {
            return IpcResponse::error(format!("Invalid formula: {}", e));
        }

        // Build bindings and evaluate
        let refs = expression::extract_channel_references(formula);
        let bindings = match expression::build_channel_bindings(&refs, &available_channels) {
            Ok(b) => b,
            Err(e) => return IpcResponse::error(e),
        };

        let all_values = match expression::evaluate_all_records(
            formula,
            &bindings,
            &file.log.data,
            file.log.get_times_as_f64(),
        ) {
            Ok(v) => v,
            Err(e) => return IpcResponse::error(e),
        };

        let all_times = file.log.get_times_as_f64().to_vec();
        let (times, values) = self.filter_by_time_range(all_times, all_values, time_range);

        let stats = self.compute_stats(&times, &values);

        IpcResponse::ok_with_data(ResponseData::FormulaResult {
            times,
            values,
            stats,
        })
    }

    fn handle_set_time_range(&mut self, start: f64, end: f64) -> IpcResponse {
        self.set_time_range(Some((start, end)));
        self.set_chart_interacted(true);
        IpcResponse::ok()
    }

    fn handle_set_cursor(&mut self, time: f64) -> IpcResponse {
        self.set_cursor_time(Some(time));
        let record = self.find_record_at_time(time);
        self.set_cursor_record(record);
        IpcResponse::ok()
    }

    fn handle_play(&mut self, speed: Option<f64>) -> IpcResponse {
        if let Some(s) = speed {
            self.playback_speed = s.clamp(0.25, 8.0);
        }
        self.is_playing = true;
        self.last_frame_time = Some(std::time::Instant::now());
        IpcResponse::ok()
    }

    fn handle_pause(&mut self) -> IpcResponse {
        self.is_playing = false;
        IpcResponse::ok()
    }

    fn handle_stop(&mut self) -> IpcResponse {
        self.is_playing = false;
        if let Some((min, _)) = self.get_time_range() {
            self.set_cursor_time(Some(min));
            self.set_cursor_record(Some(0));
        }
        IpcResponse::ok()
    }

    fn handle_get_cursor_values(&self, file_id: &str) -> IpcResponse {
        let file_idx = match file_id.parse::<usize>() {
            Ok(idx) if idx < self.files.len() => idx,
            _ => return IpcResponse::error(format!("Invalid file ID: {}", file_id)),
        };

        let cursor_record = match self.get_cursor_record() {
            Some(r) => r,
            None => return IpcResponse::error("No cursor position set"),
        };

        let file = &self.files[file_idx];
        let mut values = Vec::new();

        for (idx, channel) in file.log.channels.iter().enumerate() {
            if let Some(value) = self.get_value_at_record(file_idx, idx, cursor_record) {
                values.push(CursorValue {
                    channel_name: channel.name(),
                    value,
                    unit: channel.unit().to_string(),
                });
            }
        }

        IpcResponse::ok_with_data(ResponseData::CursorValues(values))
    }

    fn handle_find_peaks(
        &self,
        file_id: &str,
        channel_name: &str,
        min_prominence: Option<f64>,
    ) -> IpcResponse {
        let data_response = self.handle_get_channel_data(file_id, channel_name, None);

        match data_response {
            IpcResponse::Ok(Some(ResponseData::ChannelData { times, values })) => {
                let peaks = self.find_peaks_in_data(&times, &values, min_prominence.unwrap_or(0.1));
                IpcResponse::ok_with_data(ResponseData::Peaks(peaks))
            }
            IpcResponse::Error { message } => IpcResponse::error(message),
            _ => IpcResponse::error("Unexpected response"),
        }
    }

    fn handle_correlate_channels(
        &self,
        file_id: &str,
        channel_a: &str,
        channel_b: &str,
    ) -> IpcResponse {
        let data_a = self.handle_get_channel_data(file_id, channel_a, None);
        let data_b = self.handle_get_channel_data(file_id, channel_b, None);

        match (data_a, data_b) {
            (
                IpcResponse::Ok(Some(ResponseData::ChannelData { values: a, .. })),
                IpcResponse::Ok(Some(ResponseData::ChannelData { values: b, .. })),
            ) => {
                if a.len() != b.len() || a.is_empty() {
                    return IpcResponse::error("Channels have different lengths or are empty");
                }

                let coefficient = self.compute_correlation(&a, &b);
                let interpretation = self.interpret_correlation(coefficient);

                IpcResponse::ok_with_data(ResponseData::Correlation {
                    coefficient,
                    interpretation,
                })
            }
            (IpcResponse::Error { message }, _) | (_, IpcResponse::Error { message }) => {
                IpcResponse::error(message)
            }
            _ => IpcResponse::error("Unexpected response"),
        }
    }

    fn handle_show_scatter_plot(
        &mut self,
        file_id: &str,
        x_channel: &str,
        y_channel: &str,
    ) -> IpcResponse {
        let file_idx = match file_id.parse::<usize>() {
            Ok(idx) if idx < self.files.len() => idx,
            _ => return IpcResponse::error(format!("Invalid file ID: {}", file_id)),
        };

        // Find channel indices first (while we only have immutable borrow)
        let file = &self.files[file_idx];
        let x_idx = file
            .log
            .channels
            .iter()
            .position(|c| c.name().eq_ignore_ascii_case(x_channel));
        let y_idx = file
            .log
            .channels
            .iter()
            .position(|c| c.name().eq_ignore_ascii_case(y_channel));

        // Switch to scatter plot view
        self.active_tool = ActiveTool::ScatterPlot;

        // Configure the scatter plot (now we can get mutable borrow)
        if let Some(state) = self.get_scatter_plot_state_mut() {
            if let (Some(x), Some(y)) = (x_idx, y_idx) {
                state.left.x_channel = Some(x);
                state.left.y_channel = Some(y);
            }
        }

        IpcResponse::ok()
    }

    fn handle_show_chart(&mut self) -> IpcResponse {
        self.active_tool = ActiveTool::LogViewer;
        IpcResponse::ok()
    }

    // ========================================================================
    // Helper Functions
    // ========================================================================

    fn file_to_info(&self, idx: usize, file: &crate::state::LoadedFile) -> FileInfo {
        let times = file.log.get_times_as_f64();
        let duration = if times.len() >= 2 {
            times.last().unwrap_or(&0.0) - times.first().unwrap_or(&0.0)
        } else {
            0.0
        };

        let sample_rate = if duration > 0.0 && times.len() > 1 {
            (times.len() - 1) as f64 / duration
        } else {
            0.0
        };

        FileInfo {
            id: idx.to_string(),
            path: file.path.to_string_lossy().to_string(),
            name: file.name.clone(),
            ecu_type: file.ecu_type.name().to_string(),
            channel_count: file.log.channels.len(),
            record_count: file.log.data.len(),
            duration,
            sample_rate,
        }
    }

    fn filter_by_time_range(
        &self,
        times: Vec<f64>,
        values: Vec<f64>,
        time_range: Option<(f64, f64)>,
    ) -> (Vec<f64>, Vec<f64>) {
        if let Some((start, end)) = time_range {
            let filtered: Vec<(f64, f64)> = times
                .into_iter()
                .zip(values)
                .filter(|(t, _)| *t >= start && *t <= end)
                .collect();

            let times: Vec<f64> = filtered.iter().map(|(t, _)| *t).collect();
            let values: Vec<f64> = filtered.iter().map(|(_, v)| *v).collect();
            (times, values)
        } else {
            (times, values)
        }
    }

    fn compute_stats(&self, times: &[f64], values: &[f64]) -> ChannelStats {
        if values.is_empty() {
            return ChannelStats {
                min: 0.0,
                max: 0.0,
                mean: 0.0,
                std_dev: 0.0,
                median: 0.0,
                count: 0,
                min_time: 0.0,
                max_time: 0.0,
            };
        }

        let mut min = f64::INFINITY;
        let mut max = f64::NEG_INFINITY;
        let mut min_time = 0.0;
        let mut max_time = 0.0;
        let mut sum = 0.0;

        for (i, &v) in values.iter().enumerate() {
            if v < min {
                min = v;
                min_time = times.get(i).copied().unwrap_or(0.0);
            }
            if v > max {
                max = v;
                max_time = times.get(i).copied().unwrap_or(0.0);
            }
            sum += v;
        }

        let mean = sum / values.len() as f64;

        let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;
        let std_dev = variance.sqrt();

        let mut sorted = values.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let median = if sorted.len().is_multiple_of(2) {
            (sorted[sorted.len() / 2 - 1] + sorted[sorted.len() / 2]) / 2.0
        } else {
            sorted[sorted.len() / 2]
        };

        ChannelStats {
            min,
            max,
            mean,
            std_dev,
            median,
            count: values.len(),
            min_time,
            max_time,
        }
    }

    fn find_peaks_in_data(&self, times: &[f64], values: &[f64], min_prominence: f64) -> Vec<Peak> {
        let mut peaks = Vec::new();

        if values.len() < 3 {
            return peaks;
        }

        // Simple peak detection: local maxima
        for i in 1..values.len() - 1 {
            if values[i] > values[i - 1] && values[i] > values[i + 1] {
                // Calculate prominence (height above surrounding valleys)
                let left_min = values[..i]
                    .iter()
                    .rev()
                    .take(10)
                    .cloned()
                    .fold(f64::INFINITY, f64::min);
                let right_min = values[i + 1..]
                    .iter()
                    .take(10)
                    .cloned()
                    .fold(f64::INFINITY, f64::min);
                let prominence = values[i] - left_min.max(right_min);

                if prominence >= min_prominence {
                    peaks.push(Peak {
                        time: times[i],
                        value: values[i],
                        prominence,
                    });
                }
            }
        }

        peaks
    }

    fn compute_correlation(&self, a: &[f64], b: &[f64]) -> f64 {
        let n = a.len() as f64;
        let mean_a = a.iter().sum::<f64>() / n;
        let mean_b = b.iter().sum::<f64>() / n;

        let mut cov = 0.0;
        let mut var_a = 0.0;
        let mut var_b = 0.0;

        for (ai, bi) in a.iter().zip(b.iter()) {
            let da = ai - mean_a;
            let db = bi - mean_b;
            cov += da * db;
            var_a += da * da;
            var_b += db * db;
        }

        if var_a == 0.0 || var_b == 0.0 {
            return 0.0;
        }

        cov / (var_a.sqrt() * var_b.sqrt())
    }

    fn interpret_correlation(&self, r: f64) -> String {
        let abs_r = r.abs();
        let strength = if abs_r >= 0.9 {
            "very strong"
        } else if abs_r >= 0.7 {
            "strong"
        } else if abs_r >= 0.5 {
            "moderate"
        } else if abs_r >= 0.3 {
            "weak"
        } else {
            "very weak or no"
        };

        let direction = if r > 0.0 { "positive" } else { "negative" };

        format!(
            "{} {} correlation (r={:.3})",
            strength.chars().next().unwrap().to_uppercase().to_string() + &strength[1..],
            direction,
            r
        )
    }
}
