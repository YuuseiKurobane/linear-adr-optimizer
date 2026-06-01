use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ExportRow {
    pub value: Value,
    pub export_path: PathBuf,
}

impl ExportRow {
    pub fn preset_name(&self) -> String {
        preset_name(&self.value)
    }

    pub fn desired_retention(&self) -> Option<f64> {
        self.value.get("desired_retention").and_then(Value::as_f64)
    }

    pub fn deck_preset_json(&self) -> Value {
        self.value
            .get("deck_preset")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}))
    }

    pub fn fsrs_weights(&self) -> Result<[f32; 21], String> {
        let raw = self
            .value
            .get("fsrs6_weights")
            .and_then(Value::as_array)
            .ok_or("export row is missing fsrs6_weights")?;
        if raw.len() != 21 {
            return Err(format!("fsrs6_weights must have 21 values, got {}", raw.len()));
        }
        let mut out = [0.0_f32; 21];
        for (idx, item) in raw.iter().enumerate() {
            out[idx] = item
                .as_f64()
                .ok_or_else(|| format!("fsrs6_weights[{idx}] is not numeric"))?
                as f32;
        }
        Ok(out)
    }

    pub fn usage_array<const N: usize>(&self, key: &str) -> Result<[f32; N], String> {
        let raw = self
            .value
            .get("button_usage")
            .and_then(|usage| usage.get(key))
            .and_then(Value::as_array)
            .ok_or_else(|| format!("export row is missing button_usage.{key}"))?;
        if raw.len() != N {
            return Err(format!("button_usage.{key} must have {N} values, got {}", raw.len()));
        }
        let mut out = [0.0_f32; N];
        for (idx, item) in raw.iter().enumerate() {
            out[idx] = item
                .as_f64()
                .ok_or_else(|| format!("button_usage.{key}[{idx}] is not numeric"))?
                as f32;
        }
        Ok(out)
    }
}

pub fn latest_export(path: &Path) -> Result<PathBuf, String> {
    if path.is_file() {
        return Ok(path.to_path_buf());
    }
    let mut candidates = Vec::new();
    for entry in fs::read_dir(path)
        .map_err(|err| format!("failed to read export directory {}: {err}", path.display()))?
    {
        let entry = entry.map_err(|err| format!("failed to read export directory entry: {err}"))?;
        let file_name = entry.file_name().to_string_lossy().to_string();
        if file_name.starts_with("adr-input-") && file_name.ends_with(".jsonl") {
            let modified = entry
                .metadata()
                .and_then(|m| m.modified())
                .map_err(|err| format!("failed to inspect {}: {err}", entry.path().display()))?;
            candidates.push((modified, entry.path()));
        }
    }
    candidates.sort_by_key(|(modified, _)| *modified);
    candidates
        .pop()
        .map(|(_, path)| path)
        .ok_or_else(|| format!("No adr-input-*.jsonl files found in {}", path.display()))
}

pub fn load_export_row(path: &Path, selector: &str) -> Result<ExportRow, String> {
    let (export_path, rows) = load_export_rows(path)?;
    select_export_row(&export_path, &rows, selector)
}

pub fn load_export_rows(path: &Path) -> Result<(PathBuf, Vec<Value>), String> {
    let export_path = latest_export(path)?;
    let text = fs::read_to_string(&export_path)
        .map_err(|err| format!("failed to read {}: {err}", export_path.display()))?;
    let rows: Vec<Value> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_str::<Value>(line).map_err(|err| format!("invalid JSONL row: {err}")))
        .collect::<Result<_, _>>()?;
    if rows.is_empty() {
        return Err(format!("No rows found in {}", export_path.display()));
    }
    Ok((export_path, rows))
}

pub fn select_export_row(export_path: &Path, rows: &[Value], selector: &str) -> Result<ExportRow, String> {
    let needle = selector.to_lowercase();
    let passes: [fn(&Value, &str) -> bool; 4] = [
        |row, needle| preset_name(row).to_lowercase() == needle,
        |row, needle| deck_names(row).iter().any(|name| name.to_lowercase() == needle),
        |row, needle| preset_name(row).to_lowercase().contains(needle),
        |row, needle| deck_names(row).iter().any(|name| name.to_lowercase().contains(needle)),
    ];

    for matcher in passes {
        let matches: Vec<Value> = rows
            .iter()
            .filter(|row| matcher(row, &needle))
            .cloned()
            .collect();
        if matches.len() == 1 {
            return Ok(ExportRow {
                value: matches.into_iter().next().expect("one row"),
                export_path: export_path.to_path_buf(),
            });
        }
        if matches.len() > 1 {
            let available = matches
                .iter()
                .map(preset_name)
                .collect::<Vec<_>>()
                .join(", ");
            return Err(format!(
                "Preset/deck selector {selector:?} is ambiguous. Matches: {available}. Use the exact deck_preset.name."
            ));
        }
    }

    let available = rows.iter().map(preset_name).collect::<Vec<_>>().join(", ");
    Err(format!(
        "Preset/deck selector {selector:?} not found. Available presets: {available}"
    ))
}

fn preset_name(row: &Value) -> String {
    row.get("deck_preset")
        .and_then(|preset| preset.get("name"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn deck_names(row: &Value) -> Vec<String> {
    row.get("decks")
        .and_then(Value::as_array)
        .map(|decks| {
            decks
                .iter()
                .filter_map(|deck| deck.get("name").and_then(Value::as_str))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}
