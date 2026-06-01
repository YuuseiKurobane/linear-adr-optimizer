use crate::config::SearchConfig;
use crate::search::safety::safety_pool;
use crate::types::{FixedCurveEquivalence, FixedEnvelope, Point, PointKey};
use std::cmp::Ordering;
use std::collections::HashMap;

pub fn pareto_frontier(points: &[Point]) -> Vec<Point> {
    let mut ordered = points.to_vec();
    ordered.sort_by(|a, b| {
        b.memorized_cards
            .total_cmp(&a.memorized_cards)
            .then_with(|| b.memorized_per_minute.total_cmp(&a.memorized_per_minute))
    });
    let mut frontier = Vec::new();
    let mut best_y = f64::NEG_INFINITY;
    for point in ordered {
        if point.memorized_per_minute > best_y {
            best_y = point.memorized_per_minute;
            frontier.push(point);
        }
    }
    frontier.sort_by(|a, b| a.memorized_cards.total_cmp(&b.memorized_cards));
    frontier
}

pub fn format_dr_label(dr: f64) -> String {
    let pct = dr * 100.0;
    if (pct - pct.round()).abs() < 1e-6 {
        format!("{pct:.0}%")
    } else {
        format!("{pct:.1}%")
    }
}

pub fn make_fixed_envelope(fixed_points: &[(f64, Point)]) -> Result<FixedEnvelope, String> {
    let points_only = fixed_points.iter().map(|(_, point)| point.clone()).collect::<Vec<_>>();
    let frontier = pareto_frontier(&points_only);
    let dr_by_key: HashMap<_, _> = fixed_points.iter().map(|(dr, point)| (point.key(), *dr)).collect();
    let mut envelope: Vec<_> = frontier
        .into_iter()
        .filter_map(|point| dr_by_key.get(&point.key()).map(|dr| (*dr, point)))
        .collect();
    envelope.sort_by(|a, b| a.1.memorized_cards.total_cmp(&b.1.memorized_cards));
    if envelope.is_empty() {
        return Err("fixed DR envelope is empty".to_string());
    }
    let min_dr = envelope.iter().map(|(dr, _)| *dr).fold(f64::INFINITY, f64::min);
    let max_dr = envelope.iter().map(|(dr, _)| *dr).fold(f64::NEG_INFINITY, f64::max);
    let min_x = envelope.iter().map(|(_, p)| p.memorized_cards).fold(f64::INFINITY, f64::min);
    let max_x = envelope.iter().map(|(_, p)| p.memorized_cards).fold(f64::NEG_INFINITY, f64::max);
    let min_y = envelope.iter().map(|(_, p)| p.memorized_per_minute).fold(f64::INFINITY, f64::min);
    let max_y = envelope.iter().map(|(_, p)| p.memorized_per_minute).fold(f64::NEG_INFINITY, f64::max);
    Ok(FixedEnvelope {
        points: envelope,
        min_dr,
        max_dr,
        min_x,
        max_x,
        min_y,
        max_y,
        x_span: (max_x - min_x).max(1e-9),
        y_span: (max_y - min_y).max(1e-9),
    })
}

pub fn interp_by_dr(fixed_points: &[(f64, Point)], dr: f64, attr: MetricAttr) -> f64 {
    let mut ordered = fixed_points.to_vec();
    ordered.sort_by(|a, b| a.0.total_cmp(&b.0));
    if dr <= ordered[0].0 {
        return attr.value(&ordered[0].1);
    }
    if dr >= ordered[ordered.len() - 1].0 {
        return attr.value(&ordered[ordered.len() - 1].1);
    }
    for pair in ordered.windows(2) {
        let (dr1, p1) = &pair[0];
        let (dr2, p2) = &pair[1];
        if *dr1 - 1e-12 <= dr && dr <= *dr2 + 1e-12 {
            if (*dr2 - *dr1).abs() < 1e-12 {
                return attr.value(p1);
            }
            let ratio = (dr - *dr1) / (*dr2 - *dr1);
            return attr.value(p1) + ratio * (attr.value(p2) - attr.value(p1));
        }
    }
    attr.value(
        &ordered
            .iter()
            .min_by(|a, b| (a.0 - dr).abs().total_cmp(&(b.0 - dr).abs()))
            .expect("fixed curve nonempty")
            .1,
    )
}

pub fn equivalent_dr_for_x(env: &FixedEnvelope, x: f64) -> (f64, i32) {
    if x < env.min_x {
        return (env.min_dr, -1);
    }
    if x > env.max_x {
        return (env.max_dr, 1);
    }
    for pair in env.points.windows(2) {
        let (dr1, p1) = &pair[0];
        let (dr2, p2) = &pair[1];
        let x1 = p1.memorized_cards;
        let x2 = p2.memorized_cards;
        if x1.min(x2) - 1e-9 <= x && x <= x1.max(x2) + 1e-9 {
            if (x2 - x1).abs() < 1e-12 {
                return ((*dr1 + *dr2) / 2.0, 0);
            }
            let ratio = (x - x1) / (x2 - x1);
            return (*dr1 + ratio * (*dr2 - *dr1), 0);
        }
    }
    let nearest = env
        .points
        .iter()
        .min_by(|a, b| {
            (a.1.memorized_cards - x)
                .abs()
                .total_cmp(&(b.1.memorized_cards - x).abs())
        })
        .expect("fixed env nonempty");
    (nearest.0, 0)
}

pub fn equivalent_dr_for_y(env: &FixedEnvelope, y: f64) -> (f64, i32) {
    if y > env.max_y {
        return (env.min_dr, -1);
    }
    if y < env.min_y {
        return (env.max_dr, 1);
    }
    for pair in env.points.windows(2) {
        let (dr1, p1) = &pair[0];
        let (dr2, p2) = &pair[1];
        let y1 = p1.memorized_per_minute;
        let y2 = p2.memorized_per_minute;
        if y1.min(y2) - 1e-9 <= y && y <= y1.max(y2) + 1e-9 {
            if (y2 - y1).abs() < 1e-12 {
                return ((*dr1 + *dr2) / 2.0, 0);
            }
            let ratio = (y - y1) / (y2 - y1);
            return (*dr1 + ratio * (*dr2 - *dr1), 0);
        }
    }
    let nearest = env
        .points
        .iter()
        .min_by(|a, b| {
            (a.1.memorized_per_minute - y)
                .abs()
                .total_cmp(&(b.1.memorized_per_minute - y).abs())
        })
        .expect("fixed env nonempty");
    (nearest.0, 0)
}

pub fn format_equivalent_dr(dr: f64, censor: i32) -> String {
    let prefix = if censor < 0 { "<" } else if censor > 0 { ">" } else { "" };
    format!("{prefix}{:.2}%", dr * 100.0)
}

pub fn format_spread(spread: f64, lower_bound: bool) -> String {
    let prefix = if lower_bound { ">" } else { "" };
    format!("{prefix}{:.2}%", spread * 100.0)
}

pub fn fixed_curve_equivalence(point: &Point, env: &FixedEnvelope) -> FixedCurveEquivalence {
    let (memory_dr, memory_censor) = equivalent_dr_for_x(env, point.memorized_cards);
    let (efficiency_dr, efficiency_censor) = equivalent_dr_for_y(env, point.memorized_per_minute);
    let spread_floor = memory_dr - efficiency_dr;
    let efficiency_surplus = ((point.memorized_per_minute - env.max_y) / env.y_span).max(0.0);
    let memory_surplus = ((point.memorized_cards - env.max_x) / env.x_span).max(0.0);
    let censor_strength = i32::from(efficiency_censor != 0) + i32::from(memory_censor != 0);
    let lower_bound = efficiency_censor < 0 || memory_censor > 0;
    FixedCurveEquivalence {
        efficiency_equivalent_dr: efficiency_dr,
        memory_equivalent_dr: memory_dr,
        efficiency_label: format_equivalent_dr(efficiency_dr, efficiency_censor),
        memory_label: format_equivalent_dr(memory_dr, memory_censor),
        spread_floor,
        spread_label: format_spread(spread_floor, lower_bound),
        efficiency_censor,
        memory_censor,
        censor_strength,
        efficiency_surplus,
        memory_surplus,
        surplus_balanced: efficiency_surplus.min(memory_surplus),
        surplus_total: efficiency_surplus + memory_surplus,
    }
}

pub fn equivalence_map<'a, I>(points: I, env: &FixedEnvelope) -> HashMap<PointKey, FixedCurveEquivalence>
where
    I: IntoIterator<Item = &'a Point>,
{
    points
        .into_iter()
        .map(|point| (point.key(), fixed_curve_equivalence(point, env)))
        .collect()
}

pub fn equivalence_sort_key(metric: &FixedCurveEquivalence) -> (f64, i32, f64, f64) {
    (
        metric.spread_floor,
        metric.censor_strength,
        metric.surplus_balanced,
        metric.surplus_total,
    )
}

pub fn compare_tuple_desc(a: (f64, i32, f64, f64, f64, f64), b: (f64, i32, f64, f64, f64, f64)) -> Ordering {
    a.0.total_cmp(&b.0)
        .then_with(|| a.1.cmp(&b.1))
        .then_with(|| a.2.total_cmp(&b.2))
        .then_with(|| a.3.total_cmp(&b.3))
        .then_with(|| a.4.total_cmp(&b.4))
        .then_with(|| a.5.total_cmp(&b.5))
}

pub fn point_equivalence_key(
    point: &Point,
    metrics: &HashMap<PointKey, FixedCurveEquivalence>,
) -> (f64, i32, f64, f64, f64, f64) {
    let metric = &metrics[&point.key()];
    let spread = equivalence_sort_key(metric);
    (
        spread.0,
        spread.1,
        spread.2,
        spread.3,
        point.memorized_per_minute,
        point.memorized_cards,
    )
}

pub fn band_for_dr(
    fixed_points: &[(f64, Point)],
    target_dr: f64,
    band_pct: f64,
    attr: MetricAttr,
) -> (f64, f64) {
    let band = band_pct.max(0.0) / 100.0;
    let lo_dr = (target_dr - band).max(0.000001);
    let hi_dr = (target_dr + band).min(0.999999);
    let lo = interp_by_dr(fixed_points, lo_dr, attr);
    let hi = interp_by_dr(fixed_points, hi_dr, attr);
    (lo.min(hi), lo.max(hi))
}

#[derive(Debug, Clone, Copy)]
pub enum MetricAttr {
    MemorizedCards,
    MemorizedPerMinute,
}

impl MetricAttr {
    fn value(self, point: &Point) -> f64 {
        match self {
            Self::MemorizedCards => point.memorized_cards,
            Self::MemorizedPerMinute => point.memorized_per_minute,
        }
    }
}

#[derive(Default)]
pub struct Ranked {
    pub recommended: Vec<Point>,
    pub efficiency: Vec<Point>,
    pub memory: Vec<Point>,
    pub frontier: Vec<Point>,
}

pub fn classify_ranked(
    points: &[Point],
    target_fixed: &Point,
    fixed_points: &[(f64, Point)],
    env: &FixedEnvelope,
    target_dr: f64,
    band_pct: f64,
    config: &SearchConfig,
) -> Ranked {
    let pool = safety_pool(points, config);
    if pool.is_empty() {
        return Ranked::default();
    }
    let metrics = equivalence_map(&pool, env);
    let x0 = target_fixed.memorized_cards;
    let y0 = target_fixed.memorized_per_minute;
    let x_band = band_for_dr(fixed_points, target_dr, band_pct, MetricAttr::MemorizedCards);
    let y_band = band_for_dr(fixed_points, target_dr, band_pct, MetricAttr::MemorizedPerMinute);

    let recommended_pool: Vec<_> = pool
        .iter()
        .filter(|point| point.memorized_cards > x0 && point.memorized_per_minute > y0)
        .cloned()
        .collect();
    let recommended_pool = if recommended_pool.is_empty() { pool.clone() } else { recommended_pool };
    let efficiency_pool: Vec<_> = pool
        .iter()
        .filter(|point| x_band.0 <= point.memorized_cards && point.memorized_cards <= x_band.1)
        .cloned()
        .collect();
    let efficiency_pool = if efficiency_pool.is_empty() { pool.clone() } else { efficiency_pool };
    let memory_pool: Vec<_> = pool
        .iter()
        .filter(|point| y_band.0 <= point.memorized_per_minute && point.memorized_per_minute <= y_band.1)
        .cloned()
        .collect();
    let memory_pool = if memory_pool.is_empty() { pool.clone() } else { memory_pool };

    let mut recommended = recommended_pool;
    recommended.sort_by(|a, b| {
        compare_tuple_desc(point_equivalence_key(a, &metrics), point_equivalence_key(b, &metrics)).reverse()
    });
    let mut efficiency = efficiency_pool;
    efficiency.sort_by(|a, b| {
        let ka = (
            a.memorized_per_minute,
            point_equivalence_key(a, &metrics),
            -(a.memorized_cards - x0).abs(),
        );
        let kb = (
            b.memorized_per_minute,
            point_equivalence_key(b, &metrics),
            -(b.memorized_cards - x0).abs(),
        );
        compare_eff_mem_key(ka, kb).reverse()
    });
    let mut memory = memory_pool;
    memory.sort_by(|a, b| {
        let ka = (
            a.memorized_cards,
            point_equivalence_key(a, &metrics),
            -(a.memorized_per_minute - y0).abs(),
        );
        let kb = (
            b.memorized_cards,
            point_equivalence_key(b, &metrics),
            -(b.memorized_per_minute - y0).abs(),
        );
        compare_eff_mem_key(ka, kb).reverse()
    });
    let mut frontier = pareto_frontier(&pool);
    frontier.sort_by(|a, b| {
        compare_tuple_desc(point_equivalence_key(a, &metrics), point_equivalence_key(b, &metrics)).reverse()
    });
    Ranked {
        recommended,
        efficiency,
        memory,
        frontier,
    }
}

fn compare_eff_mem_key(
    a: (f64, (f64, i32, f64, f64, f64, f64), f64),
    b: (f64, (f64, i32, f64, f64, f64, f64), f64),
) -> Ordering {
    a.0.total_cmp(&b.0)
        .then_with(|| compare_tuple_desc(a.1, b.1))
        .then_with(|| a.2.total_cmp(&b.2))
}

pub fn select_promotions(
    points: &[Point],
    target_fixed: &Point,
    fixed_points: &[(f64, Point)],
    env: &FixedEnvelope,
    target_dr: f64,
    config: &SearchConfig,
    band_pct: f64,
    include_pareto_extra: bool,
    pareto_as_render_only: bool,
) -> (Vec<Point>, Vec<Point>) {
    let ranked = classify_ranked(points, target_fixed, fixed_points, env, target_dr, band_pct, config);
    let mut selected = Vec::new();
    let mut selected_keys: HashMap<PointKey, usize> = HashMap::new();
    add_top(&mut selected, &mut selected_keys, &ranked.recommended, config.promote_recommended);
    add_top(&mut selected, &mut selected_keys, &ranked.efficiency, config.promote_efficiency_potential);
    add_top(&mut selected, &mut selected_keys, &ranked.memory, config.promote_memory_potential);

    let mut extras = Vec::new();
    let mut extra_keys: HashMap<PointKey, usize> = HashMap::new();
    if include_pareto_extra {
        let mut count = 0;
        for point in ranked.frontier {
            if selected_keys.contains_key(&point.key()) || extra_keys.contains_key(&point.key()) {
                continue;
            }
            extra_keys.insert(point.key(), extras.len());
            extras.push(point);
            count += 1;
            if count >= config.promote_pareto_extra {
                break;
            }
        }
    }
    if pareto_as_render_only {
        return (selected, extras);
    }
    selected.extend(extras);
    (selected, Vec::new())
}

pub fn add_top(
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
