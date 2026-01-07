# Compatibility test suite

This folder contains a small compatibility suite meant to catch basic regressions in the CLI toolchain.

It is intentionally lightweight and runs in CI.

## What it checks
- `aura build --mode avm` succeeds for a small set of fixtures

## How to run
- Windows (PowerShell): `tools/compat/run.ps1`
- Linux/macOS (bash): `tools/compat/run.sh`
