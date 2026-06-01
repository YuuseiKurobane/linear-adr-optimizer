use crate::config::{apply_quality_preset, parse_dr, preset_names, SearchConfig};
use crate::search::batch::run_batch;
use crate::types::Candidate;
use std::env;
use std::path::PathBuf;

pub fn run_cli_from_env() -> i32 {
    let args: Vec<String> = env::args().skip(1).collect();
    match run_cli(args) {
        Ok(()) => 0,
        Err(CliExit::Help) => 0,
        Err(CliExit::Version) => 0,
        Err(CliExit::Error(message)) => {
            eprintln!("{message}");
            2
        }
    }
}

pub fn run_cli(args: Vec<String>) -> Result<(), CliExit> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h" || arg == "help") {
        println!("{}", help_text());
        return Err(CliExit::Help);
    }
    if args.iter().any(|arg| arg == "--version" || arg == "version") {
        println!("adr-optimizer {}", env!("CARGO_PKG_VERSION"));
        return Err(CliExit::Version);
    }

    let mut config = parse_args(&args).map_err(CliExit::Error)?;
    if config.presets.is_empty() {
        config.presets.push("Yuusei".to_string());
    }
    run_batch(&config).map_err(CliExit::Error)?;
    Ok(())
}

#[derive(Debug)]
pub enum CliExit {
    Help,
    Version,
    Error(String),
}

pub fn parse_args(args: &[String]) -> Result<SearchConfig, String> {
    let mut quality = "medium-high".to_string();
    let mut i = 0;
    while i < args.len() {
        let (name, inline) = split_option(&args[i]);
        if name == "--quality-preset" {
            quality = value_for(args, &mut i, inline, name)?.to_string();
        }
        i += 1;
    }

    let mut config = SearchConfig::default();
    apply_quality_preset(&mut config, &quality)?;

    let mut preset_seen = false;
    let mut point_only_count = 0;
    let mut i = 0;
    while i < args.len() {
        let token = &args[i];
        let (name, inline) = split_option(token);
        match name {
            "--quality-preset" => {
                let _ = value_for(args, &mut i, inline, name)?;
            }
            "--export" => config.export = PathBuf::from(value_for(args, &mut i, inline, name)?),
            "--preset" | "--presets" => {
                let value = value_for(args, &mut i, inline, name)?;
                if !preset_seen {
                    config.presets.clear();
                    preset_seen = true;
                }
                config
                    .presets
                    .extend(value.split(',').map(str::trim).filter(|s| !s.is_empty()).map(str::to_string));
            }
            "--target-dr" => {
                config.target_dr = Some(parse_f64(value_for(args, &mut i, inline, name)?, name)?);
                if let Some(value) = config.target_dr {
                    parse_dr(value)?;
                }
            }
            "--days" => config.days = parse_i32(value_for(args, &mut i, inline, name)?, name)?,
            "--deck-size" => config.deck_size = parse_i32(value_for(args, &mut i, inline, name)?, name)?,
            "--learn-limit" => config.learn_limit = parse_i32(value_for(args, &mut i, inline, name)?, name)?,
            "--seed" => config.seed = parse_u64(value_for(args, &mut i, inline, name)?, name)?,
            "--threads" => config.threads = parse_usize(value_for(args, &mut i, inline, name)?, name)?,
            "--matplotlib" => config.matplotlib = true,
            "--recommended-only" => {
                config.recommended_only = true;
                point_only_count += 1;
            }
            "--aggressive-only" => {
                config.aggressive_only = true;
                point_only_count += 1;
            }
            "--calm-only" => {
                config.calm_only = true;
                point_only_count += 1;
            }
            "--phase1-eval-weight" => config.phase1_eval_weight = parse_f64(value_for(args, &mut i, inline, name)?, name)?,
            "--phase2-eval-weight" => config.phase2_eval_weight = parse_f64(value_for(args, &mut i, inline, name)?, name)?,
            "--phase3-eval-weight" => config.phase3_eval_weight = parse_f64(value_for(args, &mut i, inline, name)?, name)?,
            "--phase4-eval-weight" => config.phase4_eval_weight = parse_f64(value_for(args, &mut i, inline, name)?, name)?,
            "--final-eval-weight" => config.final_eval_weight = parse_f64(value_for(args, &mut i, inline, name)?, name)?,
            "--dr-prune-weight" => config.dr_prune_weight = parse_f64(value_for(args, &mut i, inline, name)?, name)?,
            "--phase1-flat-step" => config.phase1_flat_step = parse_f64(value_for(args, &mut i, inline, name)?, name)?,
            "--phase1-flat-half-steps" => config.phase1_flat_half_steps = parse_i32(value_for(args, &mut i, inline, name)?, name)?,
            "--phase1-s-step" => config.phase1_s_step = parse_f64(value_for(args, &mut i, inline, name)?, name)?,
            "--phase1-s-max" => config.phase1_s_max = parse_f64(value_for(args, &mut i, inline, name)?, name)?,
            "--phase1-d-step" => config.phase1_d_step = parse_f64(value_for(args, &mut i, inline, name)?, name)?,
            "--phase1-d-min" => config.phase1_d_min = parse_f64(value_for(args, &mut i, inline, name)?, name)?,
            "--phase1-expand" => config.phase1_expand = true,
            "--no-phase1-expand" => config.phase1_expand = false,
            "--phase1-expand-rounds" => config.phase1_expand_rounds = parse_i32(value_for(args, &mut i, inline, name)?, name)?,
            "--phase1-expand-batch" => config.phase1_expand_batch = parse_i32(value_for(args, &mut i, inline, name)?, name)?,
            "--phase1-expand-overflow-factor" => config.phase1_expand_overflow_factor = parse_f64(value_for(args, &mut i, inline, name)?, name)?,
            "--phase2-flat-step" => config.phase2_flat_step = parse_f64(value_for(args, &mut i, inline, name)?, name)?,
            "--phase2-s-step" => config.phase2_s_step = parse_f64(value_for(args, &mut i, inline, name)?, name)?,
            "--phase2-d-step" => config.phase2_d_step = parse_f64(value_for(args, &mut i, inline, name)?, name)?,
            "--phase3-flat-step" => config.phase3_flat_step = parse_f64(value_for(args, &mut i, inline, name)?, name)?,
            "--phase3-s-step" => config.phase3_s_step = parse_f64(value_for(args, &mut i, inline, name)?, name)?,
            "--phase3-d-step" => config.phase3_d_step = parse_f64(value_for(args, &mut i, inline, name)?, name)?,
            "--phase4-flat-step" => config.phase4_flat_step = parse_f64(value_for(args, &mut i, inline, name)?, name)?,
            "--phase4-s-step" => config.phase4_s_step = parse_f64(value_for(args, &mut i, inline, name)?, name)?,
            "--phase4-d-step" => config.phase4_d_step = parse_f64(value_for(args, &mut i, inline, name)?, name)?,
            "--phase4-seeds-per-objective" => config.phase4_seeds_per_objective = parse_usize(value_for(args, &mut i, inline, name)?, name)?,
            "--phase4-max-steps" => config.phase4_max_steps = parse_i32(value_for(args, &mut i, inline, name)?, name)?,
            "--promote-recommended" => config.promote_recommended = parse_usize(value_for(args, &mut i, inline, name)?, name)?,
            "--promote-efficiency-potential" => config.promote_efficiency_potential = parse_usize(value_for(args, &mut i, inline, name)?, name)?,
            "--promote-memory-potential" => config.promote_memory_potential = parse_usize(value_for(args, &mut i, inline, name)?, name)?,
            "--promote-pareto-extra" => config.promote_pareto_extra = parse_usize(value_for(args, &mut i, inline, name)?, name)?,
            "--bridge-midpoint-limit" => config.bridge_midpoint_limit = parse_usize(value_for(args, &mut i, inline, name)?, name)?,
            "--experimental-bridge-midpoint-neighborhoods" => config.experimental_bridge_midpoint_neighborhoods = true,
            "--final-candidate-limit" => config.final_candidate_limit = parse_usize(value_for(args, &mut i, inline, name)?, name)?,
            "--max-spread-final-candidates" => config.max_spread_final_candidates = parse_usize(value_for(args, &mut i, inline, name)?, name)?,
            "--final-shortlist-recommended" => config.final_shortlist_recommended = parse_usize(value_for(args, &mut i, inline, name)?, name)?,
            "--final-shortlist-efficiency" => config.final_shortlist_efficiency = parse_usize(value_for(args, &mut i, inline, name)?, name)?,
            "--final-shortlist-memory" => config.final_shortlist_memory = parse_usize(value_for(args, &mut i, inline, name)?, name)?,
            "--final-shortlist-frontier" => config.final_shortlist_frontier = parse_usize(value_for(args, &mut i, inline, name)?, name)?,
            "--scout-potential-band-pct" => config.scout_potential_band_pct = parse_f64(value_for(args, &mut i, inline, name)?, name)?,
            "--final-potential-band-pct" => config.final_potential_band_pct = parse_f64(value_for(args, &mut i, inline, name)?, name)?,
            "--aggressive-calm-regret-pct" => config.aggressive_calm_regret_pct = parse_f64(value_for(args, &mut i, inline, name)?, name)?,
            "--safety-s-max" => config.safety_s_max = parse_f64(value_for(args, &mut i, inline, name)?, name)?,
            "--safety-checks" => config.safety_checks = parse_i32(value_for(args, &mut i, inline, name)?, name)?,
            "--ignore-safety" => config.ignore_safety = true,
            "--legacy-unsafe-plot-display" => config.legacy_unsafe_plot_display = true,
            "--include-original" => config.include_original = true,
            "--original" => {
                let a = parse_f64(value_for(args, &mut i, inline, name)?, name)?;
                let b = parse_f64(next_positional(args, &mut i, name)?, name)?;
                let c = parse_f64(next_positional(args, &mut i, name)?, name)?;
                config.original = Candidate::new(a, b, c).snap();
            }
            "--inspect-point" => {
                let a = parse_f64(value_for(args, &mut i, inline, name)?, name)?;
                let b = parse_f64(next_positional(args, &mut i, name)?, name)?;
                let c = parse_f64(next_positional(args, &mut i, name)?, name)?;
                config.inspect_point.push(Candidate::new(a, b, c).snap());
            }
            "--fixed-dr-start-pct" => config.fixed_dr_start_pct = parse_f64(value_for(args, &mut i, inline, name)?, name)?,
            "--fixed-dr-end-pct" => config.fixed_dr_end_pct = parse_f64(value_for(args, &mut i, inline, name)?, name)?,
            "--fixed-curve-coarse-weight" => config.fixed_curve_coarse_weight = parse_f64(value_for(args, &mut i, inline, name)?, name)?,
            "--fixed-curve-refine-weight" => config.fixed_curve_refine_weight = parse_f64(value_for(args, &mut i, inline, name)?, name)?,
            "--fixed-curve-coarse-step-pct" => config.fixed_curve_coarse_step_pct = parse_f64(value_for(args, &mut i, inline, name)?, name)?,
            "--fixed-curve-refine-step-pct" => config.fixed_curve_refine_step_pct = parse_f64(value_for(args, &mut i, inline, name)?, name)?,
            "--fixed-curve-initial-radius-pct" => config.fixed_curve_initial_radius_pct = parse_f64(value_for(args, &mut i, inline, name)?, name)?,
            "--fixed-curve-adapt-margin-pct" => config.fixed_curve_adapt_margin_pct = parse_f64(value_for(args, &mut i, inline, name)?, name)?,
            "--fixed-curve-adapt-top-per-bucket" => config.fixed_curve_adapt_top_per_bucket = parse_usize(value_for(args, &mut i, inline, name)?, name)?,
            "--fixed-curve-adapt-max-points" => config.fixed_curve_adapt_max_points = parse_usize(value_for(args, &mut i, inline, name)?, name)?,
            "--fixed-dr-label-step-pct" => config.fixed_dr_label_step_pct = parse_f64(value_for(args, &mut i, inline, name)?, name)?,
            "--output-dir" => config.output_dir = PathBuf::from(value_for(args, &mut i, inline, name)?),
            _ if token.starts_with('-') => return Err(format!("unknown option: {name}")),
            _ => return Err(format!("unexpected positional argument: {token}")),
        }
        i += 1;
    }

    if point_only_count > 1 {
        return Err("--recommended-only, --aggressive-only, and --calm-only are mutually exclusive".to_string());
    }
    Ok(config)
}

fn split_option(token: &str) -> (&str, Option<&str>) {
    if let Some((name, value)) = token.split_once('=') {
        (name, Some(value))
    } else {
        (token, None)
    }
}

fn value_for<'a>(
    args: &'a [String],
    index: &mut usize,
    inline: Option<&'a str>,
    name: &str,
) -> Result<&'a str, String> {
    if let Some(value) = inline {
        return Ok(value);
    }
    *index += 1;
    args.get(*index)
        .map(String::as_str)
        .ok_or_else(|| format!("{name} expects a value"))
}

fn next_positional<'a>(args: &'a [String], index: &mut usize, name: &str) -> Result<&'a str, String> {
    *index += 1;
    args.get(*index)
        .map(String::as_str)
        .ok_or_else(|| format!("{name} expects three values"))
}

fn parse_f64(value: &str, name: &str) -> Result<f64, String> {
    value.parse::<f64>().map_err(|_| format!("{name} expected a number, got {value:?}"))
}

fn parse_i32(value: &str, name: &str) -> Result<i32, String> {
    value.parse::<i32>().map_err(|_| format!("{name} expected an integer, got {value:?}"))
}

fn parse_u64(value: &str, name: &str) -> Result<u64, String> {
    value.parse::<u64>().map_err(|_| format!("{name} expected an integer, got {value:?}"))
}

fn parse_usize(value: &str, name: &str) -> Result<usize, String> {
    value.parse::<usize>().map_err(|_| format!("{name} expected a non-negative integer, got {value:?}"))
}

fn help_text() -> String {
    format!(
        "adr-optimizer {}\n\n\
         Rust-first linear ADR optimizer.\n\n\
         Usage:\n\
           adr-optimizer --export <PATH> --preset <NAME> --target-dr <DR> [options]\n\
           adr-optimizer --preset Yuusei --target-dr 85\n\n\
         Core flags:\n\
           --quality-preset <{}>   Speed/accuracy preset [default: medium-high]\n\
           --export <PATH>                JSONL file or export directory [default: exports]\n\
           --preset <NAME[,NAME...]>      Deck preset/deck selector; may be repeated\n\
           --target-dr <DR>               Desired retention as fraction or percent\n\
           --days <N> --deck-size <N> --learn-limit <N>\n\
           --seed <N> --threads <N>\n\
           --matplotlib                   Write PNG instead of HTML\n\
           --recommended-only             Write only Recommended TXT\n\
           --aggressive-only              Write only Aggressive TXT\n\
           --calm-only                    Write only Calm TXT\n\
           --ignore-safety                Skip safety checks/filtering\n\n\
         Advanced phase/search flags are preserved from the Python CLI.",
        env!("CARGO_PKG_VERSION"),
        preset_names().join("|")
    )
}
