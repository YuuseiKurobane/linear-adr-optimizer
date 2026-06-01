use crate::config::{json_safe_config, SearchConfig};
use crate::export::ExportRow;
use crate::types::{FixedCurveEquivalence, PhaseDiag, Point, PointKey};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[allow(clippy::too_many_arguments)]
pub fn write_summary(
    path: &Path,
    row: &ExportRow,
    target_dr: f64,
    config: &SearchConfig,
    selected_by_label: &HashMap<String, Point>,
    labels_by_key: &HashMap<PointKey, Vec<String>>,
    selected_metrics: &HashMap<PointKey, FixedCurveEquivalence>,
    fixed_curve_points: &[(f64, Point)],
    fixed_curve_refined_points: &HashMap<i64, Point>,
    fixed_curve_envelope: &[(f64, Point)],
    final_frontier: &[Point],
    phase3_render_extra: &[Point],
    max_spread_prefinal: &[Point],
    diagnostics: &[PhaseDiag],
    plot_layers: &HashMap<&str, Vec<Point>>,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
    }

    let selected_json = selected_by_label
        .iter()
        .map(|(label, point)| (label.clone(), json!(point)))
        .collect::<serde_json::Map<_, _>>();
    let labels_by_point = labels_by_key
        .iter()
        .map(|(key, labels)| (key.json_key(), json!(labels)))
        .collect::<serde_json::Map<_, _>>();
    let selected_fixed_curve_metrics = selected_by_label
        .iter()
        .filter_map(|(label, point)| {
            selected_metrics
                .get(&point.key())
                .map(|metric| (label.clone(), json!(metric)))
        })
        .collect::<serde_json::Map<_, _>>();

    let mut refined: Vec<_> = fixed_curve_refined_points.iter().collect();
    refined.sort_by_key(|(pct, _)| **pct);
    let plot_layers_json = plot_layers
        .iter()
        .map(|(key, points)| ((*key).to_string(), json!(points)))
        .collect::<serde_json::Map<_, _>>();

    let value = json!({
        "export": row.export_path,
        "preset": row.deck_preset_json(),
        "target_dr": target_dr,
        "args": json_safe_config(config),
        "selected": Value::Object(selected_json),
        "labels_by_point": Value::Object(labels_by_point),
        "selected_fixed_curve_metrics": Value::Object(selected_fixed_curve_metrics),
        "fixed_curve_points": fixed_curve_points.iter().map(|(dr, point)| json!({"dr": dr, "point": point})).collect::<Vec<_>>(),
        "fixed_curve_refined_points": refined.into_iter().map(|(pct, point)| json!({"dr": *pct as f64 / 1_000_000.0 / 100.0, "point": point})).collect::<Vec<_>>(),
        "fixed_curve_envelope": fixed_curve_envelope.iter().map(|(dr, point)| json!({"dr": dr, "point": point})).collect::<Vec<_>>(),
        "final_frontier": final_frontier,
        "phase3_render_extra": phase3_render_extra,
        "max_spread_prefinal": max_spread_prefinal,
        "plot_layers": Value::Object(plot_layers_json),
        "diagnostics": diagnostics,
    });

    let text = serde_json::to_string_pretty(&value)
        .map_err(|err| format!("failed to serialize summary JSON: {err}"))?;
    fs::write(path, text).map_err(|err| format!("failed to write {}: {err}", path.display()))
}
