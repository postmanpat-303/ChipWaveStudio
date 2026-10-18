/*
 * ChipWave Lite - M1: command-line .mod player
 * Copyright (C) 2026 Del and the ChipWave contributors
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 *
 * Replay: ptplayer 6.4 by Frank Wille (public domain).
 *
 *   ChipWaveLite <file.mod> [LOOPS n] [SECONDS n] [RESULT file] [QUIET]
 *   ChipWaveLite <file.mod> TICKLOG file [TICKS n] [BENCH]
 *
 * Loads the module into chip RAM and plays it with ptplayer (CIA-B timed,
 * audio channels allocated through audio.device) until Ctrl-C, or until
 * LOOPS song passes or SECONDS seconds. Then it stops the music, frees the
 * audio channels and the CIA-B interrupts and the memory, and exits.
 *
 * TICKLOG runs a second copy of ptplayer whose Paula and CIA registers
 * are RAM shadows, calls it tick by tick and writes the register state of
 * every tick to a file (format in test/ticklog.py). It never touches the
 * hardware, so it also runs under vamos. BENCH also times every replay
 * tick with the E-clock (timer.device) and reports the average and peak.
 */
#include <exec/types.h>
#include <exec/memory.h>
#include <exec/execbase.h>
#include <dos/dos.h>
#include <dos/rdargs.h>
#include <proto/exec.h>
#include <proto/dos.h>
#include <devices/timer.h>
#include <proto/timer.h>

#include "core/mod.h"
#include "replay/replay.h"

#define CWL_VERSION "0.1-m1"
#ifdef CWL_68000
#define CWL_CPU "68000"
#else
#define CWL_CPU "68020"
#endif

static const char version_tag[] __attribute__((used)) =
	"$VER: ChipWaveLite " CWL_VERSION " (18.10.2026) " CWL_CPU;

#define TEMPLATE "FILE/A,LOOPS/K/N,SECONDS/K/N,RESULT/K,TICKLOG/K,TICKS/K/N,BENCH/S,QUIET/S"
enum { A_FILE, A_LOOPS, A_SECONDS, A_RESULT, A_TICKLOG, A_TICKS, A_BENCH, A_QUIET, A_COUNT };

#define CUSTOM_DMACONR (*(volatile UWORD *)0xdff002)
#define CUSTOM_INTENAR (*(volatile UWORD *)0xdff01c)

extern struct ExecBase *SysBase;

static LONG elapsed_ticks(const struct DateStamp *a, const struct DateStamp *b)
{
	return ((b->ds_Days - a->ds_Days) * 1440L + (b->ds_Minute - a->ds_Minute)) * 3000L
		+ (b->ds_Tick - a->ds_Tick);
}

/* --- tiny text buffer for the RESULT file (no stdio needed) --- */
static char rbuf[1024];
static LONG rlen;

static void radd(const char *s)
{
	while (*s && rlen < (LONG)sizeof(rbuf) - 1)
		rbuf[rlen++] = *s++;
}

static void raddnum(const char *key, LONG v)
{
	char tmp[16];
	int n = 0, neg = v < 0;
	unsigned long u = neg ? -(unsigned long)v : (unsigned long)v;
	radd(key);
	radd("=");
	do { tmp[n++] = (char)('0' + u % 10); u /= 10; } while (u);
	if (neg) tmp[n++] = '-';
	while (n) { char c[2]; c[0] = tmp[--n]; c[1] = 0; radd(c); }
	radd("\n");
}

static void raddstr(const char *key, const char *v)
{
	radd(key); radd("="); radd(v); radd("\n");
}

static void write_result(const char *name)
{
	BPTR fh;
	if (!name)
		return;
	fh = Open((CONST_STRPTR)name, MODE_NEWFILE);
	if (fh) {
		Write(fh, rbuf, rlen);
		Close(fh);
	}
}

/* --- tick log --- */
#ifndef CWL_NOLOG
#define REC_SIZE 74
#define REC_PER_CHUNK 64
static UBYTE logbuf[REC_SIZE * REC_PER_CHUNK];

static void put16(UBYTE *p, UWORD v) { p[0] = v >> 8; p[1] = (UBYTE)v; }
static void put32(UBYTE *p, ULONG v) { put16(p, v >> 16); put16(p + 2, (UWORD)v); }
static UWORD sh16(int reg) { return *(UWORD *)(mtlog_custom + reg); }
static ULONG sh32(int reg) { return *(ULONG *)(mtlog_custom + reg); }

static ULONG rel(ULONG lc, ULONG base, ULONG size)
{
	if (lc == 0) return 0xffffffffUL;
	if (lc >= base && lc < base + size) return lc - base;
	return 0xfffffffeUL;
}

struct Device *TimerBase;
static ULONG bench_sum, bench_max, bench_n, eclock_hz;

static ULONG eclock_diff(const struct EClockVal *a, const struct EClockVal *b)
{
	return b->ev_lo - a->ev_lo;   /* a tick is far shorter than 2^32 E-clocks */
}

static ULONG tick_e;

/* call one replay step; with BENCH, add its E-clock time to tick_e */
static void timed(void (*fn)(void))
{
	struct EClockVal e0, e1;
	if (!TimerBase) {
		fn();
		return;
	}
	ReadEClock(&e0);
	fn();
	ReadEClock(&e1);
	tick_e += eclock_diff(&e0, &e1);
}

static LONG run_ticklog(UBYTE *mod, ULONG size, const char *name, LONG maxticks, LONG *songend_tick)
{
	BPTR fh;
	LONG t, n = 0, total = 0;
	UBYTE hdr[16];
	int c;

	fh = Open((CONST_STRPTR)name, MODE_NEWFILE);
	if (!fh)
		return -1;
	hdr[0] = 'C'; hdr[1] = 'W'; hdr[2] = 'T'; hdr[3] = 'L';
	put16(hdr + 4, 1);          /* format version */
	put16(hdr + 6, REC_SIZE);
	put32(hdr + 8, size);
	put32(hdr + 12, 0);
	Write(fh, hdr, 16);

	mtlog_tempo = 125;                          /* ProTracker default */
	cwl_log_init(mod);
	*songend_tick = -1;
	for (t = 0; t < maxticks; t++) {
		UBYTE *r = logbuf + n * REC_SIZE;
		UBYTE before = mtlog_SongEnd;
		*(UWORD *)(mtlog_custom + 0x96) = 0xffff;   /* "not written" */
		*(UWORD *)(mtlog_custom + 0x9c) = 0xffff;
		tick_e = 0;
		timed(cwl_log_music);
		put32(r, (ULONG)t);
		put16(r + 4, sh16(0x96));                   /* DMACON written by mt_music */
		for (c = 0; c < 4; c++) {
			int b = 0xa0 + c * 16;
			UBYTE *q = r + 6 + c * 10;
			put32(q, rel(sh32(b), (ULONG)mod, size)); /* AUDxLC */
			put16(q + 4, sh16(b + 4));                 /* AUDxLEN */
			put16(q + 6, sh16(b + 6));                 /* AUDxPER */
			put16(q + 8, sh16(b + 8));                 /* AUDxVOL */
		}
		*(UWORD *)(mtlog_custom + 0x96) = 0xffff;
		timed(cwl_log_dmaon);
		put16(r + 46, sh16(0x96));                  /* DMACON written by dmaon */
		timed(cwl_log_setrep);
		if (TimerBase) {
			bench_sum += tick_e;
			bench_n++;
			if (tick_e > bench_max)
				bench_max = tick_e;
		}
		put16(r + 48, sh16(0x9c));                  /* INTREQ written by setrep */
		for (c = 0; c < 4; c++) {
			int b = 0xa0 + c * 16;
			UBYTE *q = r + 50 + c * 5;
			ULONG lc = rel(sh32(b), (ULONG)mod, size);
			/* repeat start as 24-bit offset + low byte of the length */
			q[0] = (UBYTE)(lc >> 24 == 0xff ? 0xff : lc >> 16);
			q[1] = (UBYTE)(lc >> 8);
			q[2] = (UBYTE)lc;
			put16(q + 3, sh16(b + 4));
		}
		r[70] = mtlog_ciaa[1] & 2;                  /* LED/filter bit (E0x) */
		r[71] = mtlog_E8Trigger;
		r[72] = mtlog_tempo;                        /* last Fxx >= $20 (BPM) */
		r[73] = 0;
		n++;
		total++;
		if (n == REC_PER_CHUNK) {
			Write(fh, logbuf, n * REC_SIZE);
			n = 0;
		}
		if (mtlog_SongEnd != before && *songend_tick < 0) {
			*songend_tick = t;
			if (maxticks == 0x7fffffffL) {
				t++;
				break;
			}
		}
	}
	if (n)
		Write(fh, logbuf, n * REC_SIZE);
	cwl_log_end();
	Close(fh);
	return total;
}
#endif

int main(void)
{
	LONG args[A_COUNT] = { 0 };
	struct RDArgs *rda;
	BPTR fh;
	UBYTE *mod = NULL;
	LONG size = 0, rc = RETURN_FAIL;
	struct FileInfoBlock *fib;
	static ModInfo info;
	int err;

	rda = ReadArgs((CONST_STRPTR)TEMPLATE, args, NULL);
	if (!rda) {
		PrintFault(IoErr(), (CONST_STRPTR)"ChipWaveLite");
		PutStr((CONST_STRPTR)"Usage: ChipWaveLite <file.mod> [LOOPS n] [SECONDS n] [RESULT file] [QUIET]\n");
		return RETURN_FAIL;
	}

#ifndef CWL_68000
	if (!(SysBase->AttnFlags & AFF_68020)) {
		PutStr((CONST_STRPTR)"This is the 68020 build; on a 68000 use ChipWaveLite.000\n");
		FreeArgs(rda);
		return RETURN_FAIL;
	}
#endif

	raddstr("program", "ChipWaveLite " CWL_VERSION " " CWL_CPU);
	raddstr("file", (const char *)args[A_FILE]);
	raddnum("attnflags", SysBase->AttnFlags);
	raddnum("vblank_hz", SysBase->VBlankFrequency);
	raddnum("power_hz", SysBase->PowerSupplyFrequency);

	/* load into chip RAM: Paula can only read chip RAM */
	fh = Open((CONST_STRPTR)args[A_FILE], MODE_OLDFILE);
	if (!fh) {
		PrintFault(IoErr(), (CONST_STRPTR)args[A_FILE]);
		raddstr("error", "open");
		goto out;
	}
	fib = AllocDosObject(DOS_FIB, NULL);
	if (fib && ExamineFH(fh, fib))
		size = fib->fib_Size;
	if (fib)
		FreeDosObject(DOS_FIB, fib);
	if (size <= 0) {
		Close(fh);
		PutStr((CONST_STRPTR)"Can't get the file size\n");
		raddstr("error", "size");
		goto out;
	}
	mod = AllocMem(size, MEMF_CHIP);
	if (!mod) {
		Close(fh);
		Printf((CONST_STRPTR)"Not enough chip RAM: need %ld bytes, largest free block %ld\n",
			size, AvailMem(MEMF_CHIP | MEMF_LARGEST));
		raddstr("error", "nochipmem");
		goto out;
	}
	if (Read(fh, mod, size) != size) {
		Close(fh);
		PutStr((CONST_STRPTR)"Read error\n");
		raddstr("error", "read");
		goto out;
	}
	Close(fh);

	err = mod_parse(mod, (unsigned long)size, &info);
	if (err != MOD_OK) {
		Printf((CONST_STRPTR)"%s: %s\n", args[A_FILE], (ULONG)mod_error_text(err));
		raddstr("error", mod_error_text(err));
		goto out;
	}
	mod_prepare(mod, &info);
	raddstr("title", info.title);
	raddnum("positions", info.song_length);
	raddnum("patterns", info.num_patterns);
	raddnum("samples", info.num_samples_used);
	raddnum("chip_bytes", size);

	if (!args[A_QUIET])
		Printf((CONST_STRPTR)"ChipWave Lite " CWL_VERSION " (" CWL_CPU ")  \"%s\"  [%s]\n"
			"%ld positions, %ld patterns, %ld samples, %ld bytes in chip RAM\n",
			(ULONG)info.title, (ULONG)info.tag, info.song_length, info.num_patterns,
			info.num_samples_used, size);

#ifndef CWL_NOLOG
	if (args[A_TICKLOG]) {
		LONG end_tick = -1, n;
		LONG maxt = args[A_TICKS] ? *(LONG *)args[A_TICKS] : 0x7fffffffL;
		static struct timerequest tr;
		BYTE terr = -1;
		if (args[A_BENCH]) {
			terr = OpenDevice((CONST_STRPTR)TIMERNAME, UNIT_ECLOCK, (struct IORequest *)&tr, 0);
			if (terr == 0) {
				struct EClockVal ev;
				TimerBase = tr.tr_node.io_Device;
				eclock_hz = ReadEClock(&ev);
			} else {
				PutStr((CONST_STRPTR)"BENCH: no timer.device, timing skipped\n");
			}
		}
		n = run_ticklog(mod, (ULONG)size, (const char *)args[A_TICKLOG], maxt, &end_tick);
		raddstr("mode", "ticklog");
		raddnum("ticks", n);
		raddnum("songend_tick", end_tick);
		if (TimerBase) {
			/* microseconds; a tick at 125 BPM is 20000 us */
			ULONG avg = bench_n ? bench_sum / bench_n : 0;
			raddnum("eclock_hz", eclock_hz);
			raddnum("bench_ticks", bench_n);
			raddnum("bench_avg_us", (LONG)(avg * 1000UL / (eclock_hz / 1000UL)));
			raddnum("bench_max_us", (LONG)(bench_max * 1000UL / (eclock_hz / 1000UL)));
			if (!args[A_QUIET])
				Printf((CONST_STRPTR)"Replay time per tick: avg %ld us, peak %ld us (E-clock %ld Hz)\n",
					avg * 1000UL / (eclock_hz / 1000UL), bench_max * 1000UL / (eclock_hz / 1000UL), eclock_hz);
			TimerBase = NULL;
		}
		if (terr == 0)
			CloseDevice((struct IORequest *)&tr);
		if (!args[A_QUIET])
			Printf((CONST_STRPTR)"Tick log: %ld ticks written to %s (song end at tick %ld)\n",
				n, args[A_TICKLOG], end_tick);
		rc = n > 0 ? RETURN_OK : RETURN_ERROR;
		goto out;
	}
#endif

	{
		struct DateStamp t0, t1;
		LONG loops = args[A_LOOPS] ? *(LONG *)args[A_LOOPS] : 0;
		LONG secs = args[A_SECONDS] ? *(LONG *)args[A_SECONDS] : 0;
		UWORD intena0 = CUSTOM_INTENAR, intena1, dma1;
		const char *why = "ctrl-c";
		LONG el;

		if (!cwl_install()) {
			cwl_remove();
			PutStr((CONST_STRPTR)"Can't get the audio channels or CIA-B timers (in use by another program?)\n");
			raddstr("error", "install");
			goto out;
		}
		cwl_init(mod);
		raddstr("install", "ok");
		if (!args[A_QUIET])
			PutStr((CONST_STRPTR)"Playing. Press Ctrl-C to stop.\n");
		DateStamp(&t0);
		mt_Enable = 1;
		for (;;) {
			if (SetSignal(0, SIGBREAKF_CTRL_C) & SIGBREAKF_CTRL_C)
				break;
			if (loops && mt_SongEnd >= loops) { why = "loops"; break; }
			DateStamp(&t1);
			if (secs && elapsed_ticks(&t0, &t1) >= secs * 50L) { why = "seconds"; break; }
			Delay(1);
		}
		DateStamp(&t1);
		el = elapsed_ticks(&t0, &t1);
		raddnum("songend", mt_SongEnd);
		raddnum("e8", mt_E8Trigger);
		mt_Enable = 0;
		cwl_end();
		cwl_remove();
		intena1 = CUSTOM_INTENAR;
		dma1 = CUSTOM_DMACONR;
		raddstr("stopped_by", why);
		raddnum("elapsed_ticks50", el);
		raddnum("intena_before", intena0);
		raddnum("intena_after", intena1);
		raddnum("audio_dma_after", dma1 & 0x000f);
		if (!args[A_QUIET])
			Printf((CONST_STRPTR)"Stopped (%s) after %ld.%02ld s. Audio and interrupts released.\n",
				(ULONG)why, el / 50, (el % 50) * 2);
		rc = RETURN_OK;
	}

out:
	if (rc == RETURN_OK)
		raddstr("status", "ok");
	else
		raddstr("status", "fail");
	write_result((const char *)args[A_RESULT]);
	if (mod)
		FreeMem(mod, size);
	FreeArgs(rda);
	return (int)rc;
}
