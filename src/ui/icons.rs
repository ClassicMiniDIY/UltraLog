//! Custom icon drawing utilities.

use eframe::egui;

/// Draw a right-pointing triangle (for collapsed state)
pub fn draw_triangle_right(ui: &mut egui::Ui, center: egui::Pos2, size: f32, color: egui::Color32) {
    let painter = ui.painter();
    let half = size / 2.0;
    let points = [
        egui::pos2(center.x - half * 0.3, center.y - half),
        egui::pos2(center.x + half * 0.6, center.y),
        egui::pos2(center.x - half * 0.3, center.y + half),
    ];
    painter.add(egui::Shape::convex_polygon(
        points.to_vec(),
        color,
        egui::Stroke::NONE,
    ));
}

/// Draw a down-pointing triangle (for expanded state)
pub fn draw_triangle_down(ui: &mut egui::Ui, center: egui::Pos2, size: f32, color: egui::Color32) {
    let painter = ui.painter();
    let half = size / 2.0;
    let points = [
        egui::pos2(center.x - half, center.y - half * 0.3),
        egui::pos2(center.x + half, center.y - half * 0.3),
        egui::pos2(center.x, center.y + half * 0.6),
    ];
    painter.add(egui::Shape::convex_polygon(
        points.to_vec(),
        color,
        egui::Stroke::NONE,
    ));
}
