//! Chart design controls.

use super::arrange::arrange_group;
use super::big;
use crate::SlideApp;
use crate::icons::Icon;
use crate::widgets::{big_button, group};
use egui::Ui;
use serde_json::json;

pub(super) fn chart_design(app: &mut SlideApp, ui: &mut Ui) {
    group(ui, |ui| {
        let r = big_button(ui, Icon::ChangeChartType, "Change\nChart Type", true);
        egui::Popup::menu(&r).show(|ui| {
            for k in deckcraft_model::chart::ChartType::MAIN {
                if ui.button(k.label()).clicked() {
                    // Replace the chart's type keeping its data.
                    if let Some(id) = app.session.active().and_then(|d| d.selection.shapes.first().copied()) {
                        let _ = app.run("chart.type", json!({"id": id.0, "type": format!("{k:?}")}));
                    }
                }
            }
        });
        big(app, ui, Icon::EditData, "Edit\nData", "app.dialog", json!({"id": "chartData"}));
    });
    arrange_group(app, ui);
}
