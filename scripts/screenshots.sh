#!/usr/bin/env bash
# Capture the README screenshots from the real app (native build, needs a display:
# DISPLAY=:1 or xvfb-run). Uses the hidden --screenshot developer flag
# (src/app/shot.rs): demo song loaded, "Drop 2" playing, 1920x1080 window.
# Then quantises/optimises the PNGs (pngquant + oxipng, if installed).
# Usage: scripts/screenshots.sh [outdir]   (default docs/screenshots)
set -euo pipefail
cd "$(dirname "$0")/.."
OUT="${1:-docs/screenshots}"
mkdir -p "$OUT"
cargo build --release
BIN=./target/release/chipwave
SIZE=1920x1080
for v in pattern arranger mixer fx fill instrument settings about; do
  "$BIN" --screenshot "$OUT/dark-$v.png" --view "$v" --theme dark --size $SIZE
done
for v in pattern arranger mixer fx; do
  "$BIN" --screenshot "$OUT/amiga-$v.png" --view "$v" --theme amiga --size $SIZE
done
# synth panel close-up: the left side panel of the "instrument" capture
python3 - "$OUT/dark-instrument.png" <<'PY'
import sys
from PIL import Image
p = sys.argv[1]
Image.open(p).crop((0, 70, 334, 1058)).save(p)
PY
# Voice (glitch) panel close-ups (a Glitch Babble voice added to the demo)
for th in dark amiga; do
  "$BIN" --screenshot "$OUT/$th-voice.png" --view voice --theme $th --size $SIZE
  python3 -c "import sys; from PIL import Image; p=sys.argv[1]; Image.open(p).crop((0, 70, 334, 1058)).save(p)" "$OUT/$th-voice.png"
done
python3 scripts/theme_swatches.py "$OUT/theme-palettes.png"
if command -v pngquant >/dev/null; then
  pngquant --force --skip-if-larger --quality 80-95 --speed 1 --ext .png "$OUT"/*.png || true
fi
if command -v oxipng >/dev/null; then oxipng -q -o 4 --strip safe "$OUT"/*.png; fi
ls -la "$OUT"
