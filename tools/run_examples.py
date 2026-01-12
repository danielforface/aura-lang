from __future__ import annotations

import argparse
import os
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable, Optional


REPO_ROOT = Path(__file__).resolve().parents[1]
EXAMPLES_DIR = REPO_ROOT / "examples"
DIST_AURA_EXE = REPO_ROOT / "dist-release" / "bin" / "aura.exe"


@dataclass(frozen=True)
class Example:
    id: str
    title: str
    workdir: Path
    kind: str  # "run" | "verify" | "custom"
    command_ps_lines: list[str]

    @property
    def preview(self) -> str:
        if not self.command_ps_lines:
            return "(no command)"
        if len(self.command_ps_lines) == 1:
            return self.command_ps_lines[0]
        return " ; ".join(self.command_ps_lines)


def _is_windows() -> bool:
    return os.name == "nt"


def _read_text(path: Path) -> str:
    return path.read_text(encoding="utf-8", errors="replace")


def _find_powershell_code_blocks(md: str) -> list[list[str]]:
    blocks: list[list[str]] = []
    lines = md.splitlines()
    i = 0
    while i < len(lines):
        line = lines[i].rstrip("\n")
        m = re.match(r"^```\s*(powershell|pwsh)\s*$", line.strip(), flags=re.IGNORECASE)
        if not m:
            i += 1
            continue
        i += 1
        block: list[str] = []
        while i < len(lines) and not lines[i].strip().startswith("```"):
            block.append(lines[i].rstrip("\n"))
            i += 1
        blocks.append(block)
        while i < len(lines) and not lines[i].strip().startswith("```"):
            i += 1
        if i < len(lines) and lines[i].strip().startswith("```"):
            i += 1
    return blocks


def _extract_inline_backticked_commands(md: str) -> list[str]:
    # Captures `...` that look like commands.
    cmds: list[str] = []
    for m in re.finditer(r"`([^`]+)`", md):
        s = m.group(1).strip()
        if "aura " in s or "cargo run -p aura" in s:
            cmds.append(s)
    return cmds


def _select_run_block_from_readme(readme: Path) -> Optional[list[str]]:
    md = _read_text(readme)

    blocks = _find_powershell_code_blocks(md)
    for block in blocks:
        joined = "\n".join(block)
        if "cargo run -p aura" in joined or re.search(r"\baura(\.exe)?\s+run\b", joined):
            return [ln.strip() for ln in block if ln.strip() and not ln.strip().startswith("#")]

    inline_cmds = _extract_inline_backticked_commands(md)
    for cmd in inline_cmds:
        if "cargo run -p aura" in cmd or re.search(r"\baura(\.exe)?\s+run\b", cmd):
            return [cmd]

    return None


def _file_has_main(aura_file: Path) -> bool:
    try:
        text = _read_text(aura_file)
    except OSError:
        return False
    return re.search(r"(?m)^\s*cell\s+main\s*\(", text) is not None


def _file_uses_quantifiers(aura_file: Path) -> bool:
    try:
        text = _read_text(aura_file)
    except OSError:
        return False
    return re.search(r"\b(forall|exists)\b", text) is not None


def _file_uses_iot_hw_contracts(aura_file: Path) -> bool:
    try:
        text = _read_text(aura_file)
    except OSError:
        return False
    # These are Z3 plugin calls (not AVM runtime), so they should be verified rather than executed.
    return re.search(r"\bhw\.(open|read_u32|write_u32)\b", text) is not None


def _parse_requested_plugins(aura_toml: Path) -> set[str]:
    if not aura_toml.exists():
        return set()
    text = _read_text(aura_toml)
    # Minimal parsing: collect plugin names like { name = "aura-iot", ... }
    names = set(re.findall(r"\bname\s*=\s*\"([^\"]+)\"", text))
    # Only keep plausible plugin identifiers.
    return {n for n in names if n.startswith("aura-")}


def _needs_z3_plugins(workdir: Path) -> bool:
    plugins = _parse_requested_plugins(workdir / "aura.toml")
    # In this repo, enabling the `z3` cargo feature also enables built-in AI/IoT plugins.
    return any(p in plugins for p in {"aura-iot", "aura-ai"})


def _discover_example_dirs() -> list[Path]:
    if not EXAMPLES_DIR.exists():
        return []
    dirs: list[Path] = []
    for child in EXAMPLES_DIR.iterdir():
        if not child.is_dir():
            continue
        # Special folders are handled separately.
        if child.name in {"root-aura", "verification"}:
            continue
        if (child / "aura.toml").exists() or (child / "README.md").exists():
            dirs.append(child)
    return sorted(dirs, key=lambda p: p.name.lower())


def _discover_standalone_aura_files() -> list[Path]:
    if not EXAMPLES_DIR.exists():
        return []
    files = [p for p in EXAMPLES_DIR.glob("*.aura") if p.is_file()]
    return sorted(files, key=lambda p: p.name.lower())


def _default_aura_command_for_file(aura_file: Path, workdir: Path) -> Example:
    # If the example requests built-in plugins that are gated behind cargo features,
    # run via cargo to ensure the binary includes them.
    if _needs_z3_plugins(workdir):
        # Run from the example folder so it can pick up aura.toml.
        if _file_has_main(aura_file) and not _file_uses_iot_hw_contracts(aura_file):
            cmd = [f'cargo run -p aura --features z3 -- run "{aura_file.name}" --mode avm']
            return Example(
                id=str(aura_file.relative_to(REPO_ROOT)).replace("\\", "/"),
                title=aura_file.stem,
                workdir=workdir,
                kind="run",
                command_ps_lines=cmd,
            )

        smt = " --smt-profile thorough" if _file_uses_quantifiers(aura_file) else ""
        cmd = [f'cargo run -p aura --features z3 -- verify "{aura_file.name}"{smt}']
        return Example(
            id=str(aura_file.relative_to(REPO_ROOT)).replace("\\", "/"),
            title=aura_file.stem,
            workdir=workdir,
            kind="verify",
            command_ps_lines=cmd,
        )

    aura_exe = DIST_AURA_EXE if DIST_AURA_EXE.exists() else Path("aura")

    if _file_has_main(aura_file) and not _file_uses_iot_hw_contracts(aura_file):
        cmd = [f'& "{aura_exe}" run "{aura_file.name}" --mode avm']
        return Example(
            id=str(aura_file.relative_to(REPO_ROOT)).replace("\\", "/"),
            title=aura_file.stem,
            workdir=workdir,
            kind="run",
            command_ps_lines=cmd,
        )

    smt = " --smt-profile thorough" if _file_uses_quantifiers(aura_file) else ""
    cmd = [f'& "{aura_exe}" verify "{aura_file.name}"{smt}']
    return Example(
        id=str(aura_file.relative_to(REPO_ROOT)).replace("\\", "/"),
        title=aura_file.stem,
        workdir=workdir,
        kind="verify",
        command_ps_lines=cmd,
    )


def discover_examples() -> list[Example]:
    examples: list[Example] = []

    # Project-like directories.
    for d in _discover_example_dirs():
        readme = d / "README.md"
        aura_files = sorted([p for p in d.glob("*.aura") if p.is_file()], key=lambda p: p.name.lower())

        if readme.exists():
            cmd_lines = _select_run_block_from_readme(readme)
            if cmd_lines:
                examples.append(
                    Example(
                        id=str(d.relative_to(REPO_ROOT)).replace("\\", "/"),
                        title=d.name,
                        workdir=REPO_ROOT,  # README commands are typically from repo root
                        kind="custom",
                        command_ps_lines=cmd_lines,
                    )
                )
                continue

        # No README-derived command: infer a default.
        entry: Optional[Path] = None
        if (d / "main.aura").exists():
            entry = d / "main.aura"
        elif (d / f"{d.name}.aura").exists():
            entry = d / f"{d.name}.aura"
        else:
            for f in aura_files:
                if _file_has_main(f):
                    entry = f
                    break
            if entry is None and aura_files:
                entry = aura_files[0]

        if entry is None:
            continue

        examples.append(_default_aura_command_for_file(entry, workdir=d))

    # Standalone example files in examples/ (excluding special subfolders).
    for f in _discover_standalone_aura_files():
        examples.append(_default_aura_command_for_file(f, workdir=EXAMPLES_DIR))

    # root-aura (treat each .aura as its own selectable example)
    root_aura = EXAMPLES_DIR / "root-aura"
    if root_aura.exists():
        aura_files = sorted([p for p in root_aura.glob("*.aura") if p.is_file()], key=lambda p: p.name.lower())
        for f in aura_files:
            examples.append(_default_aura_command_for_file(f, workdir=root_aura))

    # verification (verify each file)
    verification = EXAMPLES_DIR / "verification"
    if verification.exists():
        aura_files = sorted([p for p in verification.glob("*.aura") if p.is_file()], key=lambda p: p.name.lower())
        for f in aura_files:
            # Force verify for this folder
            aura_exe = DIST_AURA_EXE if DIST_AURA_EXE.exists() else Path("aura")
            smt = " --smt-profile thorough" if _file_uses_quantifiers(f) else ""
            examples.append(
                Example(
                    id=str(f.relative_to(REPO_ROOT)).replace("\\", "/"),
                    title=f"verification/{f.stem}",
                    workdir=verification,
                    kind="verify",
                    command_ps_lines=[f'& "{aura_exe}" verify "{f.name}"{smt}'],
                )
            )

    # Stable ordering + de-dup (by id).
    unique: dict[str, Example] = {}
    for ex in examples:
        unique[ex.id] = ex
    return sorted(unique.values(), key=lambda e: (e.title.lower(), e.id.lower()))


def _parse_selection(expr: str, n: int) -> list[int]:
    expr = expr.strip().lower()
    if expr in {"a", "all"}:
        return list(range(1, n + 1))

    out: set[int] = set()
    parts = [p.strip() for p in re.split(r"[\s,]+", expr) if p.strip()]
    for part in parts:
        if re.fullmatch(r"\d+", part):
            idx = int(part)
            if 1 <= idx <= n:
                out.add(idx)
            continue
        m = re.fullmatch(r"(\d+)-(\d+)", part)
        if m:
            a = int(m.group(1))
            b = int(m.group(2))
            lo, hi = (a, b) if a <= b else (b, a)
            for idx in range(lo, hi + 1):
                if 1 <= idx <= n:
                    out.add(idx)
            continue
        raise ValueError(f"Invalid selection token: {part!r}")

    return sorted(out)


def _run_powershell_lines(lines: list[str], cwd: Path) -> int:
    if not _is_windows():
        raise RuntimeError("This runner currently supports Windows/PowerShell only.")

    # Execute the lines in one PowerShell invocation so env changes persist within the example.
    joined = " ; ".join(lines)
    ps_script = (
        "$ErrorActionPreference = 'Continue' ; "
        f"Push-Location -LiteralPath '{str(cwd)}' ; "
        f"{joined} ; "
        "$code = $LASTEXITCODE ; "
        "Pop-Location ; "
        "exit $code"
    )

    proc = subprocess.run(
        [
            "powershell",
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            ps_script,
        ],
        cwd=str(REPO_ROOT),
    )
    return int(proc.returncode)


def run_examples(selected: Iterable[Example], stop_on_fail: bool) -> int:
    failures: list[str] = []
    for ex in selected:
        print(f"\n=== [{ex.title}] ({ex.kind}) ===")
        print(f"Workdir: {ex.workdir}")
        print(f"Command: {ex.preview}")

        code = _run_powershell_lines(ex.command_ps_lines, cwd=ex.workdir)
        if code == 0:
            print(f"PASS: {ex.title}")
        else:
            print(f"FAIL: {ex.title} (exit={code})")
            failures.append(f"{ex.title} (exit={code})")
            if stop_on_fail:
                break

    print("\n=== Summary ===")
    if not failures:
        print("All selected examples succeeded.")
        return 0

    print("Some examples failed:")
    for f in failures:
        print(f"- {f}")
    return 1


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description="Run Aura examples with a numbered picker.")
    ap.add_argument("--list", action="store_true", help="List discovered examples and exit")
    ap.add_argument("--example", help="Selection like '3', '1,2,5', '2-6', or 'all'")
    ap.add_argument("--all", action="store_true", help="Run all examples")
    ap.add_argument("--stop-on-fail", action="store_true", help="Stop at first failure")

    args = ap.parse_args(argv)

    examples = discover_examples()
    if not examples:
        print(f"No examples found under {EXAMPLES_DIR}")
        return 2

    if args.list:
        for i, ex in enumerate(examples, start=1):
            print(f"{i:2d}. {ex.title:28s} [{ex.kind}]  ({ex.id})")
        return 0

    if args.all:
        selected = examples
        return run_examples(selected, stop_on_fail=args.stop_on_fail)

    if args.example:
        try:
            idxs = _parse_selection(args.example, n=len(examples))
        except ValueError as e:
            print(str(e))
            return 2
        selected = [examples[i - 1] for i in idxs]
        return run_examples(selected, stop_on_fail=args.stop_on_fail)

    # Interactive picker
    print("Discovered examples:")
    for i, ex in enumerate(examples, start=1):
        print(f"{i:2d}. {ex.title:28s} [{ex.kind}]")

    while True:
        raw = input("\nChoose example number (e.g. 3, 1-4, 2,5,7, or 'all'): ").strip()
        if not raw:
            continue
        try:
            idxs = _parse_selection(raw, n=len(examples))
        except ValueError as e:
            print(str(e))
            continue
        if not idxs:
            print("No selections. Try again.")
            continue
        selected = [examples[i - 1] for i in idxs]
        break

    return run_examples(selected, stop_on_fail=args.stop_on_fail)


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
