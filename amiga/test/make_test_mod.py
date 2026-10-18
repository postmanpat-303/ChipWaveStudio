#!/usr/bin/env python3
# ChipWave Lite - generate the M1 test module "cwlite-test.mod"
# Copyright (C) 2026 Del and the ChipWave contributors. GPL-3.0-or-later (this script).
#
# The generated module ("Paula Steps", composed and synthesised by this script
# for ChipWave Lite) is dedicated to the public domain under CC0 1.0:
# https://creativecommons.org/publicdomain/zero/1.0/
#
# It is a short A-minor groove that also exercises every ProTracker 2.3
# effect: 0 1 2 3 4 5 6 7 9 A B C D E0..EF F (speed and tempo).
# Position 3 (pattern 4) is skipped by a B jump, so if it is ever heard or
# logged, the jump is broken.
import math, random, struct, sys

RATE = 8287  # Hz at period 428 (C-2 in ProTracker notation)
random.seed(1984)

def clip8(x):
    return max(-128, min(127, int(round(x))))

def even(b):
    return b if len(b) % 2 == 0 else b + [0]

def s_kick():
    out, ph = [], 0.0
    n = 2600
    for i in range(n):
        t = i / RATE
        f = 45 + 140 * math.exp(-t * 28)
        ph += 2 * math.pi * f / RATE
        env = math.exp(-t * 9)
        click = 60 * math.exp(-t * 400)
        out.append(clip8(118 * env * math.sin(ph) + click))
    return out

def s_snare():
    out, lp = [], 0.0
    for i in range(2800):
        t = i / RATE
        nz = random.uniform(-1, 1)
        lp = lp * 0.4 + nz * 0.6
        tone = math.sin(2 * math.pi * 190 * t) * math.exp(-t * 30)
        out.append(clip8(100 * (lp * math.exp(-t * 11) + 0.6 * tone)))
    return out

def s_hat():
    out, prev = [], 0.0
    for i in range(700):
        t = i / RATE
        nz = random.uniform(-1, 1)
        hp = nz - prev
        prev = nz
        out.append(clip8(70 * hp * math.exp(-t * 45)))
    return out

def cycle(n, fn):
    return [clip8(fn(i / n)) for i in range(n)]

def s_bass():   # 64-sample saw cycle with a soft corner: one octave below C-2 pitch
    return cycle(64, lambda x: 100 * (1 - 2 * x) * (1 - 0.15 * math.sin(2 * math.pi * x)))

def s_lead():   # 32-sample pulse, 30 % duty
    return cycle(32, lambda x: 90 if x < 0.3 else -90)

def s_pad():    # 256 samples = 4 cycles of two slightly detuned saws
    def f(x):
        a = (4 * x) % 1.0
        b = (4.06 * x) % 1.0
        return 55 * ((1 - 2 * a) + (1 - 2 * b))
    return cycle(256, f)

def s_bell():   # one-shot FM-ish bell, looped tail
    out = []
    for i in range(3000):
        t = i / RATE
        m = math.sin(2 * math.pi * 3.5 * 261.6 * t) * 2.5 * math.exp(-t * 3)
        out.append(clip8(100 * math.exp(-t * 2.2) * math.sin(2 * math.pi * 261.6 * t + m)))
    return out

# name, data, volume, finetune, loop start, loop length (in bytes, 0 = no loop)
SAMPLES = [
    ("Kick", s_kick(), 64, 0, 0, 0),
    ("Snare", s_snare(), 56, 0, 0, 0),
    ("Hat", s_hat(), 40, 0, 0, 0),
    ("Bass saw", s_bass(), 48, 0, 0, 64),
    ("Lead pulse", s_lead(), 40, 0, 0, 32),
    ("Pad", s_pad(), 36, 0, 0, 256),
    ("Bell", s_bell(), 44, 3, 2000, 1000),
    ("Paula Steps - CC0 test", [0, 0], 0, 0, 0, 0),
    ("ChipWave Lite M1", [0, 0], 0, 0, 0, 0),
]

# ProTracker period table, finetune 0, C-1..B-3
PERIODS = [856, 808, 762, 720, 678, 640, 604, 570, 538, 508, 480, 453,
           428, 404, 381, 360, 339, 320, 302, 285, 269, 254, 240, 226,
           214, 202, 190, 180, 170, 160, 151, 143, 135, 127, 120, 113]
NAMES = ["C-", "C#", "D-", "D#", "E-", "F-", "F#", "G-", "G#", "A-", "A#", "B-"]

def per(note):
    """'A-1' -> period"""
    nm, octv = note[:2], int(note[2])
    return PERIODS[(octv - 1) * 12 + NAMES.index(nm)]

def empty_pattern():
    return [[None] * 4 for _ in range(64)]

def put(p, row, ch, note=None, smp=0, fx=0, prm=0):
    p[row][ch] = (per(note) if note else 0, smp, fx, prm)

KICK, SNARE, HAT, BASS, LEAD, PAD, BELL = 1, 2, 3, 4, 5, 6, 7

def drums(p, fill=False):
    for r in range(0, 64, 8):
        put(p, r, 0, "C-2", KICK)
        put(p, r + 4, 0, "C-2", SNARE)
    put(p, 30, 0, "C-2", KICK)
    if fill:
        put(p, 62, 0, "C-2", SNARE, 0xE, 0x93)       # E9x retrigger
    for r in range(2, 64, 4):
        if p[r][1] is None:
            put(p, r, 1, "C-3", HAT, 0xC, 0x28)
    for r in range(0, 64, 4):
        if p[r][1] is None:
            put(p, r, 1, "C-3", HAT, 0xC, 0x14)

BASSLINE = ["A-1", "A-1", "C-2", "A-1", "F-1", "F-1", "G-1", "G-1"]

def bass(p):
    for i, r in enumerate(range(0, 64, 8)):
        put(p, r, 2, BASSLINE[i], BASS)
        put(p, r + 6, 2, BASSLINE[i], BASS, 0xC, 0x20)

# ---------------------------------------------------------------- patterns
P = [empty_pattern() for _ in range(5)]

# pattern 0: groove + 0 (arpeggio), 1/2 (slides), 3 (portamento), 4 (vibrato), A, C, F
p = P[0]
drums(p); bass(p)
put(p, 0, 1, "C-3", HAT, 0xF, 0x06)        # F06 speed 6
put(p, 0, 2, "A-1", BASS, 0xF, 0x7D)       # F7D tempo 125
put(p, 14, 2, "C-2", BASS, 0x3, 0x08)      # 3xx tone portamento
put(p, 15, 2, None, 0, 0x3, 0x00)          # 300 continue
put(p, 30, 2, None, 0, 0x1, 0x02)          # 1xx slide up
put(p, 46, 2, None, 0, 0x2, 0x03)          # 2xx slide down
mel = [(0, "A-2"), (8, "C-3"), (12, "E-3"), (16, "D-3"), (24, "C-3"), (28, "B-2"),
       (32, "A-2"), (40, "E-3"), (48, "G-2"), (56, "A-2")]
for r, n in mel:
    put(p, r, 3, n, LEAD)
put(p, 2, 3, None, 0, 0x0, 0x37)           # 037 minor arpeggio
put(p, 3, 3, None, 0, 0x0, 0x37)
put(p, 4, 3, None, 0, 0x4, 0x46)           # 4xy vibrato
put(p, 5, 3, None, 0, 0x4, 0x00)
put(p, 6, 3, None, 0, 0xA, 0x02)           # A0y volume slide down
put(p, 18, 3, None, 0, 0x4, 0x38)
put(p, 20, 3, None, 0, 0xA, 0x20)          # Ax0 volume slide up
put(p, 34, 3, None, 0, 0x0, 0x47)          # 047 major arpeggio
put(p, 35, 3, None, 0, 0x0, 0x47)
put(p, 50, 3, None, 0, 0x4, 0x5A)

# pattern 1: 5 6 7 9, E1 E2 E3 E4 E5 E6 E7 E8 E9 EA EB EC ED, D (break)
p = P[1]
drums(p, fill=True); bass(p)
put(p, 4, 0, "C-2", SNARE, 0x9, 0x04)      # 9xx sample offset
put(p, 2, 2, None, 0, 0xE, 0x12)           # E1x fine slide up
put(p, 3, 2, None, 0, 0xE, 0x22)           # E2x fine slide down
put(p, 9, 2, None, 0, 0xE, 0x31)           # E3x glissando on
put(p, 10, 2, "C-2", BASS, 0x3, 0x06)      # with tone portamento
put(p, 11, 2, None, 0, 0x5, 0x01)          # 5xy porta + volume slide
put(p, 12, 2, None, 0, 0x5, 0x00)
put(p, 13, 2, None, 0, 0xE, 0x30)          # glissando off
put(p, 0, 3, "E-3", LEAD, 0xE, 0x41)       # E4x vibrato waveform ramp
put(p, 1, 3, None, 0, 0x4, 0x48)
put(p, 2, 3, None, 0, 0x6, 0x01)           # 6xy vibrato + volume slide
put(p, 3, 3, None, 0, 0x6, 0x00)
put(p, 8, 3, "A-2", PAD, 0xE, 0x72)        # E7x tremolo waveform square
put(p, 9, 3, None, 0, 0x7, 0x68)           # 7xy tremolo
put(p, 10, 3, None, 0, 0x7, 0x00)
put(p, 11, 3, None, 0, 0x7, 0x00)
put(p, 16, 3, "C-3", BELL, 0xE, 0x58)      # E5x set finetune
put(p, 20, 3, None, 0, 0xE, 0xA4)          # EAx fine volume up
put(p, 22, 3, None, 0, 0xE, 0xB8)          # EBx fine volume down
put(p, 24, 3, "E-3", LEAD, 0xE, 0x60)      # E60 loop start
put(p, 25, 3, "G-3", LEAD)
put(p, 26, 3, "A-3", LEAD, 0xE, 0x62)      # E62 loop rows 24-26 twice more
put(p, 28, 3, "A-2", LEAD, 0xE, 0x81)      # E8x trigger (_mt_E8Trigger)
put(p, 26, 1, "C-3", HAT, 0xE, 0xC1)       # ECx note cut
put(p, 30, 1, "C-3", HAT, 0xE, 0xD3)       # EDx note delay
put(p, 34, 1, "C-3", HAT, 0xE, 0xC2)
put(p, 36, 3, "A-2", PAD, 0xC, 0x30)
put(p, 40, 3, "C-3", PAD, 0x4, 0x24)
put(p, 48, 1, None, 0, 0xD, 0x10)          # D10: break to row 10 of the next position
for r in range(49, 64):                    # never played (after the break)
    put(p, r, 3, "B-3", BELL, 0xC, 0x40)

# pattern 2: tempo/speed changes, EE pattern delay, E0 filter, B jump
p = P[2]
for r in range(0, 10):                     # rows 0-9 are skipped by D10
    put(p, r, 3, "B-3", BELL, 0xC, 0x40)
drums(p)
for r in range(0, 10):
    p[r][0] = None
    p[r][1] = None
put(p, 10, 0, "C-2", KICK, 0xF, 0x04)      # speed 4
put(p, 10, 1, "C-3", HAT, 0xF, 0x96)       # tempo 150
put(p, 10, 2, "A-1", BASS, 0xE, 0x00)      # E00 filter (LED) on
for i, r in enumerate(range(12, 40, 2)):
    put(p, r, 3, ["A-2", "C-3", "E-3", "A-3", "G-3", "E-3", "D-3"][i % 7], LEAD, 0xC, 0x30 - i)
put(p, 16, 2, "C-2", BASS, 0xF, 0x03)      # speed 3
put(p, 24, 2, "G-1", BASS, 0xE, 0xE2)      # EEx pattern delay 2 rows
put(p, 32, 2, "A-1", BASS, 0xF, 0x06)      # speed 6
put(p, 32, 1, "C-3", HAT, 0xF, 0x7D)       # tempo 125
put(p, 36, 2, None, 0, 0xE, 0x01)          # E01 filter off
put(p, 40, 2, None, 0, 0xB, 0x04)          # B04: jump to position 4
for r in range(41, 64):
    put(p, r, 3, "B-3", BELL, 0xC, 0x40)   # never played

# pattern 4: placed at position 3 and must be skipped by B04
p = P[4]
for r in range(64):
    put(p, r, r % 4, "B-3", BELL, 0xC, 0x40)

# pattern 3: outro, fades out, ends with a cut
p = P[3]
drums(p); bass(p)
for r, n in [(0, "A-2"), (16, "C-3"), (32, "E-3"), (48, "A-3")]:
    put(p, r, 3, n, PAD)
put(p, 1, 3, None, 0, 0x4, 0x34)
put(p, 40, 2, None, 0, 0xA, 0x01)
put(p, 52, 3, None, 0, 0xA, 0x02)
put(p, 56, 1, None, 0, 0xA, 0x04)
for c in range(4):
    put(p, 63, c, None, 0, 0xE, 0xC0)      # EC0 cut everything

ORDER = [0, 1, 2, 4, 3]

# ---------------------------------------------------------------- write
def build():
    out = bytearray()
    out += b"ChipWave Lite M1".ljust(20, b"\0")
    data = []
    for i in range(31):
        if i < len(SAMPLES):
            name, d, vol, ft, ls, ll = SAMPLES[i]
            d = even(list(d))
            if ll == 0:
                d[0:2] = [0, 0]
                rs, rl = 0, 1
            else:
                rs, rl = ls // 2, ll // 2
            out += name.encode()[:22].ljust(22, b"\0")
            out += struct.pack(">HBBHH", len(d) // 2, ft & 15, vol, rs, rl)
            data.append(bytes((v & 0xFF) for v in d))
        else:
            out += b"\0" * 22 + struct.pack(">HBBHH", 0, 0, 0, 0, 1)
    out += bytes([len(ORDER), 127])
    out += bytes(ORDER + [0] * (128 - len(ORDER)))
    out += b"M.K."
    for pi in range(max(ORDER) + 1):
        for r in range(64):
            for c in range(4):
                cell = P[pi][r][c]
                if cell is None:
                    out += b"\0\0\0\0"
                else:
                    pr, s, fx, prm = cell
                    out += bytes([(s & 0xF0) | (pr >> 8), pr & 0xFF, ((s & 15) << 4) | fx, prm])
    for d in data:
        out += d
    return bytes(out)

if __name__ == "__main__":
    path = sys.argv[1] if len(sys.argv) > 1 else "cwlite-test.mod"
    m = build()
    open(path, "wb").write(m)
    print(f"{path}: {len(m)} bytes")
