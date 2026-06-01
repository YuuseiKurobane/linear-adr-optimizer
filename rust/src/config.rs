use crate::types::Candidate;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct SearchConfig {
    pub quality_preset: String,
    pub export: PathBuf,
    pub presets: Vec<String>,
    pub target_dr: Option<f64>,
    pub days: i32,
    pub deck_size: i32,
    pub learn_limit: i32,
    pub seed: u64,
    pub threads: usize,
    pub matplotlib: bool,
    pub recommended_only: bool,
    pub aggressive_only: bool,
    pub calm_only: bool,

    pub phase1_eval_weight: f64,
    pub phase2_eval_weight: f64,
    pub phase3_eval_weight: f64,
    pub phase4_eval_weight: f64,
    pub final_eval_weight: f64,
    pub dr_prune_weight: f64,

    pub phase1_flat_step: f64,
    pub phase1_flat_half_steps: i32,
    pub phase1_s_step: f64,
    pub phase1_s_max: f64,
    pub phase1_d_step: f64,
    pub phase1_d_min: f64,
    pub phase1_expand: bool,
    pub phase1_expand_rounds: i32,
    pub phase1_expand_batch: i32,
    pub phase1_expand_overflow_factor: f64,

    pub phase2_flat_step: f64,
    pub phase2_s_step: f64,
    pub phase2_d_step: f64,
    pub phase3_flat_step: f64,
    pub phase3_s_step: f64,
    pub phase3_d_step: f64,
    pub phase4_flat_step: f64,
    pub phase4_s_step: f64,
    pub phase4_d_step: f64,
    pub phase4_seeds_per_objective: usize,
    pub phase4_max_steps: i32,

    pub promote_recommended: usize,
    pub promote_efficiency_potential: usize,
    pub promote_memory_potential: usize,
    pub promote_pareto_extra: usize,
    pub bridge_midpoint_limit: usize,
    pub experimental_bridge_midpoint_neighborhoods: bool,
    pub final_candidate_limit: usize,
    pub max_spread_final_candidates: usize,
    pub final_shortlist_recommended: usize,
    pub final_shortlist_efficiency: usize,
    pub final_shortlist_memory: usize,
    pub final_shortlist_frontier: usize,

    pub scout_potential_band_pct: f64,
    pub final_potential_band_pct: f64,
    pub aggressive_calm_regret_pct: f64,

    pub safety_s_max: f64,
    pub safety_checks: i32,
    pub ignore_safety: bool,
    pub legacy_unsafe_plot_display: bool,

    pub include_original: bool,
    pub original: Candidate,
    pub inspect_point: Vec<Candidate>,

    pub fixed_dr_start_pct: f64,
    pub fixed_dr_end_pct: f64,
    pub fixed_curve_coarse_weight: f64,
    pub fixed_curve_refine_weight: f64,
    pub fixed_curve_coarse_step_pct: f64,
    pub fixed_curve_refine_step_pct: f64,
    pub fixed_curve_initial_radius_pct: f64,
    pub fixed_curve_adapt_margin_pct: f64,
    pub fixed_curve_adapt_top_per_bucket: usize,
    pub fixed_curve_adapt_max_points: usize,
    pub fixed_dr_label_step_pct: f64,

    pub output_dir: PathBuf,
}

impl Default for SearchConfig {
    fn default() -> Self {
        let root = repo_root();
        Self {
            quality_preset: "medium-high".to_string(),
            export: root.join("exports"),
            presets: vec!["Yuusei".to_string()],
            target_dr: None,
            days: 1825,
            deck_size: 10000,
            learn_limit: 10,
            seed: 1234,
            threads: 0,
            matplotlib: false,
            recommended_only: false,
            aggressive_only: false,
            calm_only: false,
            phase1_eval_weight: 2000.0,
            phase2_eval_weight: 4000.0,
            phase3_eval_weight: 4000.0,
            phase4_eval_weight: 4000.0,
            final_eval_weight: 30000.0,
            dr_prune_weight: 1.0,
            phase1_flat_step: 0.04,
            phase1_flat_half_steps: 8,
            phase1_s_step: 0.02,
            phase1_s_max: 0.26,
            phase1_d_step: 0.02,
            phase1_d_min: -0.20,
            phase1_expand: true,
            phase1_expand_rounds: 8,
            phase1_expand_batch: 2,
            phase1_expand_overflow_factor: 2.0,
            phase2_flat_step: 0.02,
            phase2_s_step: 0.01,
            phase2_d_step: 0.01,
            phase3_flat_step: 0.01,
            phase3_s_step: 0.005,
            phase3_d_step: 0.005,
            phase4_flat_step: 0.002,
            phase4_s_step: 0.001,
            phase4_d_step: 0.001,
            phase4_seeds_per_objective: 6,
            phase4_max_steps: 8,
            promote_recommended: 50,
            promote_efficiency_potential: 25,
            promote_memory_potential: 25,
            promote_pareto_extra: 100,
            bridge_midpoint_limit: 50,
            experimental_bridge_midpoint_neighborhoods: false,
            final_candidate_limit: 180,
            max_spread_final_candidates: 12,
            final_shortlist_recommended: 120,
            final_shortlist_efficiency: 70,
            final_shortlist_memory: 70,
            final_shortlist_frontier: 100,
            scout_potential_band_pct: 0.3,
            final_potential_band_pct: 0.1,
            aggressive_calm_regret_pct: 0.50,
            safety_s_max: 1000.0,
            safety_checks: 3000,
            ignore_safety: false,
            legacy_unsafe_plot_display: false,
            include_original: false,
            original: Candidate::new(1.57, 0.135, -0.085),
            inspect_point: Vec::new(),
            fixed_dr_start_pct: 60.0,
            fixed_dr_end_pct: 96.0,
            fixed_curve_coarse_weight: 10000.0,
            fixed_curve_refine_weight: 80000.0,
            fixed_curve_coarse_step_pct: 1.0,
            fixed_curve_refine_step_pct: 0.2,
            fixed_curve_initial_radius_pct: 1.0,
            fixed_curve_adapt_margin_pct: 0.2,
            fixed_curve_adapt_top_per_bucket: 8,
            fixed_curve_adapt_max_points: 80,
            fixed_dr_label_step_pct: 10.0,
            output_dir: root.join("outputs"),
        }
    }
}

pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("rust directory has parent")
        .to_path_buf()
}

pub fn parse_dr(value: f64) -> Result<f64, String> {
    let mut dr = value;
    if dr > 1.0 {
        dr /= 100.0;
    }
    if !(0.0..1.0).contains(&dr) {
        return Err(format!(
            "DR must be between 0 and 1, or 0 and 100 percent; got {value}"
        ));
    }
    Ok(dr)
}

pub fn normalize<const N: usize>(mut values: [f32; N], name: &str) -> Result<[f32; N], String> {
    let total: f32 = values.iter().sum();
    if total <= 0.0 {
        return Err(format!("{name} must have a positive sum"));
    }
    for value in &mut values {
        *value /= total;
    }
    Ok(values)
}

#[derive(Debug, Clone, Copy)]
pub struct PresetSpec {
    pub name: &'static str,
    pub values: &'static [PresetValue],
}

#[derive(Debug, Clone, Copy)]
pub enum PresetValue {
    I32(&'static str, i32),
    Usize(&'static str, usize),
    F64(&'static str, f64),
}

const FULL_HORIZON: &[PresetValue] = &[
    PresetValue::I32("days", 1825),
    PresetValue::I32("deck_size", 10000),
    PresetValue::I32("learn_limit", 10),
];

const POTATO: &[PresetValue] = &[
    PresetValue::F64("phase1_eval_weight", 300.0),
    PresetValue::F64("phase2_eval_weight", 600.0),
    PresetValue::F64("phase3_eval_weight", 600.0),
    PresetValue::F64("phase4_eval_weight", 600.0),
    PresetValue::F64("final_eval_weight", 12000.0),
    PresetValue::F64("fixed_curve_coarse_weight", 3000.0),
    PresetValue::F64("fixed_curve_refine_weight", 30000.0),
    PresetValue::F64("fixed_curve_coarse_step_pct", 4.0),
    PresetValue::F64("fixed_curve_refine_step_pct", 1.0),
    PresetValue::F64("fixed_curve_initial_radius_pct", 0.4),
    PresetValue::F64("fixed_curve_adapt_margin_pct", 0.2),
    PresetValue::Usize("fixed_curve_adapt_top_per_bucket", 1),
    PresetValue::Usize("fixed_curve_adapt_max_points", 12),
    PresetValue::F64("phase1_flat_step", 0.08),
    PresetValue::I32("phase1_flat_half_steps", 3),
    PresetValue::F64("phase1_s_step", 0.05),
    PresetValue::F64("phase1_s_max", 0.25),
    PresetValue::F64("phase1_d_step", 0.05),
    PresetValue::F64("phase1_d_min", -0.20),
    PresetValue::I32("phase1_expand_rounds", 0),
    PresetValue::Usize("promote_recommended", 6),
    PresetValue::Usize("promote_efficiency_potential", 3),
    PresetValue::Usize("promote_memory_potential", 3),
    PresetValue::Usize("promote_pareto_extra", 8),
    PresetValue::Usize("phase4_seeds_per_objective", 1),
    PresetValue::I32("phase4_max_steps", 1),
    PresetValue::Usize("final_candidate_limit", 32),
    PresetValue::Usize("max_spread_final_candidates", 2),
    PresetValue::Usize("final_shortlist_recommended", 24),
    PresetValue::Usize("final_shortlist_efficiency", 12),
    PresetValue::Usize("final_shortlist_memory", 12),
    PresetValue::Usize("final_shortlist_frontier", 16),
    PresetValue::I32("safety_checks", 3000),
];

const LITE: &[PresetValue] = &[
    PresetValue::F64("phase1_eval_weight", 600.0),
    PresetValue::F64("phase2_eval_weight", 1200.0),
    PresetValue::F64("phase3_eval_weight", 1200.0),
    PresetValue::F64("phase4_eval_weight", 1200.0),
    PresetValue::F64("final_eval_weight", 60000.0),
    PresetValue::F64("fixed_curve_coarse_weight", 5000.0),
    PresetValue::F64("fixed_curve_refine_weight", 30000.0),
    PresetValue::F64("fixed_curve_coarse_step_pct", 2.5),
    PresetValue::F64("fixed_curve_refine_step_pct", 0.5),
    PresetValue::F64("fixed_curve_initial_radius_pct", 0.7),
    PresetValue::F64("fixed_curve_adapt_margin_pct", 0.25),
    PresetValue::Usize("fixed_curve_adapt_top_per_bucket", 3),
    PresetValue::Usize("fixed_curve_adapt_max_points", 30),
    PresetValue::F64("phase1_flat_step", 0.06),
    PresetValue::I32("phase1_flat_half_steps", 6),
    PresetValue::F64("phase1_s_step", 0.035),
    PresetValue::F64("phase1_s_max", 0.28),
    PresetValue::F64("phase1_d_step", 0.035),
    PresetValue::F64("phase1_d_min", -0.21),
    PresetValue::I32("phase1_expand_rounds", 0),
    PresetValue::Usize("promote_recommended", 12),
    PresetValue::Usize("promote_efficiency_potential", 6),
    PresetValue::Usize("promote_memory_potential", 6),
    PresetValue::Usize("promote_pareto_extra", 20),
    PresetValue::Usize("phase4_seeds_per_objective", 1),
    PresetValue::I32("phase4_max_steps", 2),
    PresetValue::Usize("final_candidate_limit", 60),
    PresetValue::Usize("max_spread_final_candidates", 4),
    PresetValue::Usize("final_shortlist_recommended", 50),
    PresetValue::Usize("final_shortlist_efficiency", 25),
    PresetValue::Usize("final_shortlist_memory", 25),
    PresetValue::Usize("final_shortlist_frontier", 35),
    PresetValue::I32("safety_checks", 3000),
];

const MEDIUM: &[PresetValue] = &[
    PresetValue::F64("phase1_eval_weight", 2000.0),
    PresetValue::F64("phase2_eval_weight", 4000.0),
    PresetValue::F64("phase3_eval_weight", 4000.0),
    PresetValue::F64("phase4_eval_weight", 4000.0),
    PresetValue::F64("final_eval_weight", 100000.0),
    PresetValue::F64("fixed_curve_coarse_weight", 10000.0),
    PresetValue::F64("fixed_curve_refine_weight", 80000.0),
    PresetValue::F64("fixed_curve_coarse_step_pct", 1.5),
    PresetValue::F64("fixed_curve_refine_step_pct", 0.3),
    PresetValue::F64("fixed_curve_initial_radius_pct", 1.0),
    PresetValue::F64("fixed_curve_adapt_margin_pct", 0.2),
    PresetValue::Usize("fixed_curve_adapt_top_per_bucket", 6),
    PresetValue::Usize("fixed_curve_adapt_max_points", 60),
    PresetValue::F64("phase1_flat_step", 0.05),
    PresetValue::I32("phase1_flat_half_steps", 6),
    PresetValue::F64("phase1_s_step", 0.025),
    PresetValue::F64("phase1_s_max", 0.275),
    PresetValue::F64("phase1_d_step", 0.025),
    PresetValue::F64("phase1_d_min", -0.225),
    PresetValue::I32("phase1_expand_rounds", 1),
    PresetValue::Usize("promote_recommended", 28),
    PresetValue::Usize("promote_efficiency_potential", 14),
    PresetValue::Usize("promote_memory_potential", 14),
    PresetValue::Usize("promote_pareto_extra", 50),
    PresetValue::Usize("phase4_seeds_per_objective", 3),
    PresetValue::I32("phase4_max_steps", 4),
    PresetValue::Usize("final_candidate_limit", 100),
    PresetValue::Usize("max_spread_final_candidates", 8),
    PresetValue::Usize("final_shortlist_recommended", 90),
    PresetValue::Usize("final_shortlist_efficiency", 50),
    PresetValue::Usize("final_shortlist_memory", 50),
    PresetValue::Usize("final_shortlist_frontier", 70),
    PresetValue::I32("safety_checks", 3000),
];

const MEDIUM_HIGH: &[PresetValue] = &[
    PresetValue::F64("phase1_eval_weight", 2000.0),
    PresetValue::F64("phase2_eval_weight", 4000.0),
    PresetValue::F64("phase3_eval_weight", 4000.0),
    PresetValue::F64("phase4_eval_weight", 4000.0),
    PresetValue::F64("final_eval_weight", 200000.0),
    PresetValue::F64("fixed_curve_coarse_weight", 10000.0),
    PresetValue::F64("fixed_curve_refine_weight", 80000.0),
    PresetValue::F64("fixed_curve_coarse_step_pct", 1.0),
    PresetValue::F64("fixed_curve_refine_step_pct", 0.2),
    PresetValue::F64("fixed_curve_initial_radius_pct", 1.0),
    PresetValue::F64("fixed_curve_adapt_margin_pct", 0.2),
    PresetValue::Usize("fixed_curve_adapt_top_per_bucket", 8),
    PresetValue::Usize("fixed_curve_adapt_max_points", 80),
    PresetValue::F64("phase1_flat_step", 0.04),
    PresetValue::I32("phase1_flat_half_steps", 8),
    PresetValue::F64("phase1_s_step", 0.02),
    PresetValue::F64("phase1_s_max", 0.26),
    PresetValue::F64("phase1_d_step", 0.02),
    PresetValue::F64("phase1_d_min", -0.20),
    PresetValue::I32("phase1_expand_rounds", 8),
    PresetValue::Usize("promote_recommended", 50),
    PresetValue::Usize("promote_efficiency_potential", 25),
    PresetValue::Usize("promote_memory_potential", 25),
    PresetValue::Usize("promote_pareto_extra", 100),
    PresetValue::Usize("phase4_seeds_per_objective", 6),
    PresetValue::I32("phase4_max_steps", 8),
    PresetValue::Usize("final_candidate_limit", 180),
    PresetValue::Usize("max_spread_final_candidates", 12),
    PresetValue::Usize("final_shortlist_recommended", 120),
    PresetValue::Usize("final_shortlist_efficiency", 70),
    PresetValue::Usize("final_shortlist_memory", 70),
    PresetValue::Usize("final_shortlist_frontier", 100),
    PresetValue::I32("safety_checks", 3000),
];

const HIGH: &[PresetValue] = &[
    PresetValue::F64("phase1_eval_weight", 8000.0),
    PresetValue::F64("phase2_eval_weight", 20000.0),
    PresetValue::F64("phase3_eval_weight", 50000.0),
    PresetValue::F64("phase4_eval_weight", 50000.0),
    PresetValue::F64("final_eval_weight", 500000.0),
    PresetValue::F64("fixed_curve_coarse_weight", 20000.0),
    PresetValue::F64("fixed_curve_refine_weight", 160000.0),
    PresetValue::F64("fixed_curve_coarse_step_pct", 1.0),
    PresetValue::F64("fixed_curve_refine_step_pct", 0.2),
    PresetValue::F64("fixed_curve_initial_radius_pct", 1.2),
    PresetValue::F64("fixed_curve_adapt_margin_pct", 0.2),
    PresetValue::Usize("fixed_curve_adapt_top_per_bucket", 10),
    PresetValue::Usize("fixed_curve_adapt_max_points", 110),
    PresetValue::F64("phase1_flat_step", 0.04),
    PresetValue::I32("phase1_flat_half_steps", 9),
    PresetValue::F64("phase1_s_step", 0.02),
    PresetValue::F64("phase1_s_max", 0.28),
    PresetValue::F64("phase1_d_step", 0.02),
    PresetValue::F64("phase1_d_min", -0.22),
    PresetValue::I32("phase1_expand_rounds", 8),
    PresetValue::Usize("promote_recommended", 65),
    PresetValue::Usize("promote_efficiency_potential", 35),
    PresetValue::Usize("promote_memory_potential", 35),
    PresetValue::Usize("promote_pareto_extra", 140),
    PresetValue::Usize("phase4_seeds_per_objective", 8),
    PresetValue::I32("phase4_max_steps", 10),
    PresetValue::Usize("final_candidate_limit", 360),
    PresetValue::Usize("max_spread_final_candidates", 16),
    PresetValue::Usize("final_shortlist_recommended", 120),
    PresetValue::Usize("final_shortlist_efficiency", 70),
    PresetValue::Usize("final_shortlist_memory", 70),
    PresetValue::Usize("final_shortlist_frontier", 120),
    PresetValue::I32("safety_checks", 5000),
];

pub fn preset_names() -> &'static [&'static str] {
    &["potato", "lite", "medium", "medium-high", "high"]
}

pub fn apply_quality_preset(config: &mut SearchConfig, name: &str) -> Result<(), String> {
    config.quality_preset = name.to_string();
    for value in FULL_HORIZON {
        apply_value(config, *value)?;
    }
    let values = match name {
        "potato" => POTATO,
        "lite" => LITE,
        "medium" => MEDIUM,
        "medium-high" => MEDIUM_HIGH,
        "high" => HIGH,
        _ => {
            return Err(format!(
                "Unknown quality preset {name:?}; valid presets: {}",
                preset_names().join(", ")
            ))
        }
    };
    for value in values {
        apply_value(config, *value)?;
    }
    Ok(())
}

pub fn apply_value(config: &mut SearchConfig, value: PresetValue) -> Result<(), String> {
    match value {
        PresetValue::I32(name, v) => match name {
            "days" => config.days = v,
            "deck_size" => config.deck_size = v,
            "learn_limit" => config.learn_limit = v,
            "phase1_flat_half_steps" => config.phase1_flat_half_steps = v,
            "phase1_expand_rounds" => config.phase1_expand_rounds = v,
            "phase4_max_steps" => config.phase4_max_steps = v,
            "safety_checks" => config.safety_checks = v,
            _ => return Err(format!("unknown i32 config key {name}")),
        },
        PresetValue::Usize(name, v) => match name {
            "fixed_curve_adapt_top_per_bucket" => config.fixed_curve_adapt_top_per_bucket = v,
            "fixed_curve_adapt_max_points" => config.fixed_curve_adapt_max_points = v,
            "promote_recommended" => config.promote_recommended = v,
            "promote_efficiency_potential" => config.promote_efficiency_potential = v,
            "promote_memory_potential" => config.promote_memory_potential = v,
            "promote_pareto_extra" => config.promote_pareto_extra = v,
            "phase4_seeds_per_objective" => config.phase4_seeds_per_objective = v,
            "final_candidate_limit" => config.final_candidate_limit = v,
            "max_spread_final_candidates" => config.max_spread_final_candidates = v,
            "final_shortlist_recommended" => config.final_shortlist_recommended = v,
            "final_shortlist_efficiency" => config.final_shortlist_efficiency = v,
            "final_shortlist_memory" => config.final_shortlist_memory = v,
            "final_shortlist_frontier" => config.final_shortlist_frontier = v,
            _ => return Err(format!("unknown usize config key {name}")),
        },
        PresetValue::F64(name, v) => match name {
            "phase1_eval_weight" => config.phase1_eval_weight = v,
            "phase2_eval_weight" => config.phase2_eval_weight = v,
            "phase3_eval_weight" => config.phase3_eval_weight = v,
            "phase4_eval_weight" => config.phase4_eval_weight = v,
            "final_eval_weight" => config.final_eval_weight = v,
            "fixed_curve_coarse_weight" => config.fixed_curve_coarse_weight = v,
            "fixed_curve_refine_weight" => config.fixed_curve_refine_weight = v,
            "fixed_curve_coarse_step_pct" => config.fixed_curve_coarse_step_pct = v,
            "fixed_curve_refine_step_pct" => config.fixed_curve_refine_step_pct = v,
            "fixed_curve_initial_radius_pct" => config.fixed_curve_initial_radius_pct = v,
            "fixed_curve_adapt_margin_pct" => config.fixed_curve_adapt_margin_pct = v,
            "phase1_flat_step" => config.phase1_flat_step = v,
            "phase1_s_step" => config.phase1_s_step = v,
            "phase1_s_max" => config.phase1_s_max = v,
            "phase1_d_step" => config.phase1_d_step = v,
            "phase1_d_min" => config.phase1_d_min = v,
            _ => return Err(format!("unknown f64 config key {name}")),
        },
    }
    Ok(())
}

pub fn json_safe_config(config: &SearchConfig) -> serde_json::Value {
    serde_json::json!({
        "quality_preset": config.quality_preset,
        "export": config.export,
        "preset": config.presets.first().cloned().unwrap_or_default(),
        "presets": config.presets,
        "target_dr": config.target_dr,
        "days": config.days,
        "deck_size": config.deck_size,
        "learn_limit": config.learn_limit,
        "seed": config.seed,
        "threads": config.threads,
        "matplotlib": config.matplotlib,
        "recommended_only": config.recommended_only,
        "aggressive_only": config.aggressive_only,
        "calm_only": config.calm_only,
        "phase1_eval_weight": config.phase1_eval_weight,
        "phase2_eval_weight": config.phase2_eval_weight,
        "phase3_eval_weight": config.phase3_eval_weight,
        "phase4_eval_weight": config.phase4_eval_weight,
        "final_eval_weight": config.final_eval_weight,
        "dr_prune_weight": config.dr_prune_weight,
        "phase1_flat_step": config.phase1_flat_step,
        "phase1_flat_half_steps": config.phase1_flat_half_steps,
        "phase1_s_step": config.phase1_s_step,
        "phase1_s_max": config.phase1_s_max,
        "phase1_d_step": config.phase1_d_step,
        "phase1_d_min": config.phase1_d_min,
        "phase1_expand": config.phase1_expand,
        "phase1_expand_rounds": config.phase1_expand_rounds,
        "phase1_expand_batch": config.phase1_expand_batch,
        "phase1_expand_overflow_factor": config.phase1_expand_overflow_factor,
        "phase2_flat_step": config.phase2_flat_step,
        "phase2_s_step": config.phase2_s_step,
        "phase2_d_step": config.phase2_d_step,
        "phase3_flat_step": config.phase3_flat_step,
        "phase3_s_step": config.phase3_s_step,
        "phase3_d_step": config.phase3_d_step,
        "phase4_flat_step": config.phase4_flat_step,
        "phase4_s_step": config.phase4_s_step,
        "phase4_d_step": config.phase4_d_step,
        "phase4_seeds_per_objective": config.phase4_seeds_per_objective,
        "phase4_max_steps": config.phase4_max_steps,
        "promote_recommended": config.promote_recommended,
        "promote_efficiency_potential": config.promote_efficiency_potential,
        "promote_memory_potential": config.promote_memory_potential,
        "promote_pareto_extra": config.promote_pareto_extra,
        "bridge_midpoint_limit": config.bridge_midpoint_limit,
        "experimental_bridge_midpoint_neighborhoods": config.experimental_bridge_midpoint_neighborhoods,
        "final_candidate_limit": config.final_candidate_limit,
        "max_spread_final_candidates": config.max_spread_final_candidates,
        "final_shortlist_recommended": config.final_shortlist_recommended,
        "final_shortlist_efficiency": config.final_shortlist_efficiency,
        "final_shortlist_memory": config.final_shortlist_memory,
        "final_shortlist_frontier": config.final_shortlist_frontier,
        "scout_potential_band_pct": config.scout_potential_band_pct,
        "final_potential_band_pct": config.final_potential_band_pct,
        "aggressive_calm_regret_pct": config.aggressive_calm_regret_pct,
        "safety_s_max": config.safety_s_max,
        "safety_checks": config.safety_checks,
        "ignore_safety": config.ignore_safety,
        "legacy_unsafe_plot_display": config.legacy_unsafe_plot_display,
        "include_original": config.include_original,
        "original": [config.original.flat, config.original.s_multi, config.original.d_multi],
        "inspect_point": config.inspect_point.iter().map(|p| vec![p.flat, p.s_multi, p.d_multi]).collect::<Vec<_>>(),
        "fixed_dr_start_pct": config.fixed_dr_start_pct,
        "fixed_dr_end_pct": config.fixed_dr_end_pct,
        "fixed_curve_coarse_weight": config.fixed_curve_coarse_weight,
        "fixed_curve_refine_weight": config.fixed_curve_refine_weight,
        "fixed_curve_coarse_step_pct": config.fixed_curve_coarse_step_pct,
        "fixed_curve_refine_step_pct": config.fixed_curve_refine_step_pct,
        "fixed_curve_initial_radius_pct": config.fixed_curve_initial_radius_pct,
        "fixed_curve_adapt_margin_pct": config.fixed_curve_adapt_margin_pct,
        "fixed_curve_adapt_top_per_bucket": config.fixed_curve_adapt_top_per_bucket,
        "fixed_curve_adapt_max_points": config.fixed_curve_adapt_max_points,
        "fixed_dr_label_step_pct": config.fixed_dr_label_step_pct,
        "output_dir": config.output_dir,
    })
}
