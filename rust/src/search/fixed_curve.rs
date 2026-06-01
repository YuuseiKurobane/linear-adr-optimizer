use crate::config::SearchConfig;
use crate::search::batch::EvalEngine;
use crate::search::candidates::logit;
use crate::search::rank::{classify_ranked, fixed_curve_equivalence, make_fixed_envelope};
use crate::types::{Candidate, FixedEnvelope, PhaseDiag, Point};
use serde_json::json;
use std::collections::HashMap;
use std::time::Instant;

pub struct FixedCurveManager<'a> {
    pub engine: &'a EvalEngine,
    pub config: &'a SearchConfig,
    pub target_dr: f64,
    pub coarse_points_by_pct: HashMap<i64, Point>,
    pub refined_points_by_pct: HashMap<i64, Point>,
    pub fixed_curve: Vec<(f64, Point)>,
    pub fixed_env: FixedEnvelope,
    pub rough_env: FixedEnvelope,
    pub target_fixed: Point,
    pub diagnostics: Vec<PhaseDiag>,
}

impl<'a> FixedCurveManager<'a> {
    pub fn build(engine: &'a EvalEngine, config: &'a SearchConfig, target_dr: f64) -> Result<Self, String> {
        let empty_point = Point::new(Candidate::new(0.0, 0.0, 0.0), 0.0, 1.0, 0, 0.0, 0.0, 0.0);
        let empty_env = FixedEnvelope {
            points: vec![(target_dr, empty_point.clone())],
            min_dr: target_dr,
            max_dr: target_dr,
            min_x: 0.0,
            max_x: 1.0,
            min_y: 0.0,
            max_y: 1.0,
            x_span: 1.0,
            y_span: 1.0,
        };
        let mut manager = Self {
            engine,
            config,
            target_dr,
            coarse_points_by_pct: HashMap::new(),
            refined_points_by_pct: HashMap::new(),
            fixed_curve: Vec::new(),
            fixed_env: empty_env.clone(),
            rough_env: empty_env,
            target_fixed: empty_point,
            diagnostics: Vec::new(),
        };
        manager.evaluate_initial()?;
        Ok(manager)
    }

    fn evaluate_initial(&mut self) -> Result<(), String> {
        let start = Instant::now();
        let coarse_pcts = integer_pcts(
            self.config.fixed_dr_start_pct,
            self.config.fixed_dr_end_pct,
            self.config.fixed_curve_coarse_step_pct,
        )?;
        self.coarse_points_by_pct.extend(evaluate_fixed_pcts(
            self.engine,
            &coarse_pcts,
            self.config.fixed_curve_coarse_weight,
            self.config.seed + 700,
        ));
        self.rough_env = make_fixed_envelope(&points_from_pct_map(&self.coarse_points_by_pct))?;

        let target_pct = self.target_dr * 100.0;
        let mut initial_pcts = aligned_dense_pcts(
            target_pct - self.config.fixed_curve_initial_radius_pct,
            target_pct + self.config.fixed_curve_initial_radius_pct,
            self.config,
        )?;
        initial_pcts.push(round_pct(target_pct));
        initial_pcts.sort_by(|a, b| a.total_cmp(b));
        initial_pcts.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
        self.evaluate_refined_pcts(&initial_pcts, self.config.seed + 720);
        self.rebuild()?;

        let elapsed = start.elapsed().as_secs_f64();
        self.diagnostics.push(PhaseDiag {
            name: "fixed_curve.initial".to_string(),
            weight: self.config.fixed_curve_refine_weight,
            candidates: coarse_pcts.len() + initial_pcts.len(),
            evaluated: coarse_pcts.len() + initial_pcts.len(),
            safe: 0,
            unsafe_count: 0,
            promoted: 0,
            pareto_extra: 0,
            elapsed_s: elapsed,
            notes: Some(json!({
                "coarse_weight": self.config.fixed_curve_coarse_weight,
                "coarse_points": coarse_pcts.len(),
                "refined_weight": self.config.fixed_curve_refine_weight,
                "refined_points": initial_pcts.len(),
                "curve_points": self.fixed_curve.len(),
                "envelope_points": self.fixed_env.points.len(),
                "end_pct": self.config.fixed_dr_end_pct,
            })),
        });
        println!(
            "[fixed curve] coarse={}@{} refined={}@{} envelope={} elapsed={:.1}s",
            coarse_pcts.len(),
            fmt_g(self.config.fixed_curve_coarse_weight),
            initial_pcts.len(),
            fmt_g(self.config.fixed_curve_refine_weight),
            self.fixed_env.points.len(),
            elapsed
        );
        Ok(())
    }

    fn evaluate_refined_pcts(&mut self, pcts: &[f64], seed: u64) -> HashMap<i64, Point> {
        let new_pcts: Vec<_> = pcts
            .iter()
            .map(|pct| round_pct(*pct))
            .filter(|pct| {
                self.config.fixed_dr_start_pct - 1e-9 <= *pct
                    && *pct <= self.config.fixed_dr_end_pct + 1e-9
                    && !self.refined_points_by_pct.contains_key(&pct_key(*pct))
            })
            .collect();
        if new_pcts.is_empty() {
            return HashMap::new();
        }
        let evaluated = evaluate_fixed_pcts(
            self.engine,
            &new_pcts,
            self.config.fixed_curve_refine_weight,
            seed,
        );
        self.refined_points_by_pct
            .extend(evaluated.iter().map(|(pct, point)| (*pct, point.clone())));
        evaluated
    }

    fn rebuild(&mut self) -> Result<(), String> {
        let mut merged: HashMap<i64, &Point> = self
            .coarse_points_by_pct
            .iter()
            .map(|(pct, point)| (*pct, point))
            .collect();
        merged.extend(self.refined_points_by_pct.iter().map(|(pct, point)| (*pct, point)));
        self.fixed_curve = merged
            .into_iter()
            .map(|(pct_key, point)| (pct_key as f64 / 1_000_000.0 / 100.0, point.clone()))
            .collect();
        self.fixed_curve.sort_by(|a, b| a.0.total_cmp(&b.0));
        self.fixed_env = make_fixed_envelope(&self.fixed_curve)?;
        self.target_fixed = self
            .fixed_curve
            .iter()
            .min_by(|a, b| (a.0 - self.target_dr).abs().total_cmp(&(b.0 - self.target_dr).abs()))
            .ok_or("fixed curve is empty")?
            .1
            .clone();
        Ok(())
    }

    pub fn ensure_for_points(
        &mut self,
        points: &[Point],
        phase: &str,
        seed: u64,
        band_pct: f64,
    ) -> Result<(), String> {
        if points.is_empty() {
            return Ok(());
        }
        let needed = self.needed_refined_pcts(points, band_pct)?;
        let new_needed: Vec<_> = needed
            .into_iter()
            .filter(|pct| !self.refined_points_by_pct.contains_key(&pct_key(*pct)))
            .collect();
        if new_needed.is_empty() {
            return Ok(());
        }
        let start = Instant::now();
        let evaluated = self.evaluate_refined_pcts(&new_needed, seed);
        self.rebuild()?;
        let elapsed = start.elapsed().as_secs_f64();
        self.diagnostics.push(PhaseDiag {
            name: format!("fixed_curve.adapt.{phase}"),
            weight: self.config.fixed_curve_refine_weight,
            candidates: new_needed.len(),
            evaluated: evaluated.len(),
            safe: 0,
            unsafe_count: 0,
            promoted: 0,
            pareto_extra: 0,
            elapsed_s: elapsed,
            notes: Some(json!({
                "phase": phase,
                "min_pct": new_needed.iter().copied().reduce(f64::min),
                "max_pct": new_needed.iter().copied().reduce(f64::max),
                "refined_total": self.refined_points_by_pct.len(),
                "curve_points": self.fixed_curve.len(),
            })),
        });
        let min_pct = new_needed.iter().copied().fold(f64::INFINITY, f64::min);
        let max_pct = new_needed.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        println!(
            "[fixed curve adapt:{phase}] added={} range={min_pct:.1}-{max_pct:.1}% elapsed={elapsed:.1}s",
            evaluated.len()
        );
        Ok(())
    }

    fn needed_refined_pcts(&self, points: &[Point], band_pct: f64) -> Result<Vec<f64>, String> {
        let ranked = classify_ranked(
            points,
            &self.target_fixed,
            &self.fixed_curve,
            &self.fixed_env,
            self.target_dr,
            band_pct,
            self.config,
        );
        let mut candidates: HashMap<_, Point> = HashMap::new();
        let per_bucket = self.config.fixed_curve_adapt_top_per_bucket.max(1);
        for point in ranked
            .recommended
            .iter()
            .take(per_bucket)
            .chain(ranked.efficiency.iter().take(per_bucket))
            .chain(ranked.memory.iter().take(per_bucket))
            .chain(ranked.frontier.iter().take(per_bucket))
        {
            candidates.insert(point.key(), point.clone());
        }

        let mut needed = Vec::new();
        for point in candidates.values() {
            let rough = fixed_curve_equivalence(point, &self.rough_env);
            for (dr, censor) in [
                (rough.efficiency_equivalent_dr, rough.efficiency_censor),
                (rough.memory_equivalent_dr, rough.memory_censor),
            ] {
                if censor != 0 {
                    continue;
                }
                let pct = dr * 100.0;
                needed.extend(aligned_dense_pcts(
                    pct - self.config.fixed_curve_adapt_margin_pct,
                    pct + self.config.fixed_curve_adapt_margin_pct,
                    self.config,
                )?);
            }
        }
        needed.sort_by(|a, b| a.total_cmp(b));
        needed.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
        if needed.len() > self.config.fixed_curve_adapt_max_points {
            let target_pct = self.target_dr * 100.0;
            needed.sort_by(|a, b| (a - target_pct).abs().total_cmp(&(b - target_pct).abs()));
            needed.truncate(self.config.fixed_curve_adapt_max_points);
        }
        Ok(needed)
    }
}

pub fn integer_pcts(start_pct: f64, end_pct: f64, step_pct: f64) -> Result<Vec<f64>, String> {
    if step_pct <= 0.0 {
        return Err("--fixed-curve-coarse-step-pct must be positive".to_string());
    }
    let start = start_pct.min(end_pct);
    let end = start_pct.max(end_pct);
    let first = (start / step_pct).ceil() * step_pct;
    let mut values = Vec::new();
    let mut pct = first;
    while pct <= end + 1e-9 {
        values.push(round_pct(pct));
        pct += step_pct;
    }
    for pct in [start, end] {
        let rounded = round_pct(pct);
        if 0.0 < rounded && rounded < 100.0 {
            values.push(rounded);
        }
    }
    values.sort_by(|a, b| a.total_cmp(b));
    values.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
    Ok(values)
}

pub fn aligned_dense_pcts(start_pct: f64, end_pct: f64, config: &SearchConfig) -> Result<Vec<f64>, String> {
    let step = config.fixed_curve_refine_step_pct;
    if step <= 0.0 {
        return Err("--fixed-curve-refine-step-pct must be positive".to_string());
    }
    let lo = start_pct.min(end_pct).max(config.fixed_dr_start_pct);
    let hi = start_pct.max(end_pct).min(config.fixed_dr_end_pct);
    if lo > hi {
        return Ok(Vec::new());
    }
    let start = (lo / step).floor() * step;
    let end = (hi / step).ceil() * step;
    let mut values = Vec::new();
    let mut pct = start;
    while pct <= end + 1e-9 {
        let rounded = round_pct(pct);
        if config.fixed_dr_start_pct - 1e-9 <= rounded && rounded <= config.fixed_dr_end_pct + 1e-9 {
            values.push(rounded);
        }
        pct += step;
    }
    values.sort_by(|a, b| a.total_cmp(b));
    values.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
    Ok(values)
}

pub fn specs_for_pcts(pcts: &[f64]) -> Vec<(f64, Candidate)> {
    let mut sorted: Vec<_> = pcts.iter().copied().map(round_pct).collect();
    sorted.sort_by(|a, b| a.total_cmp(b));
    sorted.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
    sorted
        .into_iter()
        .map(|pct| {
            let dr = pct / 100.0;
            (dr, Candidate::new(logit(dr), 0.0, 0.0))
        })
        .collect()
}

pub fn evaluate_fixed_pcts(
    engine: &EvalEngine,
    pcts: &[f64],
    weight: f64,
    seed: u64,
) -> HashMap<i64, Point> {
    let specs = specs_for_pcts(pcts);
    let candidates: Vec<_> = specs.iter().map(|(_, candidate)| *candidate).collect();
    let raw = engine.evaluate_raw(&candidates, weight, seed);
    specs
        .into_iter()
        .zip(raw)
        .map(|((dr, _), mut point)| {
            point.dr_samples = 1;
            point.dr_p10 = dr;
            point.dr_mean = dr;
            point.dr_p90 = dr;
            point.dr_spread = 0.0;
            (pct_key(dr * 100.0), point)
        })
        .collect()
}

pub fn points_from_pct_map(points_by_pct: &HashMap<i64, Point>) -> Vec<(f64, Point)> {
    let mut out: Vec<_> = points_by_pct
        .iter()
        .map(|(pct_key, point)| (*pct_key as f64 / 1_000_000.0 / 100.0, point.clone()))
        .collect();
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    out
}

pub fn should_label_fixed_pct(pct: f64, target_pct: f64, config: &SearchConfig) -> bool {
    if (pct - target_pct).abs() <= 1e-6 {
        return true;
    }
    if (pct - config.fixed_dr_start_pct).abs() <= 1e-6
        || (pct - config.fixed_dr_end_pct).abs() <= 1e-6
    {
        return true;
    }
    let step = config.fixed_dr_label_step_pct;
    if step <= 0.0 {
        return false;
    }
    let offset = (pct - config.fixed_dr_start_pct) / step;
    (offset - offset.round()).abs() <= 1e-6
}

fn round_pct(pct: f64) -> f64 {
    (pct * 1_000_000.0).round() / 1_000_000.0
}

fn pct_key(pct: f64) -> i64 {
    (round_pct(pct) * 1_000_000.0).round() as i64
}

fn fmt_g(value: f64) -> String {
    if (value - value.round()).abs() < 1e-9 {
        format!("{:.0}", value)
    } else {
        format!("{value}")
    }
}
