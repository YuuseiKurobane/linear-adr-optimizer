use crate::config::SearchConfig;
use crate::model::simulate::SafetySummary;
use crate::search::batch::EvalEngine;
use crate::types::{Candidate, Point, PointKey};
use std::collections::HashMap;

pub fn safety_by_key(
    engine: &EvalEngine,
    candidates: &[Candidate],
    config: &SearchConfig,
) -> HashMap<PointKey, SafetySummary> {
    if config.ignore_safety || candidates.is_empty() {
        return HashMap::new();
    }
    engine
        .safety_many(candidates, config.safety_s_max, config.safety_checks)
        .into_iter()
        .collect()
}

pub fn safety_row_is_safe(row: &SafetySummary) -> bool {
    row.interval_flips == 0 && row.hard_shortens == 0
}

pub fn attach_safety_from_rows(points: Vec<Point>, safety: &HashMap<PointKey, SafetySummary>) -> Vec<Point> {
    if safety.is_empty() || points.is_empty() {
        return points;
    }
    points
        .into_iter()
        .map(|mut point| {
            if let Some(summary) = safety.get(&point.key()) {
                point.safety_checks = summary.checks;
                point.interval_flips = summary.interval_flips;
                point.hard_shortens = summary.hard_shortens;
            }
            point
        })
        .collect()
}

pub fn attach_safety(engine: &EvalEngine, points: Vec<Point>, config: &SearchConfig) -> Vec<Point> {
    if config.ignore_safety || points.is_empty() {
        return points;
    }
    let candidates: Vec<_> = points.iter().map(Point::candidate).collect();
    let safety = safety_by_key(engine, &candidates, config);
    attach_safety_from_rows(points, &safety)
}

pub fn safety_pool(points: &[Point], config: &SearchConfig) -> Vec<Point> {
    if config.ignore_safety {
        return points.to_vec();
    }
    let safe: Vec<_> = points.iter().filter(|point| point.safe()).cloned().collect();
    if safe.is_empty() {
        points.to_vec()
    } else {
        safe
    }
}
