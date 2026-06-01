use crate::config::SearchConfig;
use crate::export::ExportRow;
use crate::output::text::{safe_name, timestamp};
use crate::search::fixed_curve::FixedCurveManager;
use crate::search::rank::pareto_frontier;
use crate::search::safety::safety_pool;
use crate::types::Point;
use plotters::coord::types::RangedCoordf64;
use plotters::prelude::*;
use std::fs;
use std::path::PathBuf;

pub fn png_output_path(config: &SearchConfig, row: &ExportRow) -> Result<PathBuf, String> {
    fs::create_dir_all(&config.output_dir)
        .map_err(|err| format!("failed to create {}: {err}", config.output_dir.display()))?;
    Ok(config.output_dir.join(format!(
        "adr_pareto_{}_{}.png",
        safe_name(&row.preset_name()),
        timestamp()
    )))
}

pub fn write_png(
    path: &PathBuf,
    phase1: &[Point],
    phase2: &[Point],
    phase3: &[Point],
    phase4: &[Point],
    final_points: &[Point],
    fixed: &FixedCurveManager<'_>,
    config: &SearchConfig,
    row: &ExportRow,
) -> Result<(), String> {
    let mut all = Vec::new();
    all.extend_from_slice(phase1);
    all.extend_from_slice(phase2);
    all.extend_from_slice(phase3);
    all.extend_from_slice(phase4);
    all.extend(fixed.fixed_curve.iter().map(|(_, point)| point.clone()));
    if all.is_empty() {
        return Ok(());
    }
    let min_x = all.iter().map(|p| p.memorized_cards).fold(f64::INFINITY, f64::min);
    let max_x = all.iter().map(|p| p.memorized_cards).fold(f64::NEG_INFINITY, f64::max);
    let min_y = all.iter().map(|p| p.memorized_per_minute).fold(f64::INFINITY, f64::min);
    let max_y = all.iter().map(|p| p.memorized_per_minute).fold(f64::NEG_INFINITY, f64::max);
    let x_pad = ((max_x - min_x) * 0.06).max(1.0);
    let y_pad = ((max_y - min_y) * 0.12).max(0.1);

    let root = BitMapBackend::new(path, (2160, 1328)).into_drawing_area();
    root.fill(&WHITE).map_err(|err| err.to_string())?;
    let mut chart = ChartBuilder::on(&root)
        .caption(
            format!("FSRS-ADR Pareto Search: {} target DR {:.3}", row.preset_name(), fixed.target_dr),
            ("sans-serif", 30),
        )
        .margin(18)
        .x_label_area_size(70)
        .y_label_area_size(90)
        .build_cartesian_2d((min_x - x_pad)..(max_x + x_pad), (min_y - y_pad)..(max_y + y_pad))
        .map_err(|err| err.to_string())?;
    chart
        .configure_mesh()
        .x_desc("Average memorized cards")
        .y_desc("Average memorized cards per daily minute")
        .draw()
        .map_err(|err| err.to_string())?;

    draw_points(&mut chart, phase1, RGBColor(107, 174, 214).mix(0.20), 2)?;
    draw_points(&mut chart, phase2, RGBColor(158, 202, 225).mix(0.30), 2)?;
    draw_points(&mut chart, phase3, RGBColor(116, 196, 118).mix(0.35), 2)?;
    draw_points(&mut chart, phase4, RGBColor(253, 141, 60).mix(0.60), 3)?;

    let fixed_points: Vec<_> = fixed.fixed_curve.iter().map(|(_, p)| (p.memorized_cards, p.memorized_per_minute)).collect();
    chart
        .draw_series(LineSeries::new(fixed_points, &RGBColor(115, 115, 115)))
        .map_err(|err| err.to_string())?;

    let frontier = pareto_frontier(&safety_pool(final_points, config));
    if !frontier.is_empty() {
        chart
            .draw_series(LineSeries::new(
                frontier.iter().map(|p| (p.memorized_cards, p.memorized_per_minute)),
                &BLACK,
            ))
            .map_err(|err| err.to_string())?;
    }
    root.present().map_err(|err| err.to_string())
}

fn draw_points<DB: DrawingBackend>(
    chart: &mut ChartContext<'_, DB, Cartesian2d<RangedCoordf64, RangedCoordf64>>,
    points: &[Point],
    color: RGBAColor,
    radius: i32,
) -> Result<(), String> {
    chart
        .draw_series(points.iter().map(|p| {
            Circle::new((p.memorized_cards, p.memorized_per_minute), radius, color.filled())
        }))
        .map(|_| ())
        .map_err(|err| format!("{err:?}"))
}
