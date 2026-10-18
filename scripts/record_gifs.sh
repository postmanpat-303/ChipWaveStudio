#!/usr/bin/env bash
# Record the README GIFs from the real app (native build on an X display,
# e.g. DISPLAY=:1 or under Xvfb): the app runs in the hidden --screenshot
# --hold mode (demo loaded, "Drop 2" playing, see src/app/shot.rs), ffmpeg
# x11grab records the window, then each clip is cropped, scaled to ~800 px and
# turned into a looping GIF with its own palette (palettegen/paletteuse),
# then shrunk with gifsicle -O3 --lossy if installed.
# Usage: scripts/record_gifs.sh [outdir]   (default docs/screenshots)
set -euo pipefail
cd "$(dirname "$0")/.."
OUT="${1:-docs/screenshots}"
mkdir -p "$OUT"
BIN=./target/release/chipwave
DUR=${DUR:-9}
FPS=${FPS:-10}
TMP=$(mktemp -d)

# rec <name> <view> <theme> <crop w:h:x:y> <out width> [lower panel]
# (lower panel, 0.2.3.19: instrument | arranger | automation | full)
rec() {
  local name=$1 view=$2 theme=$3 crop=$4 width=$5 lower=${6:-}
  CHIPWAVE_SHOT_LOWER=$lower "$BIN" --screenshot "$TMP/unused.png" --view "$view" --theme "$theme" \
    --size 1900x1060 --hold $((DUR + 6)) >/dev/null 2>&1 &
  local pid=$!
  sleep 2.5
  local win
  win=$(xdotool search --name "^ChipWave Studio$" | tail -1)
  xdotool windowmove "$win" 0 0 || true
  xdotool mousemove 1919 1199 || true  # keep the pointer out of the clip
  sleep 0.8
  local x y
  x=$(xwininfo -id "$win" | awk '/Absolute upper-left X/ {print $4}')
  y=$(xwininfo -id "$win" | awk '/Absolute upper-left Y/ {print $4}')
  ffmpeg -v error -y -f x11grab -framerate $FPS -video_size 1900x1060 -i "$DISPLAY+$x,$y" \
    -t "$DUR" -c:v libx264 -qp 0 -preset ultrafast "$TMP/$name.mkv"
  wait $pid || true
  local f="crop=$crop,scale=$width:-1:flags=lanczos"
  ffmpeg -v error -y -i "$TMP/$name.mkv" -vf "$f,palettegen=max_colors=${COLORS:-64}:stats_mode=diff" "$TMP/$name-pal.png"
  ffmpeg -v error -y -i "$TMP/$name.mkv" -i "$TMP/$name-pal.png" \
    -lavfi "$f [v]; [v][1:v] paletteuse=dither=none:diff_mode=rectangle" \
    -loop 0 "$TMP/$name.gif"
  if command -v gifsicle >/dev/null; then
    gifsicle -O3 --lossy=30 "$TMP/$name.gif" -o "$OUT/$name.gif"
  else
    cp "$TMP/$name.gif" "$OUT/$name.gif"
  fi
  ls -la "$OUT/$name.gif"
}

rec pattern-playing pattern    dark  1555:980:345:66   800 instrument
rec arranger        arranger   dark  1555:980:345:66   800
rec mixer-meters    mixer      dark  1555:680:345:66   800
rec themes          themes     dark  1900:1060:0:0    800
rec instrument      instrument dark  1555:380:345:652  800 instrument
rm -rf "$TMP"
