//! Chart insertion and data/appearance editing commands.

use super::super::*;
use super::add_shape;
use crate::{Result, Session};
use deckcraft_geom::Xfrm;
use deckcraft_model::chart::ChartType;
use deckcraft_model::{Chart, Shape, ShapeKind};
use serde_json::{Value, json};

pub fn chart_type(name: &str) -> ChartType {
    match name.to_ascii_lowercase().replace(['-', '_', ' '], "").as_str() {
        "bar" | "clusteredbar" => ChartType::Bar,
        "stackedbar" => ChartType::StackedBar,
        "percentbar" => ChartType::PercentBar,
        "line" => ChartType::Line,
        "linemarkers" | "linewithmarkers" => ChartType::LineMarkers,
        "stackedline" => ChartType::StackedLine,
        "area" => ChartType::Area,
        "stackedarea" => ChartType::StackedArea,
        "pie" => ChartType::Pie,
        "doughnut" | "donut" => ChartType::Doughnut,
        "scatter" | "xy" => ChartType::Scatter,
        "bubble" => ChartType::Bubble,
        "radar" => ChartType::Radar,
        "filledradar" => ChartType::FilledRadar,
        "stackedcolumn" => ChartType::StackedColumn,
        "percentcolumn" => ChartType::PercentColumn,
        "waterfall" => ChartType::Waterfall,
        "funnel" => ChartType::Funnel,
        "histogram" => ChartType::Histogram,
        "pareto" => ChartType::Pareto,
        "treemap" => ChartType::Treemap,
        "sunburst" => ChartType::Sunburst,
        "boxwhisker" | "box" => ChartType::BoxWhisker,
        "stock" => ChartType::Stock,
        "surface" => ChartType::Surface,
        "combo" => ChartType::Combo,
        _ => ChartType::Column,
    }
}

pub(super) fn chart(s: &mut Session, p: &Value) -> Result<Value> {
    let kind = chart_type(str_param(p, "type").unwrap_or("column"));
    let mut c = Chart::sample(kind);
    if let Some(cats) = p.get("categories").and_then(Value::as_array) {
        c.categories = cats.iter().map(|v| v.as_str().map(String::from).unwrap_or_else(|| v.to_string())).collect();
    }
    if let Some(series) = p.get("series").and_then(Value::as_array) {
        c.series = series
            .iter()
            .map(|sv| deckcraft_model::chart::Series {
                name: sv.get("name").and_then(Value::as_str).unwrap_or("Series").to_string(),
                values: sv.get("values").and_then(Value::as_array).map(|a| a.iter().map(Value::as_f64).collect()).unwrap_or_default(),
                ..Default::default()
            })
            .collect();
    }
    if let Some(t) = p.get("title") {
        c.title = t.as_str().map(String::from);
    }
    let size = s.doc()?.doc.slide_size;
    let rect = rect_param(p, "rect").unwrap_or(Xfrm::new(size.width * 0.15, size.height * 0.15, size.width * 0.7, size.height * 0.7));
    let id = add_shape(s, Shape { xfrm: Some(rect), kind: ShapeKind::Chart(Box::new(c)), ..Default::default() }, true)?;
    Ok(json!({"id": id}))
}

pub(super) fn with_chart(s: &mut Session, p: &Value, cmd: &str, f: impl Fn(&mut Chart) -> Result<()>) -> Result<Value> {
    edit_shapes(s, p, cmd, |sh| match &mut sh.kind {
        ShapeKind::Chart(c) => f(c),
        _ => Err(bad(cmd, "not a chart")),
    })
}

pub(super) fn chart_type_cmd(s: &mut Session, p: &Value) -> Result<Value> {
    let k = chart_type(str_param(p, "type").unwrap_or("column"));
    with_chart(s, p, "chart.type", |c| {
        c.kind = k;
        c.vary_colors = matches!(k, ChartType::Pie | ChartType::Doughnut) || c.series.len() == 1 && c.vary_colors;
        c.raw = None;
        Ok(())
    })
}

pub(super) fn chart_data(s: &mut Session, p: &Value) -> Result<Value> {
    let cats: Option<Vec<String>> = p
        .get("categories")
        .and_then(Value::as_array)
        .map(|a| a.iter().map(|v| v.as_str().map(String::from).unwrap_or_else(|| v.to_string())).collect());
    let series: Option<Vec<deckcraft_model::chart::Series>> = p.get("series").and_then(Value::as_array).map(|a| {
        a.iter()
            .map(|sv| deckcraft_model::chart::Series {
                name: sv.get("name").and_then(Value::as_str).unwrap_or("Series").to_string(),
                values: sv.get("values").and_then(Value::as_array).map(|v| v.iter().map(Value::as_f64).collect()).unwrap_or_default(),
                ..Default::default()
            })
            .collect()
    });
    with_chart(s, p, "chart.data", |c| {
        if let Some(cs) = &cats {
            c.categories = cs.clone();
        }
        if let Some(se) = &series {
            // Keep series formatting by position.
            let old = std::mem::take(&mut c.series);
            c.series = se
                .iter()
                .enumerate()
                .map(|(i, n)| {
                    let mut x = old.get(i).cloned().unwrap_or_default();
                    x.name = n.name.clone();
                    x.values = n.values.clone();
                    x
                })
                .collect();
        }
        c.raw = None;
        Ok(())
    })
}

pub(super) fn chart_options(s: &mut Session, p: &Value) -> Result<Value> {
    with_chart(s, p, "chart.options", |c| {
        if let Some(t) = p.get("title") {
            c.title = t.as_str().map(String::from);
        }
        if let Some(l) = p.get("legend") {
            c.legend = l.as_str().map(String::from);
        }
        if let Some(v) = bool_param(p, "dataLabels") {
            c.data_labels = v;
        }
        if let Some(v) = bool_param(p, "gridlines") {
            c.gridlines = v;
        }
        if let Some(v) = str_param(p, "palette") {
            c.palette = v.to_string();
        }
        c.raw = None;
        Ok(())
    })
}
