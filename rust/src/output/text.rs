use crate::config::SearchConfig;
use crate::export::ExportRow;
use crate::progress::format_point_block;
use crate::types::{FixedCurveEquivalence, Point, ResultLabel};
use chrono::Local;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

pub fn safe_name(value: &str) -> String {
    let out: String = value
        .chars()
        .map(|ch| {
            if ch.is_alphanumeric() || ch == '.' || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = out.trim_matches('_').to_string();
    if trimmed.is_empty() {
        "preset".to_string()
    } else {
        trimmed
    }
}

pub fn timestamp() -> String {
    Local::now().format("%Y%m%d-%H%M%S").to_string()
}

pub fn write_point_only(
    label: ResultLabel,
    selected_by_label: &HashMap<String, Point>,
    selected_metrics: &HashMap<crate::types::PointKey, FixedCurveEquivalence>,
    config: &SearchConfig,
    row: &ExportRow,
) -> Result<PathBuf, String> {
    fs::create_dir_all(&config.output_dir)
        .map_err(|err| format!("failed to create {}: {err}", config.output_dir.display()))?;
    let preset_name = safe_name(&row.preset_name());
    let path = config.output_dir.join(format!(
        "adr_{}_{}_{}.txt",
        preset_name,
        label.file_fragment(),
        timestamp()
    ));
    let text = selected_by_label
        .get(label.as_str())
        .map(|point| format!("{}\n", format_point_block(label.as_str(), point, selected_metrics, config)))
        .unwrap_or_else(|| format!("{}\nnone\n", label.as_str()));
    fs::write(&path, text).map_err(|err| format!("failed to write {}: {err}", path.display()))?;
    Ok(path)
}
