#!/bin/sh
# ChipWave Lite - headless M1 tests
# Copyright (C) 2026 Del and the ChipWave contributors. GPL-3.0-or-later.
#
# 1. vamos (amitools, Linux): tick logs of the 68000 and 68020 builds.
# 2. libxmp reference: per-tick periods/volumes/tempo vs the tick log.
# 3. FS-UAE with its built-in AROS ROM, in an A500 (68000, OCS, 512K chip +
#    512K slow), an A500 with only 512K chip, an A500 with 1 MB chip, and an
#    A1200 (68020, AGA, 2 MB chip):
#    boots a test ADF whose startup-sequence runs the tick log (+ E-clock
#    bench), a full play-through (LOOPS 1), a Ctrl-C stop (sent by CWBreak),
#    and a second play to show that audio and the CIA-B timers were freed.
#    Results are written to a host folder mounted as DH0:.
#
# Needs: amitools (vamos, xdftool), fs-uae, an X display (DISPLAY, default :5),
# libxmp-dev, python3. Run from amiga/ (or via "make test").
# ONLY=a500|a500-512k|a500-1mbchip|a1200 runs just one FS-UAE config.
set -u
cd "$(dirname "$0")/.."
AT=${AT:-/opt/amitools-venv/bin}
OUT=${OUT:-build/test}
export DISPLAY=${DISPLAY:-:5}
MOD=test/cwlite-test.mod
rm -rf "$OUT"; mkdir -p "$OUT"
FAIL=0
say() { echo "$*" | tee -a "$OUT/summary.txt"; }
check() { if [ "$1" = ok ]; then say "  PASS  $2"; else say "  FAIL  $2"; FAIL=1; fi; }

say "ChipWave Lite M1 tests, $(date '+%Y-%m-%d %H:%M %Z')"
say "== 1. vamos (68000 and 68020 builds, tick log)"
cp "$MOD" "$OUT/song.mod"
( cd "$OUT" && "$AT/vamos" -C 68000 ../ChipWaveLite.000 song.mod TICKLOG vamos-000.tl QUIET >/dev/null 2>&1 )
( cd "$OUT" && "$AT/vamos" -C 68020 ../ChipWaveLite song.mod TICKLOG vamos-020.tl QUIET >/dev/null 2>&1 )
cmp -s "$OUT/vamos-000.tl" "$OUT/vamos-020.tl" && r=ok || r=bad
check $r "68000 and 68020 builds write identical tick logs ($(($(stat -c %s "$OUT/vamos-000.tl") / 74)) ticks)"

say "== 2. libxmp reference"
cc -O2 -Wall test/xmpref.c -lxmp -o build/xmpref
build/xmpref "$MOD" > "$OUT/libxmp-ref.txt"
python3 test/compare_ref.py "$OUT/vamos-000.tl" "$OUT/libxmp-ref.txt" --known test/ref_known_diffs.txt > "$OUT/compare-libxmp.txt"
sed 's/^/        /' "$OUT/compare-libxmp.txt" | tee -a "$OUT/summary.txt" >/dev/null
grep -q "RESULT: MATCH" "$OUT/compare-libxmp.txt" && r=ok || r=bad
check $r "tick log matches libxmp (periods, volumes, tempo; known libxmp-vs-PT cells listed in test/ref_known_diffs.txt)"
python3 test/ticklog.py "$OUT/vamos-000.tl" > "$OUT/ticklog.txt"

# --- FS-UAE ---------------------------------------------------------------
run_uae() { # name model exe extra-config
	name=$1 model=$2 exe=$3 extra=$4 nobreak=${5:-}
	[ -n "${ONLY:-}" ] && [ "$ONLY" != "$name" ] && return 0
	d="$OUT/$name"; mkdir -p "$d/dh0"
	cat > "$d/startup-sequence" <<SS
Echo "boot" >DH0:boot.txt
ChipWaveLite song.mod TICKLOG DH0:uae.tl RESULT DH0:tick.res BENCH QUIET
ChipWaveLite >DH0:loop.out song.mod LOOPS 1 RESULT DH0:loop.res
$( [ -z "$nobreak" ] && printf '%s\n%s' 'Run >NIL: CWBreak 8 ChipWaveLite RESULT DH0:break.res' 'ChipWaveLite >DH0:ctrlc.out song.mod RESULT DH0:ctrlc.res' )
ChipWaveLite song.mod SECONDS 2 RESULT DH0:again.res
Echo "done" >DH0:done.txt
SS
	"$AT/xdftool" "$d/test.adf" create + format CWLiteTest ffs + boot install + makedir s \
		+ write "$d/startup-sequence" s/startup-sequence + write "build/$exe" ChipWaveLite \
		+ write build/CWBreak CWBreak + write "$MOD" song.mod >/dev/null
	cat > "$d/uae.fs-uae" <<CFG
[fs-uae]
amiga_model = $model
floppy_drive_0 = $(pwd)/$d/test.adf
hard_drive_0 = $(pwd)/$d/dh0
hard_drive_0_label = DH0
hard_drive_0_priority = -128
automatic_input_grab = 0
audio_driver = null
$extra
CFG
	fs-uae "$d/uae.fs-uae" > "$d/fs-uae.out" 2>&1 &
	pid=$!
	t0=$(date +%s)
	i=0
	while [ $i -lt 150 ] && [ ! -f "$d/dh0/done.txt" ]; do sleep 1; i=$((i + 1)); done
	sleep 1
	[ -f "$d/dh0/done.txt" ] || DISPLAY=$DISPLAY xwd -root -silent 2>/dev/null | convert xwd:- "$d/timeout.png" 2>/dev/null
	kill $pid 2>/dev/null; wait $pid 2>/dev/null
	cp ~/FS-UAE/Cache/Logs/fs-uae.log.txt "$d/fs-uae.log.txt" 2>/dev/null
	say "== 3. FS-UAE $name ($model, $exe), finished in $(( $(date +%s) - t0 ))s"
	[ -f "$d/dh0/done.txt" ] && r=ok || r=bad
	check $r "booted the test ADF and ran the startup-sequence to the end"
	cmp -s "$d/dh0/uae.tl" "$OUT/vamos-000.tl" && r=ok || r=bad
	check $r "tick log from the emulated Amiga is identical to the vamos/reference log"
	v() { sed -n "s/^$2=//p" "$d/dh0/$1" 2>/dev/null; }
	say "        cpu attnflags=$(v tick.res attnflags), vblank=$(v tick.res vblank_hz) Hz, replay per tick: avg $(v tick.res bench_avg_us) us, peak $(v tick.res bench_max_us) us"
	el=$(v loop.res elapsed_ticks50)
	[ "$(v loop.res status)" = ok ] && [ "$(v loop.res stopped_by)" = loops ] && r=ok || r=bad
	check $r "LOOPS 1: played the whole song through ptplayer in $((${el:-0} / 50)).$(( (${el:-0} % 50) * 2 )) s (libxmp: 24.34 s), E8 trigger=$(v loop.res e8)"
	exp=1217   # libxmp: 24.34 s, in 1/50 s
	[ -n "$el" ] && [ $el -ge $((exp - 25)) ] && [ $el -le $((exp + 25)) ] && r=ok || r=bad
	check $r "song length within 0.5 s of the reference"
	[ "$(v loop.res audio_dma_after)" = 0 ] && [ "$(v loop.res intena_before)" = "$(v loop.res intena_after)" ] && r=ok || r=bad
	check $r "after the song: audio DMA off, INTENA restored ($(v loop.res intena_before) -> $(v loop.res intena_after))"
	say "        chip RAM free before loading: $(v loop.res chip_free) bytes (largest block $(v loop.res chip_largest))"
	if [ -n "$nobreak" ]; then
		say "  SKIP  Ctrl-C step: a 512K-only machine with the AROS ROM has no room for the background shell that sends it (covered by the other configs)"
	else
		el2=$(v ctrlc.res elapsed_ticks50); el2=${el2:-0}
		[ "$(v ctrlc.res stopped_by)" = ctrl-c ] && [ "$(v ctrlc.res status)" = ok ] && r=ok || r=bad
		check $r "Ctrl-C stops playback cleanly ($(v break.res ctrl_c_sent_to_cli | sed 's/^/sent to CLI /'), after $((el2 / 50)) s, audio DMA $(v ctrlc.res audio_dma_after))"
	fi
	[ "$(v again.res install)" = ok ] && [ "$(v again.res stopped_by)" = seconds ] && r=ok || r=bad
	check $r "a second run gets the audio channels and CIA-B timers again"
}
run_uae a500 A500 ChipWaveLite.000 ""
run_uae a500-512k A500 ChipWaveLite.000 "chip_memory = 512
slow_memory = 0" nobreak
run_uae a500-1mbchip A500 ChipWaveLite.000 "chip_memory = 1024
slow_memory = 0"
run_uae a1200 A1200 ChipWaveLite ""

say ""
[ $FAIL = 0 ] && say "ALL TESTS PASSED" || say "SOME TESTS FAILED"
exit $FAIL
