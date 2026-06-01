use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Candidate {
    pub flat: f64,
    pub s_multi: f64,
    pub d_multi: f64,
}

impl Candidate {
    pub fn new(flat: f64, s_multi: f64, d_multi: f64) -> Self {
        Self {
            flat,
            s_multi,
            d_multi,
        }
    }

    pub fn snap(self) -> Self {
        Self::new(
            snap_value(self.flat),
            snap_value(self.s_multi),
            snap_value(self.d_multi),
        )
    }

    pub fn key(self) -> PointKey {
        PointKey::from_candidate(self)
    }

    pub fn in_quadrant(self) -> bool {
        self.s_multi >= -1e-9 && self.d_multi <= 1e-9
    }
}

pub fn snap_value(value: f64) -> f64 {
    (value * 1_000_000.0).round() / 1_000_000.0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PointKey(pub i64, pub i64, pub i64);

impl PointKey {
    pub fn from_values(flat: f64, s_multi: f64, d_multi: f64) -> Self {
        Self(
            (flat * 1_000_000.0).round() as i64,
            (s_multi * 1_000_000.0).round() as i64,
            (d_multi * 1_000_000.0).round() as i64,
        )
    }

    pub fn from_candidate(candidate: Candidate) -> Self {
        Self::from_values(candidate.flat, candidate.s_multi, candidate.d_multi)
    }

    pub fn as_candidate(self) -> Candidate {
        Candidate::new(
            self.0 as f64 / 1_000_000.0,
            self.1 as f64 / 1_000_000.0,
            self.2 as f64 / 1_000_000.0,
        )
    }

    pub fn json_key(self) -> String {
        let c = self.as_candidate();
        format!("{:.6},{:.6},{:.6}", c.flat, c.s_multi, c.d_multi)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Point {
    pub flat: f64,
    pub s_multi: f64,
    pub d_multi: f64,
    pub total_average_memorized: f64,
    pub total_cost: f64,
    pub total_iters: i32,
    pub memorized_fraction: f64,
    pub memorized_cards: f64,
    pub memorized_per_minute: f64,
    pub safety_checks: i32,
    pub interval_flips: i32,
    pub hard_shortens: i32,
    pub dr_samples: i64,
    pub dr_p10: f64,
    pub dr_mean: f64,
    pub dr_p90: f64,
    pub dr_spread: f64,
}

impl Point {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        candidate: Candidate,
        total_average_memorized: f64,
        total_cost: f64,
        total_iters: i32,
        memorized_fraction: f64,
        memorized_cards: f64,
        memorized_per_minute: f64,
    ) -> Self {
        Self {
            flat: candidate.flat,
            s_multi: candidate.s_multi,
            d_multi: candidate.d_multi,
            total_average_memorized,
            total_cost,
            total_iters,
            memorized_fraction,
            memorized_cards,
            memorized_per_minute,
            safety_checks: 0,
            interval_flips: 0,
            hard_shortens: 0,
            dr_samples: 0,
            dr_p10: 0.0,
            dr_mean: 0.0,
            dr_p90: 0.0,
            dr_spread: 0.0,
        }
    }

    pub fn candidate(&self) -> Candidate {
        Candidate::new(self.flat, self.s_multi, self.d_multi)
    }

    pub fn key(&self) -> PointKey {
        PointKey::from_values(self.flat, self.s_multi, self.d_multi)
    }

    pub fn safe(&self) -> bool {
        self.interval_flips == 0 && self.hard_shortens == 0
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct FixedCurveEquivalence {
    pub efficiency_equivalent_dr: f64,
    pub memory_equivalent_dr: f64,
    pub efficiency_label: String,
    pub memory_label: String,
    pub spread_floor: f64,
    pub spread_label: String,
    pub efficiency_censor: i32,
    pub memory_censor: i32,
    pub censor_strength: i32,
    pub efficiency_surplus: f64,
    pub memory_surplus: f64,
    pub surplus_balanced: f64,
    pub surplus_total: f64,
}

#[derive(Debug, Clone)]
pub struct FixedEnvelope {
    pub points: Vec<(f64, Point)>,
    pub min_dr: f64,
    pub max_dr: f64,
    pub min_x: f64,
    pub max_x: f64,
    pub min_y: f64,
    pub max_y: f64,
    pub x_span: f64,
    pub y_span: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PhaseDiag {
    pub name: String,
    pub weight: f64,
    pub candidates: usize,
    pub evaluated: usize,
    pub safe: usize,
    pub unsafe_count: usize,
    pub promoted: usize,
    pub pareto_extra: usize,
    pub elapsed_s: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<serde_json::Value>,
}

impl PhaseDiag {
    pub fn new(name: impl Into<String>, weight: f64, points: &[Point], elapsed_s: f64) -> Self {
        Self {
            name: name.into(),
            weight,
            candidates: points.len(),
            evaluated: points.len(),
            safe: points.iter().filter(|point| point.safe()).count(),
            unsafe_count: points.iter().filter(|point| !point.safe()).count(),
            promoted: 0,
            pareto_extra: 0,
            elapsed_s,
            notes: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Phase1Domain {
    pub center: f64,
    pub flat_step: f64,
    pub s_step: f64,
    pub d_step: f64,
    pub flat_low: i32,
    pub flat_high: i32,
    pub s_high: i32,
    pub d_high: i32,
    pub init_flat_low: i32,
    pub init_flat_high: i32,
    pub init_s_high: i32,
    pub init_d_high: i32,
    pub flat_extra_limit: i32,
    pub s_extra_limit: i32,
    pub d_extra_limit: i32,
}

impl Phase1Domain {
    pub fn expand(&mut self, directions: &[String], batch: i32) -> HashMap<String, i32> {
        let mut changed = HashMap::new();
        if directions.iter().any(|d| d == "flat_low") {
            let limit = self.init_flat_low - self.flat_extra_limit;
            let old = self.flat_low;
            self.flat_low = (self.flat_low - batch).max(limit);
            if old - self.flat_low > 0 {
                changed.insert("flat_low".to_string(), old - self.flat_low);
            }
        }
        if directions.iter().any(|d| d == "flat_high") {
            let limit = self.init_flat_high + self.flat_extra_limit;
            let old = self.flat_high;
            self.flat_high = (self.flat_high + batch).min(limit);
            if self.flat_high - old > 0 {
                changed.insert("flat_high".to_string(), self.flat_high - old);
            }
        }
        if directions.iter().any(|d| d == "s_high") {
            let limit = self.init_s_high + self.s_extra_limit;
            let old = self.s_high;
            self.s_high = (self.s_high + batch).min(limit);
            if self.s_high - old > 0 {
                changed.insert("s_high".to_string(), self.s_high - old);
            }
        }
        if directions.iter().any(|d| d == "d_high") {
            let limit = self.init_d_high + self.d_extra_limit;
            let old = self.d_high;
            self.d_high = (self.d_high + batch).min(limit);
            if self.d_high - old > 0 {
                changed.insert("d_high".to_string(), self.d_high - old);
            }
        }
        changed
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResultLabel {
    Recommended,
    Aggressive,
    Calm,
}

impl ResultLabel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Recommended => "Recommended",
            Self::Aggressive => "Aggressive",
            Self::Calm => "Calm",
        }
    }

    pub fn file_fragment(self) -> &'static str {
        match self {
            Self::Recommended => "recommended",
            Self::Aggressive => "aggressive",
            Self::Calm => "calm",
        }
    }
}

pub struct SearchResult {
    pub plot_path: PathBuf,
    pub summary_path: PathBuf,
    pub selected_by_label: HashMap<String, Point>,
    pub labels_by_key: HashMap<PointKey, Vec<String>>,
    pub diagnostics: Vec<PhaseDiag>,
}
