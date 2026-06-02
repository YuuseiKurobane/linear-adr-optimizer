Original concept by 1DWalker: [srs-simulator on the `fsrs-sa` branch](https://github.com/1DWalker/srs-simulator/tree/fsrs-sa).

Anki add-on: [YuuseiKurobane/linear-adr-ankiaddon](https://github.com/YuuseiKurobane/linear-adr-ankiaddon).

Legacy Python version: [YuuseiKurobane/linear-adr-optimizer-python](https://github.com/YuuseiKurobane/linear-adr-optimizer-python).

# Linear ADR Optimizer

This repository is the Rust-first rewrite of the linear ADR optimizer. It is a standalone optimizer for linear Adaptive Desired Retention parameters for FSRS-style scheduling. It searches candidate ADR formulas, compares them against fixed desired-retention baselines, and writes HTML, JSON, PNG, TXT, and batch-summary artifacts that can be used from the command line or by the Anki add-on.

The design is:

- Python is only a compatibility launcher and Anki add-on bridge.
- Rust owns CLI parsing, export loading, simulation, search phases, ranking, safety, final label selection, progress messages, JSON/HTML/TXT/PNG output, and multi-preset batch execution.
- Current CLI entry points are preserved:
  - `python adr_pareto_search.py ...`
  - `python -m adr_pareto ...`
- The native Rust executable is `adr-optimizer`.

## Repository Layout

```text
adr_pareto/              Minimal Python bridge.
adr_pareto_search.py     Compatibility launcher.
exports/                 Anki JSONL exports named adr-input-*.jsonl.
outputs/                 Generated reports and point-only TXT files.
helper/                  Bundled release binary for Anki/CLI use.
rust/                    Rust optimizer crate.
rust/assets/web/         Source Plotly HTML assets embedded into the helper and written beside generated reports.
```

## What It Optimizes

FSRS schedules reviews from a card memory state, primarily stability and difficulty. A fixed desired retention, or fixed DR, uses one desired retention value for every card. Linear ADR instead computes a different desired retention from the card state:

```text
desired_retention = sigmoid(flat + s_multi * ln(stability) + d_multi * difficulty)
```

The Rust implementation clamps the logit input inside `sigmoid` for numerical stability and clamps the final desired retention to `0.0..=0.995`. A candidate is the parameter triple `(flat, s_multi, d_multi)`. The search normally enforces the useful quadrant where `s_multi >= 0` and `d_multi <= 0`, meaning higher-stability cards can receive higher desired retention and harder cards can receive lower desired retention.

A fixed-DR baseline is represented as the same formula with `s_multi = 0` and `d_multi = 0`, where `flat = logit(fixed_dr)`.

## What The Simulator Does

The simulator evaluates one ADR candidate by running an FSRS v6 scheduling model over a synthetic deck derived from an Anki export row.

Inputs from the export row:

- `fsrs6_weights`: 21 FSRS v6 weights used by the scheduler.
- `button_usage.first_rating_prob`: probability distribution for the first Again/Hard/Good/Easy rating.
- `button_usage.review_rating_prob`: conditional probability distribution for successful review ratings Hard/Good/Easy.
- `button_usage.learn_costs`: time cost for first ratings.
- `button_usage.review_costs`: time cost for review ratings.
- `deck_preset.name` and `decks[].name`: names used by `--preset`.
- `desired_retention`: fallback target DR when `--target-dr` is omitted.

For each candidate:

1. Build an `FSRSv6` predictor from the exported FSRS weights.
2. Build a behavior model from exported rating probabilities and review/learning costs. The review rating distribution is `Again = 1 - retrievability`; successful reviews are split across Hard/Good/Easy according to `review_rating_prob`.
3. Simulate first-review states for Again, Hard, Good, and Easy according to `first_rating_prob`.
4. Simulate review histories until `--days`. The scheduler asks the ADR formula for desired retention at each state, converts that DR to an FSRS interval, rounds the interval, computes retrievability at the next review, samples or branches the next rating, applies the FSRS transition, and repeats.
5. Model deck growth with `--deck-size` and `--learn-limit`. `deck_size / learn_limit` gives the number of learn days needed to introduce the deck. The simulator integrates review histories across the horizon with a taper near the end so cards introduced late contribute proportionally less remaining time.
6. Accumulate total memorized volume by integrating the FSRS forgetting curve over time, not just by checking retention at the end.
7. Accumulate total review/learning cost from the exported button costs.
8. Report:
   - `total_average_memorized`: integrated expected remembered-card volume over the simulation.
   - `total_cost`: total weighted study cost.
   - `memorized_fraction`: `total_average_memorized / (eval_weight * days)`.
   - `memorized_cards`: `memorized_fraction * deck_size`.
   - `memorized_per_minute`: `60 * total_average_memorized / total_cost`.
   - `total_iters`: number of simulated review iterations.

`phase*_eval_weight`, `final_eval_weight`, and fixed-curve weights are simulator evaluation weights. They control verification effort/noise and runtime. They are not the simulated deck size; the deck size is controlled by `--deck-size`.

The simulator is hybrid deterministic/probabilistic. It branches through high-weight rating paths, then switches low-weight paths to seeded Monte Carlo sampling with pruning. The base seed comes from `--seed`; each phase offsets it so runs are reproducible with the same inputs and configuration.

## Safety Checks

Safety checks explore likely future FSRS states without running the full cost simulation. For each candidate, safety verifies sampled states with stability below `--safety-s-max` and checks:

- Interval order does not flip: Again should not schedule after Hard, Hard should not schedule after Good, and Good should not schedule after Easy.
- Hard should not shorten the interval below the previous interval.

By default, unsafe candidates are filtered from final selection. Phase 1 also pre-screens unsafe candidates before full simulation. `--ignore-safety` skips safety checks and filtering. `--legacy-unsafe-plot-display` keeps safety on but uses the older behavior where Phase 1 simulates and plots unsafe points instead of pre-screening them.

## Fixed-DR Comparison

The optimizer builds a fixed-DR curve from fixed desired-retention baselines. It evaluates coarse fixed-DR points across `--fixed-dr-start-pct` to `--fixed-dr-end-pct`, refines points near the target DR, and adaptively adds refined fixed-DR points near strong ADR candidates.

The fixed curve is used for:

- The target baseline point at `--target-dr`.
- The Pareto envelope of fixed-DR memory versus efficiency.
- Equivalent-DR labels such as `eff=84.20%` and `mem=88.10%`.
- The spread metric: memory-equivalent DR minus efficiency-equivalent DR.

This lets the search describe an ADR point as, for example, "fixed-DR-like efficiency near 84% but memory near 88%" instead of only reporting raw memorized cards and speed.

## Search Flow

1. Load the selected export row and target desired retention.
2. Build the fixed-DR curve where `s_multi=0` and `d_multi=0`.
3. Run Phase 1: a coarse grid centered at `logit(target_dr)`, with positive `s_multi` and negative `d_multi`. If promising points sit on the boundary, the grid can expand outward.
4. Promote promising points for recommended-like, efficiency, memory, and Pareto-frontier behavior.
5. Run Phase 2 and Phase 3 hypercube refinements around promoted points.
6. Optionally bridge midpoint gaps between promoted points, and optionally generate bridge-neighborhood candidates.
7. Run Phase 4 micro-hillclimbs around objective-specific seeds.
8. Build a final shortlist from recommended, efficiency, memory, frontier, max-spread, original, and inspect candidates.
9. Evaluate the final shortlist at higher weight and attach final DR distribution summaries.
10. Choose labels and write outputs.

Final labels:

- `Recommended`: a safe point strictly northeast of the target fixed-DR point, meaning both more memorized cards and higher memorized-per-minute, ranked by fixed-curve spread and surplus.
- `Aggressive`: among near-recommended northeast points, the largest final `p90 - p10` DR spread.
- `Calm`: among near-recommended northeast points, the smallest final `p90 - p10` DR spread.
- `Efficiency Potential`: highest memorized-per-minute among points with memorized cards near the target fixed-DR memory band.
- `Memory Potential`: highest memorized-card count among points with memorized-per-minute near the target fixed-DR efficiency band.
- `Max Spread`: largest fixed-curve spread among safe final candidates.
- `Original`: optional verification point from `--include-original`.
- `Inspect N`: optional verification points from `--inspect-point`.

## Build

Build the Rust optimizer:

```powershell
cd C:\Users\admin\Documents\Codex\linear-adr-optimizer\rust
cargo build --release --bin adr-optimizer
```

For the Python bridge or add-on bundle, copy the executable into `helper/`:

```powershell
Copy-Item .\target\release\adr-optimizer.exe ..\helper\adr-optimizer.exe -Force
```

The Python launcher looks for `adr-optimizer` or `adr-optimizer.exe` in:

1. `helper/`
2. `helper/<platform-artifact-name>/`
3. `rust/target/release/`
4. `rust/target/debug/`

You can override discovery with `ADR_OPTIMIZER_BINARY`.

## Run

From the repository root:

```powershell
cd C:\Users\admin\Documents\Codex\linear-adr-optimizer
python -m adr_pareto --export ".\exports\adr-input-example.jsonl" --preset "Yuusei" --target-dr 85
```

The wrapper script is equivalent:

```powershell
python adr_pareto_search.py --export ".\exports\adr-input-example.jsonl" --preset "Yuusei" --target-dr 85
```

You can also call the Rust binary directly:

```powershell
.\helper\adr-optimizer.exe --export ".\exports\adr-input-example.jsonl" --preset "Yuusei" --target-dr 85
```

If `--export` points to a directory, the optimizer picks the newest `adr-input-*.jsonl` in that directory. If `--export` is omitted, it uses `exports/`.

If `--target-dr` is omitted, the selected export row must contain `desired_retention`.

## Multi-Preset Runs

The Rust rewrite can optimize multiple presets in one process by passing a comma-separated list or by repeating `--preset`/`--presets`:

```powershell
python -m adr_pareto --export ".\exports" --preset "Preset A,Preset B" --target-dr 85
```

Each preset is selected from the same export input. Exact deck preset names are preferred before exact deck names, then partial preset names, then partial deck names. Ambiguous partial matches fail with the matching preset names.

## Batch Config

For add-on integrations that need different target DR, quality preset, output path, threads, or point-only selection per preset, use a batch config:

```powershell
adr-optimizer --batch-config ".\exports\adr-batch-example.json"
```

`--batch-config` is mutually exclusive with ordinary CLI options.

Example batch config:

```json
{
  "export": ".\\exports\\adr-input-example.jsonl",
  "output_dir": ".\\outputs",
  "batch_output": ".\\outputs\\adr_batch_latest.json",
  "quality_preset": "medium-high",
  "threads": 0,
  "jobs": [
    {
      "id": "kanji-recommended",
      "preset": "Kanji Kentei",
      "target_dr": 85,
      "quality_preset": "lite",
      "selection": "recommended"
    },
    {
      "id": "reading-calm-custom",
      "preset": "Reading",
      "target_dr": 90,
      "quality_preset": "medium-high",
      "selection": "calm",
      "config": {
        "final_eval_weight": 60000,
        "phase4_max_steps": 2
      }
    }
  ]
}
```

Top-level batch fields:

- `export`: default export path for jobs.
- `output_dir`: default output directory for jobs and generated batch summary.
- `batch_output`: explicit path for the machine-readable batch summary JSON. Aliases: `output_path`, `result_path`.
- `quality_preset`: default quality preset for jobs.
- `threads`: default thread count for jobs.
- `config`: global config overrides. Alias: `settings`.
- `overrides`: global config overrides applied after `config`.
- `jobs`: non-empty list of job objects.

Per-job fields:

- `id`: optional identifier copied into batch summary output.
- `preset`: required deck preset/deck selector for this job.
- `export`: job-specific export path.
- `output_dir`: job-specific output directory.
- `quality_preset`: job-specific quality preset.
- `target_dr`: job-specific target desired retention as fraction or percent.
- `selection`: optional point-only selection: `recommended`, `aggressive`, or `calm`. Aliases: `optimizer_strategy`, `point_only`.
- `threads`: job-specific thread count.
- `config`: job-specific config overrides. Alias: `settings`.
- `overrides`: job-specific overrides applied after `config`.

Batch config override keys use snake_case versions of the CLI flags, for example `final_eval_weight`, `phase4_max_steps`, `ignore_safety`, or `fixed_curve_refine_step_pct`. JSON candidate values such as `original` and `inspect_point` can be arrays like `[1.57, 0.135, -0.085]`; `inspect_point` can also be a list of such arrays.

The helper writes one machine-readable batch summary JSON with schema `linear-adr-optimizer.batch.v1`. Each result includes the job id, preset, target DR, quality preset, selected point, all selected labels, plot path, summary path, diagnostics, and resolved config.

## CLI Flags

All ordinary flags support both `--flag value` and `--flag=value` forms unless the flag is boolean or consumes multiple positional values.

### Help And Mode Flags

- `--help`, `-h`, `help`: print help and exit.
- `--version`, `version`: print the Rust package version and exit.
- `--batch-config <JSON>`: run a JSON batch config. This cannot be combined with ordinary CLI flags.

### Input, Selection, And Output

- `--export <PATH>`: JSONL file or directory containing `adr-input-*.jsonl`. Default: `exports/`.
- `--preset <NAME[,NAME...]>`: deck preset or deck name selector. Can be comma-separated and can be repeated. Default: `Yuusei`.
- `--presets <NAME[,NAME...]>`: alias of `--preset`.
- `--target-dr <DR>`: target desired retention as a fraction or percent. `0.85` and `85` both mean 85%. If omitted, uses the export row's `desired_retention`.
- `--quality-preset <potato|lite|medium|medium-high|high>`: speed/accuracy preset. Default: `medium-high`.
- `--output-dir <PATH>`: directory for generated outputs. Default: `outputs/`.

### Simulation And Execution

- `--days <N>`: simulation horizon in days. Default: `1825`.
- `--deck-size <N>`: simulated deck size used to convert memorized fraction into memorized-card count. Default: `10000`.
- `--learn-limit <N>`: new cards per day used to model deck growth. Default: `10`.
- `--seed <N>`: base seed for reproducible simulation/search phases. Default: `1234`.
- `--threads <N>`: Rayon worker thread count. `0` uses available CPU parallelism. Default: `0`.
- `--matplotlib`: write a PNG plot instead of the default standalone HTML plot. The flag name is kept for Python compatibility even though Rust writes the PNG.

### Point-Only Output

These flags still run the search but write and print only one selected point as TXT. They are mutually exclusive.

- `--recommended-only`: write only the `Recommended` point.
- `--aggressive-only`: write only the `Aggressive` point.
- `--calm-only`: write only the `Calm` point.

### Evaluation Weights

These control simulator verification effort and runtime. Higher values generally reduce noise and cost more CPU.

- `--phase1-eval-weight <N>`: evaluation weight for Phase 1 coarse candidates.
- `--phase2-eval-weight <N>`: evaluation weight for Phase 2 refinement.
- `--phase3-eval-weight <N>`: evaluation weight for Phase 3 refinement.
- `--phase4-eval-weight <N>`: evaluation weight for Phase 4 micro-hillclimb candidates.
- `--final-eval-weight <N>`: evaluation weight for final shortlisted candidates.
- `--dr-prune-weight <N>`: minimum branch weight kept while computing final DR distribution summaries. Default: `1.0`.

### Phase 1 Coarse Grid

- `--phase1-flat-step <N>`: step size for the `flat` axis.
- `--phase1-flat-half-steps <N>`: number of `flat` steps on each side of `logit(target_dr)`.
- `--phase1-s-step <N>`: step size for `s_multi`.
- `--phase1-s-max <N>`: initial maximum `s_multi`.
- `--phase1-d-step <N>`: step size for `d_multi`.
- `--phase1-d-min <N>`: initial minimum `d_multi`; normally negative.
- `--phase1-expand`: enable boundary-driven Phase 1 grid expansion.
- `--no-phase1-expand`: disable boundary-driven Phase 1 grid expansion.
- `--phase1-expand-rounds <N>`: maximum number of extra Phase 1 expansion rounds.
- `--phase1-expand-batch <N>`: number of grid steps added in each boundary direction per expansion.
- `--phase1-expand-overflow-factor <N>`: cap for expansion beyond the initial grid, as a multiple of initial grid size.

### Refinement And Hillclimb

- `--phase2-flat-step <N>`: Phase 2 local step for `flat`.
- `--phase2-s-step <N>`: Phase 2 local step for `s_multi`.
- `--phase2-d-step <N>`: Phase 2 local step for `d_multi`.
- `--phase3-flat-step <N>`: Phase 3 local step for `flat`.
- `--phase3-s-step <N>`: Phase 3 local step for `s_multi`.
- `--phase3-d-step <N>`: Phase 3 local step for `d_multi`.
- `--phase4-flat-step <N>`: Phase 4 hillclimb step for `flat`.
- `--phase4-s-step <N>`: Phase 4 hillclimb step for `s_multi`.
- `--phase4-d-step <N>`: Phase 4 hillclimb step for `d_multi`.
- `--phase4-seeds-per-objective <N>`: number of seeds taken for each Phase 4 objective: recommended, efficiency, memory, and frontier.
- `--phase4-max-steps <N>`: maximum hillclimb moves from each Phase 4 seed.

### Promotion And Final Shortlist

- `--promote-recommended <N>`: number of recommended-like candidates promoted between search phases.
- `--promote-efficiency-potential <N>`: number of efficiency-potential candidates promoted between phases.
- `--promote-memory-potential <N>`: number of memory-potential candidates promoted between phases.
- `--promote-pareto-extra <N>`: number of extra Pareto-frontier candidates promoted or rendered.
- `--bridge-midpoint-limit <N>`: limit for adding already-evaluated midpoint bridge points between promoted candidates.
- `--experimental-bridge-midpoint-neighborhoods`: additionally generate and evaluate neighborhoods around qualifying bridge midpoints.
- `--final-candidate-limit <N>`: maximum ranked candidates carried into final high-weight evaluation before max-spread/reference additions.
- `--max-spread-final-candidates <N>`: number of high-spread candidates forced into final evaluation.
- `--final-shortlist-recommended <N>`: recommended-like shortlist size considered for final candidates.
- `--final-shortlist-efficiency <N>`: efficiency shortlist size considered for final candidates.
- `--final-shortlist-memory <N>`: memory shortlist size considered for final candidates.
- `--final-shortlist-frontier <N>`: frontier shortlist size considered for final candidates.

### Selection Bands And Label Behavior

- `--scout-potential-band-pct <N>`: DR percentage-point band around the target used while scouting/promoting efficiency and memory potential. Default: `0.3`.
- `--final-potential-band-pct <N>`: DR percentage-point band around the target used for final efficiency/memory labels. Default: `0.1`.
- `--aggressive-calm-regret-pct <N>`: allowed spread-regret window, in percentage points, for Aggressive/Calm relative to Recommended. Default: `0.50`.

### Safety And Debug Labels

- `--safety-s-max <N>`: only safety-check states with stability below this value. Default: `1000.0`.
- `--safety-checks <N>`: maximum number of safety states checked per candidate. Default: `3000`.
- `--ignore-safety`: skip safety checks and safety filtering entirely.
- `--legacy-unsafe-plot-display`: keep safety checks but use legacy Phase 1 behavior where unsafe points are simulated/plotted instead of pre-screened.
- `--include-original`: include the configured original reference point in final verification and output labels. Default original: `flat=1.57, s=0.135, d=-0.085`.
- `--original <flat> <s_multi> <d_multi>`: set the original reference point.
- `--inspect-point <flat> <s_multi> <d_multi>`: add a custom point to final verification and output labels. Can be repeated.

### Fixed-DR Curve

- `--fixed-dr-start-pct <N>`: lowest fixed DR percentage evaluated for the baseline curve. Default: `60.0`.
- `--fixed-dr-end-pct <N>`: highest fixed DR percentage evaluated for the baseline curve. Default: `96.0`.
- `--fixed-curve-coarse-weight <N>`: evaluation weight for coarse fixed-DR curve points.
- `--fixed-curve-refine-weight <N>`: evaluation weight for refined fixed-DR curve points.
- `--fixed-curve-coarse-step-pct <N>`: coarse fixed-DR grid step in percentage points.
- `--fixed-curve-refine-step-pct <N>`: refined fixed-DR grid step in percentage points.
- `--fixed-curve-initial-radius-pct <N>`: refined radius around target DR at startup, in percentage points.
- `--fixed-curve-adapt-margin-pct <N>`: refinement margin around equivalent-DR locations discovered during search.
- `--fixed-curve-adapt-top-per-bucket <N>`: number of strong points per ranking bucket used to request adaptive fixed-curve refinement.
- `--fixed-curve-adapt-max-points <N>`: maximum adaptive fixed-curve points added in one adaptation pass.
- `--fixed-dr-label-step-pct <N>`: label interval for fixed-DR points on generated plots. Default: `10.0`.

## Quality Presets

Quality presets always keep the same default simulation horizon (`days=1825`, `deck_size=10000`, `learn_limit=10`) unless you override those flags. They mainly change evaluation weights, fixed-curve density, Phase 1 grid density/expansion, promotion counts, Phase 4 effort, final shortlist sizes, and safety check count.

- `potato`: fastest smoke-test preset with very coarse search.
- `lite`: faster interactive preset.
- `medium`: balanced preset.
- `medium-high`: default higher-confidence preset.
- `high`: slowest built-in preset with the largest search and final verification effort.

Any explicit CLI flag overrides the value selected by `--quality-preset`.

## Outputs

Normal runs write to `outputs/` unless `--output-dir` is set:

- `adr_pareto_<preset>_<timestamp>.html`
- `adr_pareto_<preset>_<timestamp>.json`

With `--matplotlib`, the plot is written as:

- `adr_pareto_<preset>_<timestamp>.png`

Point-only modes write TXT files named like:

- `adr_<preset>_<recommended|aggressive|calm>_<timestamp>.txt`

Batch config writes a batch summary path from `batch_output` or:

- `adr_batch_<timestamp>.json`

The per-run JSON summary includes:

- export path, selected preset metadata, target DR, and resolved args.
- selected points by label.
- labels grouped by point.
- selected fixed-curve metrics.
- fixed curve points, refined points, and fixed-curve envelope.
- final frontier points.
- phase render layers.
- max-spread prefinal points.
- diagnostics for fixed-curve and search phases.

The terminal progress format is human-readable and includes lines such as:

```text
[fixed curve] coarse=37@10000 refined=11@80000 envelope=30 elapsed=0.5s
[phase 1.0] new=1199 screened_unsafe=1419 pool=1199 promote=148 boundary=d_high,flat_high,flat_low,s_high elapsed=1.3s
```
