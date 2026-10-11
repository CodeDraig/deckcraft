//! Chart data grid editing.

use super::{Dialog, buttons};
use crate::SlideApp;
use egui::Ui;
use serde_json::{Value, json};

pub(super) fn chart_data_dialog(app: &mut SlideApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    let ch = app.session.active().and_then(|s| {
        s.selected_shapes()
            .into_iter()
            .find_map(|x| if let deckcraft_model::ShapeKind::Chart(c) = &x.kind { Some(((**c).clone(), x.id)) } else { None })
    });
    let Some((c, id)) = ch else {
        ui.label("Select a chart first.");
        let (ok, cancel) = buttons(ui, "Close");
        return ok || cancel;
    };
    ui.label("Edit the data: first row = series names, first column = categories.");
    let mut grid: Vec<Vec<String>> = d.params.get("grid").and_then(|g| serde_json::from_value(g.clone()).ok()).unwrap_or_else(|| {
        let mut g = vec![std::iter::once(String::new()).chain(c.series.iter().map(|s| s.name.clone())).collect::<Vec<_>>()];
        for (i, cat) in c.categories.iter().enumerate() {
            let mut row = vec![cat.clone()];
            for s in &c.series {
                row.push(s.values.get(i).copied().flatten().map(|v| v.to_string()).unwrap_or_default());
            }
            g.push(row);
        }
        g
    });
    egui::Grid::new("chartdata").striped(true).show(ui, |ui| {
        for row in grid.iter_mut() {
            for cell in row.iter_mut() {
                ui.add(egui::TextEdit::singleline(cell).desired_width(70.0));
            }
            ui.end_row();
        }
    });
    ui.horizontal(|ui| {
        if ui.button("+ Row").clicked() {
            let n = grid.first().map(|r| r.len()).unwrap_or(2);
            grid.push(vec![String::new(); n]);
        }
        if ui.button("+ Series").clicked() {
            for r in grid.iter_mut() {
                r.push(String::new());
            }
        }
    });
    d.params = json!({"grid": grid});
    let (ok, cancel) = buttons(ui, "OK");
    if ok {
        let names: Vec<String> = grid.first().map(|r| r.iter().skip(1).cloned().collect()).unwrap_or_default();
        let cats: Vec<String> = grid.iter().skip(1).map(|r| r.first().cloned().unwrap_or_default()).collect();
        let series: Vec<Value> = names.iter().enumerate().map(|(k, n)| json!({"name": n, "values": grid.iter().skip(1).map(|r| r.get(k + 1).and_then(|v| v.trim().parse::<f64>().ok())).collect::<Vec<_>>()})).collect();
        let _ = app.run("chart.data", json!({"id": id.0, "categories": cats, "series": series}));
    }
    ok || cancel
}
