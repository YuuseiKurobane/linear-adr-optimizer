use crate::config::SearchConfig;
use crate::types::{snap_value, Candidate, Phase1Domain, Point, PointKey};
use std::collections::HashMap;

pub fn logit(dr: f64) -> f64 {
    let dr = dr.clamp(1e-6, 1.0 - 1e-6);
    (dr / (1.0 - dr)).ln()
}

pub fn dedupe_candidates<I>(candidates: I, enforce_quadrant: bool) -> Vec<Candidate>
where
    I: IntoIterator<Item = Candidate>,
{
    let mut unique = HashMap::<PointKey, usize>::new();
    let mut out = Vec::new();
    for candidate in candidates {
        let snapped = candidate.snap();
        if enforce_quadrant && !snapped.in_quadrant() {
            continue;
        }
        if let Some(idx) = unique.get(&snapped.key()).copied() {
            out[idx] = snapped;
        } else {
            unique.insert(snapped.key(), out.len());
            out.push(snapped);
        }
    }
    out
}

pub fn dedupe_points<I>(points: I) -> Vec<Point>
where
    I: IntoIterator<Item = Point>,
{
    let mut unique = HashMap::<PointKey, usize>::new();
    let mut out = Vec::new();
    for point in points {
        if let Some(idx) = unique.get(&point.key()).copied() {
            out[idx] = point;
        } else {
            unique.insert(point.key(), out.len());
            out.push(point);
        }
    }
    out
}

pub fn dedupe_point_refs<'a, I>(points: I) -> Vec<Point>
where
    I: IntoIterator<Item = &'a Point>,
{
    let mut unique = HashMap::<PointKey, usize>::new();
    let mut out = Vec::new();
    for point in points {
        if let Some(idx) = unique.get(&point.key()).copied() {
            out[idx] = point.clone();
        } else {
            unique.insert(point.key(), out.len());
            out.push(point.clone());
        }
    }
    out
}

pub fn make_phase1_domain(config: &SearchConfig, target_dr: f64) -> Phase1Domain {
    let center = (logit(target_dr) * 1000.0).round() / 1000.0;
    let flat_half = config.phase1_flat_half_steps;
    let s_high = (config.phase1_s_max / config.phase1_s_step).round() as i32;
    let d_high = (config.phase1_d_min.abs() / config.phase1_d_step).round() as i32;
    let flat_count = flat_half * 2 + 1;
    let s_count = s_high + 1;
    let d_count = d_high + 1;
    let factor = config.phase1_expand_overflow_factor.max(0.0);
    Phase1Domain {
        center,
        flat_step: config.phase1_flat_step,
        s_step: config.phase1_s_step,
        d_step: config.phase1_d_step,
        flat_low: -flat_half,
        flat_high: flat_half,
        s_high,
        d_high,
        init_flat_low: -flat_half,
        init_flat_high: flat_half,
        init_s_high: s_high,
        init_d_high: d_high,
        flat_extra_limit: (flat_count as f64 * factor).ceil() as i32,
        s_extra_limit: (s_count as f64 * factor).ceil() as i32,
        d_extra_limit: (d_count as f64 * factor).ceil() as i32,
    }
}

pub fn phase1_candidates(domain: &Phase1Domain) -> Vec<Candidate> {
    let mut out = Vec::new();
    for flat_idx in domain.flat_low..=domain.flat_high {
        let flat = domain.center + flat_idx as f64 * domain.flat_step;
        for s_idx in 0..=domain.s_high {
            let s_multi = s_idx as f64 * domain.s_step;
            for d_idx in 0..=domain.d_high {
                let d_multi = -(d_idx as f64) * domain.d_step;
                out.push(Candidate::new(flat, s_multi, d_multi).snap());
            }
        }
    }
    out
}

pub fn phase1_index_of(point: &Point, domain: &Phase1Domain) -> (i32, i32, i32) {
    let flat_idx = ((point.flat - domain.center) / domain.flat_step).round() as i32;
    let s_idx = (point.s_multi / domain.s_step).round() as i32;
    let d_idx = (-point.d_multi / domain.d_step).round() as i32;
    (flat_idx, s_idx, d_idx)
}

pub fn phase1_boundary_directions(promoted: &[Point], domain: &Phase1Domain) -> Vec<String> {
    let mut directions = Vec::<String>::new();
    for point in promoted {
        let (flat_idx, s_idx, d_idx) = phase1_index_of(point, domain);
        push_unique(&mut directions, flat_idx <= domain.flat_low, "flat_low");
        push_unique(&mut directions, flat_idx >= domain.flat_high, "flat_high");
        push_unique(&mut directions, s_idx >= domain.s_high, "s_high");
        push_unique(&mut directions, d_idx >= domain.d_high, "d_high");
    }
    directions.sort();
    directions
}

fn push_unique(values: &mut Vec<String>, condition: bool, value: &str) {
    if condition && !values.iter().any(|item| item == value) {
        values.push(value.to_string());
    }
}

pub fn hypercube_candidates(
    centers: &[Point],
    steps: (f64, f64, f64),
    include_center: bool,
) -> Vec<Candidate> {
    let mut out = Vec::new();
    for center in centers {
        for flat_offset in [-1.0, 0.0, 1.0] {
            for s_offset in [-1.0, 0.0, 1.0] {
                for d_offset in [-1.0, 0.0, 1.0] {
                    if !include_center && flat_offset == 0.0 && s_offset == 0.0 && d_offset == 0.0 {
                        continue;
                    }
                    out.push(Candidate::new(
                        center.flat + flat_offset * steps.0,
                        center.s_multi + s_offset * steps.1,
                        center.d_multi + d_offset * steps.2,
                    ));
                }
            }
        }
    }
    dedupe_candidates(out, true)
}

pub fn should_include_hypercube_center(current_weight: f64, previous_weight: f64) -> bool {
    current_weight > previous_weight + 1e-9
}

pub fn add_existing_bridge_midpoints(
    promoted: &[Point],
    pool: &[Point],
    steps: (f64, f64, f64),
    limit: usize,
) -> Vec<Point> {
    if limit == 0 {
        return promoted.to_vec();
    }
    let by_key: HashMap<_, _> = pool.iter().map(|point| (point.key(), point.clone())).collect();
    let mut selected = promoted.to_vec();
    let mut selected_by_key: HashMap<_, _> = promoted
        .iter()
        .enumerate()
        .map(|(idx, point)| (point.key(), idx))
        .collect();
    let mut added = 0;
    for idx in 0..promoted.len() {
        for b in promoted.iter().skip(idx + 1) {
            let Some(midpoint) = qualifying_midpoint(&promoted[idx], b, steps) else {
                continue;
            };
            let key = midpoint.key();
            if let Some(point) = by_key.get(&key) {
                if let std::collections::hash_map::Entry::Vacant(entry) = selected_by_key.entry(key) {
                    entry.insert(selected.len());
                    selected.push(point.clone());
                    added += 1;
                    if added >= limit {
                        return selected;
                    }
                }
            }
        }
    }
    selected
}

pub fn bridge_midpoint_neighborhoods(promoted: &[Point], steps: (f64, f64, f64)) -> Vec<Candidate> {
    let mut out = Vec::new();
    let step_values = [steps.0, steps.1, steps.2];
    for idx in 0..promoted.len() {
        for b in promoted.iter().skip(idx + 1) {
            let Some(midpoint) = qualifying_midpoint(&promoted[idx], b, steps) else {
                continue;
            };
            let Some(bridge_axis) = bridge_axis(&promoted[idx], b, steps) else {
                continue;
            };
            let face_axes: Vec<_> = (0..3).filter(|axis| *axis != bridge_axis).collect();
            let mut values = [midpoint.flat, midpoint.s_multi, midpoint.d_multi];
            for offset_a in [-1.0, 0.0, 1.0] {
                for offset_b in [-1.0, 0.0, 1.0] {
                    values[0] = midpoint.flat;
                    values[1] = midpoint.s_multi;
                    values[2] = midpoint.d_multi;
                    values[face_axes[0]] += offset_a * step_values[face_axes[0]];
                    values[face_axes[1]] += offset_b * step_values[face_axes[1]];
                    out.push(Candidate::new(values[0], values[1], values[2]));
                }
            }
        }
    }
    dedupe_candidates(out, true)
}

fn qualifying_midpoint(a: &Point, b: &Point, steps: (f64, f64, f64)) -> Option<Candidate> {
    bridge_axis(a, b, steps)?;
    Some(Candidate::new(
        snap_value((a.flat + b.flat) / 2.0),
        snap_value((a.s_multi + b.s_multi) / 2.0),
        snap_value((a.d_multi + b.d_multi) / 2.0),
    ))
}

fn bridge_axis(a: &Point, b: &Point, steps: (f64, f64, f64)) -> Option<usize> {
    let av = [a.flat, a.s_multi, a.d_multi];
    let bv = [b.flat, b.s_multi, b.d_multi];
    let step_values = [steps.0, steps.1, steps.2];
    let diffs = [bv[0] - av[0], bv[1] - av[1], bv[2] - av[2]];
    let varying: Vec<_> = diffs
        .iter()
        .enumerate()
        .filter_map(|(idx, diff)| (diff.abs() > 1e-9).then_some(idx))
        .collect();
    if varying.len() != 1 {
        return None;
    }
    let axis = varying[0];
    if (diffs[axis].abs() - 2.0 * step_values[axis]).abs() > 1e-6 {
        return None;
    }
    Some(axis)
}

#[derive(Default)]
pub struct PointStore {
    points: HashMap<PointKey, Point>,
    eval_weight_by_key: HashMap<PointKey, f64>,
}

impl PointStore {
    pub fn add(&mut self, points: &[Point], eval_weight: f64) {
        for point in points {
            let previous = self.eval_weight_by_key.get(&point.key()).copied().unwrap_or(-1.0);
            if eval_weight >= previous {
                self.points.insert(point.key(), point.clone());
                self.eval_weight_by_key.insert(point.key(), eval_weight);
            }
        }
    }

    pub fn missing_or_lower_weight(&self, candidates: Vec<Candidate>, eval_weight: f64) -> Vec<Candidate> {
        candidates
            .into_iter()
            .filter(|candidate| {
                self.eval_weight_by_key
                    .get(&candidate.key())
                    .copied()
                    .unwrap_or(-1.0)
                    + 1e-9
                    < eval_weight
            })
            .collect()
    }
}
