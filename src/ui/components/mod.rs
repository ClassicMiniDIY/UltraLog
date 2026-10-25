//! Night Drive components shared by every view.
//!
//! Each follows its spec in `design/design-system/components/<Name>/README.md` and reads colours
//! from `crate::ui::theme::current`, so it can be called from any `Ui` without an app reference.

pub mod button;
pub mod controls;
pub mod notice;
pub mod window;

pub use button::{Kind, button, button_sm, icon_button};
pub use controls::{checkbox, intensity_slider, segmented};
pub use notice::{Status, ecu_badge, notice, status_badge};
pub use window::{finish_window, window};

/// Phosphor icons (regular weight, outline). Filled glyphs are only for transport (play, stop)
/// and the favourite star: draw those with `FontFamily::Name(fonts::PHOSPHOR_FILL)`.
pub use egui_phosphor::regular as icon;
