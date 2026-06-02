use crate::config::{normalize, parse_dr, SearchConfig};
use crate::export::{load_export_rows, select_export_row, ExportRow};
use crate::model::adr::FSRSADR;
use crate::model::behavior::BehaviorModel;
use crate::model::fsrs_v6::FSRSv6;
use crate::model::simulate::{
    dr_summary_by_weight, safety_summary, safety_summary_checks_only, simulate, DrSummary,
    SafetySummary,
};
use crate::output::html::{html_output_path, write_plot_html};
use crate::output::png::{png_output_path, write_png};
use crate::output::summary::write_summary;
use crate::output::text::write_point_only;
use crate::progress::{print_intro, print_results};
use crate::search::candidates::{dedupe_points, PointStore};
use crate::search::fixed_curve::FixedCurveManager;
use crate::search::phases::{
    evaluate_unique, phase4_seed_profiles, phase_diag, run_micro_hillclimb, run_phase1,
    run_refinement_phase,
};
use crate::search::rank::{equivalence_map, pareto_frontier, select_promotions};
use crate::search::safety::{attach_safety, safety_pool};
use crate::search::select::{
    add_reference_labels, choose_final_candidates, choose_points, labels_by_key, reference_labels,
};
use crate::types::{Candidate, Point, PointKey, ResultLabel, SearchResult};
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use rayon::prelude::*;
use rayon::ThreadPool;
use rayon::ThreadPoolBuilder;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

#[derive(Clone)]
pub struct EvalEngine {
    fsrs: FSRSv6,
    behavior_model: BehaviorModel,
    days: i32,
    deck_size: i32,
    new_cards_per_day: i32,
    pool: Arc<ThreadPool>,
}

impl EvalEngine {
    pub fn new(row: &ExportRow, config: &SearchConfig) -> Result<Self, String> {
        let usage_first = normalize(row.usage_array::<4>("first_rating_prob")?, "first_rating_prob")?;
        let usage_review = normalize(
            row.usage_array::<3>("review_rating_prob")?,
            "review_rating_prob",
        )?;
        let threads = configured_threads(config.threads);
        let pool = ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .map_err(|err| format!("failed to build Rayon thread pool: {err}"))?;
        Ok(Self {
            fsrs: FSRSv6::new(row.fsrs_weights()?),
            behavior_model: BehaviorModel::new(
                usage_first,
                row.usage_array::<4>("learn_costs")?,
                usage_review,
                row.usage_array::<4>("review_costs")?,
            ),
            days: config.days,
            deck_size: config.deck_size,
            new_cards_per_day: config.learn_limit,
            pool: Arc::new(pool),
        })
    }

    pub fn evaluate_raw(&self, candidates: &[Candidate], weight: f64, seed: u64) -> Vec<Point> {
        self.pool.install(|| {
            candidates
                .par_iter()
                .enumerate()
                .map(|(idx, candidate)| {
                    let adr = FSRSADR::linear(
                        candidate.flat as f32,
                        candidate.s_multi as f32,
                        candidate.d_multi as f32,
                    );
                    let mut rng = ChaCha8Rng::seed_from_u64(seed.wrapping_add(idx as u64 * 1_000_003));
                    let result = simulate(
                        weight as f32,
                        self.deck_size,
                        self.new_cards_per_day,
                        self.days as f32,
                        &self.fsrs,
                        &adr,
                        &self.behavior_model,
                        &mut rng,
                    );
                    let memorized_fraction = result.memorized();
                    let memorized_cards = memorized_fraction * self.deck_size as f64;
                    let memorized_per_minute = 60.0 * result.efficiency();
                    Point::new(
                        candidate.snap(),
                        result.total_average_memorized,
                        result.total_cost,
                        result.total_iters,
                        memorized_fraction,
                        memorized_cards,
                        memorized_per_minute,
                    )
                })
                .collect()
        })
    }

    pub fn evaluate_search(
        &self,
        candidates: &[Candidate],
        weight: f64,
        seed: u64,
        config: &SearchConfig,
    ) -> Vec<Point> {
        attach_safety(self, self.evaluate_raw(candidates, weight, seed), config)
    }

    pub fn safety_many(
        &self,
        candidates: &[Candidate],
        s_max: f64,
        max_checks: i32,
    ) -> Vec<(PointKey, SafetySummary)> {
        self.pool.install(|| {
            candidates
                .par_iter()
                .map(|candidate| {
                    let adr = FSRSADR::linear(
                        candidate.flat as f32,
                        candidate.s_multi as f32,
                        candidate.d_multi as f32,
                    );
                    let summary = safety_summary(
                        &self.fsrs,
                        &adr,
                        &self.behavior_model,
                        self.days as f32,
                        s_max as f32,
                        max_checks,
                    );
                    (candidate.key(), summary)
                })
                .collect()
        })
    }

    pub fn safety_checks_many(
        &self,
        candidates: &[Candidate],
        s_max: f64,
        max_checks: i32,
    ) -> Vec<(PointKey, SafetySummary)> {
        self.pool.install(|| {
            candidates
                .par_iter()
                .map(|candidate| {
                    let adr = FSRSADR::linear(
                        candidate.flat as f32,
                        candidate.s_multi as f32,
                        candidate.d_multi as f32,
                    );
                    let summary = safety_summary_checks_only(
                        &self.fsrs,
                        &adr,
                        &self.behavior_model,
                        self.days as f32,
                        s_max as f32,
                        max_checks,
                    );
                    (candidate.key(), summary)
                })
                .collect()
        })
    }

    pub fn dr_summary_many(
        &self,
        candidates: &[Candidate],
        start_weight: f64,
        prune_weight: f64,
    ) -> Vec<(PointKey, DrSummary)> {
        self.pool.install(|| {
            candidates
                .par_iter()
                .map(|candidate| {
                    let adr = FSRSADR::linear(
                        candidate.flat as f32,
                        candidate.s_multi as f32,
                        candidate.d_multi as f32,
                    );
                    let summary = dr_summary_by_weight(
                        &self.fsrs,
                        &adr,
                        &self.behavior_model,
                        self.days as f32,
                        start_weight as f32,
                        prune_weight as f32,
                    );
                    (candidate.key(), summary)
                })
                .collect()
        })
    }

    pub fn attach_dr_summary(&self, points: Vec<Point>, weight: f64, config: &SearchConfig) -> Vec<Point> {
        if points.is_empty() {
            return points;
        }
        let mut unique = HashMap::<PointKey, Candidate>::new();
        for point in &points {
            unique.insert(point.key(), point.candidate());
        }
        let candidates: Vec<_> = unique.values().copied().collect();
        let summaries: HashMap<_, _> = self
            .dr_summary_many(&candidates, weight, config.dr_prune_weight)
            .into_iter()
            .collect();
        points
            .into_iter()
            .map(|mut point| {
                if let Some(summary) = summaries.get(&point.key()) {
                    point.dr_samples = summary.samples;
                    point.dr_p10 = summary.dr_p10 as f64;
                    point.dr_mean = summary.dr_mean as f64;
                    point.dr_p90 = summary.dr_p90 as f64;
                    point.dr_spread = summary.aggression as f64;
                }
                point
            })
            .collect()
    }
}

pub fn run_batch(config: &SearchConfig) -> Result<Vec<SearchResult>, String> {
    let (export_path, rows) = load_export_rows(&config.export)?;
    let mut results = Vec::new();
    for preset in &config.presets {
        let row = select_export_row(&export_path, &rows, preset)?;
        results.push(run_one(config, row)?);
    }
    Ok(results)
}

pub fn run_configured_batch(configs: &[SearchConfig]) -> Result<Vec<SearchResult>, String> {
    let mut export_cache = HashMap::<PathBuf, (PathBuf, Vec<Value>)>::new();
    let mut results = Vec::new();
    for config in configs {
        let (export_path, rows) = match export_cache.get(&config.export) {
            Some(cached) => cached,
            None => {
                let loaded = load_export_rows(&config.export)?;
                export_cache.insert(config.export.clone(), loaded);
                export_cache
                    .get(&config.export)
                    .expect("just inserted export cache entry")
            }
        };
        for preset in &config.presets {
            let row = select_export_row(export_path, rows, preset)?;
            results.push(run_one(config, row)?);
        }
    }
    Ok(results)
}

fn run_one(config: &SearchConfig, row: ExportRow) -> Result<SearchResult, String> {
    let target_dr = parse_dr(config.target_dr.or_else(|| row.desired_retention()).ok_or(
        "target DR was not provided and export row is missing desired_retention",
    )?)?;
    let refs = reference_labels(config);
    print_intro(&row, target_dr, config);

    let engine = EvalEngine::new(&row, config)?;
    let global_start = Instant::now();
    let mut store = PointStore::default();
    let mut fixed = FixedCurveManager::build(&engine, config, target_dr)?;

    let (phase1, _phase1_promoted, phase1_diags) =
        run_phase1(&engine, &mut fixed, config, &mut store)?;
    fixed.ensure_for_points(
        &phase1,
        "phase1",
        config.seed + 760,
        config.scout_potential_band_pct,
    )?;
    let phase1_promoted = select_promotions(
        &phase1,
        &fixed.target_fixed,
        &fixed.fixed_curve,
        &fixed.fixed_env,
        fixed.target_dr,
        config,
        config.scout_potential_band_pct,
        true,
        false,
    )
    .0;

    let phase2_result = run_refinement_phase(
        &engine,
        "phase2",
        &phase1_promoted,
        &[],
        config.phase1_eval_weight,
        config.phase2_eval_weight,
        (
            config.phase2_flat_step,
            config.phase2_s_step,
            config.phase2_d_step,
        ),
        config.seed + 2000,
        config.seed + 770,
        &mut fixed,
        config,
        &mut store,
        false,
    )?;

    let phase3_result = run_refinement_phase(
        &engine,
        "phase3",
        &phase2_result.promoted,
        &phase2_result.points,
        config.phase2_eval_weight,
        config.phase3_eval_weight,
        (
            config.phase3_flat_step,
            config.phase3_s_step,
            config.phase3_d_step,
        ),
        config.seed + 3000,
        config.seed + 780,
        &mut fixed,
        config,
        &mut store,
        true,
    )?;
    let phase3_pool = dedupe_points(
        phase2_result
            .points
            .iter()
            .cloned()
            .chain(phase3_result.points.iter().cloned())
            .collect::<Vec<_>>(),
    );

    let phase4_seeds = phase4_seed_profiles(&phase3_result.promoted, &fixed, config);
    let (phase4, diag4) =
        run_micro_hillclimb(&engine, &phase4_seeds, &phase3_pool, &fixed, config, &mut store);
    fixed.ensure_for_points(
        &phase4,
        "phase4",
        config.seed + 790,
        config.scout_potential_band_pct,
    )?;
    println!(
        "[phase 4] seeds={} visited={} elapsed={:.1}s",
        phase4_seeds.len(),
        phase4.len(),
        diag4.elapsed_s
    );

    let start = Instant::now();
    let remote_pool = dedupe_points(
        phase3_result
            .promoted
            .iter()
            .cloned()
            .chain(phase4.iter().cloned())
            .collect::<Vec<_>>(),
    );
    let all_computed_pool = dedupe_points(
        phase1
            .iter()
            .cloned()
            .chain(phase2_result.points.iter().cloned())
            .chain(phase3_result.points.iter().cloned())
            .chain(phase4.iter().cloned())
            .chain(phase3_result.render_extra.iter().cloned())
            .collect::<Vec<_>>(),
    );
    fixed.ensure_for_points(
        &remote_pool,
        "prefinal",
        config.seed + 795,
        config.final_potential_band_pct,
    )?;
    let (final_candidates, max_spread_prefinal) =
        choose_final_candidates(&remote_pool, &all_computed_pool, &fixed, &refs, config);
    fixed.ensure_for_points(
        &max_spread_prefinal,
        "maxspread",
        config.seed + 797,
        config.final_potential_band_pct,
    )?;
    let final_points = evaluate_unique(
        &engine,
        final_candidates.clone(),
        config.final_eval_weight,
        config.seed + 5000,
        config,
        &mut store,
        false,
    );
    fixed.ensure_for_points(
        &final_points,
        "final",
        config.seed + 799,
        config.final_potential_band_pct,
    )?;
    let final_points = engine.attach_dr_summary(final_points, config.final_eval_weight, config);
    let mut diag5 = phase_diag(
        "phase5.final",
        config.final_eval_weight,
        start,
        &final_points,
        Some(serde_json::json!({ "generated": final_candidates.len() })),
    );
    diag5.candidates = final_candidates.len();
    println!(
        "[phase 5] candidates={} evaluated={} elapsed={:.1}s",
        final_candidates.len(),
        final_points.len(),
        diag5.elapsed_s
    );

    let selected = add_reference_labels(choose_points(&final_points, &fixed, config), &final_points, &refs);
    let grouped_labels = labels_by_key(&selected);
    let selected_values: Vec<_> = selected.values().cloned().collect();
    let selected_metrics = equivalence_map(&selected_values, &fixed.fixed_env);

    let point_only_label = point_only_label(config);
    let mut diagnostics = fixed.diagnostics.clone();
    diagnostics.extend(phase1_diags);
    diagnostics.push(phase2_result.diag.clone());
    diagnostics.push(phase3_result.diag.clone());
    diagnostics.push(diag4.clone());
    diagnostics.push(diag5.clone());

    if let Some(label) = point_only_label {
        let txt_path = write_point_only(label, &selected, &selected_metrics, config, &row)?;
        println!();
        let text = std::fs::read_to_string(&txt_path)
            .map_err(|err| format!("failed to read {}: {err}", txt_path.display()))?;
        println!("{}", text.trim_end());
        return Ok(SearchResult {
            plot_path: txt_path.clone(),
            summary_path: txt_path,
            selected_by_label: selected,
            labels_by_key: grouped_labels,
            diagnostics,
        });
    }

    let plot_path = if config.matplotlib {
        png_output_path(config, &row)?
    } else {
        html_output_path(config, &row)?
    };
    let summary_path = plot_path.with_extension("json");
    let final_frontier = pareto_frontier(&safety_pool(&final_points, config));
    let mut plot_layers = HashMap::new();
    plot_layers.insert(
        "phase1_safe",
        phase1.iter().filter(|point| point.safe()).cloned().collect::<Vec<_>>(),
    );
    plot_layers.insert(
        "phase1_unsafe",
        phase1.iter().filter(|point| !point.safe()).cloned().collect::<Vec<_>>(),
    );
    plot_layers.insert("phase2", phase2_result.points.clone());
    plot_layers.insert("phase3", phase3_result.points.clone());
    plot_layers.insert("phase4", phase4.clone());
    plot_layers.insert("phase3_render_extra", phase3_result.render_extra.clone());
    write_summary(
        &summary_path,
        &row,
        target_dr,
        config,
        &selected,
        &grouped_labels,
        &selected_metrics,
        &fixed.fixed_curve,
        &fixed.refined_points_by_pct,
        &fixed.fixed_env.points,
        &final_frontier,
        &phase3_result.render_extra,
        &max_spread_prefinal,
        &diagnostics,
        &plot_layers,
    )?;
    if config.matplotlib {
        write_png(
            &plot_path,
            &phase1,
            &phase2_result.points,
            &phase3_result.points,
            &phase4,
            &final_points,
            &fixed,
            config,
            &row,
        )?;
    } else {
        write_plot_html(&plot_path, &summary_path)?;
    }

    print_results(
        &selected,
        &selected_metrics,
        &diagnostics,
        &plot_path,
        &summary_path,
        global_start.elapsed().as_secs_f64(),
        config,
    );
    Ok(SearchResult {
        plot_path,
        summary_path,
        selected_by_label: selected,
        labels_by_key: grouped_labels,
        diagnostics,
    })
}

fn point_only_label(config: &SearchConfig) -> Option<ResultLabel> {
    if config.recommended_only {
        Some(ResultLabel::Recommended)
    } else if config.aggressive_only {
        Some(ResultLabel::Aggressive)
    } else if config.calm_only {
        Some(ResultLabel::Calm)
    } else {
        None
    }
}

fn configured_threads(requested: usize) -> usize {
    if requested > 0 {
        requested
    } else {
        std::thread::available_parallelism().map_or(1, usize::from)
    }
}
