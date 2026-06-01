use crate::config::SearchConfig;
use crate::search::batch::EvalEngine;
use crate::search::candidates::{
    add_existing_bridge_midpoints, bridge_midpoint_neighborhoods, dedupe_candidates, dedupe_points,
    hypercube_candidates, make_phase1_domain, phase1_boundary_directions, phase1_candidates,
    should_include_hypercube_center, PointStore,
};
use crate::search::fixed_curve::FixedCurveManager;
use crate::search::rank::{
    band_for_dr, classify_ranked, compare_tuple_desc, equivalence_sort_key,
    fixed_curve_equivalence, point_equivalence_key, select_promotions, MetricAttr,
};
use crate::search::safety::{attach_safety_from_rows, safety_by_key, safety_row_is_safe};
use crate::types::{Candidate, PhaseDiag, Point, PointKey};
use serde_json::json;
use std::collections::HashMap;
use std::time::Instant;

pub fn phase_diag(
    name: &str,
    weight: f64,
    start: Instant,
    points: &[Point],
    notes: Option<serde_json::Value>,
) -> PhaseDiag {
    let mut diag = PhaseDiag::new(name, weight, points, start.elapsed().as_secs_f64());
    diag.notes = notes;
    diag
}

pub fn evaluate_unique(
    engine: &EvalEngine,
    candidates: Vec<Candidate>,
    weight: f64,
    seed: u64,
    config: &SearchConfig,
    store: &mut PointStore,
    enforce_quadrant: bool,
) -> Vec<Point> {
    let mut unique = dedupe_candidates(candidates, enforce_quadrant);
    unique = store.missing_or_lower_weight(unique, weight);
    if unique.is_empty() {
        return Vec::new();
    }
    let points = engine.evaluate_search(&unique, weight, seed, config);
    store.add(&points, weight);
    points
}

pub struct Phase1Eval {
    pub points: Vec<Point>,
    pub skipped_keys: Vec<PointKey>,
    pub candidate_count: usize,
    pub evaluated_candidates: usize,
    pub safety_prescreen: bool,
    pub screened_unsafe: usize,
}

pub fn evaluate_phase1_unique(
    engine: &EvalEngine,
    candidates: Vec<Candidate>,
    weight: f64,
    seed: u64,
    config: &SearchConfig,
    store: &mut PointStore,
) -> Phase1Eval {
    let mut unique = dedupe_candidates(candidates, true);
    unique = store.missing_or_lower_weight(unique, weight);
    if unique.is_empty() {
        return Phase1Eval {
            points: Vec::new(),
            skipped_keys: Vec::new(),
            candidate_count: 0,
            evaluated_candidates: 0,
            safety_prescreen: false,
            screened_unsafe: 0,
        };
    }
    if config.ignore_safety || config.legacy_unsafe_plot_display {
        let points = engine.evaluate_search(&unique, weight, seed, config);
        store.add(&points, weight);
        return Phase1Eval {
            candidate_count: unique.len(),
            evaluated_candidates: unique.len(),
            points,
            skipped_keys: Vec::new(),
            safety_prescreen: false,
            screened_unsafe: 0,
        };
    }

    let safety = safety_by_key(engine, &unique, config);
    let mut safe_candidates = Vec::new();
    let mut skipped_keys = Vec::new();
    for candidate in unique.iter().copied() {
        let key = candidate.key();
        if safety.get(&key).is_some_and(safety_row_is_safe) {
            safe_candidates.push(candidate);
        } else {
            skipped_keys.push(key);
        }
    }
    if safe_candidates.is_empty() {
        return Phase1Eval {
            points: Vec::new(),
            skipped_keys,
            candidate_count: unique.len(),
            evaluated_candidates: 0,
            safety_prescreen: true,
            screened_unsafe: unique.len(),
        };
    }
    let points = attach_safety_from_rows(engine.evaluate_raw(&safe_candidates, weight, seed), &safety);
    store.add(&points, weight);
    Phase1Eval {
        candidate_count: unique.len(),
        evaluated_candidates: safe_candidates.len(),
        screened_unsafe: skipped_keys.len(),
        points,
        skipped_keys,
        safety_prescreen: true,
    }
}

pub fn run_phase1(
    engine: &EvalEngine,
    fixed: &mut FixedCurveManager<'_>,
    config: &SearchConfig,
    store: &mut PointStore,
) -> Result<(Vec<Point>, Vec<Point>, Vec<PhaseDiag>), String> {
    let mut all_points: Vec<Point> = Vec::new();
    let mut all_points_by_key: HashMap<PointKey, usize> = HashMap::new();
    let mut screened_unsafe_keys: HashMap<PointKey, ()> = HashMap::new();
    let mut diagnostics = Vec::new();
    let mut domain = make_phase1_domain(config, fixed.target_dr);

    for round_idx in 0..=config.phase1_expand_rounds {
        let start = Instant::now();
        let candidates = phase1_candidates(&domain);
        let new_candidates: Vec<_> = candidates
            .into_iter()
            .filter(|candidate| {
                !all_points_by_key.contains_key(&candidate.key())
                    && !screened_unsafe_keys.contains_key(&candidate.key())
            })
            .collect();
        let result = evaluate_phase1_unique(
            engine,
            new_candidates,
            config.phase1_eval_weight,
            config.seed + round_idx as u64,
            config,
            store,
        );
        for key in &result.skipped_keys {
            screened_unsafe_keys.insert(*key, ());
        }
        let new_points = result.points;
        for point in &new_points {
            if let Some(idx) = all_points_by_key.get(&point.key()).copied() {
                all_points[idx] = point.clone();
            } else {
                all_points_by_key.insert(point.key(), all_points.len());
                all_points.push(point.clone());
            }
        }
        let pool = all_points.clone();
        let (promoted, _) = select_promotions(
            &pool,
            &fixed.target_fixed,
            &fixed.fixed_curve,
            &fixed.fixed_env,
            fixed.target_dr,
            config,
            config.scout_potential_band_pct,
            true,
            false,
        );
        let directions = phase1_boundary_directions(&promoted, &domain);
        let directions_display = if directions.is_empty() {
            "-".to_string()
        } else {
            directions.join(",")
        };
        let mut changed = HashMap::new();
        if config.phase1_expand && !directions.is_empty() && round_idx < config.phase1_expand_rounds {
            changed = domain.expand(&directions, config.phase1_expand_batch.max(1));
        }
        let mut diag = phase_diag(
            &format!("phase1.{round_idx}"),
            config.phase1_eval_weight,
            start,
            &new_points,
            Some(json!({
                "total_pool": pool.len(),
                "promoted": promoted.len(),
                "boundary": directions,
                "expanded": changed,
                "screened_unsafe": result.screened_unsafe,
                "safety_prescreen": result.safety_prescreen,
            })),
        );
        diag.candidates = result.candidate_count;
        diag.evaluated = result.evaluated_candidates;
        diag.promoted = promoted.len();
        let screened_note = if result.screened_unsafe > 0 {
            format!(" screened_unsafe={}", result.screened_unsafe)
        } else {
            String::new()
        };
        println!(
            "[phase 1.{round_idx}] new={}{} pool={} promote={} boundary={} elapsed={:.1}s",
            diag.evaluated,
            screened_note,
            pool.len(),
            promoted.len(),
            directions_display,
            diag.elapsed_s
        );
        diagnostics.push(diag);
        if changed.is_empty() {
            break;
        }
    }
    let all = all_points;
    let (final_promoted, _) = select_promotions(
        &all,
        &fixed.target_fixed,
        &fixed.fixed_curve,
        &fixed.fixed_env,
        fixed.target_dr,
        config,
        config.scout_potential_band_pct,
        true,
        false,
    );
    Ok((all, final_promoted, diagnostics))
}

pub struct RefinementResult {
    pub points: Vec<Point>,
    pub promoted: Vec<Point>,
    pub render_extra: Vec<Point>,
    pub diag: PhaseDiag,
}

#[allow(clippy::too_many_arguments)]
pub fn run_refinement_phase(
    engine: &EvalEngine,
    phase_name: &str,
    centers: &[Point],
    base_pool: &[Point],
    previous_weight: f64,
    current_weight: f64,
    steps: (f64, f64, f64),
    seed: u64,
    adapt_seed: u64,
    fixed: &mut FixedCurveManager<'_>,
    config: &SearchConfig,
    store: &mut PointStore,
    pareto_as_render_only: bool,
) -> Result<RefinementResult, String> {
    let start = Instant::now();
    let include_center = should_include_hypercube_center(current_weight, previous_weight);
    let candidates = hypercube_candidates(centers, steps, include_center);
    let mut points = evaluate_unique(engine, candidates.clone(), current_weight, seed, config, store, true);
    let mut pool = dedupe_points(base_pool.iter().cloned().chain(points.iter().cloned()).collect::<Vec<_>>());
    fixed.ensure_for_points(&pool, phase_name, adapt_seed, config.scout_potential_band_pct)?;
    let (mut promoted, render_extra) = select_promotions(
        &pool,
        &fixed.target_fixed,
        &fixed.fixed_curve,
        &fixed.fixed_env,
        fixed.target_dr,
        config,
        config.scout_potential_band_pct,
        true,
        pareto_as_render_only,
    );
    let (bridged, bridge_points, bridge_generated) = apply_bridge_promotions(
        engine,
        &promoted,
        &pool,
        steps,
        current_weight,
        seed + 9100,
        config,
        store,
    );
    promoted = bridged;
    if !bridge_points.is_empty() {
        points = dedupe_points(points.into_iter().chain(bridge_points.iter().cloned()).collect::<Vec<_>>());
        pool = dedupe_points(pool.into_iter().chain(bridge_points.iter().cloned()).collect::<Vec<_>>());
        fixed.ensure_for_points(
            &bridge_points,
            &format!("{phase_name}.bridge"),
            adapt_seed + 20,
            config.scout_potential_band_pct,
        )?;
    }
    let mut diag = phase_diag(
        &format!("{phase_name}.hypercube"),
        current_weight,
        start,
        &points,
        Some(json!({
            "generated": candidates.len(),
            "include_center": include_center,
            "pool": pool.len(),
            "bridge_generated": bridge_generated,
            "bridge_evaluated": bridge_points.len(),
        })),
    );
    diag.candidates = candidates.len();
    diag.promoted = promoted.len();
    diag.pareto_extra = render_extra.len();
    println!(
        "[{}] candidates={} new={} pool={} promote={} render_extra={} bridge_eval={} elapsed={:.1}s",
        phase_name.replace("phase", "phase "),
        candidates.len(),
        points.len(),
        pool.len(),
        promoted.len(),
        render_extra.len(),
        bridge_points.len(),
        diag.elapsed_s
    );
    Ok(RefinementResult {
        points,
        promoted,
        render_extra,
        diag,
    })
}

fn apply_bridge_promotions(
    engine: &EvalEngine,
    promoted: &[Point],
    pool: &[Point],
    steps: (f64, f64, f64),
    weight: f64,
    seed: u64,
    config: &SearchConfig,
    store: &mut PointStore,
) -> (Vec<Point>, Vec<Point>, usize) {
    let bridged = add_existing_bridge_midpoints(promoted, pool, steps, config.bridge_midpoint_limit);
    if !config.experimental_bridge_midpoint_neighborhoods {
        return (bridged, Vec::new(), 0);
    }
    let candidates = bridge_midpoint_neighborhoods(&bridged, steps);
    let points = evaluate_unique(engine, candidates.clone(), weight, seed, config, store, true);
    if points.is_empty() {
        return (bridged, Vec::new(), candidates.len());
    }
    let promoted = dedupe_points(bridged.into_iter().chain(points.iter().cloned()).collect::<Vec<_>>());
    (promoted, points, candidates.len())
}

pub fn phase4_seed_profiles(
    pool: &[Point],
    fixed: &FixedCurveManager<'_>,
    config: &SearchConfig,
) -> Vec<(String, Point)> {
    let ranked = classify_ranked(
        pool,
        &fixed.target_fixed,
        &fixed.fixed_curve,
        &fixed.fixed_env,
        fixed.target_dr,
        config.scout_potential_band_pct,
        config,
    );
    let mut seeds = Vec::new();
    for (label, points) in [
        ("recommended", ranked.recommended),
        ("efficiency", ranked.efficiency),
        ("memory", ranked.memory),
        ("frontier", ranked.frontier),
    ] {
        for point in points.into_iter().take(config.phase4_seeds_per_objective) {
            seeds.push((label.to_string(), point));
        }
    }
    let mut unique = HashMap::new();
    let mut out = Vec::new();
    for (label, point) in seeds {
        let key = (label.clone(), point.key());
        if !unique.contains_key(&key) {
            unique.insert(key, out.len());
            out.push((label, point));
        }
    }
    out
}

pub fn run_micro_hillclimb(
    engine: &EvalEngine,
    seeds: &[(String, Point)],
    starting_pool: &[Point],
    fixed: &FixedCurveManager<'_>,
    config: &SearchConfig,
    store: &mut PointStore,
) -> (Vec<Point>, PhaseDiag) {
    let start = Instant::now();
    let mut evaluated: HashMap<PointKey, Point> =
        starting_pool.iter().map(|point| (point.key(), point.clone())).collect();
    let mut visited = Vec::new();
    let mut visited_by_key: HashMap<PointKey, usize> = HashMap::new();
    let steps = (config.phase4_flat_step, config.phase4_s_step, config.phase4_d_step);
    let mut eval_count = 0_u64;

    for (label, seed_point) in seeds {
        let Some(mut current) = evaluated.get(&seed_point.key()).cloned() else {
            continue;
        };
        upsert_point(&mut visited, &mut visited_by_key, current.clone());
        for step_idx in 0..config.phase4_max_steps {
            let neighbors = hypercube_candidates(&[current.clone()], steps, false);
            let missing: Vec<_> = neighbors
                .iter()
                .copied()
                .filter(|candidate| !evaluated.contains_key(&candidate.key()))
                .collect();
            let new_points = evaluate_unique(
                engine,
                missing,
                config.phase4_eval_weight,
                config.seed + 4000 + eval_count + step_idx as u64,
                config,
                store,
                true,
            );
            eval_count += new_points.len() as u64;
            for point in new_points {
                evaluated.insert(point.key(), point.clone());
                upsert_point(&mut visited, &mut visited_by_key, point);
            }
            let mut neighbor_points: Vec<_> = neighbors
                .iter()
                .filter_map(|candidate| evaluated.get(&candidate.key()).cloned())
                .collect();
            neighbor_points.push(current.clone());
            let best = neighbor_points
                .into_iter()
                .max_by(|a, b| compare_objective(label, a, b, fixed, config))
                .expect("current is present");
            if compare_objective(label, &best, &current, fixed, config) != std::cmp::Ordering::Greater {
                break;
            }
            current = best;
            upsert_point(&mut visited, &mut visited_by_key, current.clone());
        }
    }
    let points = visited;
    let mut diag = phase_diag(
        "phase4.microhill",
        config.phase4_eval_weight,
        start,
        &points,
        Some(json!({ "seeds": seeds.len() })),
    );
    diag.candidates = points.len();
    (points, diag)
}

fn upsert_point(points: &mut Vec<Point>, index: &mut HashMap<PointKey, usize>, point: Point) {
    if let Some(idx) = index.get(&point.key()).copied() {
        points[idx] = point;
    } else {
        index.insert(point.key(), points.len());
        points.push(point);
    }
}

fn compare_objective(
    label: &str,
    a: &Point,
    b: &Point,
    fixed: &FixedCurveManager<'_>,
    config: &SearchConfig,
) -> std::cmp::Ordering {
    let x_band = band_for_dr(
        &fixed.fixed_curve,
        fixed.target_dr,
        config.scout_potential_band_pct,
        MetricAttr::MemorizedCards,
    );
    let y_band = band_for_dr(
        &fixed.fixed_curve,
        fixed.target_dr,
        config.scout_potential_band_pct,
        MetricAttr::MemorizedPerMinute,
    );
    let x0 = fixed.target_fixed.memorized_cards;
    let y0 = fixed.target_fixed.memorized_per_minute;
    let ma = fixed_curve_equivalence(a, &fixed.fixed_env);
    let mb = fixed_curve_equivalence(b, &fixed.fixed_env);
    let spread_a = equivalence_sort_key(&ma);
    let spread_b = equivalence_sort_key(&mb);

    match label {
        "recommended" => {
            let in_a = a.memorized_cards > x0 && a.memorized_per_minute > y0;
            let in_b = b.memorized_cards > x0 && b.memorized_per_minute > y0;
            (in_a as i32)
                .cmp(&(in_b as i32))
                .then_with(|| compare_spread(spread_a, spread_b))
                .then_with(|| a.memorized_per_minute.total_cmp(&b.memorized_per_minute))
                .then_with(|| a.memorized_cards.total_cmp(&b.memorized_cards))
        }
        "efficiency" => {
            let in_a = x_band.0 <= a.memorized_cards && a.memorized_cards <= x_band.1;
            let in_b = x_band.0 <= b.memorized_cards && b.memorized_cards <= x_band.1;
            (in_a as i32)
                .cmp(&(in_b as i32))
                .then_with(|| a.memorized_per_minute.total_cmp(&b.memorized_per_minute))
                .then_with(|| compare_spread(spread_a, spread_b))
                .then_with(|| (-(a.memorized_cards - x0).abs()).total_cmp(&(-(b.memorized_cards - x0).abs())))
        }
        "memory" => {
            let in_a = y_band.0 <= a.memorized_per_minute && a.memorized_per_minute <= y_band.1;
            let in_b = y_band.0 <= b.memorized_per_minute && b.memorized_per_minute <= y_band.1;
            (in_a as i32)
                .cmp(&(in_b as i32))
                .then_with(|| a.memorized_cards.total_cmp(&b.memorized_cards))
                .then_with(|| compare_spread(spread_a, spread_b))
                .then_with(|| (-(a.memorized_per_minute - y0).abs()).total_cmp(&(-(b.memorized_per_minute - y0).abs())))
        }
        _ => compare_tuple_desc(
            point_equivalence_key(a, &HashMap::from([(a.key(), ma.clone()), (b.key(), mb.clone())])),
            point_equivalence_key(b, &HashMap::from([(a.key(), ma), (b.key(), mb)])),
        ),
    }
}

fn compare_spread(a: (f64, i32, f64, f64), b: (f64, i32, f64, f64)) -> std::cmp::Ordering {
    a.0.total_cmp(&b.0)
        .then_with(|| a.1.cmp(&b.1))
        .then_with(|| a.2.total_cmp(&b.2))
        .then_with(|| a.3.total_cmp(&b.3))
}
