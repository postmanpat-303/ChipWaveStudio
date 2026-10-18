; ChipWave Lite - C glue for ptplayer
; Copyright (C) 2026 Del and the ChipWave contributors
;
; This program is free software: you can redistribute it and/or modify
; it under the terms of the GNU General Public License as published by
; the Free Software Foundation, either version 3 of the License, or
; (at your option) any later version.
;
; This program is distributed in the hope that it will be useful,
; but WITHOUT ANY WARRANTY; without even the implied warranty of
; MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
; GNU General Public License for more details.
;
; You should have received a copy of the GNU General Public License
; along with this program.  If not, see <https://www.gnu.org/licenses/>.
;
; ptplayer itself (ptplayer.asm) is by Frank Wille and is in the public
; domain; see LICENSE.ptplayer. This file only adapts its register-based
; calls to the stack-based C calling convention of m68k-amigaos-gcc.
; Every wrapper saves all registers, because ptplayer uses most of them.
;
; Live player (ptplayer built with OSCOMPAT=1, CIA-B timed, hardware):
;   LONG cwl_install(void)            -> 1 ok, 0 failed (call cwl_remove anyway)
;   void cwl_remove(void)
;   void cwl_init(APTR mod)           -> mt_init(a6=CUSTOM,a0=mod,a1=NULL,d0=0)
;   void cwl_end(void)                -> stop music, silence Paula
;
; Tick-log player (a second copy of ptplayer, NO_TIMERS=1, whose CUSTOM,
; CIAA and CIAB point at RAM shadows instead of the hardware; symbols
; renamed _mt_ -> _mtlog_ by the Makefile):
;   void cwl_log_init(APTR mod)
;   void cwl_log_music(void)          -> one replay tick
;   void cwl_log_dmaon(void)          -> the "DMA on" part of a tick
;   void cwl_log_setrep(void)         -> the "set repeat" part of a tick
;   void cwl_log_end(void)

CUSTOM	equ	$dff000

	xref	_mt_install
	xref	_mt_remove
	xref	_mt_init
	xref	_mt_end

	ifnd	NOLOG
	xref	_mtlog_init
	xref	_mtlog_end
	xref	_mtlog_music
	xref	_mtlog_dmaon
	xref	_mtlog_setrep
	xref	_mtlog_custom
	endc

	section	code,code

	xdef	_cwl_install
_cwl_install:
	movem.l	d2-d7/a2-a6,-(sp)
	jsr	_mt_install
	movem.l	(sp)+,d2-d7/a2-a6
	rts

	xdef	_cwl_remove
_cwl_remove:
	movem.l	d2-d7/a2-a6,-(sp)
	jsr	_mt_remove
	movem.l	(sp)+,d2-d7/a2-a6
	rts

	xdef	_cwl_init
_cwl_init:
	move.l	4(sp),a0
	movem.l	d2-d7/a2-a6,-(sp)
	lea	CUSTOM,a6
	sub.l	a1,a1
	moveq	#0,d0
	jsr	_mt_init
	movem.l	(sp)+,d2-d7/a2-a6
	rts

	xdef	_cwl_end
_cwl_end:
	movem.l	d2-d7/a2-a6,-(sp)
	lea	CUSTOM,a6
	jsr	_mt_end
	movem.l	(sp)+,d2-d7/a2-a6
	rts

	ifnd	NOLOG
	xdef	_cwl_log_init
_cwl_log_init:
	move.l	4(sp),a0
	movem.l	d2-d7/a2-a6,-(sp)
	lea	_mtlog_custom,a6
	sub.l	a1,a1
	moveq	#0,d0
	jsr	_mtlog_init
	movem.l	(sp)+,d2-d7/a2-a6
	rts

	xdef	_cwl_log_end
_cwl_log_end:
	movem.l	d2-d7/a2-a6,-(sp)
	lea	_mtlog_custom,a6
	jsr	_mtlog_end
	movem.l	(sp)+,d2-d7/a2-a6
	rts

	xdef	_cwl_log_music
_cwl_log_music:
	movem.l	d2-d7/a2-a6,-(sp)
	lea	_mtlog_custom,a6
	jsr	_mtlog_music
	movem.l	(sp)+,d2-d7/a2-a6
	rts

	xdef	_cwl_log_dmaon
_cwl_log_dmaon:
	movem.l	d2-d7/a2-a6,-(sp)
	jsr	_mtlog_dmaon
	movem.l	(sp)+,d2-d7/a2-a6
	rts

	xdef	_cwl_log_setrep
_cwl_log_setrep:
	movem.l	d2-d7/a2-a6,-(sp)
	jsr	_mtlog_setrep
	movem.l	(sp)+,d2-d7/a2-a6
	rts
	endc

	end
