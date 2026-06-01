# Linear ADR Optimizer

Rust-first rewrite of the linear ADR optimizer.

The design is:

- Python is only a compatibility launcher and Anki add-on bridge.
- Rust owns CLI parsing, export loading, simulation, search phases, ranking,
  safety, final label selection, progress messages, JSON/HTML/TXT/PNG output,
  and multi-preset batch execution.
- Current CLI entry points are preserved:
  - `python adr_pareto_search.py ...`
  - `python -m adr_pareto ...`

The Python launcher looks for a Rust executable named `adr-optimizer` or
`adr-optimizer.exe` in:

1. `helper/`
2. `helper/<platform-artifact-name>/`
3. `rust/target/release/`
4. `rust/target/debug/`

You can override discovery with `ADR_OPTIMIZER_BINARY`.

## Layout

```text
adr_pareto/              Minimal Python bridge.
exports/                 Anki JSONL exports.
outputs/                 Generated reports and point-only TXT files.
helper/                  Bundled release binary for Anki/CLI use.
rust/                    Rust optimizer crate.
rust/assets/web/         Plotly HTML assets copied beside generated reports.
```

## Build

```powershell
cd C:\Users\admin\Documents\Codex\linear-adr-optimizer\rust
cargo build --release --bin adr-optimizer
Copy-Item .\target\release\adr-optimizer.exe ..\helper\adr-optimizer.exe -Force
```

## Run

```powershell
cd C:\Users\admin\Documents\Codex\linear-adr-optimizer
python -m adr_pareto --export ".\exports\adr-input-example.jsonl" --preset "Yuusei" --target-dr 85
```

Multiple presets can be optimized in one process:

```powershell
python -m adr_pareto --export ".\exports" --preset "Preset A,Preset B" --target-dr 85
```

Point-only compatibility flags are preserved:

```powershell
python -m adr_pareto --preset "Yuusei" --target-dr 85 --recommended-only
python -m adr_pareto --preset "Yuusei" --target-dr 85 --aggressive-only
python -m adr_pareto --preset "Yuusei" --target-dr 85 --calm-only
```

The terminal progress format is still human-readable, including lines such as:

```text
[fixed curve] coarse=37@10000 refined=11@80000 envelope=30 elapsed=0.5s
[phase 1.0] new=1199 screened_unsafe=1419 pool=1199 promote=148 boundary=d_high,flat_high,flat_low,s_high elapsed=1.3s
```
