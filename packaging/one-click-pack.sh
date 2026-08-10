#!/usr/bin/env bash
# One-click desktop pack — Windows · macOS · Ubuntu layout under dist/desktop/
#
# Local: builds the host OS binary; stubs the other two.
# Full three-OS artifacts: GitHub Actions .github/workflows/pack-desktop.yml
#
# References (ideas only — Shiloh-owned implementation):
#   - cargo-dist multi-OS matrix: https://github.com/axodotdev/cargo-dist
#   - Godot “Export Project” one-click UX
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"
export TMPDIR="${TMPDIR:-$ROOT/.tmp}"
mkdir -p "$TMPDIR"

echo "==> Building shiloh-cli"
cargo build -p shiloh-cli --release

echo "==> Packing desktop binaries"
./target/release/shiloh-cli pack --workspace "$ROOT" --out "$ROOT/dist/desktop"

echo "==> Done"
ls -la "$ROOT/dist/desktop"/*/ 2>/dev/null || true
cat "$ROOT/dist/desktop/README.md"
