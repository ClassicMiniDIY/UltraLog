//! Table generator tools: lambda delay (#4) and acceleration enrichment (#3)
//! tables mined from the loaded logs.
//!
//! Each generator is an `ActiveTool` (`ActiveTool::LambdaDelay`,
//! `ActiveTool::AccelEnrich`), split the way Histogram is:
//!
//! - **Tool Properties panel** (`render_table_tool_properties`) - channel roles
//!   (auto-suggested, ⚠ on ambiguity), load axis, axes, parameters, and
//!   *Run / Add current file*.
//! - **Central panel** (`render_table_tool_view`) - Viridis heatmap with value
//!   text and a count badge coloured by confidence; hover for the cell tooltip;
//!   click for the event inspector with jump-to-time; toolbar for removing
//!   logs, measure, unit, export. With no accepted events it shows the run
//!   report's rejection breakdown so threshold tuning is guided.
//!
//! State is **app-level, not per-tab**: the accumulators are multi-log by
//! design, so switching tabs only changes which file *Run* reads.

use std::collections::HashMap;

use eframe::egui;
use rust_i18n::t;

use crate::analysis::tables::binning::Confidence;
use crate::analysis::tables::channel_map::Candidate;
use crate::analysis::tables::export::{DelayUnit, ExportOptions, to_clipboard_tsv, to_csv};
use crate::analysis::tables::{
    AxisSpec, ChannelMapping, ChannelRole, GeneratorContext, GeneratorKind, LoadKind, RunReport,
    TableAccumulator, TableAnalyzer, TableParamKind, suggest_mapping,
};
use crate::analytics;
use crate::app::UltraLogApp;
use crate::colormap::{Colormap, sample};
use crate::ui::histogram::get_aaa_text_color;

/// Per-generator setup that is re-derived per log.
#[derive(Clone)]
struct MappingState {
    /// Load nonce of the file the mapping was suggested for.
    load_id: u64,
    mapping: ChannelMapping,
    ambiguous: Vec<ChannelRole>,
    candidates: HashMap<ChannelRole, Vec<Candidate>>,
    axes: (AxisSpec, AxisSpec),
    x_text: String,
    y_text: String,
}

/// All table-generator window state, held on `UltraLogApp`.
pub struct TableGeneratorState {
    generators: HashMap<GeneratorKind, Box<dyn TableAnalyzer>>,
    mappings: HashMap<GeneratorKind, MappingState>,
    pub accumulators: HashMap<GeneratorKind, TableAccumulator>,
    last_report: HashMap<GeneratorKind, RunReport>,
    last_error: Option<String>,
    measure: HashMap<GeneratorKind, usize>,
    selected_cell: Option<(usize, usize)>,
    pub export: ExportOptions,
    show_warnings: bool,
}

impl Default for TableGeneratorState {
    fn default() -> Self {
        let generators = GeneratorKind::ALL
            .into_iter()
            .map(|k| (k, k.create()))
            .collect();
        Self {
            generators,
            mappings: HashMap::new(),
            accumulators: HashMap::new(),
            last_report: HashMap::new(),
            last_error: None,
            measure: HashMap::new(),
            selected_cell: None,
            export: ExportOptions::default(),
            show_warnings: false,
        }
    }
}

impl TableGeneratorState {
    /// Status line for the tools panel, e.g. `2 logs, 14 events`.
    pub fn status(&self, kind: GeneratorKind) -> Option<String> {
        let acc = self.accumulators.get(&kind)?;
        if acc.logs.is_empty() {
            return None;
        }
        Some(
            t!(
                "table_gen.status",
                logs = acc.logs.len(),
                events = acc.accepted_count()
            )
            .to_string(),
        )
    }

    fn generator(&self, kind: GeneratorKind) -> &dyn TableAnalyzer {
        self.generators[&kind].as_ref()
    }

    /// Selected measure for `kind`, clamped to the accumulator's measure list.
    pub fn measure_index(&self, kind: GeneratorKind) -> usize {
        let n = self
            .accumulators
            .get(&kind)
            .map(|a| a.measures.len())
            .unwrap_or(1);
        self.measure
            .get(&kind)
            .copied()
            .unwrap_or(0)
            .min(n.saturating_sub(1))
    }

    /// The accumulator for `kind` when it holds at least one log.
    pub fn table(&self, kind: GeneratorKind) -> Option<&TableAccumulator> {
        self.accumulators.get(&kind).filter(|a| !a.logs.is_empty())
    }

    /// Forget the selected cell (called when the active tool changes).
    pub fn clear_selection(&mut self) {
        self.selected_cell = None;
    }
}

const CONFIDENCE_COLORS: [(Confidence, egui::Color32); 3] = [
    (Confidence::Low, egui::Color32::from_rgb(220, 80, 80)),
    (Confidence::Medium, egui::Color32::from_rgb(230, 170, 50)),
    (Confidence::High, egui::Color32::from_rgb(100, 200, 100)),
];

fn confidence_color(c: Confidence) -> egui::Color32 {
    CONFIDENCE_COLORS
        .iter()
        .find(|(k, _)| *k == c)
        .map(|(_, col)| *col)
        .unwrap_or(egui::Color32::GRAY)
}

fn fmt_value(v: f64, decimals: usize) -> String {
    if v.is_finite() {
        format!("{v:.decimals$}")
    } else {
        "–".to_string()
    }
}

impl UltraLogApp {
    /// Central-panel view for a table tool: results, or the empty state.
    pub fn render_table_tool_view(&mut self, ui: &mut egui::Ui) {
        let Some(kind) = self.active_tool.generator_kind() else {
            return;
        };
        let has_table = self.table_generator.table(kind).is_some();
        let file_index = self.selected_file.filter(|&i| i < self.files.len());

        if !has_table {
            ui.vertical_centered(|ui| {
                ui.add_space(30.0);
                if file_index.is_none() {
                    ui.label(
                        egui::RichText::new(t!("analysis.no_file_loaded"))
                            .color(egui::Color32::GRAY)
                            .size(16.0),
                    );
                    ui.label(
                        egui::RichText::new(t!("analysis.load_file_help"))
                            .color(egui::Color32::GRAY)
                            .small(),
                    );
                } else {
                    ui.label(
                        egui::RichText::new(self.table_generator.generator(kind).description())
                            .color(egui::Color32::GRAY)
                            .size(14.0),
                    );
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new(t!("table_gen.configure_hint"))
                            .color(egui::Color32::GRAY)
                            .small(),
                    );
                    if let Some(report) = self.table_generator.last_report.get(&kind) {
                        ui.add_space(12.0);
                        ui.label(
                            egui::RichText::new(format!(
                                "{}: {}",
                                report.log_name,
                                report.summary()
                            ))
                            .color(egui::Color32::from_rgb(230, 170, 50)),
                        );
                        for w in &report.warnings {
                            ui.label(
                                egui::RichText::new(format!("• {w}"))
                                    .small()
                                    .color(egui::Color32::GRAY),
                            );
                        }
                    }
                    if let Some(err) = &self.table_generator.last_error {
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new(err).color(egui::Color32::from_rgb(220, 80, 80)),
                        );
                    }
                }
                ui.add_space(30.0);
            });
            return;
        }

        egui::ScrollArea::vertical()
            .id_salt(format!("table_tool_view_{}", kind.id()))
            .show(ui, |ui| {
                self.render_table_results(ui, kind);
            });
    }

    /// Tool Properties panel content for a table tool: the setup controls.
    pub fn render_table_tool_properties(&mut self, ui: &mut egui::Ui) {
        let Some(kind) = self.active_tool.generator_kind() else {
            return;
        };
        let font_12 = self.scaled_font(12.0);
        let font_14 = self.scaled_font(14.0);

        ui.label(
            egui::RichText::new(self.table_generator.generator(kind).name())
                .size(font_14)
                .strong(),
        );
        ui.label(
            egui::RichText::new(self.table_generator.generator(kind).description())
                .size(font_12)
                .color(egui::Color32::GRAY),
        );
        ui.add_space(8.0);

        let file_index = self.selected_file.filter(|&i| i < self.files.len());
        let Some(fi) = file_index else {
            ui.label(
                egui::RichText::new(t!("table_gen.load_file_hint"))
                    .size(font_12)
                    .color(egui::Color32::GRAY),
            );
            return;
        };
        self.ensure_table_mapping(kind, fi);

        ui.label(
            egui::RichText::new(t!(
                "table_gen.setup_header",
                file = self.files[fi].name.as_str()
            ))
            .size(font_12)
            .color(egui::Color32::GRAY),
        );
        ui.add_space(4.0);
        egui::ScrollArea::vertical()
            .id_salt(format!("table_tool_props_{}", kind.id()))
            .show(ui, |ui| {
                self.render_table_setup(ui, kind, fi);
            });
    }

    /// Make sure a mapping exists for the current generator and file,
    /// re-suggesting when the file changed.
    fn ensure_table_mapping(&mut self, kind: GeneratorKind, file_index: usize) {
        let load_id = self.files[file_index].load_id;
        if self
            .table_generator
            .mappings
            .get(&kind)
            .is_some_and(|m| m.load_id == load_id)
        {
            return;
        }
        self.suggest_table_mapping(kind, file_index, true);
    }

    fn suggest_table_mapping(
        &mut self,
        kind: GeneratorKind,
        file_index: usize,
        keep_overrides: bool,
    ) {
        let file = &self.files[file_index];
        let generator = &self.table_generator.generators[&kind];
        let suggestion = suggest_mapping(
            &file.log,
            &generator.roles(),
            Some(&self.custom_normalizations),
        );
        let mut mapping = suggestion.mapping;
        if keep_overrides && let Some(prev) = self.table_generator.mappings.get(&kind) {
            // Keep explicit choices that still exist in this log.
            let names: Vec<String> = file.log.channels.iter().map(|c| c.name()).collect();
            for (role, name) in &prev.mapping.assignments {
                if names.iter().any(|n| n == name) {
                    mapping.assignments.insert(*role, name.clone());
                }
            }
            mapping.load_kind = prev.mapping.load_kind;
        }
        let axes = generator.default_axes(&file.log, &mapping);
        let state = MappingState {
            load_id: file.load_id,
            x_text: axes.0.edges_text(),
            y_text: axes.1.edges_text(),
            mapping,
            ambiguous: suggestion.ambiguous,
            candidates: suggestion.candidates,
            axes,
        };
        self.table_generator.mappings.insert(kind, state);
    }

    fn render_table_setup(&mut self, ui: &mut egui::Ui, kind: GeneratorKind, file_index: usize) {
        let channel_names: Vec<String> = self.files[file_index]
            .log
            .channels
            .iter()
            .map(|c| c.name())
            .collect();
        let roles = self.table_generator.generators[&kind].roles();
        let Some(state) = self.table_generator.mappings.get_mut(&kind) else {
            return;
        };

        // --- Channels -----------------------------------------------------
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(t!("table_gen.channels")).strong());
            if ui.small_button(t!("table_gen.auto_detect")).clicked() {
                // Handled below (needs &mut self).
                ui.ctx()
                    .data_mut(|d| d.insert_temp(egui::Id::new("table_gen_redetect"), true));
            }
        });
        egui::Grid::new(format!("table_gen_roles_{}", kind.id()))
            .num_columns(3)
            .spacing([8.0, 4.0])
            .show(ui, |ui| {
                for spec in &roles {
                    let label = if spec.required {
                        format!("{} *", spec.role.label())
                    } else {
                        spec.role.label().to_string()
                    };
                    ui.label(label).on_hover_text(spec.role.hint());
                    let current = state.mapping.get(spec.role).map(str::to_string);
                    let shown = current
                        .clone()
                        .unwrap_or_else(|| t!("table_gen.none").to_string());
                    let mut changed: Option<Option<String>> = None;
                    egui::ComboBox::from_id_salt(format!(
                        "table_gen_role_{}_{:?}",
                        kind.id(),
                        spec.role
                    ))
                    .width((ui.available_width() - 40.0).clamp(120.0, 320.0))
                    .selected_text(shown)
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(current.is_none(), t!("table_gen.none"))
                            .clicked()
                        {
                            changed = Some(None);
                        }
                        // Suggested candidates first, then everything else.
                        if let Some(cands) = state.candidates.get(&spec.role) {
                            for c in cands {
                                let text = format!("★ {}", c.channel);
                                if ui
                                    .selectable_label(current.as_deref() == Some(&c.channel), text)
                                    .clicked()
                                {
                                    changed = Some(Some(c.channel.clone()));
                                }
                            }
                            ui.separator();
                        }
                        for name in &channel_names {
                            if ui
                                .selectable_label(current.as_deref() == Some(name.as_str()), name)
                                .clicked()
                            {
                                changed = Some(Some(name.clone()));
                            }
                        }
                    });
                    if let Some(new) = changed {
                        state.mapping.set(spec.role, new);
                        state.ambiguous.retain(|r| *r != spec.role);
                    }
                    if state.ambiguous.contains(&spec.role) {
                        ui.label(
                            egui::RichText::new("⚠").color(egui::Color32::from_rgb(230, 170, 50)),
                        )
                        .on_hover_text(t!("table_gen.ambiguous_hint"));
                    } else if spec.required && !state.mapping.is_mapped(spec.role) {
                        ui.label(
                            egui::RichText::new("!").color(egui::Color32::from_rgb(220, 80, 80)),
                        )
                        .on_hover_text(t!("table_gen.required_hint"));
                    } else {
                        ui.label("");
                    }
                    ui.end_row();
                }
            });

        if kind == GeneratorKind::LambdaDelay {
            ui.horizontal(|ui| {
                ui.label(t!("table_gen.load_axis"));
                let mut lk = state.mapping.load_kind;
                ui.selectable_value(&mut lk, LoadKind::Map, LoadKind::Map.label());
                ui.selectable_value(&mut lk, LoadKind::Tps, LoadKind::Tps.label());
                if lk != state.mapping.load_kind {
                    state.mapping.load_kind = lk;
                    ui.ctx()
                        .data_mut(|d| d.insert_temp(egui::Id::new("table_gen_reaxis"), true));
                }
            });
        }

        // --- Axes ---------------------------------------------------------
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(t!("table_gen.axes")).strong());
            if ui.small_button(t!("table_gen.reset_axes")).clicked() {
                ui.ctx()
                    .data_mut(|d| d.insert_temp(egui::Id::new("table_gen_reaxis"), true));
            }
        });
        for (i, (text, axis)) in [
            (&mut state.x_text, &mut state.axes.0),
            (&mut state.y_text, &mut state.axes.1),
        ]
        .into_iter()
        .enumerate()
        {
            let valid = AxisSpec::parse_edges(text.as_str()).is_some();
            ui.horizontal(|ui| {
                ui.label(axis.header());
                ui.label(if valid {
                    egui::RichText::new(t!("table_gen.bins", n = axis.bins()))
                        .small()
                        .color(egui::Color32::GRAY)
                } else {
                    egui::RichText::new(t!("table_gen.invalid_axis"))
                        .small()
                        .color(egui::Color32::from_rgb(220, 80, 80))
                });
            });
            let resp = ui.add(
                egui::TextEdit::singleline(text)
                    .id_salt(format!("table_gen_axis_{}_{}", kind.id(), i))
                    .desired_width(f32::INFINITY),
            );
            if resp.changed()
                && let Some(edges) = AxisSpec::parse_edges(text.as_str())
            {
                *axis = AxisSpec::new(axis.label.clone(), axis.unit.clone(), edges);
            }
        }

        // --- Parameters ---------------------------------------------------
        ui.add_space(6.0);
        let generator = self
            .table_generator
            .generators
            .get_mut(&kind)
            .expect("generator registered");
        egui::CollapsingHeader::new(egui::RichText::new(t!("table_gen.parameters")).strong())
            .default_open(false)
            .show(ui, |ui| {
                let mut config = generator.get_config();
                let mut changed = false;
                egui::Grid::new(format!("table_gen_params_{}", kind.id()))
                    .num_columns(2)
                    .spacing([8.0, 4.0])
                    .show(ui, |ui| {
                        for p in generator.params() {
                            ui.label(p.label).on_hover_text(p.tooltip);
                            match p.kind {
                                TableParamKind::Float { min, max, speed } => {
                                    let mut v: f64 = config
                                        .parameters
                                        .get(p.key)
                                        .and_then(|s| s.parse().ok())
                                        .unwrap_or(min);
                                    if ui
                                        .add(
                                            egui::DragValue::new(&mut v)
                                                .range(min..=max)
                                                .speed(speed),
                                        )
                                        .changed()
                                    {
                                        config.parameters.insert(p.key.to_string(), v.to_string());
                                        changed = true;
                                    }
                                }
                                TableParamKind::Integer { min, max } => {
                                    let mut v: i64 = config
                                        .parameters
                                        .get(p.key)
                                        .and_then(|s| s.parse().ok())
                                        .unwrap_or(min);
                                    if ui
                                        .add(egui::DragValue::new(&mut v).range(min..=max))
                                        .changed()
                                    {
                                        config.parameters.insert(p.key.to_string(), v.to_string());
                                        changed = true;
                                    }
                                }
                                TableParamKind::Choice(choices) => {
                                    let current =
                                        config.parameters.get(p.key).cloned().unwrap_or_default();
                                    egui::ComboBox::from_id_salt(format!(
                                        "table_gen_param_{}_{}",
                                        kind.id(),
                                        p.key
                                    ))
                                    .selected_text(current.clone())
                                    .show_ui(ui, |ui| {
                                        for c in choices {
                                            if ui.selectable_label(current == *c, *c).clicked() {
                                                config
                                                    .parameters
                                                    .insert(p.key.to_string(), c.to_string());
                                                changed = true;
                                            }
                                        }
                                    });
                                }
                            }
                            ui.end_row();
                        }
                    });
                if changed {
                    generator.set_config(&config);
                }
            });

        // --- Run ----------------------------------------------------------
        ui.add_space(8.0);
        let missing = self
            .table_generator
            .mappings
            .get(&kind)
            .map(|m| m.mapping.missing_required(&roles))
            .unwrap_or_default();
        let axes_ok = self
            .table_generator
            .mappings
            .get(&kind)
            .is_some_and(|m| m.axes.0.is_valid() && m.axes.1.is_valid());
        let already = self
            .table_generator
            .accumulators
            .get(&kind)
            .is_some_and(|a| a.contains_log(self.files[file_index].load_id));
        let label = if already {
            t!("table_gen.rerun")
        } else if self
            .table_generator
            .accumulators
            .get(&kind)
            .is_some_and(|a| !a.logs.is_empty())
        {
            t!("table_gen.add_file")
        } else {
            t!("table_gen.run")
        };
        ui.horizontal(|ui| {
            let button =
                egui::Button::new(egui::RichText::new(label.as_ref()).color(egui::Color32::WHITE))
                    .fill(egui::Color32::from_rgb(113, 120, 78));
            if ui
                .add_enabled(missing.is_empty() && axes_ok, button)
                .clicked()
            {
                self.run_table_generator(kind, file_index);
            }
            if !missing.is_empty() {
                let names: Vec<&str> = missing.iter().map(|r| r.label()).collect();
                ui.label(
                    egui::RichText::new(t!("table_gen.missing_roles", roles = names.join(", ")))
                        .small()
                        .color(egui::Color32::from_rgb(220, 80, 80)),
                );
            }
        });
        if let Some(err) = &self.table_generator.last_error {
            ui.label(egui::RichText::new(err).color(egui::Color32::from_rgb(220, 80, 80)));
        }

        // Deferred actions that need `&mut self` outside the borrows above.
        let redetect = ui
            .ctx()
            .data_mut(|d| d.remove_temp::<bool>(egui::Id::new("table_gen_redetect")))
            .unwrap_or(false);
        let reaxis = ui
            .ctx()
            .data_mut(|d| d.remove_temp::<bool>(egui::Id::new("table_gen_reaxis")))
            .unwrap_or(false);
        if redetect {
            self.suggest_table_mapping(kind, file_index, false);
        } else if reaxis && let Some(state) = self.table_generator.mappings.get_mut(&kind) {
            let generator = &self.table_generator.generators[&kind];
            state.axes = generator.default_axes(&self.files[file_index].log, &state.mapping);
            state.x_text = state.axes.0.edges_text();
            state.y_text = state.axes.1.edges_text();
        }
    }

    /// Run the current generator on `file_index` and fold the events into
    /// the accumulator. Axes are frozen when the accumulator is created; a
    /// later file with different axes replaces the table.
    fn run_table_generator(&mut self, kind: GeneratorKind, file_index: usize) {
        let Some(state) = self.table_generator.mappings.get(&kind).cloned() else {
            return;
        };
        let file = &self.files[file_index];
        let delay_grid = if kind == GeneratorKind::AccelEnrich {
            self.table_generator
                .accumulators
                .get(&GeneratorKind::LambdaDelay)
                .filter(|a| !a.is_empty())
                .map(|a| a.grid(0))
        } else {
            None
        };
        let ctx = GeneratorContext {
            delay_table: delay_grid.as_ref(),
        };
        let generator = &self.table_generator.generators[&kind];
        let axes_match = self
            .table_generator
            .accumulators
            .get(&kind)
            .is_some_and(|a| a.axes == state.axes && !a.logs.is_empty());
        let result = generator.analyze(&file.log, &file.name, &state.mapping, &state.axes, &ctx);
        match result {
            Ok((mut events, report)) => {
                for e in &mut events {
                    e.log_id = file.load_id;
                }
                let acc = if axes_match {
                    self.table_generator
                        .accumulators
                        .get_mut(&kind)
                        .expect("checked")
                } else {
                    self.table_generator.accumulators.insert(
                        kind,
                        TableAccumulator::new(kind, state.axes.clone(), generator.measures()),
                    );
                    self.table_generator
                        .accumulators
                        .get_mut(&kind)
                        .expect("just inserted")
                };
                let summary = report.summary();
                acc.add_log(file.load_id, &file.name, events, report.clone());
                self.table_generator.last_report.insert(kind, report);
                self.table_generator.last_error = None;
                self.table_generator.selected_cell = None;
                self.show_toast(&summary);
            }
            Err(e) => {
                self.table_generator.last_error = Some(e.to_string());
                self.show_toast_error(&e.to_string());
            }
        }
    }

    fn render_table_results(&mut self, ui: &mut egui::Ui, kind: GeneratorKind) {
        let Some(acc) = self.table_generator.accumulators.get(&kind) else {
            return;
        };
        if acc.logs.is_empty() {
            return;
        }
        let measures = acc.measures.clone();
        let measure_idx = self.table_generator.measure_index(kind);
        let grid = acc.grid(measure_idx);
        let logs: Vec<(u64, String)> = acc.logs.iter().map(|l| (l.id, l.name.clone())).collect();
        let accepted = acc.accepted_count();
        let reports: Vec<RunReport> = acc.logs.iter().map(|l| l.report.clone()).collect();

        ui.separator();
        // --- Toolbar --------------------------------------------------------
        let mut remove_log: Option<u64> = None;
        let mut reset = false;
        let mut export_csv = false;
        let mut copy = false;
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(t!("table_gen.results")).strong());
            egui::ComboBox::from_id_salt(format!("table_gen_measure_{}", kind.id()))
                .selected_text(format!(
                    "{} ({})",
                    measures[measure_idx].label, measures[measure_idx].unit
                ))
                .show_ui(ui, |ui| {
                    for (i, m) in measures.iter().enumerate() {
                        if ui
                            .selectable_label(i == measure_idx, format!("{} ({})", m.label, m.unit))
                            .clicked()
                        {
                            self.table_generator.measure.insert(kind, i);
                        }
                    }
                });
            if ui.button(t!("table_gen.export_csv")).clicked() {
                export_csv = true;
            }
            if ui
                .button(t!("table_gen.copy"))
                .on_hover_text(t!("table_gen.copy_hint"))
                .clicked()
            {
                copy = true;
            }
            ui.checkbox(
                &mut self.table_generator.export.exclude_low,
                t!("table_gen.exclude_low"),
            )
            .on_hover_text(t!("table_gen.exclude_low_hint"));
            if kind == GeneratorKind::LambdaDelay {
                egui::ComboBox::from_id_salt("table_gen_delay_unit")
                    .selected_text(self.table_generator.export.delay_unit.label())
                    .show_ui(ui, |ui| {
                        for u in DelayUnit::ALL {
                            ui.selectable_value(
                                &mut self.table_generator.export.delay_unit,
                                u,
                                u.label(),
                            );
                        }
                    });
                if self.table_generator.export.delay_unit == DelayUnit::IgnitionEvents {
                    ui.label(t!("table_gen.cylinders"));
                    ui.add(
                        egui::DragValue::new(&mut self.table_generator.export.cylinders)
                            .range(1..=16),
                    );
                }
            }
            egui::ComboBox::from_id_salt(format!("table_gen_remove_{}", kind.id()))
                .selected_text(t!("table_gen.remove_log"))
                .show_ui(ui, |ui| {
                    for (id, name) in &logs {
                        if ui.selectable_label(false, name).clicked() {
                            remove_log = Some(*id);
                        }
                    }
                });
            if ui.button(t!("table_gen.reset")).clicked() {
                reset = true;
            }
        });

        // --- Coverage / report -------------------------------------------
        let (filled, high) = grid.coverage();
        ui.label(
            egui::RichText::new(t!(
                "table_gen.coverage",
                events = accepted,
                logs = logs.len(),
                filled = filled,
                total = grid.rows() * grid.cols(),
                high = high
            ))
            .small()
            .color(egui::Color32::GRAY),
        );
        for r in &reports {
            ui.label(egui::RichText::new(format!("{}: {}", r.log_name, r.summary())).small());
        }
        let warnings: Vec<String> = reports
            .iter()
            .flat_map(|r| r.warnings.iter().cloned())
            .collect();
        if !warnings.is_empty() {
            let header = t!("table_gen.notes", n = warnings.len());
            egui::CollapsingHeader::new(egui::RichText::new(header.as_ref()).small())
                .default_open(self.table_generator.show_warnings)
                .show(ui, |ui| {
                    for w in &warnings {
                        ui.label(
                            egui::RichText::new(format!("• {w}"))
                                .small()
                                .color(egui::Color32::GRAY),
                        );
                    }
                });
        }

        // --- Heatmap --------------------------------------------------------
        ui.add_space(4.0);
        let decimals = measures[measure_idx].decimals;
        if accepted == 0 {
            ui.label(
                egui::RichText::new(t!("table_gen.no_events"))
                    .color(egui::Color32::from_rgb(230, 170, 50)),
            );
        } else {
            self.render_table_heatmap(ui, &grid, decimals);
        }

        // --- Inspector ------------------------------------------------------
        if let Some((row, col)) = self.table_generator.selected_cell
            && let Some(cell) = grid.cell(row, col)
        {
            ui.add_space(6.0);
            let acc = &self.table_generator.accumulators[&kind];
            let events: Vec<_> = acc.events_in_cell(row, col).into_iter().cloned().collect();
            ui.label(
                egui::RichText::new(t!(
                    "table_gen.cell_title",
                    x = format!(
                        "{} {}",
                        grid.x_axis.label,
                        fmt_value(grid.x_axis.lower_edge(col), 0)
                    ),
                    y = format!(
                        "{} {}",
                        grid.y_axis.label,
                        fmt_value(grid.y_axis.lower_edge(row), 0)
                    ),
                    n = cell.count,
                    median = fmt_value(cell.median, decimals),
                    mad = fmt_value(cell.mad, decimals.max(1)),
                    confidence = cell.confidence.label()
                ))
                .strong(),
            );
            let mut jump: Option<(u64, f64)> = None;
            egui::ScrollArea::vertical()
                .max_height(160.0)
                .id_salt("table_gen_inspector")
                .show(ui, |ui| {
                    egui::Grid::new("table_gen_inspector_grid")
                        .striped(true)
                        .num_columns(5)
                        .show(ui, |ui| {
                            ui.label(
                                egui::RichText::new(t!("table_gen.col_time"))
                                    .small()
                                    .strong(),
                            );
                            ui.label(
                                egui::RichText::new(t!("table_gen.col_log"))
                                    .small()
                                    .strong(),
                            );
                            ui.label(
                                egui::RichText::new(measures[measure_idx].label)
                                    .small()
                                    .strong(),
                            );
                            ui.label(
                                egui::RichText::new(t!("table_gen.col_note"))
                                    .small()
                                    .strong(),
                            );
                            ui.label("");
                            ui.end_row();
                            for e in &events {
                                ui.label(egui::RichText::new(format!("{:.2} s", e.time)).small());
                                ui.label(egui::RichText::new(&e.log_name).small());
                                ui.label(
                                    egui::RichText::new(fmt_value(e.value(measure_idx), decimals))
                                        .small(),
                                );
                                ui.label(
                                    egui::RichText::new(&e.note)
                                        .small()
                                        .color(egui::Color32::GRAY),
                                );
                                let loaded = self.files.iter().any(|f| f.load_id == e.log_id);
                                if ui
                                    .add_enabled(
                                        loaded,
                                        egui::Button::new(t!("table_gen.jump")).small(),
                                    )
                                    .clicked()
                                {
                                    jump = Some((e.log_id, e.time));
                                }
                                ui.end_row();
                            }
                        });
                });
            if let Some((log_id, time)) = jump {
                self.jump_to_table_event(log_id, time);
            }
        }

        // --- Deferred actions ---------------------------------------------
        if let Some(id) = remove_log
            && let Some(acc) = self.table_generator.accumulators.get_mut(&kind)
        {
            acc.remove_log(id);
            self.table_generator.selected_cell = None;
        }
        if reset {
            if let Some(acc) = self.table_generator.accumulators.get_mut(&kind) {
                acc.reset();
            }
            self.table_generator.last_report.remove(&kind);
            self.table_generator.selected_cell = None;
        }
        if export_csv {
            self.export_table_csv(kind);
        }
        if copy {
            let acc = &self.table_generator.accumulators[&kind];
            let tsv = to_clipboard_tsv(acc, measure_idx, &self.table_generator.export);
            match arboard::Clipboard::new().and_then(|mut c| c.set_text(tsv)) {
                Ok(()) => self.show_toast_success(&t!("table_gen.copied")),
                Err(e) => self.show_toast_error(&format!("{}: {e}", t!("table_gen.copy_failed"))),
            }
        }
    }

    fn render_table_heatmap(
        &mut self,
        ui: &mut egui::Ui,
        grid: &crate::analysis::tables::TableGrid,
        decimals: usize,
    ) {
        let rows = grid.rows();
        let cols = grid.cols();
        if rows == 0 || cols == 0 {
            return;
        }
        let label_w = 64.0;
        let label_h = 22.0;
        let avail = ui.available_width().max(200.0);
        let cell_w = ((avail - label_w) / cols as f32).clamp(28.0, 110.0);
        let cell_h = (cell_w * 0.55).clamp(20.0, 40.0);
        let size = egui::vec2(
            label_w + cell_w * cols as f32,
            label_h + cell_h * rows as f32 + 18.0,
        );
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
        let painter = ui.painter_at(rect);
        let font = egui::FontId::proportional((cell_h * 0.42).clamp(9.0, 13.0));
        let small = egui::FontId::proportional(9.0);
        let text_color = ui.visuals().text_color();
        let (lo, hi) = grid.value_range().unwrap_or((0.0, 1.0));
        let range = if (hi - lo).abs() < f64::EPSILON {
            1.0
        } else {
            hi - lo
        };
        let origin = rect.min + egui::vec2(label_w, label_h);
        let hover = response.hover_pos();
        let mut hovered: Option<(usize, usize)> = None;

        // Column headers (X axis lower edges) and axis title.
        for c in 0..cols {
            let x = origin.x + cell_w * (c as f32 + 0.5);
            painter.text(
                egui::pos2(x, rect.min.y + label_h * 0.5),
                egui::Align2::CENTER_CENTER,
                fmt_value(grid.x_axis.lower_edge(c), 0),
                small.clone(),
                text_color,
            );
        }
        painter.text(
            egui::pos2(origin.x + cell_w * cols as f32 * 0.5, rect.max.y - 8.0),
            egui::Align2::CENTER_CENTER,
            grid.x_axis.header(),
            small.clone(),
            text_color,
        );
        painter.text(
            egui::pos2(rect.min.x + 2.0, rect.min.y + label_h * 0.5),
            egui::Align2::LEFT_CENTER,
            grid.y_axis.header(),
            small.clone(),
            text_color,
        );

        // Rows are drawn top-down with the highest Y bin at the top, the way
        // ECU tables are laid out.
        for r in 0..rows {
            let draw_row = rows - 1 - r;
            let y = origin.y + cell_h * draw_row as f32;
            painter.text(
                egui::pos2(rect.min.x + label_w - 6.0, y + cell_h * 0.5),
                egui::Align2::RIGHT_CENTER,
                fmt_value(grid.y_axis.lower_edge(r), 0),
                small.clone(),
                text_color,
            );
            for c in 0..cols {
                let cell_rect = egui::Rect::from_min_size(
                    egui::pos2(origin.x + cell_w * c as f32, y),
                    egui::vec2(cell_w, cell_h),
                );
                let Some(cell) = grid.cell(r, c) else {
                    continue;
                };
                if let Some(h) = hover
                    && cell_rect.contains(h)
                {
                    hovered = Some((r, c));
                }
                if cell.is_empty() {
                    painter.rect_filled(
                        cell_rect.shrink(0.5),
                        2.0,
                        egui::Color32::from_rgb(36, 36, 36),
                    );
                    continue;
                }
                let t = ((cell.median - lo) / range) as f32;
                let fill = sample(Colormap::Viridis, t);
                painter.rect_filled(cell_rect.shrink(0.5), 2.0, fill);
                let fg = get_aaa_text_color(fill);
                let value_text = fmt_value(cell.median, decimals);
                let text = if cell.confidence == Confidence::Low {
                    egui::RichText::new(value_text).italics()
                } else {
                    egui::RichText::new(value_text)
                };
                painter.text(
                    cell_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    text.text(),
                    font.clone(),
                    fg,
                );
                // Count badge, coloured by confidence.
                let badge = egui::Rect::from_min_size(
                    cell_rect.right_top() + egui::vec2(-14.0, 1.0),
                    egui::vec2(13.0, 9.0),
                );
                painter.rect_filled(badge, 2.0, confidence_color(cell.confidence));
                painter.text(
                    badge.center(),
                    egui::Align2::CENTER_CENTER,
                    cell.count.to_string(),
                    egui::FontId::proportional(7.5),
                    egui::Color32::BLACK,
                );
                if self.table_generator.selected_cell == Some((r, c)) {
                    painter.rect_stroke(
                        cell_rect.shrink(1.0),
                        2.0,
                        egui::Stroke::new(2.0, egui::Color32::WHITE),
                        egui::StrokeKind::Inside,
                    );
                }
            }
        }

        if let Some((r, c)) = hovered {
            if response.clicked() {
                self.table_generator.selected_cell =
                    if self.table_generator.selected_cell == Some((r, c)) {
                        None
                    } else {
                        Some((r, c))
                    };
            }
            if let Some(cell) = grid.cell(r, c) {
                let text = if cell.is_empty() {
                    t!("table_gen.empty_cell").to_string()
                } else {
                    t!(
                        "table_gen.cell_tooltip",
                        median = fmt_value(cell.median, decimals),
                        mad = fmt_value(cell.mad, decimals.max(1)),
                        n = cell.count,
                        confidence = cell.confidence.label()
                    )
                    .to_string()
                };
                response.clone().on_hover_text(text);
            }
        }
    }

    /// Activate the tab showing the event's log and jump the chart to its time.
    fn jump_to_table_event(&mut self, log_id: u64, time: f64) {
        let Some(file_index) = self.files.iter().position(|f| f.load_id == log_id) else {
            self.show_toast_warning(&t!("table_gen.log_unloaded"));
            return;
        };
        if let Some(tab_idx) = self.tabs.iter().position(|t| t.file_index == file_index) {
            self.active_tab = Some(tab_idx);
            self.selected_file = Some(file_index);
            self.set_active_tool(crate::state::ActiveTool::LogViewer);
            // Same sequence as the min/max jump buttons in channels.rs. The
            // record is looked up on the event's own file, not `files.first()`
            // as `find_record_at_time` does.
            let times = self.files[file_index].log.get_times_as_f64();
            let record = times
                .partition_point(|&t| t < time)
                .min(times.len().saturating_sub(1));
            self.set_cursor_time(Some(time));
            self.set_cursor_record(Some(record));
            self.set_jump_to_time(Some(time));
            self.is_playing = false;
            self.last_frame_time = None;
        }
    }

    fn export_table_csv(&mut self, kind: GeneratorKind) {
        let Some(acc) = self.table_generator.accumulators.get(&kind) else {
            return;
        };
        let measure_idx = self.table_generator.measure_index(kind);
        let generated = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();
        let csv = to_csv(acc, measure_idx, &self.table_generator.export, &generated);
        let Some(path) = rfd::FileDialog::new()
            .add_filter("CSV", &["csv"])
            .set_file_name(format!("ultralog_{}.csv", kind.id()))
            .save_file()
        else {
            return;
        };
        match std::fs::write(&path, csv) {
            Ok(()) => {
                analytics::track_export(kind.id());
                self.show_toast_success(&t!(
                    "table_gen.exported",
                    path = path.display().to_string()
                ));
            }
            Err(e) => self.show_toast_error(&format!("{}: {e}", t!("table_gen.export_failed"))),
        }
    }
}
