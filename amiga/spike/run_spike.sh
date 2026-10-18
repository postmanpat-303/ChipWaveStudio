#!/usr/bin/env bash
# ChipWave Lite feasibility spike (2026-10-18): build the asm + C test programs,
# run the OS-only one under vamos, then boot an ADF in FS-UAE with its built-in
# AROS ROM (no Kickstart needed) and check the marker files the programs write.
# Needs: AmigaPorts m68k-amigaos-gcc (prebuilt x86_64-linux tarball) as $AMIGA_GCC,
# vasm (m68k, mot syntax) as $VASM, amitools (vamos, xdftool), fs-uae, an X display.
set -euo pipefail
cd "$(dirname "$0")"
AMIGA_GCC=${AMIGA_GCC:-/workspace/amiga-spike/toolchain/m68k-amigaos-gcc-16.2}
VASM=${VASM:-/workspace/amiga-spike/vasm/vasmm68k_mot}
AT=${AT:-/opt/amitools-venv/bin}
OUT=${OUT:-/tmp/cwlite-spike}
mkdir -p "$OUT/dh0"; rm -f "$OUT"/dh0/*
"$VASM" -m68020 -Fhunkexe -nosym -quiet -o "$OUT/hello" hello.s
"$VASM" -m68020 -Fhunkexe -nosym -quiet -o "$OUT/beep" beep.s
"$AMIGA_GCC/bin/m68k-amigaos-gcc" -m68020 -Os -noixemul -Wall -Wno-pointer-sign -o "$OUT/cwlite_spike" cwlite_spike.c
"$AT/vamos" -C 68020 "$OUT/hello"
printf 'hello\nbeep\ncwlite_spike\n' > "$OUT/startup-sequence"
rm -f "$OUT/spike.adf"
"$AT/xdftool" "$OUT/spike.adf" create + format CWLiteSpike ffs + boot install + makedir s \
  + write "$OUT/startup-sequence" s/startup-sequence + write "$OUT/hello" hello \
  + write "$OUT/beep" beep + write "$OUT/cwlite_spike" cwlite_spike >/dev/null
cat > "$OUT/spike.fs-uae" <<CFG
[fs-uae]
amiga_model = A1200
floppy_drive_0 = $OUT/spike.adf
hard_drive_0 = $OUT/dh0
hard_drive_0_label = DH0
hard_drive_0_priority = -128
automatic_input_grab = 0
audio_driver = null
CFG
timeout 40 fs-uae "$OUT/spike.fs-uae" >/dev/null 2>&1 || true
echo "beep (asm, hardware): $(cat "$OUT/dh0/beep-result.txt" 2>/dev/null || echo missing)"
echo "C spike (window + audio.device): $(cat "$OUT/dh0/c-spike-result.txt" 2>/dev/null || echo missing)"
