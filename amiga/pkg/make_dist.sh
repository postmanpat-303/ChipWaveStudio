#!/bin/sh
# ChipWave Lite - build the bootable ADF and the LHA archive
# Copyright (C) 2026 Del and the ChipWave contributors. GPL-3.0-or-later.
#
# Output: dist/ChipWaveLite-0.1-m1.adf  bootable, starts the player
#         dist/ChipWaveLite-0.1-m1.lha  drawer for a hard disk / Workbench
# Needs xdftool (amitools) and jlha (Debian jlha-utils).
set -e
cd "$(dirname "$0")/.."
AT=${AT:-/opt/amitools-venv/bin}
V=0.1-m1
D=dist
S=$D/stage/ChipWaveLite
rm -rf "$D"; mkdir -p "$S/Songs"
cp build/ChipWaveLite build/ChipWaveLite.000 "$S/"
cp test/cwlite-test.mod "$S/Songs/"
cp pkg/README.txt "$S/ReadMe.txt"
cp ../LICENSE "$S/LICENSE"
cp src/replay/LICENSE.ptplayer "$S/LICENSE.ptplayer"

# startup-sequence for the bootable disk. Only commands that are built into
# the Kickstart 3.x shell (and the AROS ROM) are used: no C: directory needed.
cat > "$D/stage/startup-sequence" <<'SS'
Echo "ChipWave Lite 0.1-m1 - Amiga .mod player (GPL-3)"
Echo "Playing Songs/cwlite-test.mod. Press Ctrl-C in this window to stop."
ChipWaveLite.000 Songs/cwlite-test.mod
Echo ""
Echo "To play it again:  ChipWaveLite.000 Songs/cwlite-test.mod"
Echo "68020+ build:      ChipWaveLite Songs/cwlite-test.mod LOOPS 1"
SS

"$AT/xdftool" "$D/ChipWaveLite-$V.adf" create + format "ChipWaveLite" ffs + boot install \
	+ makedir s + write "$D/stage/startup-sequence" s/startup-sequence \
	+ write "$S/ChipWaveLite" ChipWaveLite + write "$S/ChipWaveLite.000" ChipWaveLite.000 \
	+ makedir Songs + write "$S/Songs/cwlite-test.mod" Songs/cwlite-test.mod \
	+ write "$S/ReadMe.txt" ReadMe.txt + write "$S/LICENSE" LICENSE \
	+ write "$S/LICENSE.ptplayer" LICENSE.ptplayer >/dev/null

( cd "$D/stage" && jlha a "../ChipWaveLite-$V.lha" ChipWaveLite >/dev/null )
rm -rf "$D/stage"
"$AT/xdftool" "$D/ChipWaveLite-$V.adf" list
lhasa l "$D/ChipWaveLite-$V.lha" 2>/dev/null || jlha l "$D/ChipWaveLite-$V.lha"
ls -l "$D"
