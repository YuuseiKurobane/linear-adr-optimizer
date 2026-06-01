use crate::config::SearchConfig;
use crate::search::candidates::{dedupe_candidates, dedupe_point_refs};
use crate::search::fixed_curve::FixedCurveManager;
use crate::search::rank::{
    band_for_dr, classify_ranked, compare_tuple_desc, equivalence_map, equivalence_sort_key,
    point_equivalence_key, MetricAttr,
};
use crate::search::safety::safety_pool;
use crate::types::{Candidate, Point, PointKey};
use std::collections::HashMap;

pub fn reference_labels(config: &SearchConfig) -> HashMap<PointKey, Vec<String>> {
    let mut labels: HashMap<PointKey, Vec<String>> = HashMap::new();
    if config.include_original {
        labels
            .entry(config.original.snap().key())
            .or_default()
            .push("Original".to_string());
    }
    for (idx, values) in config.inspect_point.iter().enumerate() {
        labels
            .entry(values.snap().key())
            .or_default()
            .push(format!("Inspect {}", idx + 1));
    }
    labels
}

pub fn reference_candidates(labels_by_key: &HashMap<PointKey, Vec<String>>) -> Vec<Candidate> {
    labels_by_key.keys().map(|key| key.as_candidate()).collect()
}

pub fn choose_final_candidates(
    pool: &[Point],
    max_spread_pool: &[Point],
    fixed: &FixedCurveManager<'_>,
    refs: &HashMap<PointKey, Vec<String>>,
    config: &SearchConfig,
) -> (Vec<Candidate>, Vec<Point>) {
    let ranked = classify_ranked(
        pool,
        &fixed.target_fixed,
        &fixed.fixed_curve,
        &fixed.fixed_env,
        fixed.target_dr,
        config.final_potential_band_pct,
        config,
    );
    let mut selected = Vec::new();
    let mut selected_keys: HashMap<PointKey, usize> = HashMap::new();
    add_top(
        &mut selected,
        &mut selected_keys,
        &ranked.recommended,
        config.promote_recommended.max(config.final_shortlist_recommended),
    );
    add_top(
        &mut selected,
        &mut selected_keys,
        &ranked.efficiency,
        config.promote_efficiency_potential.max(config.final_shortlist_efficiency),
    );
    add_top(
        &mut selected,
        &mut selected_keys,
        &ranked.memory,
        config.promote_memory_potential.max(config.final_shortlist_memory),
    );
    add_top(&mut selected, &mut selected_keys, &ranked.frontier, config.final_shortlist_frontier);

    let metrics = equivalence_map(&selected, &fixed.fixed_env);
    let mut ordered = selected;
    ordered.sort_by(|a, b| {
        compare_tuple_desc(point_equivalence_key(a, &metrics), point_equivalence_key(b, &metrics)).reverse()
    });
    let mut candidates: Vec<_> = ordered
        .iter()
        .take(config.final_candidate_limit)
        .map(Point::candidate)
        .collect();

    let max_spread_prefinal = max_spread_points(
        max_spread_pool,
        fixed,
        config,
        config.max_spread_final_candidates,
    );
    candidates.extend(max_spread_prefinal.iter().map(Point::candidate));
    candidates.extend(reference_candidates(refs));
    (dedupe_candidates(candidates, false), max_spread_prefinal)
}

pub fn choose_points(
    final_points: &[Point],
    fixed: &FixedCurveManager<'_>,
    config: &SearchConfig,
) -> HashMap<String, Point> {
    let pool = safety_pool(final_points, config);
    if pool.is_empty() {
        return HashMap::new();
    }
    let metrics = equivalence_map(&pool, &fixed.fixed_env);
    let x0 = fixed.target_fixed.memorized_cards;
    let y0 = fixed.target_fixed.memorized_per_minute;
    let x_band = band_for_dr(
        &fixed.fixed_curve,
        fixed.target_dr,
        config.final_potential_band_pct,
        MetricAttr::MemorizedCards,
    );
    let y_band = band_for_dr(
        &fixed.fixed_curve,
        fixed.target_dr,
        config.final_potential_band_pct,
        MetricAttr::MemorizedPerMinute,
    );
    let mut selected = HashMap::new();
    let northeast_pool: Vec<_> = pool
        .iter()
        .filter(|point| point.memorized_cards > x0 && point.memorized_per_minute > y0)
        .cloned()
        .collect();
    if !northeast_pool.is_empty() {
        let recommended = northeast_pool
            .iter()
            .max_by(|a, b| compare_tuple_desc(point_equivalence_key(a, &metrics), point_equivalence_key(b, &metrics)))
            .expect("nonempty")
            .clone();
        selected.insert("Recommended".to_string(), recommended.clone());

        let spread_floor =
            metrics[&recommended.key()].spread_floor - config.aggressive_calm_regret_pct / 100.0;
        let mut aggressive_calm_pool: Vec<_> = northeast_pool
            .iter()
            .filter(|point| metrics[&point.key()].spread_floor >= spread_floor)
            .cloned()
            .collect();
        if aggressive_calm_pool.is_empty() {
            aggressive_calm_pool.push(recommended.clone());
        }
        let aggressive = aggressive_calm_pool
            .iter()
            .max_by(|a, b| compare_aggressive(a, b, &metrics))
            .expect("nonempty")
            .clone();
        selected.insert("Aggressive".to_string(), aggressive);
        let calm = aggressive_calm_pool
            .iter()
            .min_by(|a, b| compare_calm(a, b, &metrics))
            .expect("nonempty")
            .clone();
        selected.insert("Calm".to_string(), calm);
    }

    let efficiency_pool: Vec<_> = pool
        .iter()
        .filter(|point| x_band.0 <= point.memorized_cards && point.memorized_cards <= x_band.1)
        .cloned()
        .collect();
    let efficiency_pool = if efficiency_pool.is_empty() { pool.clone() } else { efficiency_pool };
    let efficiency = efficiency_pool
        .iter()
        .max_by(|a, b| {
            compare_eff_mem_key(
                (
                    a.memorized_per_minute,
                    point_equivalence_key(a, &metrics),
                    -(a.memorized_cards - x0).abs(),
                ),
                (
                    b.memorized_per_minute,
                    point_equivalence_key(b, &metrics),
                    -(b.memorized_cards - x0).abs(),
                ),
            )
        })
        .expect("nonempty")
        .clone();
    selected.insert("Efficiency Potential".to_string(), efficiency);

    let memory_pool: Vec<_> = pool
        .iter()
        .filter(|point| y_band.0 <= point.memorized_per_minute && point.memorized_per_minute <= y_band.1)
        .cloned()
        .collect();
    let memory_pool = if memory_pool.is_empty() { pool.clone() } else { memory_pool };
    let memory = memory_pool
        .iter()
        .max_by(|a, b| {
            compare_eff_mem_key(
                (
                    a.memorized_cards,
                    point_equivalence_key(a, &metrics),
                    -(a.memorized_per_minute - y0).abs(),
                ),
                (
                    b.memorized_cards,
                    point_equivalence_key(b, &metrics),
                    -(b.memorized_per_minute - y0).abs(),
                ),
            )
        })
        .expect("nonempty")
        .clone();
    selected.insert("Memory Potential".to_string(), memory);

    let max_spread = pool
        .iter()
        .max_by(|a, b| compare_max_spread(a, b, &metrics))
        .expect("nonempty")
        .clone();
    selected.insert("Max Spread".to_string(), max_spread);
    selected
}

pub fn add_reference_labels(
    selected_by_label: HashMap<String, Point>,
    final_points: &[Point],
    refs: &HashMap<PointKey, Vec<String>>,
) -> HashMap<String, Point> {
    let final_by_key: HashMap<_, _> = final_points.iter().map(|point| (point.key(), point.clone())).collect();
    let mut out = selected_by_label;
    for (key, labels) in refs {
        if let Some(point) = final_by_key.get(key) {
            for label in labels {
                out.insert(label.clone(), point.clone());
            }
        }
    }
    out
}

pub fn labels_by_key(selected_by_label: &HashMap<String, Point>) -> HashMap<PointKey, Vec<String>> {
    let mut grouped: HashMap<PointKey, Vec<String>> = HashMap::new();
    for (label, point) in selected_by_label {
        grouped.entry(point.key()).or_default().push(label.clone());
    }
    grouped
}

pub fn max_spread_points(
    points: &[Point],
    fixed: &FixedCurveManager<'_>,
    config: &SearchConfig,
    limit: usize,
) -> Vec<Point> {
    let deduped = dedupe_point_refs(points.iter());
    let pool = safety_pool(&deduped, config);
    if pool.is_empty() || limit == 0 {
        return Vec::new();
    }
    let metrics = equivalence_map(&pool, &fixed.fixed_env);
    let mut out = pool;
    out.sort_by(|a, b| compare_max_spread(a, b, &metrics).reverse());
    out.truncate(limit);
    out
}

fn add_top(
    selected: &mut Vec<Point>,
    selected_keys: &mut HashMap<PointKey, usize>,
    ranked: &[Point],
    limit: usize,
) {
    let mut count = 0;
    for point in ranked {
        if !selected_keys.contains_key(&point.key()) {
            count += 1;
            selected_keys.insert(point.key(), selected.len());
            selected.push(point.clone());
        } else if let Some(idx) = selected_keys.get(&point.key()).copied() {
            selected[idx] = point.clone();
        }
        if count >= limit {
            break;
        }
    }
}

fn compare_aggressive(
    a: &Point,
    b: &Point,
    metrics: &HashMap<PointKey, crate::types::FixedCurveEquivalence>,
) -> std::cmp::Ordering {
    let sa = if a.dr_samples > 0 { a.dr_spread } else { f64::NEG_INFINITY };
    let sb = if b.dr_samples > 0 { b.dr_spread } else { f64::NEG_INFINITY };
    sa.total_cmp(&sb)
        .then_with(|| compare_spread_tuple(equivalence_sort_key(&metrics[&a.key()]), equivalence_sort_key(&metrics[&b.key()])))
        .then_with(|| a.memorized_per_minute.total_cmp(&b.memorized_per_minute))
        .then_with(|| a.memorized_cards.total_cmp(&b.memorized_cards))
}

fn compare_calm(
    a: &Point,
    b: &Point,
    metrics: &HashMap<PointKey, crate::types::FixedCurveEquivalence>,
) -> std::cmp::Ordering {
    let sa = if a.dr_samples > 0 { a.dr_spread } else { f64::INFINITY };
    let sb = if b.dr_samples > 0 { b.dr_spread } else { f64::INFINITY };
    sa.total_cmp(&sb)
        .then_with(|| compare_spread_tuple(equivalence_sort_key(&metrics[&b.key()]), equivalence_sort_key(&metrics[&a.key()])))
        .then_with(|| b.memorized_per_minute.total_cmp(&a.memorized_per_minute))
        .then_with(|| b.memorized_cards.total_cmp(&a.memorized_cards))
}

fn compare_max_spread(
    a: &Point,
    b: &Point,
    metrics: &HashMap<PointKey, crate::types::FixedCurveEquivalence>,
) -> std::cmp::Ordering {
    compare_spread_tuple(equivalence_sort_key(&metrics[&a.key()]), equivalence_sort_key(&metrics[&b.key()]))
        .then_with(|| a.memorized_per_minute.total_cmp(&b.memorized_per_minute))
        .then_with(|| a.memorized_cards.total_cmp(&b.memorized_cards))
}

fn compare_spread_tuple(a: (f64, i32, f64, f64), b: (f64, i32, f64, f64)) -> std::cmp::Ordering {
    a.0.total_cmp(&b.0)
        .then_with(|| a.1.cmp(&b.1))
        .then_with(|| a.2.total_cmp(&b.2))
        .then_with(|| a.3.total_cmp(&b.3))
}

fn compare_eff_mem_key(
    a: (f64, (f64, i32, f64, f64, f64, f64), f64),
    b: (f64, (f64, i32, f64, f64, f64, f64), f64),
) -> std::cmp::Ordering {
    a.0.total_cmp(&b.0)
        .then_with(|| compare_tuple_desc(a.1, b.1))
        .then_with(|| a.2.total_cmp(&b.2))
}
