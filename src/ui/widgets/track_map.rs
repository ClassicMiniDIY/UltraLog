//! Track Map widget - GPS lat/lon visualization on a 2D map.
//!
//! Owns:
//! - GPS channel detection (lat/lon resolution via OECUA canonical IDs)
//! - Track polyline rendering (no tiles in v1; tile rendering slots in
//!   later via the `tiles` module)
//! - Cursor sync: hovering the track scrubs the chart cursor/timeline
//!   while playback is stopped; clicking seeks (and stops playback)
//! - Lap dropdown wired to [`crate::laps`]

use std::f64::consts::PI;
use std::sync::OnceLock;

use eframe::egui;
use rust_i18n::t;

use crate::app::UltraLogApp;
use crate::colormap::{Colormap, sample as colormap_sample};
use crate::laps::{
    GateParams, GpsCoordSpec, detect_laps, longitude_delta_degrees, sanitize_gps_track,
};
use crate::state::{ActiveTool, ColorSignature, GpsChannelCache, MapView, TrackCache};

use super::DataWidget;

const EARTH_RADIUS_M: f64 = 6_378_137.0;
const TRACK_STROKE_WIDTH: f32 = 2.0;
const FADED_STROKE_WIDTH: f32 = 1.0;
const TRACK_BG: egui::Color32 = egui::Color32::from_rgb(20, 22, 28);
const MAX_RENDER_POINTS: usize = 20_000;
type ColorCacheStatus = (Option<(usize, usize)>, Colormap, Option<(f64, f64)>);
static TILE_SOURCE: OnceLock<crate::tiles::TileSource> = OnceLock::new();

pub struct TrackMapWidget;

impl DataWidget for TrackMapWidget {
    fn id(&self) -> &'static str {
        "track_map"
    }

    fn title(&self, _app: &UltraLogApp) -> String {
        t!("track_map.title").to_string()
    }

    fn is_available(&self, app: &UltraLogApp) -> bool {
        // Pure query - background-work cancellation for unavailable states
        // is handled centrally by `maintain_tile_source` each frame.
        detect_gps_channels(app).is_some()
    }

    fn is_enabled(&self, app: &UltraLogApp) -> bool {
        app.active_tab
            .map(|i| app.tabs[i].data_panel_state.track_map.enabled)
            .unwrap_or(false)
    }

    fn set_enabled(&self, app: &mut UltraLogApp, enabled: bool) {
        if let Some(i) = app.active_tab {
            app.tabs[i].data_panel_state.track_map.enabled = enabled;
        }
        if !enabled {
            cancel_tile_requests();
        }
    }

    fn cancel_background_work(&self) {
        cancel_tile_requests();
    }

    fn render(&self, ui: &mut egui::Ui, app: &mut UltraLogApp) {
        let Some((lat_idx, lon_idx)) = detect_gps_channels(app) else {
            ui.weak(t!("track_map.no_gps"));
            return;
        };
        let Some(tab_idx) = app.active_tab else {
            return;
        };
        let file_idx = app.tabs[tab_idx].file_index;

        ensure_cache(app, tab_idx, file_idx, lat_idx, lon_idx);

        if app.tabs[tab_idx].data_panel_state.track_map.cache.is_none() {
            ui.weak(t!("track_map.no_gps"));
            return;
        }

        render_toolbar(ui, app, tab_idx);
        ui.add_space(2.0);
        render_canvas(ui, app, tab_idx, file_idx);
    }
}

/// Returns `(lat_channel_index, lon_channel_index)` from the active tab's
/// log if both GPS channels are present. Resolution prefers the OECUA
/// metadata map so any parser whose channel names map to canonical
/// `gps_latitude` / `gps_longitude` IDs is picked up automatically; falls
/// back to a small set of well-known column names so detection still works
/// when the API-refreshed adapter spec drops or renames the GPS entries.
pub fn detect_gps_channels(app: &UltraLogApp) -> Option<(usize, usize)> {
    let tab_idx = app.active_tab?;
    let file_idx = app.tabs[tab_idx].file_index;
    let file = app.files.get(file_idx)?;
    let spec_generation = crate::adapters::spec_generation();
    let cache = &app.tabs[tab_idx].data_panel_state.track_map.gps_channels;
    if let Some(cached) = cache.get()
        && cached.file_index == file_idx
        && cached.spec_generation == spec_generation
    {
        return cached.channels;
    }

    let channels = find_gps_channels(&file.log.channels);
    cache.set(Some(GpsChannelCache {
        file_index: file_idx,
        spec_generation,
        channels,
    }));
    channels
}

fn find_gps_channels(channels: &[crate::parsers::Channel]) -> Option<(usize, usize)> {
    let mut lat_idx = None;
    let mut lon_idx = None;
    for (i, ch) in channels.iter().enumerate() {
        let name = ch.name();
        let canonical =
            crate::adapters::registry::get_channel_metadata(&name).map(|m| m.canonical_id);
        // Exact alias matching stays an independent fallback: a refreshed
        // adapter spec that maps a known alias like "Latitude" to some
        // other (or renamed) canonical ID must not break GPS detection.
        let is_lat =
            canonical.as_deref() == Some("gps_latitude") || matches_gps_name(&name, GpsAxis::Lat);
        let is_lon =
            canonical.as_deref() == Some("gps_longitude") || matches_gps_name(&name, GpsAxis::Lon);
        if is_lat && lat_idx.is_none() {
            lat_idx = Some(i);
        }
        if is_lon && lon_idx.is_none() {
            lon_idx = Some(i);
        }
        if lat_idx.is_some() && lon_idx.is_some() {
            break;
        }
    }
    Some((lat_idx?, lon_idx?))
}

#[derive(Clone, Copy)]
enum GpsAxis {
    Lat,
    Lon,
}

/// Heuristic match for a GPS column when the OECUA registry doesn't
/// resolve it. Errs on the side of common motorsport / OBD / GPS-logger
/// column names rather than every conceivable string containing "lat".
fn matches_gps_name(name: &str, axis: GpsAxis) -> bool {
    let lc = name.trim().to_lowercase();
    let candidates: &[&str] = match axis {
        GpsAxis::Lat => &[
            "gps latitude",
            "gps_latitude",
            "gps lat",
            "gps_lat",
            "latitude",
            "lat",
        ],
        GpsAxis::Lon => &[
            "gps longitude",
            "gps_longitude",
            "gps long",
            "gps_long",
            "gps lon",
            "gps_lon",
            "longitude",
            "long",
            "lon",
        ],
    };
    candidates.iter().any(|c| lc == *c)
}

fn ensure_cache(
    app: &mut UltraLogApp,
    tab_idx: usize,
    file_idx: usize,
    lat_idx: usize,
    lon_idx: usize,
) {
    let needs_rebuild = match &app.tabs[tab_idx].data_panel_state.track_map.cache {
        Some(c) => c.file_index != file_idx || c.lat_idx != lat_idx || c.lon_idx != lon_idx,
        None => true,
    };
    if !needs_rebuild {
        return;
    }

    let mut lat_data = app.get_channel_data(file_idx, lat_idx);
    let mut lon_data = app.get_channel_data(file_idx, lon_idx);
    let n = lat_data.len().min(lon_data.len());
    if n == 0 {
        app.tabs[tab_idx].data_panel_state.track_map.cache = None;
        app.tabs[tab_idx].data_panel_state.track_map.laps = Vec::new();
        return;
    }

    let times = app.files[file_idx].log.get_times_as_f64();
    // Detect how this log encodes coordinates (NMEA DDM, scaled-integer
    // degrees, 0..360 longitude) and normalize to decimal degrees before
    // anything downstream touches the data. Identity for valid degrees.
    let coord_spec = GpsCoordSpec::detect(&lat_data, &lon_data);
    coord_spec.normalize_in_place(&mut lat_data, &mut lon_data);
    sanitize_gps_track(&mut lat_data, &mut lon_data, times);

    // Pick a mid-latitude that is finite. Falls back to the first finite
    // sample.
    let mid_lat_deg = lat_data
        .iter()
        .copied()
        .find(|v| v.is_finite())
        .unwrap_or(0.0);
    let mid_lat_rad = mid_lat_deg * PI / 180.0;
    let m_per_deg_lat = EARTH_RADIUS_M * PI / 180.0;
    let m_per_deg_lon = m_per_deg_lat * mid_lat_rad.cos();

    // Anchor the projection at the first finite sample so coordinates stay
    // small (better f32 precision after the f64 -> f32 cast).
    let anchor = lat_data
        .iter()
        .zip(lon_data.iter())
        .find(|(la, lo)| la.is_finite() && lo.is_finite())
        .map(|(la, lo)| (*la, *lo))
        .unwrap_or((0.0, 0.0));

    let mut points_m = Vec::with_capacity(n);
    let mut bbox_min = egui::vec2(f32::INFINITY, f32::INFINITY);
    let mut bbox_max = egui::vec2(f32::NEG_INFINITY, f32::NEG_INFINITY);
    let (mut west, mut east) = (f64::INFINITY, f64::NEG_INFINITY);
    let (mut south, mut north) = (f64::INFINITY, f64::NEG_INFINITY);
    for i in 0..n {
        let la = lat_data[i];
        let lo = lon_data[i];
        if la.is_finite() && lo.is_finite() {
            let continuous_lon = anchor.1 + longitude_delta_degrees(anchor.1, lo);
            let x = ((continuous_lon - anchor.1) * m_per_deg_lon) as f32;
            // Y axis points up in our local frame; screen flips it later.
            let y = ((la - anchor.0) * m_per_deg_lat) as f32;
            let p = egui::vec2(x, y);
            points_m.push(p);
            bbox_min.x = bbox_min.x.min(p.x);
            bbox_min.y = bbox_min.y.min(p.y);
            bbox_max.x = bbox_max.x.max(p.x);
            bbox_max.y = bbox_max.y.max(p.y);
            west = west.min(continuous_lon);
            east = east.max(continuous_lon);
            south = south.min(la);
            north = north.max(la);
        } else {
            // Push NaN sentinels so the polyline can break at gaps.
            points_m.push(egui::vec2(f32::NAN, f32::NAN));
        }
    }

    if !bbox_min.x.is_finite() {
        // No finite samples - degenerate cache.
        app.tabs[tab_idx].data_panel_state.track_map.cache = None;
        app.tabs[tab_idx].data_panel_state.track_map.laps = Vec::new();
        return;
    }

    let (west_px, north_py) = crate::tiles::lonlat_to_pixel(west, north, 0);
    let (east_px, south_py) = crate::tiles::lonlat_to_pixel(east, south, 0);
    let mercator_center = ((west_px + east_px) * 0.5, (north_py + south_py) * 0.5);
    let mercator_offsets_z0: Vec<egui::Vec2> = lat_data
        .iter()
        .zip(lon_data.iter())
        .map(|(lat, lon)| {
            if lat.is_finite() && lon.is_finite() {
                let continuous_lon = anchor.1 + longitude_delta_degrees(anchor.1, *lon);
                let (x, y) = crate::tiles::lonlat_to_pixel(continuous_lon, *lat, 0);
                egui::vec2(
                    (x - mercator_center.0) as f32,
                    (y - mercator_center.1) as f32,
                )
            } else {
                egui::vec2(f32::NAN, f32::NAN)
            }
        })
        .collect();
    let (render_indices, render_segments) = build_render_samples(&points_m);

    let cache = TrackCache {
        file_index: file_idx,
        lat_idx,
        lon_idx,
        coord_spec,
        points_m: points_m.into(),
        mercator_offsets_z0: mercator_offsets_z0.into(),
        render_indices: render_indices.into(),
        render_segments: render_segments.into(),
        bbox_min,
        bbox_max,
        lonlat_bbox: (west, east, south, north),
        color_cache: None,
        color_signature: None,
        color_range: None,
    };

    let laps = detect_laps(&lat_data, &lon_data, times, GateParams::default());

    // Map look settings persist globally - seed the per-tab state from the
    // live app preferences so re-opening a log behaves consistently.
    let default_provider = app.tile_provider;
    let default_tiles_enabled = app.tiles_enabled;
    let default_tile_opacity = app.tile_opacity;
    let default_tile_grayscale = app.tile_grayscale;
    let st = &mut app.tabs[tab_idx].data_panel_state.track_map;
    st.cache = Some(cache);
    st.laps = laps;
    st.view = Default::default();
    st.color_channel = None;
    st.color_min = None;
    st.color_max = None;
    st.selected_lap = None;
    st.tile_provider = default_provider;
    st.tiles_enabled = default_tiles_enabled;
    st.tile_opacity = default_tile_opacity;
    st.tile_grayscale = default_tile_grayscale;
}

fn build_render_samples(points: &[egui::Vec2]) -> (Vec<usize>, Vec<Option<u32>>) {
    if points.is_empty() {
        return (Vec::new(), Vec::new());
    }

    let sample_count = points.len().min(MAX_RENDER_POINTS);
    let indices: Vec<usize> = if points.len() <= MAX_RENDER_POINTS {
        (0..points.len()).collect()
    } else {
        let last_record = points.len() - 1;
        let last_sample = sample_count - 1;
        (0..sample_count)
            .map(|sample| ((sample as u128 * last_record as u128) / last_sample as u128) as usize)
            .collect()
    };

    let mut segments = Vec::with_capacity(indices.len());
    let mut sample_position = 0;
    let mut segment_id = 0u32;
    let mut in_segment = false;
    for (record, point) in points.iter().enumerate() {
        let valid = point.x.is_finite() && point.y.is_finite();
        if valid && !in_segment {
            segment_id = segment_id.saturating_add(1);
        }
        in_segment = valid;

        if indices.get(sample_position).copied() == Some(record) {
            segments.push(valid.then_some(segment_id));
            sample_position += 1;
            if sample_position == indices.len() {
                break;
            }
        }
    }

    debug_assert_eq!(indices.len(), segments.len());
    (indices, segments)
}

fn render_toolbar(ui: &mut egui::Ui, app: &mut UltraLogApp, tab_idx: usize) {
    ui.horizontal_wrapped(|ui| {
        // Color by selector
        let selected_channels = app.tabs[tab_idx].selected_channels.clone();
        let color_label = match app.tabs[tab_idx].data_panel_state.track_map.color_channel {
            Some(i) => selected_channels
                .get(i)
                .map(|c| c.channel.name())
                .unwrap_or_else(|| "–".to_string()),
            None => t!("track_map.solid").to_string(),
        };
        ui.label(t!("track_map.color_by"));
        egui::ComboBox::from_id_salt("track_map_color_by")
            .selected_text(color_label)
            .show_ui(ui, |ui| {
                let st = &mut app.tabs[tab_idx].data_panel_state.track_map;
                if ui
                    .selectable_label(st.color_channel.is_none(), t!("track_map.solid"))
                    .clicked()
                {
                    st.color_channel = None;
                }
                for (i, sc) in selected_channels.iter().enumerate() {
                    if ui
                        .selectable_label(st.color_channel == Some(i), sc.channel.name())
                        .clicked()
                    {
                        st.color_channel = Some(i);
                        st.color_min = None;
                        st.color_max = None;
                    }
                }
            });

        ui.separator();
        ui.label(t!("track_map.colormap"));
        let st = &mut app.tabs[tab_idx].data_panel_state.track_map;
        let cmap_label = match st.colormap {
            Colormap::Viridis => t!("track_map.colormap.viridis"),
            Colormap::Turbo => t!("track_map.colormap.turbo"),
        };
        egui::ComboBox::from_id_salt("track_map_colormap")
            .selected_text(cmap_label)
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut st.colormap,
                    Colormap::Viridis,
                    t!("track_map.colormap.viridis"),
                );
                ui.selectable_value(
                    &mut st.colormap,
                    Colormap::Turbo,
                    t!("track_map.colormap.turbo"),
                );
            });

        // Color range: editable min/max with a return-to-automatic reset.
        // `None` means automatic; a committed drag pins the value. Edits
        // are ordered (min <= max) so the color cache never sees an
        // inverted range.
        if st.color_channel.is_some() {
            ui.separator();
            ui.label(t!("track_map.range"));
            let (auto_min, auto_max) = st
                .cache
                .as_ref()
                .and_then(|cache| cache.color_range)
                .unwrap_or((0.0, 1.0));
            let speed = ((auto_max - auto_min).abs() / 200.0).max(0.01);
            let mut min_value = st.color_min.unwrap_or(auto_min);
            let mut max_value = st.color_max.unwrap_or(auto_max);
            if ui
                .add(egui::DragValue::new(&mut min_value).speed(speed))
                .changed()
            {
                st.color_min = Some(min_value.min(max_value));
            }
            if ui
                .add(egui::DragValue::new(&mut max_value).speed(speed))
                .changed()
            {
                st.color_max = Some(max_value.max(min_value));
            }
            if (st.color_min.is_some() || st.color_max.is_some())
                && ui.button(t!("track_map.range_auto")).clicked()
            {
                st.color_min = None;
                st.color_max = None;
            }
        }

        ui.separator();
        let lap_count = st.laps.len();
        if lap_count > 0 {
            ui.label(t!("track_map.lap"));
            let lap_label = match st.selected_lap {
                Some(i) => st
                    .laps
                    .get(i)
                    .map(localized_lap_label)
                    .unwrap_or_else(|| "–".to_string()),
                None => t!("track_map.lap_all").to_string(),
            };
            egui::ComboBox::from_id_salt("track_map_lap")
                .selected_text(lap_label)
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_label(st.selected_lap.is_none(), t!("track_map.lap_all"))
                        .clicked()
                    {
                        st.selected_lap = None;
                    }
                    for (i, lap) in st.laps.iter().enumerate() {
                        if ui
                            .selectable_label(st.selected_lap == Some(i), localized_lap_label(lap))
                            .clicked()
                        {
                            st.selected_lap = Some(i);
                        }
                    }
                });
            ui.separator();
        }

        if ui.button(t!("track_map.reset_view")).clicked() {
            st.view = Default::default();
        }

        ui.separator();
        // Snapshot the pre-edit toggle so the first-ever enable can show
        // the one-time privacy notice below.
        let prev_tiles_enabled = st.tiles_enabled;
        ui.checkbox(&mut st.tiles_enabled, t!("track_map.tiles.show"));
        if st.tiles_enabled {
            let provider_label = match st.tile_provider {
                crate::state::TileProviderId::EsriWorldImagery => "Esri World Imagery",
                crate::state::TileProviderId::OpenStreetMap => "OpenStreetMap",
            };
            egui::ComboBox::from_id_salt("track_map_tile_provider")
                .selected_text(provider_label)
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut st.tile_provider,
                        crate::state::TileProviderId::EsriWorldImagery,
                        "Esri World Imagery",
                    );
                    ui.selectable_value(
                        &mut st.tile_provider,
                        crate::state::TileProviderId::OpenStreetMap,
                        "OpenStreetMap",
                    );
                });

            ui.separator();
            ui.label(t!("track_map.tiles.opacity"));
            ui.add(
                egui::Slider::new(&mut st.tile_opacity, 0.1..=1.0)
                    .show_value(false)
                    .clamping(egui::SliderClamping::Always),
            );
            ui.checkbox(&mut st.tile_grayscale, t!("track_map.tiles.grayscale"));
        }

        let new_tiles_enabled = st.tiles_enabled;
        let new_tile_provider = st.tile_provider;
        let new_tile_grayscale = st.tile_grayscale;
        let new_tile_opacity = st.tile_opacity;

        // Mirror the per-tab look settings into the live app preferences.
        // Persistence happens on eframe's auto-save/shutdown cycle
        // (eframe::App::save), same as every other setting - never a disk
        // write on the UI thread here.
        app.tiles_enabled = new_tiles_enabled;
        app.tile_provider = new_tile_provider;
        app.tile_grayscale = new_tile_grayscale;
        app.tile_opacity = new_tile_opacity;

        if new_tiles_enabled && !prev_tiles_enabled && !app.tile_privacy_notice_seen {
            app.tile_privacy_notice_seen = true;
            let provider_label = match new_tile_provider {
                crate::state::TileProviderId::EsriWorldImagery => "Esri World Imagery",
                crate::state::TileProviderId::OpenStreetMap => "OpenStreetMap",
            };
            let notice =
                t!("track_map.tiles.privacy_notice", provider = provider_label).to_string();
            app.show_toast(&notice);
        }
        if !new_tiles_enabled {
            cancel_tile_requests();
        }
    });
}

fn localized_lap_label(lap: &crate::laps::LapInfo) -> String {
    format!(
        "{} {} ({})",
        t!("track_map.lap"),
        lap.index + 1,
        crate::laps::fmt_mmssms(lap.duration_s)
    )
}

fn zoom_around_pointer(view: &mut MapView, pointer_offset: egui::Vec2, requested_factor: f32) {
    let old_zoom = view.zoom.clamp(0.1, 50.0);
    let new_zoom = (old_zoom * requested_factor).clamp(0.1, 50.0);
    let effective_factor = new_zoom / old_zoom;
    view.pan_px = pointer_offset - (pointer_offset - view.pan_px) * effective_factor;
    view.zoom = new_zoom;
}

fn layout_tile_attribution(
    ui: &egui::Ui,
    provider_id: crate::state::TileProviderId,
    font_id: egui::FontId,
    color: egui::Color32,
    max_width: f32,
) -> std::sync::Arc<egui::Galley> {
    let provider = crate::tiles::provider_for(provider_id);
    ui.painter().layout(
        format!(
            "{}{}",
            t!("track_map.tiles.attribution_prefix"),
            provider.attribution()
        ),
        font_id,
        color,
        max_width.max(1.0),
    )
}

fn render_canvas(ui: &mut egui::Ui, app: &mut UltraLogApp, tab_idx: usize, file_idx: usize) {
    let (color_channel, colormap, color_range) = ensure_color_cache(app, tab_idx);
    let cache = match app.tabs[tab_idx].data_panel_state.track_map.cache.as_ref() {
        Some(c) => c.clone(),
        None => return,
    };

    // Claim the entire remaining vertical space as one rect. Footer
    // (legend / hint / attribution) is painted into a reserved band
    // *inside* this rect via the same `Painter`, so egui never sees any
    // extra widgets below the canvas - that means no auto `item_spacing`
    // can leak past the container and trigger the parent ScrollArea.
    let avail = ui.available_size();
    let tiles_enabled = app.tabs[tab_idx].data_panel_state.track_map.tiles_enabled;
    let tile_provider_id = app.tabs[tab_idx].data_panel_state.track_map.tile_provider;
    let line_h = ui.text_style_height(&egui::TextStyle::Body);
    let weak_color = ui.style().visuals.weak_text_color();
    let body_font = egui::TextStyle::Body.resolve(ui.style());
    let attribution_galley = tiles_enabled.then(|| {
        layout_tile_attribution(ui, tile_provider_id, body_font.clone(), weak_color, avail.x)
    });
    // Footer band: 4 px gap + legend/hint row + an optional wrapped
    // attribution block. Its measured height keeps every line inside the
    // reserved area at narrow panel widths.
    let attribution_band_h = attribution_galley
        .as_ref()
        .map_or(0.0, |galley| 4.0 + galley.size().y);
    let footer_band_h = 4.0 + line_h + attribution_band_h;

    let canvas_size = egui::vec2(avail.x, avail.y.max(160.0).max(footer_band_h + 80.0));
    let (rect, _) = ui.allocate_exact_size(canvas_size, egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, TRACK_BG);

    let map_rect = egui::Rect::from_min_max(
        rect.min,
        egui::pos2(rect.right(), rect.bottom() - footer_band_h),
    );
    let footer_rect =
        egui::Rect::from_min_max(egui::pos2(rect.left(), map_rect.bottom()), rect.max);
    let map_painter = painter.with_clip_rect(map_rect);
    let response = ui.interact(
        map_rect,
        ui.make_persistent_id(("track_map_canvas", app.tabs[tab_idx].id)),
        egui::Sense::click_and_drag(),
    );

    // Apply pan and zoom before projecting the bounded render set.
    if response.dragged() {
        let delta = ui.input(|input| input.pointer.delta());
        app.tabs[tab_idx].data_panel_state.track_map.view.pan_px += delta;
    }

    let scroll = ui.input(|input| input.smooth_scroll_delta.y);
    if response.hovered()
        && scroll.abs() > 0.0
        && let Some(pointer) = response.hover_pos()
    {
        let factor = (1.0_f32 + scroll * 0.0015).clamp(0.5, 2.0);
        let st = &mut app.tabs[tab_idx].data_panel_state.track_map;
        let before = pointer - map_rect.center();
        zoom_around_pointer(&mut st.view, before, factor);
        ui.input_mut(|input| input.smooth_scroll_delta.y = 0.0);
    }

    let inner = map_rect.shrink(8.0);
    let canvas_center = map_rect.center();
    let view_zoom = app.tabs[tab_idx]
        .data_panel_state
        .track_map
        .view
        .zoom
        .max(0.05);
    let pan = app.tabs[tab_idx].data_panel_state.track_map.view.pan_px;

    let projection = if tiles_enabled {
        let provider = crate::tiles::provider_for(tile_provider_id);
        let (west, east, south, north) = cache.lonlat_bbox;
        // Pick a base zoom from the static track bbox, then promote/demote
        // by `view_zoom` so a tile on screen stays close to its native 256px
        // even as the user zooms in or out (otherwise tiles either bloat
        // and blur, or trip the n_tiles safety cap and blank out).
        let base_z =
            crate::tiles::fit_zoom(west, east, south, north, inner.size(), provider.max_zoom());
        let zoom_steps = view_zoom.log2().round() as i32;
        let z = (base_z as i32 + zoom_steps).clamp(0, provider.max_zoom() as i32) as u8;
        // Effective scale at chosen z relative to view_zoom: 1 mercator px at
        // base_z = 2^(z - base_z) px at z, so the on-screen scale we apply
        // to mercator pixels is view_zoom / 2^(z - base_z).
        let z_scale = (z as i32 - base_z as i32) as f32;
        let render_scale = view_zoom / 2f32.powf(z_scale);
        let (west_px, north_py) = crate::tiles::lonlat_to_pixel(west, north, z);
        let (east_px, south_py) = crate::tiles::lonlat_to_pixel(east, south, z);
        let center_world = ((west_px + east_px) * 0.5, (north_py + south_py) * 0.5);

        // Draw tiles before polyline so they sit underneath.
        let tile_opacity = app.tabs[tab_idx]
            .data_panel_state
            .track_map
            .tile_opacity
            .clamp(0.0, 1.0);
        let tile_grayscale = app.tabs[tab_idx].data_panel_state.track_map.tile_grayscale;
        draw_tiles(
            ui,
            &map_painter,
            map_rect,
            tile_provider_id,
            z,
            center_world,
            render_scale,
            canvas_center,
            pan,
            tile_opacity,
            tile_grayscale,
            app.tile_cache_max_mb,
        );

        TrackProjection::Mercator {
            scale: 2f32.powi(base_z as i32) * view_zoom,
        }
    } else {
        let bbox_size = cache.bbox_max - cache.bbox_min;
        let bbox_w = bbox_size.x.max(1.0);
        let bbox_h = bbox_size.y.max(1.0);
        let fit_scale = (inner.width() / bbox_w)
            .min(inner.height() / bbox_h)
            .max(1e-6);
        let scale = fit_scale * view_zoom;
        let bbox_center = (cache.bbox_min + cache.bbox_max) * 0.5;
        TrackProjection::Local { scale, bbox_center }
    };

    let project_record = |record| projection.project(&cache, record, canvas_center, pan);
    let points_screen: Vec<ScreenPoint> = cache
        .render_indices
        .iter()
        .zip(cache.render_segments.iter())
        .map(|(record, segment_id)| ScreenPoint {
            record: *record,
            position: project_record(*record),
            segment_id: *segment_id,
        })
        .collect();

    let (lap_start, lap_end) = match app.tabs[tab_idx].data_panel_state.track_map.selected_lap {
        Some(i) => app.tabs[tab_idx]
            .data_panel_state
            .track_map
            .laps
            .get(i)
            .map(|l| (l.start_record, l.end_record))
            .unwrap_or((0, cache.points_m.len().saturating_sub(1))),
        None => (0, cache.points_m.len().saturating_sub(1)),
    };

    let colors_ref = cache.color_cache.as_deref();
    for pair in points_screen.windows(2) {
        if pair[0].segment_id.is_none() || pair[0].segment_id != pair[1].segment_id {
            continue;
        }
        let (Some(a), Some(b)) = (pair[0].position, pair[1].position) else {
            continue;
        };
        let record = pair[1].record;
        let in_window = record > lap_start && record <= lap_end;
        let color = match colors_ref {
            Some(colors) => colors
                .get(record.saturating_sub(1))
                .copied()
                .unwrap_or(egui::Color32::LIGHT_GRAY),
            None => egui::Color32::from_rgb(120, 200, 255),
        };
        let (color, width) = if in_window {
            (color, TRACK_STROKE_WIDTH)
        } else {
            (color.gamma_multiply(0.25), FADED_STROKE_WIDTH)
        };
        map_painter.line_segment([a, b], egui::Stroke::new(width, color));
    }

    if let Some(record) = app.tabs[tab_idx].cursor_record
        && let Some(position) = project_record(record)
    {
        map_painter.circle_stroke(position, 6.0, egui::Stroke::new(2.0, egui::Color32::WHITE));
        map_painter.circle_filled(position, 4.0, egui::Color32::from_rgb(255, 200, 0));
    }

    if response.clicked()
        && let Some(pointer) = response.interact_pointer_pos()
        && let Some(record) = nearest_record(&points_screen, pointer, &project_record)
    {
        let times = app.files[file_idx].log.get_times_as_f64();
        if let Some(time) = times.get(record).copied() {
            app.is_playing = false;
            app.last_frame_time = None;
            app.tabs[tab_idx].cursor_time = Some(time);
            app.tabs[tab_idx].cursor_record = Some(record);
            ui.ctx().request_repaint();
        }
    }

    if let Some(pointer) = response.hover_pos()
        && let Some(record) = nearest_record(&points_screen, pointer, &project_record)
    {
        let times = app.files[file_idx].log.get_times_as_f64();
        if let Some(time) = times.get(record).copied() {
            // Hover scrubs the chart cursor/timeline while playback is
            // stopped; during playback hover only shows the tooltip so it
            // cannot fight the advancing cursor. Click-to-seek (above)
            // remains the way to jump while playing.
            if !app.is_playing {
                app.tabs[tab_idx].cursor_time = Some(time);
                app.tabs[tab_idx].cursor_record = Some(record);
            }
            // Raw channel reads bypass the normalized cache, so convert
            // through the detected coordinate encoding for display.
            let latitude = app
                .get_value_at_record(file_idx, cache.lat_idx, record)
                .map(|value| cache.coord_spec.lat_to_degrees(value))
                .unwrap_or(f64::NAN);
            let longitude = app
                .get_value_at_record(file_idx, cache.lon_idx, record)
                .map(|value| cache.coord_spec.lon_to_degrees(value))
                .unwrap_or(f64::NAN);
            response.clone().on_hover_text(format!(
                "{}: {}\n{}: {:.5}, {:.5}",
                t!("track_map.tooltip.time"),
                crate::laps::fmt_mmssms(time),
                t!("track_map.tooltip.position"),
                latitude,
                longitude,
            ));
        }
    }

    // --- Legend + tile attribution (painter-based, fits in footer_rect) ---
    let row1_y = footer_rect.top() + 4.0 + line_h * 0.5;
    let row1_left = footer_rect.left();

    if color_channel.is_some() {
        // Painter-rendered legend: min text + gradient bar + max text on one row.
        let bar_w = 180.0;
        let bar_h = 12.0;
        let (minimum, maximum) = color_range.unwrap_or((0.0, 1.0));
        let min_text = format!("{minimum:.2}");
        let max_text = format!("{maximum:.2}");
        // Measure left label so the bar starts after it.
        let min_galley = painter.layout_no_wrap(min_text.clone(), body_font.clone(), weak_color);
        painter.text(
            egui::pos2(row1_left, row1_y),
            egui::Align2::LEFT_CENTER,
            &min_text,
            body_font.clone(),
            weak_color,
        );
        let bar_left = row1_left + min_galley.size().x + 6.0;
        let bar_top = row1_y - bar_h * 0.5;
        let bar_rect =
            egui::Rect::from_min_size(egui::pos2(bar_left, bar_top), egui::vec2(bar_w, bar_h));
        let steps = 64;
        for i in 0..steps {
            let t0 = i as f32 / steps as f32;
            let t1 = (i + 1) as f32 / steps as f32;
            let x0 = bar_rect.left() + t0 * bar_rect.width();
            let x1 = bar_rect.left() + t1 * bar_rect.width();
            let color = colormap_sample(colormap, (t0 + t1) * 0.5);
            painter.rect_filled(
                egui::Rect::from_min_max(
                    egui::pos2(x0, bar_rect.top()),
                    egui::pos2(x1, bar_rect.bottom()),
                ),
                0.0,
                color,
            );
        }
        painter.text(
            egui::pos2(bar_rect.right() + 6.0, row1_y),
            egui::Align2::LEFT_CENTER,
            &max_text,
            body_font.clone(),
            weak_color,
        );
    } else {
        painter.text(
            egui::pos2(row1_left, row1_y),
            egui::Align2::LEFT_CENTER,
            t!("track_map.legend.solid_hint"),
            body_font.clone(),
            weak_color,
        );
    }

    if let Some(attribution_galley) = attribution_galley {
        let row2_top = footer_rect.top() + 4.0 + line_h + 4.0;
        painter.galley(
            egui::pos2(footer_rect.left(), row2_top),
            attribution_galley,
            weak_color,
        );
    }
}

fn ensure_color_cache(app: &mut UltraLogApp, tab_idx: usize) -> ColorCacheStatus {
    let state = &app.tabs[tab_idx].data_panel_state.track_map;
    let colormap = state.colormap;
    let requested_min = state.color_min;
    let requested_max = state.color_max;
    let channel = state.color_channel.and_then(|slot| {
        app.tabs[tab_idx]
            .selected_channels
            .get(slot)
            .map(|selected| (selected.file_index, selected.channel_index))
    });
    let (data_address, data_length) = channel.map_or((0, 0), |(file_index, channel_index)| {
        let values = app.get_channel_data_ref(file_index, channel_index);
        (values.as_ptr() as usize, values.len())
    });
    let signature = ColorSignature {
        channel,
        data_address,
        data_length,
        colormap,
        requested_min,
        requested_max,
    };

    let needs_rebuild = app.tabs[tab_idx]
        .data_panel_state
        .track_map
        .cache
        .as_ref()
        .is_some_and(|cache| cache.color_signature != Some(signature));

    if needs_rebuild {
        let record_count = app.tabs[tab_idx]
            .data_panel_state
            .track_map
            .cache
            .as_ref()
            .map_or(0, |cache| cache.points_m.len());
        let rebuilt = channel.and_then(|(file_index, channel_index)| {
            let values = app.get_channel_data_ref(file_index, channel_index);
            let (mut automatic_min, mut automatic_max) = (f64::INFINITY, f64::NEG_INFINITY);
            for value in values
                .iter()
                .take(record_count)
                .filter(|value| value.is_finite())
            {
                automatic_min = automatic_min.min(*value);
                automatic_max = automatic_max.max(*value);
            }
            if !automatic_min.is_finite() || !automatic_max.is_finite() {
                return None;
            }

            let minimum = requested_min.unwrap_or(automatic_min);
            let maximum = requested_max.unwrap_or(automatic_max);
            let (minimum, maximum) = if maximum > minimum {
                (minimum, maximum)
            } else {
                (minimum - 0.5, maximum + 0.5)
            };
            let colors = build_color_cache(record_count, colormap, minimum, maximum, values);
            let colors: std::sync::Arc<[egui::Color32]> = colors.into();
            Some((colors, (minimum, maximum)))
        });

        if let Some(cache) = app.tabs[tab_idx].data_panel_state.track_map.cache.as_mut() {
            cache.color_cache = rebuilt.as_ref().map(|(colors, _)| colors.clone());
            cache.color_range = rebuilt.map(|(_, range)| range);
            cache.color_signature = Some(signature);
        }
    }

    let color_range = app.tabs[tab_idx]
        .data_panel_state
        .track_map
        .cache
        .as_ref()
        .and_then(|cache| cache.color_range);
    (channel, colormap, color_range)
}

fn build_color_cache(
    record_count: usize,
    colormap: Colormap,
    minimum: f64,
    maximum: f64,
    values: &[f64],
) -> Vec<egui::Color32> {
    let span = (maximum - minimum).max(f64::EPSILON);
    (0..record_count.saturating_sub(1))
        .map(|index| {
            let value = values.get(index + 1).copied().unwrap_or(f64::NAN);
            if value.is_finite() {
                let position = ((value - minimum) / span).clamp(0.0, 1.0) as f32;
                colormap_sample(colormap, position)
            } else {
                egui::Color32::DARK_GRAY
            }
        })
        .collect()
}

#[derive(Clone, Copy)]
enum TrackProjection {
    Local { scale: f32, bbox_center: egui::Vec2 },
    Mercator { scale: f32 },
}

impl TrackProjection {
    fn project(
        self,
        cache: &TrackCache,
        record: usize,
        canvas_center: egui::Pos2,
        pan: egui::Vec2,
    ) -> Option<egui::Pos2> {
        let offset = match self {
            Self::Local { scale, bbox_center } => {
                let point = *cache.points_m.get(record)?;
                if !point.x.is_finite() {
                    return None;
                }
                egui::vec2(
                    (point.x - bbox_center.x) * scale,
                    -(point.y - bbox_center.y) * scale,
                )
            }
            Self::Mercator { scale } => {
                let point = *cache.mercator_offsets_z0.get(record)?;
                if !point.x.is_finite() {
                    return None;
                }
                point * scale
            }
        };
        Some(canvas_center + pan + offset)
    }
}

#[derive(Clone, Copy)]
struct ScreenPoint {
    record: usize,
    position: Option<egui::Pos2>,
    segment_id: Option<u32>,
}

fn nearest_record(
    points_screen: &[ScreenPoint],
    pointer: egui::Pos2,
    project_record: &impl Fn(usize) -> Option<egui::Pos2>,
) -> Option<usize> {
    let mut best_sample = None;
    let mut best_d2 = f32::INFINITY;
    for (sample_index, point) in points_screen.iter().enumerate() {
        if let Some(position) = point.position {
            let dx = position.x - pointer.x;
            let dy = position.y - pointer.y;
            let d2 = dx * dx + dy * dy;
            if d2 < best_d2 {
                best_d2 = d2;
                best_sample = Some(sample_index);
            }
        }
    }

    let sample_index = best_sample?;
    let lower = sample_index
        .checked_sub(1)
        .and_then(|index| points_screen.get(index))
        .map_or(points_screen[sample_index].record, |point| point.record);
    let upper = points_screen
        .get(sample_index + 1)
        .map_or(points_screen[sample_index].record, |point| point.record);
    let mut best_record = Some(points_screen[sample_index].record);
    for record in lower..=upper {
        if let Some(position) = project_record(record) {
            let dx = position.x - pointer.x;
            let dy = position.y - pointer.y;
            let d2 = dx * dx + dy * dy;
            if d2 < best_d2 {
                best_d2 = d2;
                best_record = Some(record);
            }
        }
    }

    if best_d2 <= 32.0 * 32.0 {
        best_record
    } else {
        None
    }
}

/// Single global tile source. Lazy-initialized on first access.
fn tile_source(ctx: &egui::Context, disk_cache_max_mb: u32) -> &'static crate::tiles::TileSource {
    TILE_SOURCE.get_or_init(|| crate::tiles::TileSource::new(ctx.clone(), disk_cache_max_mb))
}

fn cancel_tile_requests() {
    if let Some(source) = TILE_SOURCE.get() {
        source.cancel_pending();
    }
}

/// Keep the shared tile source responsive even when the map widget is not
/// rendered. Workers request a repaint when a fetch completes, so polling from
/// the application loop prevents completed responses from accumulating while
/// another tool, a collapsed panel, or a hidden widget is active.
pub(crate) fn maintain_tile_source(ctx: &egui::Context, app: &UltraLogApp) {
    let Some(source) = TILE_SOURCE.get() else {
        return;
    };

    if !tile_rendering_is_active(app) {
        source.cancel_pending();
    }
    source.poll(ctx);
}

fn tile_rendering_is_active(app: &UltraLogApp) -> bool {
    let Some(tab_idx) = app.active_tab else {
        return false;
    };
    let Some(tab) = app.tabs.get(tab_idx) else {
        return false;
    };

    app.active_tool == ActiveTool::LogViewer
        && tab.data_panel_state.visible
        && tab.data_panel_state.track_map.enabled
        && tab.data_panel_state.track_map.tiles_enabled
        && tab.data_panel_state.track_map.cache.is_some()
        && !app.hidden_widgets.contains("track_map")
        && detect_gps_channels(app).is_some()
}

/// Paint Web Mercator tiles covering `rect`. The screen -> world mapping is
/// the same as the polyline projection: a Mercator pixel `p` maps to
/// `canvas_center + (p - center_world) * view_zoom + pan`.
///
/// `opacity` is applied as an alpha tint so the polyline stays at full
/// strength on top. `grayscale` selects the desaturated texture variant
/// from the tile cache.
#[allow(clippy::too_many_arguments)]
fn draw_tiles(
    ui: &egui::Ui,
    painter: &egui::Painter,
    rect: egui::Rect,
    provider_id: crate::state::TileProviderId,
    z: u8,
    center_world: (f64, f64),
    view_zoom: f32,
    canvas_center: egui::Pos2,
    pan: egui::Vec2,
    opacity: f32,
    grayscale: bool,
    disk_cache_max_mb: u32,
) {
    let src = tile_source(ui.ctx(), disk_cache_max_mb);

    let alpha = (opacity.clamp(0.0, 1.0) * 255.0).round() as u8;
    let tint = egui::Color32::from_white_alpha(alpha);

    // Inverse: world_px = canvas_center + (world - center_world) * view_zoom + pan.
    let view_scale = f64::from(view_zoom);
    let to_world = |screen: egui::Pos2| -> (f64, f64) {
        (
            center_world.0 + f64::from(screen.x - canvas_center.x - pan.x) / view_scale,
            center_world.1 + f64::from(screen.y - canvas_center.y - pan.y) / view_scale,
        )
    };

    let world_tl = to_world(rect.min);
    let world_br = to_world(rect.max);

    let tile_size_world = 256.0_f64;
    let tile_x_min = (world_tl.0 / tile_size_world).floor() as i64;
    let tile_x_max = (world_br.0 / tile_size_world).floor() as i64;
    let tile_y_min = (world_tl.1 / tile_size_world).floor() as i64;
    let tile_y_max = (world_br.1 / tile_size_world).floor() as i64;

    let max_tiles = 256; // safety cap so a degenerate zoom can't melt the GPU
    let n_tiles = ((tile_x_max - tile_x_min + 1) * (tile_y_max - tile_y_min + 1)).max(0);
    if n_tiles > max_tiles {
        src.cancel_pending();
        return;
    }

    let tile_center_x = (tile_x_min + tile_x_max) as f64 * 0.5;
    let tile_center_y = (tile_y_min + tile_y_max) as f64 * 0.5;
    let mut tiles = Vec::with_capacity(n_tiles as usize);
    let world_dim = (1u64 << z as u64) as i64;
    for ty in tile_y_min..=tile_y_max {
        for tx in tile_x_min..=tile_x_max {
            if ty < 0 || ty >= world_dim {
                continue;
            }
            let wx = tx as f64 * tile_size_world;
            let wy = ty as f64 * tile_size_world;
            let s_min = egui::pos2(
                canvas_center.x + ((wx - center_world.0) * view_scale) as f32 + pan.x,
                canvas_center.y + ((wy - center_world.1) * view_scale) as f32 + pan.y,
            );
            let s_max = egui::pos2(
                s_min.x + (tile_size_world * view_scale) as f32,
                s_min.y + (tile_size_world * view_scale) as f32,
            );
            let dst = egui::Rect::from_min_max(s_min, s_max);

            let key = crate::tiles::TileKey {
                provider: provider_id,
                z,
                x: tx.rem_euclid(world_dim) as u32,
                y: ty as u32,
            };
            let center_distance =
                (tx as f64 - tile_center_x).powi(2) + (ty as f64 - tile_center_y).powi(2);
            tiles.push((center_distance, key, dst));
        }
    }

    tiles.sort_by(|left, right| left.0.total_cmp(&right.0));
    let visible_keys: Vec<_> = tiles.iter().map(|(_, key, _)| *key).collect();
    src.set_visible_tiles(&visible_keys);
    src.poll(ui.ctx());

    for (_, key, dst) in tiles {
        if let Some(tex) = src.request(ui.ctx(), key, grayscale) {
            painter.image(
                tex.id(),
                dst,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                tint,
            );
        } else {
            // Loading placeholder: subtle outline so the user knows tiles
            // are arriving.
            painter.rect_stroke(
                dst,
                0.0,
                egui::Stroke::new(1.0, egui::Color32::from_rgb(40, 50, 60)),
                egui::StrokeKind::Inside,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::parsers::aim::AimChannel;
    use crate::parsers::types::Meta;
    use crate::parsers::{Channel, EcuType, Log, Value};
    use crate::state::{LoadedFile, Tab, TileProviderId};

    fn app_with_active_tile_map() -> UltraLogApp {
        let log = Log {
            meta: Meta::Empty,
            channels: vec![
                Channel::Aim(AimChannel {
                    name: "GPS Latitude".to_string(),
                    unit: "deg".to_string(),
                }),
                Channel::Aim(AimChannel {
                    name: "GPS Longitude".to_string(),
                    unit: "deg".to_string(),
                }),
            ],
            times: vec![0.0],
            data: vec![vec![Value::Float(43.1), Value::Float(131.9)]],
        };

        let mut app = UltraLogApp::default();
        app.files.push(LoadedFile::new(
            PathBuf::from("gps.xrk"),
            "gps.xrk".to_string(),
            EcuType::Aim,
            log,
        ));
        app.tabs.push(Tab::new(0, "gps.xrk".to_string()));
        app.active_tab = Some(0);
        ensure_cache(&mut app, 0, 0, 0, 1);
        app.tabs[0].data_panel_state.track_map.tiles_enabled = true;
        app
    }

    #[test]
    fn ensure_cache_normalizes_nmea_ddm_tracks_to_degrees() {
        // 48° 07.038' N, 11° 31.324' E in NMEA DDMM.mmmm packing.
        let log = Log {
            meta: Meta::Empty,
            channels: vec![
                Channel::Aim(AimChannel {
                    name: "GPS Latitude".to_string(),
                    unit: "deg".to_string(),
                }),
                Channel::Aim(AimChannel {
                    name: "GPS Longitude".to_string(),
                    unit: "deg".to_string(),
                }),
            ],
            times: vec![0.0, 1.0],
            data: vec![
                vec![Value::Float(4807.038), Value::Float(1131.324)],
                vec![Value::Float(4807.040), Value::Float(1131.326)],
            ],
        };

        let mut app = UltraLogApp::default();
        app.files.push(LoadedFile::new(
            PathBuf::from("ddm.csv"),
            "ddm.csv".to_string(),
            EcuType::Aim,
            log,
        ));
        app.tabs.push(Tab::new(0, "ddm.csv".to_string()));
        app.active_tab = Some(0);
        ensure_cache(&mut app, 0, 0, 0, 1);

        let cache = app.tabs[0]
            .data_panel_state
            .track_map
            .cache
            .as_ref()
            .expect("DDM track should build a cache");
        assert_eq!(
            cache.coord_spec.format,
            crate::laps::GpsCoordFormat::DegreesDecimalMinutes
        );
        let (west, east, south, north) = cache.lonlat_bbox;
        assert!((south - 48.1173).abs() < 1e-3, "south was {south}");
        assert!((north - 48.1173).abs() < 1e-3, "north was {north}");
        assert!((west - 11.5221).abs() < 1e-3, "west was {west}");
        assert!((east - 11.5221).abs() < 1e-3, "east was {east}");
        // The raw-value converters used by the hover tooltip agree.
        assert!((cache.coord_spec.lat_to_degrees(4807.038) - 48.1173).abs() < 1e-4);
        assert!((cache.coord_spec.lon_to_degrees(1131.324) - 11.5221).abs() < 1e-4);
    }

    #[test]
    fn tile_rendering_activity_tracks_every_visibility_gate() {
        let mut app = app_with_active_tile_map();
        assert!(tile_rendering_is_active(&app));

        app.active_tool = ActiveTool::ScatterPlot;
        assert!(!tile_rendering_is_active(&app));
        app.active_tool = ActiveTool::LogViewer;

        app.tabs[0].data_panel_state.visible = false;
        assert!(!tile_rendering_is_active(&app));
        app.tabs[0].data_panel_state.visible = true;

        app.tabs[0].data_panel_state.track_map.enabled = false;
        assert!(!tile_rendering_is_active(&app));
        app.tabs[0].data_panel_state.track_map.enabled = true;

        app.hidden_widgets.insert("track_map".to_string());
        assert!(!tile_rendering_is_active(&app));
        app.hidden_widgets.remove("track_map");

        app.tabs[0].data_panel_state.track_map.cache = None;
        assert!(!tile_rendering_is_active(&app));
    }

    #[test]
    fn tile_attribution_wraps_to_narrow_footer() {
        let ctx = egui::Context::default();
        let mut row_count = 0;
        let mut laid_out_width = f32::INFINITY;
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let max_width = 180.0;
            let galley = layout_tile_attribution(
                ui,
                TileProviderId::EsriWorldImagery,
                egui::TextStyle::Body.resolve(ui.style()),
                egui::Color32::WHITE,
                max_width,
            );
            row_count = galley.rows.len();
            laid_out_width = galley.size().x;
        });
        output.drop_without_applying_deltas();

        assert!(row_count > 1);
        assert!(laid_out_width <= 181.0);
    }

    #[test]
    fn render_indices_bound_large_tracks() {
        let points = vec![egui::vec2(1.0, 1.0); 100_000];
        let (indices, segments) = build_render_samples(&points);
        assert!(indices.len() <= MAX_RENDER_POINTS);
        assert_eq!(indices.len(), segments.len());
        assert_eq!(indices.first(), Some(&0));
        assert_eq!(indices.last(), Some(&(points.len() - 1)));
    }

    #[test]
    fn render_samples_preserve_gap_segments() {
        let mut points = vec![egui::vec2(1.0, 1.0); 100_000];
        points[50_000] = egui::vec2(f32::NAN, f32::NAN);
        let (indices, segments) = build_render_samples(&points);
        let before = indices
            .iter()
            .enumerate()
            .rfind(|(_, record)| **record < 50_000)
            .and_then(|(index, _)| segments[index]);
        let after = indices
            .iter()
            .enumerate()
            .find(|(_, record)| **record > 50_000)
            .and_then(|(index, _)| segments[index]);
        assert!(before.is_some());
        assert!(after.is_some());
        assert_ne!(before, after);
    }

    #[test]
    fn render_samples_remain_bounded_with_frequent_gaps() {
        let points: Vec<egui::Vec2> = (0..100_000)
            .map(|index| {
                if index % 2 == 0 {
                    egui::vec2(1.0, 1.0)
                } else {
                    egui::vec2(f32::NAN, f32::NAN)
                }
            })
            .collect();
        let (indices, segments) = build_render_samples(&points);
        assert!(indices.len() <= MAX_RENDER_POINTS);
        assert_eq!(indices.len(), segments.len());
    }

    #[test]
    fn zoom_keeps_world_point_under_pointer_after_pan() {
        let mut view = MapView {
            pan_px: egui::vec2(40.0, -20.0),
            zoom: 2.0,
        };
        let pointer_offset = egui::vec2(15.0, 30.0);
        let world_point = (pointer_offset - view.pan_px) / view.zoom;

        zoom_around_pointer(&mut view, pointer_offset, 1.5);

        let projected = view.pan_px + world_point * view.zoom;
        assert!((projected.x - pointer_offset.x).abs() < 1e-5);
        assert!((projected.y - pointer_offset.y).abs() < 1e-5);
        assert_eq!(view.zoom, 3.0);
    }
}
