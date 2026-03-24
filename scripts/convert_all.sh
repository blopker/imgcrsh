#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
INPUT_DIR="$PROJECT_DIR/example_images"
OUTPUT_DIR="$PROJECT_DIR/outputs"
FORMAT="${1:-webp}"

mkdir -p "$OUTPUT_DIR"

# Build release binary
echo "Building imgcrsh..."
cargo build --release --manifest-path "$PROJECT_DIR/Cargo.toml"
BIN="$PROJECT_DIR/target/release/imgcrsh"

for img in "$INPUT_DIR"/*; do
    [ -f "$img" ] || continue
    basename="$(basename "$img")"
    name="${basename%.*}"
    output="$OUTPUT_DIR/${name}.${FORMAT}"
    echo "Converting $basename -> ${name}.${FORMAT}"
    "$BIN" "$img" "$output" || echo "  Failed: $basename"
done

echo "Done. Output in $OUTPUT_DIR"
