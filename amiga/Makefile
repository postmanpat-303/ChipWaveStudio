# ChipWave Lite (Amiga) - Makefile
# Copyright (C) 2026 Del and the ChipWave contributors. GPL-3.0-or-later.
#
# Builds two programs from the same source:
#   build/ChipWaveLite      68020+ (A1200 and up)
#   build/ChipWaveLite.000  68000  (A500/A600/A2000, OCS/ECS)
#
#   make                 both CPU builds
#   make test            headless tests (vamos + FS-UAE/AROS, A500 and A1200)
#   make dist            bootable ADF + LHA in dist/
#
# Needs m68k-amigaos-gcc (AmigaPorts/bebbo) and vasm (Motorola syntax).

AMIGA_GCC ?= /workspace/amiga-spike/toolchain/m68k-amigaos-gcc-16.2
CC        := $(AMIGA_GCC)/bin/m68k-amigaos-gcc
VASM      ?= $(firstword $(wildcard $(AMIGA_GCC)/bin/vasmm68k_mot) /workspace/amiga-spike/vasm/vasmm68k_mot)

B         := build
CFLAGS    := -Os -noixemul -Wall -Wno-pointer-sign -fomit-frame-pointer -Isrc
LDFLAGS   := -noixemul -s
# ptplayer: OS-friendly (audio.device + cia.resource), CIA timed
PTFLAGS   := -DOSCOMPAT=1
# tick-log copy: no timers, Paula/CIAs replaced by RAM shadows
PTLOGFLAGS := -DNO_TIMERS=1 -DOSCOMPAT=0
VFLAGS    := -Fhunk -quiet -nosym -no-opt

all: $(B)/ChipWaveLite $(B)/ChipWaveLite.000

$(B)/020 $(B)/000:
	mkdir -p $@

$(B)/ptplayer_log.asm: src/replay/ptplayer.asm src/replay/mklog.py | $(B)/020
	python3 src/replay/mklog.py $< > $@

# --- per-CPU objects ---------------------------------------------------
define cpu_rules
$(B)/$(1)/main.o: src/main.c src/core/mod.h src/replay/replay.h | $(B)/$(1)
	$(CC) $(CFLAGS) -m$(2) $(3) -c $$< -o $$@
$(B)/$(1)/mod.o: src/core/mod.c src/core/mod.h | $(B)/$(1)
	$(CC) $(CFLAGS) -m$(2) -c $$< -o $$@
$(B)/$(1)/ptplayer.o: src/replay/ptplayer.asm | $(B)/$(1)
	$(VASM) -m$(2) $(VFLAGS) $(PTFLAGS) -o $$@ $$<
$(B)/$(1)/ptplayer_log.o: $(B)/ptplayer_log.asm | $(B)/$(1)
	$(VASM) -m$(2) $(VFLAGS) $(PTLOGFLAGS) -o $$@ $$<
$(B)/$(1)/replay.o: src/replay/replay.s | $(B)/$(1)
	$(VASM) -m$(2) $(VFLAGS) -o $$@ $$<
endef
$(eval $(call cpu_rules,020,68020,))
$(eval $(call cpu_rules,000,68000,-DCWL_68000))

OBJS = main.o mod.o replay.o ptplayer.o ptplayer_log.o

$(B)/ChipWaveLite: $(addprefix $(B)/020/,$(OBJS))
	$(CC) -m68020 $(LDFLAGS) $^ -o $@

$(B)/ChipWaveLite.000: $(addprefix $(B)/000/,$(OBJS))
	$(CC) -m68000 $(LDFLAGS) $^ -o $@

# --- test song ----------------------------------------------------------
test/cwlite-test.mod: test/make_test_mod.py
	python3 $< $@

test: all test/cwlite-test.mod
	sh test/run_tests.sh

dist: all test/cwlite-test.mod
	sh pkg/make_dist.sh

clean:
	rm -rf $(B)

.PHONY: all test dist clean

# --- test helper ---------------------------------------------------------
$(B)/CWBreak: test/cwbreak.c | $(B)/000
	$(CC) $(CFLAGS) -m68000 $(LDFLAGS) $< -o $@
all: $(B)/CWBreak
