#!/usr/bin/env bash
# Capture the README screenshots from the real app (native build, needs a display:
# DISPLAY=:1 or xvfb-run). Uses the hidden --screenshot developer flag
# (src/app/shot.rs): a demo song loaded and playing, 1920x1080 window.
# Then quantises/optimises the PNGs (pngquant + oxipng, if installed).
# Usage: scripts/screenshots.sh [outdir]   (default docs/screenshots)
set -euo pipefail
cd "$(dirname "$0")/.."
OUT="${1:-docs/screenshots}"
mkdir -p "$OUT"
cargo build --release
BIN=./target/release/chipwave
SIZE=1920x1080
DISCO=demos/disco.cwt.toml
# shot <out> <view> <theme> [song] ; CHIPWAVE_SHOT_* env vars pick the lower panel
# (LOWER=instrument|arranger|automation|full), the tab (TAB), the block (BLOCK)
# and the instrument (INSTR, 1-based). See src/app/shot.rs.
shot() { "$BIN" --screenshot "$1" --view "$2" --theme "$3" --size $SIZE ${4:+--song "$4"}; }
# main window: the "Velvet Mirrorball" Chorus with the Cyber Choir in the instrument editor
for th in dark amiga; do
  CHIPWAVE_SHOT_LOWER=instrument CHIPWAVE_SHOT_INSTR=11 CHIPWAVE_SHOT_BLOCK=6 shot "$OUT/$th-pattern.png" pattern $th $DISCO
  shot "$OUT/$th-arranger.png" arranger $th $DISCO
  CHIPWAVE_SHOT_BLOCK=6 CHIPWAVE_SHOT_INSTR=8 shot "$OUT/$th-fx.png" fx $th $DISCO
  shot "$OUT/$th-mixer.png" mixer $th
done
CHIPWAVE_SHOT_LOWER=instrument CHIPWAVE_SHOT_INSTR=2 CHIPWAVE_SHOT_BLOCK=6 shot "$OUT/dark-instrument-editor.png" pattern dark $DISCO
CHIPWAVE_SHOT_LOWER=instrument CHIPWAVE_SHOT_TAB=fx CHIPWAVE_SHOT_INSTR=8 CHIPWAVE_SHOT_BLOCK=6 shot "$OUT/dark-instrument-fx.png" pattern dark $DISCO
CHIPWAVE_SHOT_LOWER=full CHIPWAVE_SHOT_BLOCK=6 shot "$OUT/dark-pattern-full.png" pattern dark $DISCO
CHIPWAVE_SHOT_BLOCK=6 shot "$OUT/dark-mixer-limiter.png" mixer dark $DISCO
shot "$OUT/dark-arranger-play-marker.png" cue dark $DISCO
CHIPWAVE_SHOT_BLOCK=6 shot "$OUT/dark-chord-helper.png" chords dark $DISCO
for v in fill settings about library; do
  f=$v; [ $v = library ] && f=instrument-library
  shot "$OUT/dark-$f.png" $v dark $DISCO
done
# close-ups of the lower-panel editor: Disco Strings (synth), Glitch Babble (Voice)
lower_crop() { python3 -c "import sys; from PIL import Image; p=sys.argv[1]; Image.open(p).crop((347, 676, 1905, 1040)).save(p)" "$1"; }
CHIPWAVE_SHOT_LOWER=instrument CHIPWAVE_SHOT_INSTR=8 CHIPWAVE_SHOT_BLOCK=6 shot "$OUT/dark-instrument.png" pattern dark $DISCO
lower_crop "$OUT/dark-instrument.png"
for th in dark amiga; do
  CHIPWAVE_SHOT_LOWER=instrument shot "$OUT/$th-voice.png" voice $th
  lower_crop "$OUT/$th-voice.png"
done
CHIPWAVE_SHOT_LOWER=instrument shot "$OUT/dark-voice-words.png" speech dark
CHIPWAVE_SHOT_LOWER=instrument CHIPWAVE_SHOT_TAB=fx shot "$OUT/dark-voice-transform.png" transform dark
CHIPWAVE_SHOT_LOWER=instrument shot "$OUT/dark-sample-slicing.png" slice dark
# chrome FX devices (0.2.3.11) on the demo's Supersaw Pad: a copy of the demo with
# Metal resonator, Digital distortion and Sub bass in its chain
python3 - <<'PY'
s = open("demos/first_run.cwt.toml").read()
a = 'fx = [{ kind = "Filter", mode = 1.0, cutoff = 320.0, resonance = 0.1 }, '
s = s.replace(a, 'fx = [{ kind = "Metal resonator", mix = 0.5 }, { kind = "Digital distortion", mix = 0.35 }, '
              '{ kind = "Sub bass", mix = 0.6 }, ', 1)
open("/tmp/chrome.cwt.toml", "w").write(s)
PY
shot "$OUT/dark-fx-chrome.png" fx dark /tmp/chrome.cwt.toml
# the generator panels
for v in kit chipfx analog fm noise; do
  CHIPWAVE_SHOT_LOWER=instrument shot "$OUT/dark-generators-$v.png" $v dark
done
# CLAP plugin (needs a Surge XT .clap; set SURGE_CLAP to its path)
if [ -n "${SURGE_CLAP:-}" ]; then
  cat > /tmp/surge-song.cwt.toml <<TOML
format = "chipwave-song-text"
format_version = 2
[song]
title = "Surge XT test"
bpm = 140
tracks = 4
[[instrument]]
slot = 1
name = "Surge XT"
patch = "clap:org.surge-synth-team.surge-xt"
plugin_path = '$SURGE_CLAP'
[[instrument]]
slot = 2
name = "Trance Kick"
patch = "builtin:trance-kick"
[[block]]
name = "A"
lines = 16
rows = """
00 | C-4 01 .. .... | C-2 02 .. ....
04 | D#4 01 .. .... | C-2 02 .. ....
08 | G-4 01 .. .... | C-2 02 .. ....
0C | OFF .. .. .... | C-2 02 .. ....
"""
TOML
  CHIPWAVE_SHOT_LOWER=instrument shot "$OUT/plugin-clap.png" plugin dark /tmp/surge-song.cwt.toml
  CHIPWAVE_SHOT_LOWER=instrument CHIPWAVE_SHOT_TAB=advanced shot "$OUT/plugin-clap-params.png" plugin dark /tmp/surge-song.cwt.toml
fi
python3 scripts/theme_swatches.py "$OUT/theme-palettes.png"
if command -v pngquant >/dev/null; then
  pngquant --force --skip-if-larger --quality 80-95 --speed 1 --ext .png "$OUT"/*.png || true
fi
if command -v oxipng >/dev/null; then oxipng -q -o 4 --strip safe "$OUT"/*.png; fi
ls -la "$OUT"
