use crate::config::SearchConfig;
use crate::export::ExportRow;
use crate::types::{FixedCurveEquivalence, PhaseDiag, Point};
use std::collections::HashMap;
use std::path::Path;

pub fn print_intro(row: &ExportRow, target_dr: f64, config: &SearchConfig) {
    println!("Loaded export: {}", row.export_path.display());
    println!("Preset: {}", row.preset_name());
    println!("Quality preset: {}", config.quality_preset);
    println!(
        "Using simulation defaults: days={}, deck_size={}, learn_limit={}",
        config.days, config.deck_size, config.learn_limit
    );
    println!("Target DR: {target_dr:.4}");
    println!(
        "Safety checks/filtering: {}",
        if config.ignore_safety { "skipped" } else { "on" }
    );
    if !config.ignore_safety {
        let phase1_safety = if config.legacy_unsafe_plot_display {
            "legacy display; unsafe points are simulated and plotted"
        } else {
            "pre-screen; unsafe points are skipped before simulation"
        };
        println!("Phase 1 unsafe handling: {phase1_safety}");
    }
    println!(
        "Original final verification: {}",
        if config.include_original { "on" } else { "off" }
    );
    println!(
        "Aggressive/Calm: northeast-only, spread regret <= {:.2} percentage points",
        config.aggressive_calm_regret_pct
    );
    println!("Aggressive: largest final p90-p10 DR spread; Calm: smallest final p90-p10 DR spread");
    println!(
        "Experimental bridge midpoint neighborhoods: {}",
        if config.experimental_bridge_midpoint_neighborhoods {
            "on"
        } else {
            "off"
        }
    );
}

pub fn format_point_block(
    label: &str,
    point: &Point,
    metrics: &HashMap<crate::types::PointKey, FixedCurveEquivalence>,
    config: &SearchConfig,
) -> String {
    let metric = &metrics[&point.key()];
    let dr_line = if point.dr_samples > 0 {
        format!("dr:{:.4} band={:.2}%", point.dr_mean, point.dr_spread * 100.0)
    } else {
        "dr:n/a band=n/a".to_string()
    };
    let spread_prefix = if metric.spread_label.starts_with('>') { ">" } else { "" };
    let spread = format!("{spread_prefix}{:.3}%", metric.spread_floor * 100.0);
    let _ = config;
    format!(
        "{label}\nflat={:.3}, s={:.3}, d={:.3}\n{dr_line}\neff:{} mem={} spread:{spread}",
        point.flat, point.s_multi, point.d_multi, metric.efficiency_label, metric.memory_label
    )
}

pub fn print_point(
    label: &str,
    point: &Point,
    metrics: &HashMap<crate::types::PointKey, FixedCurveEquivalence>,
    config: &SearchConfig,
) {
    let metric = &metrics[&point.key()];
    let dr = if point.dr_samples > 0 {
        format!(
            " dr_mean={:.4} dr_band={:.2}% dr_n={}",
            point.dr_mean,
            point.dr_spread * 100.0,
            point.dr_samples
        )
    } else {
        String::new()
    };
    let safety = if config.ignore_safety {
        "safety=skipped".to_string()
    } else {
        format!(
            "safe={} flips={} hard_shortens={}",
            point.safe(),
            point.interval_flips,
            point.hard_shortens
        )
    };
    println!(
        "{label:22} flat={:.4} s={:.4} d={:.4} x={:.2} y={:.4} eff={} mem={} spread={}{} {}",
        point.flat,
        point.s_multi,
        point.d_multi,
        point.memorized_cards,
        point.memorized_per_minute,
        metric.efficiency_label,
        metric.memory_label,
        metric.spread_label,
        dr,
        safety
    );
}

pub fn print_results(
    selected_by_label: &HashMap<String, Point>,
    selected_metrics: &HashMap<crate::types::PointKey, FixedCurveEquivalence>,
    diagnostics: &[PhaseDiag],
    plot_path: &Path,
    summary_path: &Path,
    elapsed_s: f64,
    config: &SearchConfig,
) {
    println!();
    for label in [
        "Recommended",
        "Aggressive",
        "Calm",
        "Efficiency Potential",
        "Memory Potential",
        "Max Spread",
        "Original",
    ] {
        if let Some(point) = selected_by_label.get(label) {
            print_point(label, point, selected_metrics, config);
        }
    }
    let mut inspect: Vec<_> = selected_by_label
        .keys()
        .filter(|label| label.starts_with("Inspect "))
        .cloned()
        .collect();
    inspect.sort();
    for label in inspect {
        if let Some(point) = selected_by_label.get(&label) {
            print_point(&label, point, selected_metrics, config);
        }
    }
    if !selected_by_label.contains_key("Recommended") {
        println!("Recommended            none: no final candidate was strictly northeast of the fixed target DR point");
    }

    println!();
    println!("Diagnostics:");
    for diag in diagnostics {
        let skipped = diag
            .notes
            .as_ref()
            .and_then(|notes| notes.get("screened_unsafe"))
            .and_then(|value| value.as_i64())
            .filter(|value| *value > 0)
            .map(|value| format!(" skipped_unsafe={value}"))
            .unwrap_or_default();
        println!(
            "  {:28} weight={} evaluated={} promoted={} extra={}{} elapsed={:.1}s",
            diag.name,
            fmt_g(diag.weight),
            diag.evaluated,
            diag.promoted,
            diag.pareto_extra,
            skipped,
            diag.elapsed_s
        );
    }
    println!();
    println!("Plot: {}", plot_path.display());
    println!("Summary: {}", summary_path.display());
    println!("Elapsed: {elapsed_s:.1}s");
}

fn fmt_g(value: f64) -> String {
    if (value - value.round()).abs() < 1e-9 {
        format!("{:.0}", value)
    } else {
        format!("{value}")
    }
}
