//! File export selection and delivery.

use super::{Dialog, buttons};
use crate::SlideApp;
use egui::Ui;
use serde_json::json;

pub(super) fn export_dialog(app: &mut SlideApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    ui.label("Export the presentation as:");
    let fmt = d.get("fmt", "pdf");
    for (l, f) in [
        ("PDF document (.pdf)", "pdf"),
        ("PNG images (current slide)", "png"),
        ("PNG images (all slides)", "pngAll"),
        ("JPEG image (current slide)", "jpeg"),
        ("PowerPoint Presentation (.pptx)", "pptx"),
        ("Outline (.txt)", "outline"),
    ] {
        if ui.radio(fmt == f, l).clicked() {
            d.fields.insert("fmt".into(), f.into());
        }
    }
    let pdf_layout = d.get("pdfLayout", "slides");
    if fmt == "pdf" {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label("Print layout:");
            egui::ComboBox::from_id_salt("pdf_layout")
                .selected_text(match pdf_layout.as_str() {
                    "notes" => "Notes Pages",
                    "h1" => "Handouts (1 slide per page)",
                    "h2" => "Handouts (2 slides per page)",
                    "h3" => "Handouts (3 slides per page)",
                    "h4" => "Handouts (4 slides per page)",
                    "h6" => "Handouts (6 slides per page)",
                    "h9" => "Handouts (9 slides per page)",
                    _ => "Full Page Slides",
                })
                .show_ui(ui, |ui| {
                    for (k, l) in [
                        ("slides", "Full Page Slides"),
                        ("notes", "Notes Pages"),
                        ("h1", "Handouts (1 slide per page)"),
                        ("h2", "Handouts (2 slides per page)"),
                        ("h3", "Handouts (3 slides per page)"),
                        ("h4", "Handouts (4 slides per page)"),
                        ("h6", "Handouts (6 slides per page)"),
                        ("h9", "Handouts (9 slides per page)"),
                    ] {
                        if ui.selectable_label(pdf_layout == k, l).clicked() {
                            d.fields.insert("pdfLayout".into(), k.into());
                        }
                    }
                });
        });
        let mut hidden = d.get("pdfHidden", "false") == "true";
        if ui.checkbox(&mut hidden, "Include hidden slides").changed() {
            d.fields.insert("pdfHidden".into(), hidden.to_string());
        }
    }
    let pdf_params = {
        let (layout, per) = match pdf_layout.as_str() {
            "notes" => ("notes", 0),
            l if l.starts_with('h') => ("handouts", l[1..].parse().unwrap_or(6)),
            _ => ("slides", 0),
        };
        json!({"layout": layout, "perPage": per, "includeHidden": d.get("pdfHidden", "false") == "true"})
    };
    let (ok, cancel) = buttons(ui, "Export");
    if ok {
        let name = app.session.active().map(|s| s.title()).unwrap_or_else(|| "Presentation".into());
        let (ext, all) = match fmt.as_str() {
            "pdf" => ("pdf", false),
            "pngAll" => ("png", true),
            "jpeg" => ("jpg", false),
            "pptx" => ("pptx", false),
            "outline" => ("txt", false),
            _ => ("png", false),
        };
        let suggested = format!("{name}.{ext}");
        if let Some(pick) = app.services.pick_save.as_mut() {
            if let Some(path) = pick(&suggested) {
                let mut params = json!({"path": path, "all": all});
                if ext == "pdf"
                    && let (Some(o), Some(extra)) = (params.as_object_mut(), pdf_params.as_object())
                {
                    o.extend(extra.clone());
                }
                let r = app.session.execute("file.export", &params);
                match r {
                    Ok(_) => app.set_status(format!("Exported {path}")),
                    Err(e) => app.set_status(e.to_string()),
                }
            }
        } else if let (Some(dl), Some(s)) = (app.services.download.as_mut(), app.session.active()) {
            let bytes = match ext {
                "png" | "jpg" => {
                    let img =
                        deckcraft_render::render_slide(&s.doc, s.selection.slide, &deckcraft_render::RenderOpts { scale: 2.0, ..Default::default() });
                    Ok(if ext == "png" { img.to_png() } else { img.to_jpeg(92) })
                }
                "pdf" => deckcraft_engine::cmd::file::pdf_bytes(&s.doc, &pdf_params),
                other => deckcraft_engine::cmd::file::save_bytes(&s.doc, other),
            };
            match bytes {
                Ok(bytes) => dl(&suggested, &bytes),
                Err(e) => app.set_status(e.to_string()),
            }
        }
    }
    ok || cancel
}
