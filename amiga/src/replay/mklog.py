#!/usr/bin/env python3
# ChipWave Lite - make the tick-log copy of ptplayer
# Copyright (C) 2026 Del and the ChipWave contributors. GPL-3.0-or-later.
#
# Usage: mklog.py ptplayer.asm > ptplayer_log.asm
#
# The copy is built with NO_TIMERS=1 so the program can call it tick by tick.
# It is identical to ptplayer.asm (Frank Wille, public domain) except:
#  1. exported symbols are renamed _mt_* -> _mtlog_*, so both copies link
#     into one program;
#  2. CUSTOM, CIAA and CIAB point at RAM shadows (_mtlog_custom, _mtlog_ciaa,
#     _mtlog_ciab), so the copy never touches Paula or the CIAs (and runs
#     under vamos);
#  3. mt_setspeed: with NO_TIMERS ptplayer treats Fxx >= $20 as a speed,
#     because there is no CIA timer to set. The copy keeps the speed path
#     unchanged and stores tempo values in _mtlog_tempo instead, which is
#     what the live (CIA) build turns into a CIA-B timer A period.
import re, sys

src = open(sys.argv[1]).read()
s = src.replace("_mt_", "_mtlog_")
s, n1 = re.subn(r"^CUSTOM(\s+)equ.*$", r"CUSTOM\1equ\t_mtlog_custom", s, flags=re.M)
s, n2 = re.subn(r"^CIAA(\s+)equ.*$", r"CIAA\1equ\t_mtlog_ciaa+1", s, flags=re.M)
s, n3 = re.subn(r"^CIAB(\s+)equ.*$", r"CIAB\1equ\t_mtlog_ciab", s, flags=re.M)
a = s.index("\nmt_setspeed:\n")
b = s.index("\nmt_e_cmds:\n")
s = s[:a] + """
mt_setspeed:
; cmd F x y (xy<$20 new speed, xy>=$20 new tempo)  [patched by mklog.py]
; d4 = xy
	cmp.b	#$20,d4
	bhs	.1
	move.b	d4,mt_Speed(a4)
	beq	_mtlog_end
	rts
.1:	move.b	d4,_mtlog_tempo
	rts

""" + s[b:]
s, n4 = re.subn(r"^\s*end\s*$", "", s, flags=re.M)
assert (n1, n2, n3) == (1, 1, 1) and n4 >= 1, (n1, n2, n3, n4)
s += """
; --- ChipWave Lite tick-log shadows (appended by mklog.py) ---
	section	logshadow,bss
	xdef	_mtlog_custom
	xdef	_mtlog_ciaa
	xdef	_mtlog_ciab
	xdef	_mtlog_tempo
	cnop	0,4
_mtlog_custom:	ds.b	$200
_mtlog_ciaa:	ds.b	$1000
_mtlog_ciab:	ds.b	$1000
_mtlog_tempo:	ds.b	2
	end
"""
sys.stdout.write(s)
