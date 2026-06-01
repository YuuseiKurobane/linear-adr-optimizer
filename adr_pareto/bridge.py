from __future__ import annotations

import os
import platform
import subprocess
import sys
from pathlib import Path
from typing import Iterable


ROOT = Path(__file__).resolve().parents[1]
ENV_BINARY = "ADR_OPTIMIZER_BINARY"


class OptimizerBinaryNotFound(RuntimeError):
    pass


def executable_name() -> str:
    return "adr-optimizer.exe" if os.name == "nt" else "adr-optimizer"


def platform_artifact_name() -> str | None:
    system = platform.system().lower()
    machine = platform.machine().lower()
    if system == "windows" and machine in {"amd64", "x86_64"}:
        return "adr-optimizer-windows-x86_64"
    if system == "darwin" and machine in {"arm64", "aarch64"}:
        return "adr-optimizer-macos-aarch64"
    if system == "darwin" and machine in {"amd64", "x86_64"}:
        return "adr-optimizer-macos-x86_64"
    if system == "linux" and machine in {"amd64", "x86_64"}:
        return "adr-optimizer-linux-x86_64"
    return None


def candidate_binaries() -> list[Path]:
    exe = executable_name()
    candidates: list[Path] = []

    env_path = os.environ.get(ENV_BINARY)
    if env_path:
        candidates.append(Path(env_path))

    candidates.append(ROOT / "helper" / exe)
    artifact = platform_artifact_name()
    if artifact is not None:
        candidates.append(ROOT / "helper" / artifact / exe)

    candidates.extend(
        [
            ROOT / "rust" / "target" / "release" / exe,
            ROOT / "rust" / "target" / "debug" / exe,
        ]
    )
    return candidates


def resolve_binary() -> Path:
    for path in candidate_binaries():
        if path.exists():
            if os.name != "nt":
                path.chmod(path.stat().st_mode | 0o755)
            return path

    searched = "\n".join(f"  {path}" for path in candidate_binaries())
    raise OptimizerBinaryNotFound(
        "No adr-optimizer binary found. Build Rust with:\n"
        "  cd rust\n"
        "  cargo build --release --bin adr-optimizer\n"
        f"Or set {ENV_BINARY}. Searched:\n{searched}"
    )


def run_optimizer(
    args: Iterable[str] | None = None,
    *,
    stdin: bytes | None = None,
    stream_output: bool = True,
) -> int:
    binary = resolve_binary()
    cmd = [str(binary), *(list(args) if args is not None else sys.argv[1:])]

    if not stream_output:
        completed = subprocess.run(cmd, input=stdin, cwd=ROOT)
        return int(completed.returncode)

    proc = subprocess.Popen(
        cmd,
        stdin=subprocess.PIPE if stdin is not None else None,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        cwd=ROOT,
        text=False,
    )
    if stdin is not None:
        assert proc.stdin is not None
        proc.stdin.write(stdin)
        proc.stdin.close()

    assert proc.stdout is not None
    for raw in iter(proc.stdout.readline, b""):
        sys.stdout.write(raw.decode("utf-8", errors="replace"))
        sys.stdout.flush()
    return int(proc.wait())


def main(argv: list[str] | None = None) -> int:
    return run_optimizer(sys.argv[1:] if argv is None else argv)
