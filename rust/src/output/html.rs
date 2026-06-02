use crate::config::SearchConfig;
use crate::export::ExportRow;
use crate::output::text::{safe_name, timestamp};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

pub fn html_output_path(config: &SearchConfig, row: &ExportRow) -> Result<PathBuf, String> {
    fs::create_dir_all(&config.output_dir)
        .map_err(|err| format!("failed to create {}: {err}", config.output_dir.display()))?;
    Ok(config.output_dir.join(format!(
        "adr_pareto_{}_{}.html",
        safe_name(&row.preset_name()),
        timestamp()
    )))
}

pub fn write_plot_html(plot_path: &Path, summary_path: &Path) -> Result<(), String> {
    if plot_path.extension().and_then(|ext| ext.to_str()) != Some("html") {
        return Ok(());
    }
    let summary_text = fs::read_to_string(summary_path)
        .map_err(|err| format!("failed to read {}: {err}", summary_path.display()))?;
    let summary: Value = serde_json::from_str(&summary_text)
        .map_err(|err| format!("failed to parse {}: {err}", summary_path.display()))?;
    let output_dir = plot_path.parent().unwrap_or_else(|| Path::new("."));
    ensure_assets(output_dir)?;
    let summary_json = script_json(&summary)?;
    let html = format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>ADR Pareto Plot</title>
  <link rel="stylesheet" href="adr_plot_assets/adr_plot.css?v=rust-rewrite-2">
</head>
<body>
  <main class="app-shell">
    <section class="plot-panel">
      <div class="plot-frame">
        <div id="plot" class="plot" aria-label="ADR Pareto Plot"></div>
        <aside id="result-box" class="result-box" aria-label="ADR plot labels"></aside>
      </div>
    </section>
  </main>
  <script>
    window.ADR_INITIAL_SUMMARY = {summary_json};
  </script>
  <script src="adr_plot_assets/vendor/plotly.min.js"></script>
  <script src="adr_plot_assets/adr_plot.js?v=rust-rewrite-2"></script>
</body>
</html>
"#
    );
    fs::write(plot_path, html).map_err(|err| format!("failed to write {}: {err}", plot_path.display()))
}

fn ensure_assets(output_dir: &Path) -> Result<(), String> {
    let target = output_dir.join("adr_plot_assets");
    fs::create_dir_all(target.join("vendor"))
        .map_err(|err| format!("failed to create {}: {err}", target.display()))?;
    write_asset(
        &target.join("adr_plot.css"),
        include_bytes!("../../assets/web/adr_plot.css"),
    )?;
    write_asset(
        &target.join("adr_plot.js"),
        include_bytes!("../../assets/web/adr_plot.js"),
    )?;
    write_asset(
        &target.join("vendor").join("plotly.min.js"),
        include_bytes!("../../assets/web/vendor/plotly.min.js"),
    )?;
    Ok(())
}

fn write_asset(path: &Path, bytes: &[u8]) -> Result<(), String> {
    fs::write(path, bytes).map_err(|err| format!("failed to write {}: {err}", path.display()))
}

fn script_json<T: serde::Serialize>(value: &T) -> Result<String, String> {
    serde_json::to_string(value)
        .map(|text| text.replace("</", "<\\/"))
        .map_err(|err| format!("failed to serialize script JSON: {err}"))
}
