//! Data-panel widget registry.
//!
//! A widget is a small unit-struct that implements [`DataWidget`]. Widget
//! instances are static singletons; per-tab state lives in
//! [`crate::state::DataPanelState`] (or its nested per-widget structs) so
//! widgets themselves carry no runtime state.
//!
//! Adding a new widget is a one-file change:
//! 1. Create `src/ui/widgets/<your_widget>.rs` with a unit-struct and
//!    `impl DataWidget`.
//! 2. Register it in [`registered`] below.
//! 3. Add per-tab state to `DataPanelState` if needed.

use eframe::egui;

use crate::app::UltraLogApp;

pub mod track_map;

/// Contract every right-side data-panel widget implements.
///
/// `&self` (rather than `&mut self`) on every method is intentional: it
/// lets the host iterate over `&'static` widget singletons and pass
/// `&mut UltraLogApp` to each `render` without borrow conflicts. All
/// mutable state lives on the app.
pub trait DataWidget {
    /// Stable identifier used for state lookup, telemetry, and i18n keys.
    fn id(&self) -> &'static str;

    /// Localized header title for the widget pane.
    fn title(&self, app: &UltraLogApp) -> String;

    /// Whether this widget has data to show for the active tab. The host
    /// hides the widget header (and the panel itself, if no widget is
    /// available) when this returns `false`.
    fn is_available(&self, app: &UltraLogApp) -> bool;

    /// Whether the user has enabled this widget on the active tab. When
    /// `false`, the widget pane shows only its header (collapsed).
    fn is_enabled(&self, app: &UltraLogApp) -> bool;

    /// Toggle the per-tab enabled flag.
    fn set_enabled(&self, app: &mut UltraLogApp, enabled: bool);

    /// Cancel background work when the widget or its host panel is hidden.
    fn cancel_background_work(&self) {}

    /// Render the widget body. The host has already drawn the header.
    fn render(&self, ui: &mut egui::Ui, app: &mut UltraLogApp);
}

/// Static registry of all data-panel widgets, in display order.
pub fn registered() -> &'static [&'static dyn DataWidget] {
    &[&track_map::TrackMapWidget]
}
