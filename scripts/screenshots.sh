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
# chrome FX devices (0.2.3.11) on the demo's Supersaw Pad: a copy of the demo with
# Metal resonator, Digital distortion and Sub bass in its chain
python3 - "$OUT" "$BIN" <<'PY'
import subprocess, sys
out, binp = sys.argv[1], sys.argv[2]
s = open("demos/first_run.cwt.toml").read()
a = 'fx = [{ kind = "Filter", mode = 1.0, cutoff = 320.0, resonance = 0.1 }, '
s = s.replace(a, 'fx = [{ kind = "Metal resonator", mix = 0.5 }, { kind = "Digital distortion", mix = 0.35 }, '
              '{ kind = "Sub bass", mix = 0.6 }, ', 1)
open("/tmp/chrome.cwt.toml", "w").write(s)
subprocess.run([binp, "--screenshot", f"{out}/dark-fx-chrome.png", "--view", "fx", "--theme", "dark",
                "--size", "1920x1080", "--song", "/tmp/chrome.cwt.toml"], check=True)
PY
# the generator panels (1280x1024)
for v in kit chipfx analog fm noise; do
  "$BIN" --screenshot "$OUT/dark-generators-$v.png" --view "$v" --theme dark --size 1280x1024
done
python3 scripts/theme_swatches.py "$OUT/theme-palettes.png"
if command -v pngquant >/dev/null; then
  pngquant --force --skip-if-larger --quality 80-95 --speed 1 --ext .png "$OUT"/*.png || true
fi
if command -v oxipng >/dev/null; then oxipng -q -o 4 --strip safe "$OUT"/*.png; fi
ls -la "$OUT"
