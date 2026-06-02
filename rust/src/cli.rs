use crate::config::{apply_quality_preset, json_safe_config, parse_dr, preset_names, SearchConfig};
use crate::output::text::timestamp;
use crate::search::batch::{run_batch, run_configured_batch};
use crate::types::{Candidate, ResultLabel, SearchResult};
use chrono::Local;
use serde::Deserialize;
use serde_json::{json, Value};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

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

    if let Some(path) = batch_config_path(&args).map_err(CliExit::Error)? {
        run_batch_config(&path).map_err(CliExit::Error)?;
        return Ok(());
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

#[derive(Debug, Deserialize)]
struct BatchConfigFile {
    export: Option<PathBuf>,
    output_dir: Option<PathBuf>,
    #[serde(alias = "output_path", alias = "result_path")]
    batch_output: Option<PathBuf>,
    quality_preset: Option<String>,
    threads: Option<usize>,
    #[serde(alias = "settings")]
    config: Option<Value>,
    overrides: Option<Value>,
    jobs: Vec<BatchJobSpec>,
}

#[derive(Debug, Deserialize)]
struct BatchJobSpec {
    id: Option<String>,
    preset: String,
    export: Option<PathBuf>,
    output_dir: Option<PathBuf>,
    quality_preset: Option<String>,
    target_dr: Option<f64>,
    #[serde(alias = "optimizer_strategy", alias = "point_only")]
    selection: Option<String>,
    threads: Option<usize>,
    #[serde(alias = "settings")]
    config: Option<Value>,
    overrides: Option<Value>,
}

#[derive(Debug, Clone)]
struct BatchJobRuntime {
    id: Option<String>,
    preset: String,
    config: SearchConfig,
}

fn batch_config_path(args: &[String]) -> Result<Option<PathBuf>, String> {
    if !args
        .iter()
        .any(|arg| arg == "--batch-config" || arg.starts_with("--batch-config="))
    {
        return Ok(None);
    }

    let mut path = None;
    let mut i = 0;
    while i < args.len() {
        let token = &args[i];
        let (name, inline) = split_option(token);
        if name == "--batch-config" {
            if path.is_some() {
                return Err("--batch-config may only be provided once".to_string());
            }
            path = Some(PathBuf::from(value_for(args, &mut i, inline, name)?));
        } else {
            return Err("--batch-config cannot be combined with ordinary CLI options".to_string());
        }
        i += 1;
    }
    path.ok_or_else(|| "--batch-config expects a path".to_string()).map(Some)
}

fn run_batch_config(path: &Path) -> Result<(), String> {
    let text = fs::read_to_string(path)
        .map_err(|err| format!("failed to read batch config {}: {err}", path.display()))?;
    let text = text.trim_start_matches('\u{feff}');
    let spec: BatchConfigFile = serde_json::from_str(text)
        .map_err(|err| format!("failed to parse batch config {}: {err}", path.display()))?;
    let jobs = build_batch_jobs(&spec)?;
    let configs = jobs.iter().map(|job| job.config.clone()).collect::<Vec<_>>();
    let results = run_configured_batch(&configs)?;
    if results.len() != jobs.len() {
        return Err(format!(
            "batch result count mismatch: got {}, expected {}",
            results.len(),
            jobs.len()
        ));
    }

    let output_path = batch_output_path(&spec, &jobs)?;
    write_batch_summary(&output_path, &jobs, &results)?;
    println!("BatchSummary: {}", output_path.display());
    Ok(())
}

fn build_batch_jobs(spec: &BatchConfigFile) -> Result<Vec<BatchJobRuntime>, String> {
    if spec.jobs.is_empty() {
        return Err("batch config must contain at least one job".to_string());
    }

    let mut jobs = Vec::new();
    for (index, job) in spec.jobs.iter().enumerate() {
        let context = format!("jobs[{index}]");
        let quality = job
            .quality_preset
            .as_deref()
            .or(spec.quality_preset.as_deref())
            .unwrap_or("medium-high");

        let mut config = SearchConfig::default();
        apply_quality_preset(&mut config, quality)?;

        if let Some(export) = &spec.export {
            config.export = export.clone();
        }
        if let Some(output_dir) = &spec.output_dir {
            config.output_dir = output_dir.clone();
        }
        if let Some(threads) = spec.threads {
            config.threads = threads;
        }
        apply_json_overrides(&mut config, spec.config.as_ref(), "batch config")?;
        apply_json_overrides(&mut config, spec.overrides.as_ref(), "batch overrides")?;

        if let Some(export) = &job.export {
            config.export = export.clone();
        }
        if let Some(output_dir) = &job.output_dir {
            config.output_dir = output_dir.clone();
        }
        if let Some(threads) = job.threads {
            config.threads = threads;
        }
        apply_json_overrides(&mut config, job.config.as_ref(), &context)?;
        apply_json_overrides(&mut config, job.overrides.as_ref(), &format!("{context}.overrides"))?;

        if let Some(target_dr) = job.target_dr {
            parse_dr(target_dr).map_err(|err| format!("{context}.target_dr: {err}"))?;
            config.target_dr = Some(target_dr);
        }
        config.presets = vec![job.preset.clone()];
        if let Some(selection) = &job.selection {
            set_point_only_selection(&mut config, Some(parse_result_label(selection)?));
        }
        validate_point_only(&config, &context)?;

        jobs.push(BatchJobRuntime {
            id: job.id.clone(),
            preset: job.preset.clone(),
            config,
        });
    }
    Ok(jobs)
}

fn batch_output_path(spec: &BatchConfigFile, jobs: &[BatchJobRuntime]) -> Result<PathBuf, String> {
    if let Some(path) = &spec.batch_output {
        return Ok(path.clone());
    }
    let output_dir = spec
        .output_dir
        .clone()
        .or_else(|| jobs.first().map(|job| job.config.output_dir.clone()))
        .unwrap_or_else(|| SearchConfig::default().output_dir);
    Ok(output_dir.join(format!("adr_batch_{}.json", timestamp())))
}

fn write_batch_summary(
    path: &Path,
    jobs: &[BatchJobRuntime],
    results: &[SearchResult],
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
    }

    let result_values = jobs
        .iter()
        .zip(results.iter())
        .map(|(job, result)| {
            let selection = point_only_label(&job.config).map(|label| label.as_str());
            let selected = selection.and_then(|label| result.selected_by_label.get(label));
            json!({
                "id": job.id.clone(),
                "preset": job.preset.clone(),
                "target_dr": job.config.target_dr,
                "quality_preset": job.config.quality_preset.clone(),
                "selection": selection,
                "selected": selected,
                "selected_by_label": &result.selected_by_label,
                "plot_path": result.plot_path.to_string_lossy().to_string(),
                "summary_path": result.summary_path.to_string_lossy().to_string(),
                "diagnostics": &result.diagnostics,
                "config": json_safe_config(&job.config),
            })
        })
        .collect::<Vec<_>>();

    let value = json!({
        "schema": "linear-adr-optimizer.batch.v1",
        "created_at": Local::now().to_rfc3339(),
        "results": result_values,
    });
    let text = serde_json::to_string_pretty(&value)
        .map_err(|err| format!("failed to serialize batch summary: {err}"))?;
    fs::write(path, text).map_err(|err| format!("failed to write {}: {err}", path.display()))
}

fn apply_json_overrides(
    config: &mut SearchConfig,
    overrides: Option<&Value>,
    context: &str,
) -> Result<(), String> {
    let Some(overrides) = overrides else {
        return Ok(());
    };
    if overrides.is_null() {
        return Ok(());
    }
    let object = overrides
        .as_object()
        .ok_or_else(|| format!("{context} must be a JSON object"))?;

    if let Some(value) = object.get("quality_preset") {
        apply_quality_preset(config, json_string(value, "quality_preset", context)?)?;
    }

    for (key, value) in object {
        match key.as_str() {
            "quality_preset" => {}
            "export" => config.export = json_path(value, key, context)?,
            "preset" => config.presets = vec![json_string(value, key, context)?.to_string()],
            "presets" => config.presets = json_string_list(value, key, context)?,
            "target_dr" => {
                if value.is_null() {
                    config.target_dr = None;
                } else {
                    let target_dr = json_f64(value, key, context)?;
                    parse_dr(target_dr).map_err(|err| format!("{context}.{key}: {err}"))?;
                    config.target_dr = Some(target_dr);
                }
            }
            "days" => config.days = json_i32(value, key, context)?,
            "deck_size" => config.deck_size = json_i32(value, key, context)?,
            "learn_limit" => config.learn_limit = json_i32(value, key, context)?,
            "seed" => config.seed = json_u64(value, key, context)?,
            "threads" => config.threads = json_usize(value, key, context)?,
            "matplotlib" => config.matplotlib = json_bool(value, key, context)?,
            "recommended_only" => config.recommended_only = json_bool(value, key, context)?,
            "aggressive_only" => config.aggressive_only = json_bool(value, key, context)?,
            "calm_only" => config.calm_only = json_bool(value, key, context)?,
            "selection" | "optimizer_strategy" | "point_only" => {
                if value.is_null() {
                    set_point_only_selection(config, None);
                } else {
                    set_point_only_selection(
                        config,
                        Some(parse_result_label(json_string(value, key, context)?)?),
                    );
                }
            }
            "phase1_eval_weight" => config.phase1_eval_weight = json_f64(value, key, context)?,
            "phase2_eval_weight" => config.phase2_eval_weight = json_f64(value, key, context)?,
            "phase3_eval_weight" => config.phase3_eval_weight = json_f64(value, key, context)?,
            "phase4_eval_weight" => config.phase4_eval_weight = json_f64(value, key, context)?,
            "final_eval_weight" => config.final_eval_weight = json_f64(value, key, context)?,
            "dr_prune_weight" => config.dr_prune_weight = json_f64(value, key, context)?,
            "phase1_flat_step" => config.phase1_flat_step = json_f64(value, key, context)?,
            "phase1_flat_half_steps" => config.phase1_flat_half_steps = json_i32(value, key, context)?,
            "phase1_s_step" => config.phase1_s_step = json_f64(value, key, context)?,
            "phase1_s_max" => config.phase1_s_max = json_f64(value, key, context)?,
            "phase1_d_step" => config.phase1_d_step = json_f64(value, key, context)?,
            "phase1_d_min" => config.phase1_d_min = json_f64(value, key, context)?,
            "phase1_expand" => config.phase1_expand = json_bool(value, key, context)?,
            "phase1_expand_rounds" => config.phase1_expand_rounds = json_i32(value, key, context)?,
            "phase1_expand_batch" => config.phase1_expand_batch = json_i32(value, key, context)?,
            "phase1_expand_overflow_factor" => config.phase1_expand_overflow_factor = json_f64(value, key, context)?,
            "phase2_flat_step" => config.phase2_flat_step = json_f64(value, key, context)?,
            "phase2_s_step" => config.phase2_s_step = json_f64(value, key, context)?,
            "phase2_d_step" => config.phase2_d_step = json_f64(value, key, context)?,
            "phase3_flat_step" => config.phase3_flat_step = json_f64(value, key, context)?,
            "phase3_s_step" => config.phase3_s_step = json_f64(value, key, context)?,
            "phase3_d_step" => config.phase3_d_step = json_f64(value, key, context)?,
            "phase4_flat_step" => config.phase4_flat_step = json_f64(value, key, context)?,
            "phase4_s_step" => config.phase4_s_step = json_f64(value, key, context)?,
            "phase4_d_step" => config.phase4_d_step = json_f64(value, key, context)?,
            "phase4_seeds_per_objective" => config.phase4_seeds_per_objective = json_usize(value, key, context)?,
            "phase4_max_steps" => config.phase4_max_steps = json_i32(value, key, context)?,
            "promote_recommended" => config.promote_recommended = json_usize(value, key, context)?,
            "promote_efficiency_potential" => config.promote_efficiency_potential = json_usize(value, key, context)?,
            "promote_memory_potential" => config.promote_memory_potential = json_usize(value, key, context)?,
            "promote_pareto_extra" => config.promote_pareto_extra = json_usize(value, key, context)?,
            "bridge_midpoint_limit" => config.bridge_midpoint_limit = json_usize(value, key, context)?,
            "experimental_bridge_midpoint_neighborhoods" => {
                config.experimental_bridge_midpoint_neighborhoods = json_bool(value, key, context)?
            }
            "final_candidate_limit" => config.final_candidate_limit = json_usize(value, key, context)?,
            "max_spread_final_candidates" => config.max_spread_final_candidates = json_usize(value, key, context)?,
            "final_shortlist_recommended" => config.final_shortlist_recommended = json_usize(value, key, context)?,
            "final_shortlist_efficiency" => config.final_shortlist_efficiency = json_usize(value, key, context)?,
            "final_shortlist_memory" => config.final_shortlist_memory = json_usize(value, key, context)?,
            "final_shortlist_frontier" => config.final_shortlist_frontier = json_usize(value, key, context)?,
            "scout_potential_band_pct" => config.scout_potential_band_pct = json_f64(value, key, context)?,
            "final_potential_band_pct" => config.final_potential_band_pct = json_f64(value, key, context)?,
            "aggressive_calm_regret_pct" => config.aggressive_calm_regret_pct = json_f64(value, key, context)?,
            "safety_s_max" => config.safety_s_max = json_f64(value, key, context)?,
            "safety_checks" => config.safety_checks = json_i32(value, key, context)?,
            "ignore_safety" => config.ignore_safety = json_bool(value, key, context)?,
            "legacy_unsafe_plot_display" => config.legacy_unsafe_plot_display = json_bool(value, key, context)?,
            "include_original" => config.include_original = json_bool(value, key, context)?,
            "original" => config.original = json_candidate(value, key, context)?,
            "inspect_point" => config.inspect_point = json_candidate_list(value, key, context)?,
            "fixed_dr_start_pct" => config.fixed_dr_start_pct = json_f64(value, key, context)?,
            "fixed_dr_end_pct" => config.fixed_dr_end_pct = json_f64(value, key, context)?,
            "fixed_curve_coarse_weight" => config.fixed_curve_coarse_weight = json_f64(value, key, context)?,
            "fixed_curve_refine_weight" => config.fixed_curve_refine_weight = json_f64(value, key, context)?,
            "fixed_curve_coarse_step_pct" => config.fixed_curve_coarse_step_pct = json_f64(value, key, context)?,
            "fixed_curve_refine_step_pct" => config.fixed_curve_refine_step_pct = json_f64(value, key, context)?,
            "fixed_curve_initial_radius_pct" => config.fixed_curve_initial_radius_pct = json_f64(value, key, context)?,
            "fixed_curve_adapt_margin_pct" => config.fixed_curve_adapt_margin_pct = json_f64(value, key, context)?,
            "fixed_curve_adapt_top_per_bucket" => {
                config.fixed_curve_adapt_top_per_bucket = json_usize(value, key, context)?
            }
            "fixed_curve_adapt_max_points" => {
                config.fixed_curve_adapt_max_points = json_usize(value, key, context)?
            }
            "fixed_dr_label_step_pct" => config.fixed_dr_label_step_pct = json_f64(value, key, context)?,
            "output_dir" => config.output_dir = json_path(value, key, context)?,
            _ => return Err(format!("{context} contains unknown config key {key:?}")),
        }
    }
    validate_point_only(config, context)
}

fn json_f64(value: &Value, key: &str, context: &str) -> Result<f64, String> {
    value
        .as_f64()
        .or_else(|| value.as_str().and_then(|text| text.parse::<f64>().ok()))
        .ok_or_else(|| format!("{context}.{key} expected a number"))
}

fn json_i32(value: &Value, key: &str, context: &str) -> Result<i32, String> {
    value
        .as_i64()
        .or_else(|| value.as_str().and_then(|text| text.parse::<i64>().ok()))
        .and_then(|value| i32::try_from(value).ok())
        .ok_or_else(|| format!("{context}.{key} expected a 32-bit integer"))
}

fn json_u64(value: &Value, key: &str, context: &str) -> Result<u64, String> {
    value
        .as_u64()
        .or_else(|| value.as_str().and_then(|text| text.parse::<u64>().ok()))
        .ok_or_else(|| format!("{context}.{key} expected a non-negative integer"))
}

fn json_usize(value: &Value, key: &str, context: &str) -> Result<usize, String> {
    let raw = json_u64(value, key, context)?;
    usize::try_from(raw).map_err(|_| format!("{context}.{key} is too large"))
}

fn json_bool(value: &Value, key: &str, context: &str) -> Result<bool, String> {
    if let Some(value) = value.as_bool() {
        return Ok(value);
    }
    match value.as_str().map(str::to_ascii_lowercase).as_deref() {
        Some("true") | Some("1") | Some("yes") => Ok(true),
        Some("false") | Some("0") | Some("no") => Ok(false),
        _ => Err(format!("{context}.{key} expected a boolean")),
    }
}

fn json_string<'a>(value: &'a Value, key: &str, context: &str) -> Result<&'a str, String> {
    value
        .as_str()
        .ok_or_else(|| format!("{context}.{key} expected a string"))
}

fn json_path(value: &Value, key: &str, context: &str) -> Result<PathBuf, String> {
    Ok(PathBuf::from(json_string(value, key, context)?))
}

fn json_string_list(value: &Value, key: &str, context: &str) -> Result<Vec<String>, String> {
    if let Some(text) = value.as_str() {
        return Ok(text
            .split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(str::to_string)
            .collect());
    }
    let array = value
        .as_array()
        .ok_or_else(|| format!("{context}.{key} expected a string or string array"))?;
    array
        .iter()
        .map(|value| json_string(value, key, context).map(str::to_string))
        .collect()
}

fn json_candidate(value: &Value, key: &str, context: &str) -> Result<Candidate, String> {
    if let Some(text) = value.as_str() {
        let parts = text
            .split(|ch: char| ch == ',' || ch.is_whitespace())
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(|part| {
                part.parse::<f64>()
                    .map_err(|_| format!("{context}.{key} expected candidate numbers"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if parts.len() != 3 {
            return Err(format!("{context}.{key} expected exactly three candidate numbers"));
        }
        return Ok(Candidate::new(parts[0], parts[1], parts[2]).snap());
    }

    let array = value
        .as_array()
        .ok_or_else(|| format!("{context}.{key} expected a candidate array"))?;
    if array.len() != 3 {
        return Err(format!("{context}.{key} expected exactly three candidate numbers"));
    }
    Ok(Candidate::new(
        json_f64(&array[0], key, context)?,
        json_f64(&array[1], key, context)?,
        json_f64(&array[2], key, context)?,
    )
    .snap())
}

fn json_candidate_list(value: &Value, key: &str, context: &str) -> Result<Vec<Candidate>, String> {
    if value.is_null() {
        return Ok(Vec::new());
    }
    if value.as_str().is_some() {
        return Ok(vec![json_candidate(value, key, context)?]);
    }
    let array = value
        .as_array()
        .ok_or_else(|| format!("{context}.{key} expected a candidate array list"))?;
    if array.is_empty() {
        return Ok(Vec::new());
    }
    if array.len() == 3 && !array[0].is_array() {
        return Ok(vec![json_candidate(value, key, context)?]);
    }
    array
        .iter()
        .map(|value| json_candidate(value, key, context))
        .collect()
}

fn parse_result_label(value: &str) -> Result<ResultLabel, String> {
    match value.trim().to_ascii_lowercase().replace('_', "-").as_str() {
        "recommended" => Ok(ResultLabel::Recommended),
        "aggressive" => Ok(ResultLabel::Aggressive),
        "calm" => Ok(ResultLabel::Calm),
        _ => Err(format!(
            "unknown selection {value:?}; expected recommended, aggressive, or calm"
        )),
    }
}

fn set_point_only_selection(config: &mut SearchConfig, label: Option<ResultLabel>) {
    config.recommended_only = false;
    config.aggressive_only = false;
    config.calm_only = false;
    match label {
        Some(ResultLabel::Recommended) => config.recommended_only = true,
        Some(ResultLabel::Aggressive) => config.aggressive_only = true,
        Some(ResultLabel::Calm) => config.calm_only = true,
        None => {}
    }
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

fn validate_point_only(config: &SearchConfig, context: &str) -> Result<(), String> {
    let count = [config.recommended_only, config.aggressive_only, config.calm_only]
        .into_iter()
        .filter(|enabled| *enabled)
        .count();
    if count > 1 {
        Err(format!(
            "{context} has mutually exclusive point-only selections enabled"
        ))
    } else {
        Ok(())
    }
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
           adr-optimizer --batch-config <JSON>\n\n\
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
         Batch config accepts multiple jobs with per-preset target DR, quality preset,\n\
         point-only selection, and config overrides.\n\n\
         Advanced phase/search flags are preserved from the Python CLI.",
        env!("CARGO_PKG_VERSION"),
        preset_names().join("|")
    )
}
