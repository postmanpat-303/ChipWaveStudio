#!/usr/bin/env python3
"""Generate demos/chrome_throat.cwt.toml, the psy / dark trance demo "Chrome Throat".

Psy / dark trance in F# Phrygian, 145 BPM, about 4:30. Simple and rhythmic: a
four-on-the-floor kick with the classic rolling psy bass (kick + three bass 16ths),
offbeat hats only, quiet claps on 2 and 4, a 303-style acid line (glide 3Cxx, accent
3Dxx, cutoff 38xx-3Bxx) and lots of voices, all from the Voice instrument: a throat
chant, a vowel drone, whispered consonant stabs and chopped syllables.

The centrepiece is the "liquid chrome" breakdown (the mirror scene): one Throat-mode
voice sings an open "aa", then is gradually swallowed: the vowels close (A > O > U),
gurgled words (gu, ugu, ngu) re-spoken on the held note (3Exx), a 1/64 chop gate and
babble (37xy / 36xy) for the gargle, the tract lengthened (formant shift 32xx down to
x0.5) while the Transform blend (3Fxx), metal resonators (40xx), ring-mod crush (41xx)
and sub (42xx) rise, so the formants slide down into a metallic gurgle. Its FX chain
adds a note-tuned Metal resonator (comb-like ring), a Frequency shifter sliding down
with feedback and a 12-stage Phaser, all automated. Over it a Wave wavetable "chrome
shimmer" rises (table position, cutoff and an upward barber-pole frequency shift).
It builds into the drop and comes back, shorter, in the second groove and the outro.

Drag-to-modulate LFOs ([modulation]) move the acid cutoff, the drone's phaser and the
chop voice's filter; automation lanes do the long sweeps. Master limiter, about
-14 LUFS. Notes: docs/research/chrome-throat.md.

  python3 scripts/make_chrome_throat.py > demos/chrome_throat.cwt.toml
  DEMO_SOLO=10 python3 scripts/make_chrome_throat.py > /tmp/x.toml   (debug: only these channels)
"""
import os

ROWS = 64
B = 16  # rows per bar
BPM = 145
(KICK, BASS, HAT, PERC, ACID, CHANT, DRONE, STAB, CHOP, THROAT, SHIM, PAD, FX) = range(13)
CH = 13
(I_KICK, I_BASS, I_HAT, I_CLAP, I_SNARE, I_ACID, I_CHANT, I_DRONE, I_WHISP, I_CHOP,
 I_THROAT, I_SHIM, I_PAD, I_RISE, I_IMPACT, I_DOWN, I_CRASH) = range(1, 18)


class Block:
    def __init__(self, name, role):
        self.name, self.role = name, role
        self.cells = [[None] * CH for _ in range(ROWS)]

    def put(self, row, ch, note, ins=None, vol=None, fx=None):
        if 0 <= row < ROWS:
            self.cells[row][ch] = (note, ins, vol, fx)

    def fx(self, row, ch, fx):
        """effect only (keeps a note already there)"""
        if not 0 <= row < ROWS:
            return
        c = self.cells[row][ch]
        self.cells[row][ch] = (c[0], c[1], c[2], fx) if c else ("---", None, None, fx)

    def off(self, row, ch):
        self.put(row, ch, "===")

    def text(self):
        solo = os.environ.get("DEMO_SOLO")
        keep = {int(c) - 1 for c in solo.split(",")} if solo else set(range(CH))
        out = []
        for r, row in enumerate(self.cells):
            row = [c if k in keep else None for k, c in enumerate(row)]
            if all(c is None for c in row):
                continue
            parts = []
            for c in row:
                if c is None:
                    parts.append("--- .. .. ....")
                else:
                    n, i, v, f = c
                    parts.append(f"{n} {('%02X' % i) if i else '..'} {('%02X' % v) if v is not None else '..'} {f or '....'}")
            out.append("%02X | " % r + " | ".join(parts))
        return "\n".join(out)


_rng = [145]


def rnd(n):
    _rng[0] = (_rng[0] * 1103515245 + 12345) & 0x7FFFFFFF
    return (_rng[0] >> 16) % n


def jitter(v, amt=2):
    return max(1, min(0x40, v + rnd(2 * amt + 1) - amt))


def ramp(a, b, k, n):
    return int(round(a + (b - a) * k / max(1, n - 1)))


# ---- drums -------------------------------------------------------------------------------
def kick(b, bars=range(4), skip=(), vol=0x3E):
    for bar in bars:
        for beat in range(4):
            if (bar, beat) not in skip:
                b.put(bar * B + beat * 4, KICK, "A-2", I_KICK, vol)


def hats(b, bars=range(4), vol=0x22, skip=()):
    """house rule: hats on the offbeat only (rows 2, 6, 10, 14)"""
    for bar in bars:
        for beat in range(4):
            if (bar, beat) not in skip:
                b.put(bar * B + beat * 4 + 2, HAT, "C-5", I_HAT, jitter(vol, 2))


def claps(b, bars=range(4), vol=0x20):
    for bar in bars:
        for beat in (1, 3):
            b.put(bar * B + beat * 4, PERC, "C-4", I_CLAP, vol)


def snare_roll(b, start_bar, bars=2, lo=0x10, hi=0x34):
    rows = range(start_bar * B, (start_bar + bars) * B)
    for k, r in enumerate(rows):
        f = k / max(1, len(rows) - 1)
        step = 4 if f < 0.4 else (2 if f < 0.75 else 1)
        if r % step == 0:
            b.put(r, PERC, "C-4", I_SNARE, int(lo + (hi - lo) * f))


# ---- bass: kick + three rolling 16ths (F# pedal, phrygian G / A / D turns) -------------


def bass(b, bars=range(4), vol=0x3A, turns=True, skip=()):
    for bar in bars:
        for beat in range(4):
            if (bar, beat) in skip:
                continue
            for s in (1, 2, 3):
                b.put(bar * B + beat * 4 + s, BASS, "F#1", I_BASS, vol - (0 if s == 1 else 3))
        if turns and bar == 3:
            for r, n in ((13, "G-1"), (14, "F#1"), (15, "A-1")):
                if (bar, 3) not in skip:
                    b.put(bar * B + r, BASS, n, I_BASS, vol - 2)


# ---- 303-style acid: glide 3C01, accent 3D, filter by 38 / 3A / 3B ------------------------
# (step, note, effect) per bar: "G" = glide, "A" = accent
RIFF_A = [(0, "F#3", "A"), (1, "F#3", None), (2, "F#4", "G"), (3, "F#3", None), (5, "G-3", "G"),
          (6, "F#3", None), (7, "A-3", None), (8, "F#3", "A"), (9, "C#4", "G"), (10, "F#3", None),
          (12, "E-3", None), (13, "F#3", "G"), (14, "G-3", "A"), (15, "F#3", None)]
RIFF_B = [(0, "F#3", "A"), (2, "F#3", None), (3, "A-3", "G"), (4, "G-3", None), (6, "F#3", "A"),
          (7, "F#4", "G"), (8, "F#3", None), (10, "D-4", "A"), (11, "C#4", "G"), (12, "F#3", None),
          (14, "E-4", "G"), (15, "F#3", "A")]


def acid(b, bars=range(4), riff=RIFF_A, vol=0x34, sweep=None):
    """sweep: (cutoff_byte_start, slide command for the bar ends) e.g. (0x70, '3A06')"""
    for bar in bars:
        for s, n, f in riff:
            fx = {"G": "3C01", "A": "3D30"}.get(f) if f else None
            b.put(bar * B + s, ACID, n, I_ACID, jitter(vol, 1) + (4 if f == "A" else 0), fx)
    if sweep:
        start, slide = sweep
        first = bars[0] * B
        # 38 sets the start on an empty step, the slide runs on later empty steps
        for r in range(first, (bars[-1] + 1) * B):
            c = b.cells[r][ACID]
            if c is None or c[3] is None:
                if r == first + 4:
                    b.fx(r, ACID, "38%02X" % start)
                elif r > first + 4 and r % 16 == 11:
                    b.fx(r, ACID, slide)


# ---- voices -------------------------------------------------------------------------------
def chant(b, bars=(0, 2), vol=0x34, words=(0, 1), note="F#3", answer="A-3"):
    """throat chant: 'om' on the bar, answered by 'na' half a bar later"""
    for bar in bars:
        r = bar * B
        b.put(r, CHANT, note, I_CHANT, vol, "3E%02X" % words[0])
        b.off(r + 6, CHANT)
        b.put(r + 8 + 16, CHANT, answer, I_CHANT, vol - 4, "3E%02X" % words[1]) if r + 24 < ROWS else None
        b.off(r + 30, CHANT) if r + 30 < ROWS else None


def drone(b, note="F#2", vol=0x2C, vowel="3000"):
    b.put(0, DRONE, note, I_DRONE, vol, vowel)


def stabs(b, bars=range(4), vol=0x30, rows=(3, 11), every=1):
    """whispered consonant stabs off the grid of the hats (rows 3 / 11)"""
    for bar in bars:
        if bar % every:
            continue
        for k, rr in enumerate(rows):
            r = bar * B + rr
            b.put(r, STAB, "C#4" if k == 0 else "F#4", I_WHISP, jitter(vol, 2), "3E%02X" % ((bar * 2 + k) % 5))
            b.off(r + 2, STAB)


def chop(b, bar=3, vol=0x30, stutter=True):
    """chopped syllables: 16ths da-ki-ta-ka on the last bar, stutter on the tail"""
    words = [0, 1, 2, 3, 0, 1, 2, 4, 0, 2, 1, 3, 5, 5, 5, 5]
    notes = ["C#4", "C#4", "F#4", "C#4", "C#4", "E-4", "C#4", "D-4"] * 2
    for s in range(16):
        r = bar * B + s
        if s in (6, 14):
            continue
        b.put(r, CHOP, notes[s], I_CHOP, jitter(vol, 2), "3E%02X" % words[s])
    if stutter:
        b.fx(bar * B + 14, CHOP, "3333")
    b.off(bar * B + 15 + 1, CHOP) if bar * B + 16 < ROWS else None


def chop_short(b, bar, vol=0x2C):
    for s, w in ((8, 0), (9, 1), (10, 2), (12, 3)):
        b.put(bar * B + s, CHOP, "C#4", I_CHOP, vol, "3E%02X" % w)
    b.off(bar * B + 14, CHOP)


# ---- the chrome throat (channel THROAT) ---------------------------------------------------
# words: 0 aa, 1 o, 2 u, 3 gu, 4 ugu, 5 ngu, 6 gugu, 7 uk (a gulp: ends closed)
def throat_line(b, cmds):
    """cmds: {row: fx} on the held throat note"""
    for r, f in cmds.items():
        b.fx(r, THROAT, f)


def shimmer(b, bars=range(4), step=4, vol=0x22, up=0, notes=("F#5", "C#6", "A-5", "E-6", "G-5", "D-6")):
    """chrome shimmer: glassy wavetable arpeggio (8ths / 16ths), rising"""
    k = 0
    for bar in bars:
        for s in range(0, 16, step):
            n = notes[k % len(notes)]
            if up and bar >= 2:
                n = n[:-1] + str(int(n[-1]) + 1)
            b.put(bar * B + s, SHIM, n, I_SHIM, jitter(vol, 2))
            k += 1


def fxhit(b, row, ins, vol):
    b.put(row, FX, "C-4", ins, vol)


blocks = []


def add(b):
    blocks.append(b)
    return len(blocks) - 1


# ================= intro (16 bars) =================
b = Block("Intro", "intro"); kick(b); hats(b, bars=(2, 3), vol=0x1C); drone(b, vol=0x28)
fxhit(b, 0, I_CRASH, 0x22); stabs(b, bars=(1, 3), vol=0x26, rows=(11,)); IN_A = add(b)
b = Block("Intro chant", "intro"); kick(b); hats(b, vol=0x1E); drone(b, vowel="3010", vol=0x28)
chant(b, vol=0x30); stabs(b, bars=(1, 3), vol=0x28, rows=(11,)); IN_B = add(b)
b = Block("Intro bass", "intro"); kick(b); hats(b); claps(b, vol=0x1A); bass(b, vol=0x34, turns=False)
drone(b, vol=0x26); chant(b, vol=0x30); IN_C = add(b)
b = Block("Intro turn", "intro"); kick(b, skip={(3, 3)}); hats(b); claps(b, vol=0x1A)
bass(b, vol=0x36, skip={(3, 3)}); chant(b, bars=(0,), vol=0x30); chop(b, 3, vol=0x2A)
fxhit(b, 32, I_RISE, 0x2A); IN_D = add(b)

# ================= build (16 bars): the acid comes in, filtered =================
b = Block("Build acid", "build"); kick(b); hats(b); claps(b); bass(b)
acid(b, vol=0x30, sweep=(0x60, "3A03")); chant(b, vol=0x30); fxhit(b, 0, I_IMPACT, 0x2C); BU_A = add(b)
b = Block("Build acid 2", "build"); kick(b); hats(b); claps(b); bass(b)
acid(b, vol=0x30, sweep=(0x70, "3A03")); stabs(b, every=2); chop_short(b, 3); BU_B = add(b)
b = Block("Build roll", "build"); kick(b, bars=range(3)); kick(b, bars=(3,), skip={(3, 3)})
hats(b); bass(b, bars=range(3)); acid(b, vol=0x32, sweep=(0x78, "3A08")); snare_roll(b, 2, 2)
fxhit(b, 0, I_RISE, 0x32); chant(b, bars=(0,), vol=0x30); BU_C = add(b)

# ================= groove 1 (32 bars) =================
def groove(name, riff=RIFF_A, sweep=(0x90, "3A02"), chopbar=None, turn=False, chants=True,
           stab_every=1, impact=False, squelch=False, shim=False, throat_stabs=False):
    d = Block(name, "verse")
    sk = {(3, 3)} if turn else set()
    kick(d, skip=sk); hats(d); claps(d); bass(d, skip=sk)
    acid(d, riff=riff, sweep=sweep)
    if squelch:     # resonance up, slides down then up: the squelch
        d.fx(16 + 4, ACID, "39E0"); d.fx(32 + 4, ACID, "3B04"); d.fx(48 + 4, ACID, "3A06")
    if chants:
        chant(d, vol=0x30, words=(0, 2) if riff is RIFF_A else (1, 3))
    stabs(d, every=stab_every)
    if chopbar is not None:
        chop(d, chopbar)
    if impact:
        fxhit(d, 0, I_IMPACT, 0x30)
    if turn:
        fxhit(d, 48, I_DOWN, 0x2C)
    if shim:
        shimmer(d, step=2, vol=0x1A)
    if throat_stabs:   # the chrome voice comes back as short metallic gulps on the 'a's
        for bar in range(4):
            for rr, w in ((7, 4), (15, 3)):
                if bar == 3 and rr == 15 and turn:
                    continue
                d.put(bar * B + rr, THROAT, "F#3", I_THROAT, 0x3C, "3E%02X" % w)
                d.fx(bar * B + rr + 1, THROAT, "3FE0")
                d.off(bar * B + rr + 3, THROAT) if bar * B + rr + 3 < ROWS else None
    return d


G_A = add(groove("Groove", impact=True)); G_B = add(groove("Groove chop", chopbar=3))
G_C = add(groove("Groove squelch", riff=RIFF_B, squelch=True, stab_every=2))
G_D = add(groove("Groove turn", chopbar=None, turn=True))

# ================= chrome breakdown (24 bars) =================
TH = {}
# C1 "Mirror": no kick; drone, pad, the voice sings an open 'aa'; first glints of shimmer
b = Block("Chrome mirror", "breakdown"); drone(b, vol=0x2A, vowel="3000"); b.put(0, PAD, "F#3", I_PAD, 0x30)
fxhit(b, 0, I_DOWN, 0x30); b.put(0, THROAT, "F#3", I_THROAT, 0x34, "3E00")
throat_line(b, {2: "3280", 4: "3F00", 32: "3E00", 40: "3011"})
for r in (16, 28, 40, 52, 58):
    b.put(r, SHIM, ["F#6", "C#6", "A-5", "E-6", "C#6"][(r // 6) % 5], I_SHIM, 0x16)
chant(b, bars=(0,), vol=0x2C)
C1 = add(b)
# C2 "Touch": the vowel closes A > O > U, formants start to slide, transform creeps in
b = Block("Chrome touch", "breakdown"); drone(b, vol=0x2A, vowel="3030"); b.put(0, PAD, "F#3", I_PAD, 0x30)
b.put(0, THROAT, "F#3", I_THROAT, 0x34, "3E01")
cm = {}
for k in range(16):
    r = 2 + k * 4
    cm[r] = "32%02X" % ramp(0x80, 0x64, k, 16)
    cm[r + 1] = "3F%02X" % ramp(0x10, 0x60, k, 16)
cm[16] = "3022"; cm[32] = "3E02"; cm[48] = "3043"; cm[49] = "4030"
throat_line(b, cm); shimmer(b, step=4, vol=0x1A)
C2 = add(b)
# C3 "Spread": gurgled words, the 1/64 chop gate (gargle), metal resonators rising
b = Block("Chrome spread", "breakdown"); drone(b, vol=0x28, vowel="3040"); b.put(0, PAD, "F#3", I_PAD, 0x2E)
b.put(0, THROAT, "F#3", I_THROAT, 0x36, "3E03")
cm = {}
for k in range(16):
    r = 1 + k * 4
    cm[r] = "32%02X" % ramp(0x62, 0x40, k, 16)
    cm[r + 1] = "3F%02X" % ramp(0x68, 0xB0, k, 16)
    cm[r + 2] = "40%02X" % ramp(0x40, 0x90, k, 16)
for r, w in ((8, 4), (16, 3), (24, 4), (32, 5), (40, 4), (44, 3), (48, 6), (56, 4), (60, 6)):
    cm[r] = "3E%02X" % w
cm[12] = "371A"; cm[36] = "3628"; cm[52] = "371C"
throat_line(b, cm); shimmer(b, step=2, vol=0x1C)
b.put(32, BASS, "F#1", I_BASS, 0x20); b.off(34, BASS)
C3 = add(b)
# C4 "Swallow": the throat closes; formants slide to x0.5, babble + breath, sub rises,
# half-time kick thumps and the filtered bass come back
b = Block("Chrome swallow", "breakdown"); b.put(0, PAD, "F#3", I_PAD, 0x2C)
b.put(0, THROAT, "F#3", I_THROAT, 0x38, "3E05")
cm = {}
for k in range(16):
    r = 1 + k * 4
    cm[r] = "32%02X" % ramp(0x3E, 0x00, k, 16)
    cm[r + 1] = "3F%02X" % ramp(0xB0, 0xFF, k, 16)
    cm[r + 2] = "42%02X" % ramp(0x20, 0x90, k, 16)
for r, w in ((4, 6), (8, 4), (12, 5), (16, 6), (20, 4), (24, 6), (28, 5), (32, 6), (36, 4), (40, 6),
             (44, 5), (48, 6), (52, 6), (56, 7)):
    cm[r] = "3E%02X" % w
cm[0] = "3E05"; cm[18] = "362C"; cm[34] = "3530"; cm[50] = "371B"
throat_line(b, cm); shimmer(b, step=2, vol=0x1E, up=1)
for bar in range(4):
    b.put(bar * B, KICK, "A-2", I_KICK, 0x36)
    if bar >= 2:
        b.put(bar * B + 8, KICK, "A-2", I_KICK, 0x32)
bass(b, bars=(2, 3), vol=0x2C, turns=False)
fxhit(b, 32, I_RISE, 0x2A)
C4 = add(b)
# C5 "Pour": the chrome pours in: four-on-the-floor again, snare roll, the lowest metallic
# gurgle, shimmer an octave up, acid hint sliding open
b = Block("Chrome pour", "build"); kick(b); hats(b, bars=(2, 3), vol=0x1C)
bass(b, vol=0x30, turns=False); b.put(0, THROAT, "F#2", I_THROAT, 0x38, "3E06")
cm = {}
for bar in range(4):
    for s, w in ((0, 6), (4, 4), (8, 5), (12, 6)):
        cm[bar * B + s] = "3E%02X" % w
    cm[bar * B + 1] = "3200"; cm[bar * B + 2] = "3FFF"; cm[bar * B + 3] = "40%02X" % (0xA0 + bar * 0x10)
    cm[bar * B + 6] = "371%X" % (0xC - bar)
throat_line(b, cm); shimmer(b, step=1, vol=0x1E, up=1)
acid(b, bars=(2, 3), vol=0x2C, sweep=(0x50, "3A0C")); snare_roll(b, 0, 4, lo=0x0C, hi=0x30)
fxhit(b, 0, I_RISE, 0x34)
C5 = add(b)
# C6 "Throat close": two bars of everything, then a bar and a half of only the gulping
# chrome voice and the shimmer, the last 'uk' gulp, impact on the drop
b = Block("Chrome close", "build"); kick(b, bars=(0, 1)); hats(b, bars=(0, 1)); bass(b, bars=(0, 1), turns=False)
acid(b, bars=(0, 1), vol=0x30, sweep=(0x90, "3A10")); snare_roll(b, 0, 2, lo=0x24, hi=0x3A)
b.put(0, THROAT, "F#2", I_THROAT, 0x3A, "3E06")
cm = {}
for k, r in enumerate(range(2, 40, 2)):
    cm[r] = "3E%02X" % (4 + k % 3)
cm[1] = "3FFF"; cm[3] = "40D0"; cm[5] = "4280"; cm[33] = "3628"; cm[41] = "371E"
cm[52] = "3E07"
throat_line(b, cm); b.off(60, THROAT)
shimmer(b, bars=(0, 1, 2), step=1, vol=0x20, up=1); b.off(56, SHIM)
b.off(32, BASS); b.off(32, ACID); b.off(36, PAD)
fxhit(b, 32, I_DOWN, 0x20)
C6 = add(b)

# ================= drop (32 bars): chrome voice gulps, shimmer, acid open =================
D_A = add(groove("Drop", sweep=(0xB0, "3A01"), impact=True, throat_stabs=True, shim=True))
D_B = add(groove("Drop chop", sweep=(0xA8, "3A02"), chopbar=3, throat_stabs=True))
D_C = add(groove("Drop squelch", riff=RIFF_B, sweep=(0xA0, "3A02"), squelch=True, throat_stabs=True, shim=True))
D_D = add(groove("Drop turn", sweep=(0xA0, "3B02"), turn=True, throat_stabs=True))
for i in (D_A,):
    blocks[i].put(0, FX, "C-4", I_CRASH, 0x2A)

# ================= second groove, with a short chrome return =================
G2_A = add(groove("Groove 2", riff=RIFF_B, sweep=(0x88, "3A02"), chants=True, stab_every=2))
G2_B = add(groove("Groove 2 chop", riff=RIFF_B, sweep=(0x90, "3A03"), chopbar=3))
# chrome return: kick and bass stay, the throat is swallowed again in 8 bars
b = Block("Chrome return", "breakdown"); kick(b); hats(b); bass(b, turns=False)
acid(b, bars=(0, 1), riff=RIFF_A, vol=0x2C, sweep=(0x70, "3B03"))
b.put(0, THROAT, "F#3", I_THROAT, 0x36, "3E00")
cm = {}
for k in range(16):
    r = 1 + k * 4
    cm[r] = "32%02X" % ramp(0x80, 0x30, k, 16)
    cm[r + 1] = "3F%02X" % ramp(0x20, 0xC0, k, 16)
for r, w in ((16, 1), (24, 2), (32, 3), (40, 4), (48, 5), (52, 4), (56, 6), (60, 5)):
    cm[r] = "3E%02X" % w
cm[42] = "4070"; cm[46] = "371A"
throat_line(b, cm); shimmer(b, step=2, vol=0x1C); fxhit(b, 0, I_DOWN, 0x28)
R1 = add(b)
b = Block("Chrome return gulp", "build"); kick(b, bars=range(3)); hats(b, bars=range(3)); bass(b, bars=range(3), turns=False)
b.put(0, THROAT, "F#2", I_THROAT, 0x38, "3E06")
cm = {}
for k, r in enumerate(range(2, 48, 2)):
    cm[r] = "3E%02X" % (4 + k % 3)
cm[1] = "3200"; cm[3] = "3FFF"; cm[5] = "40C0"; cm[7] = "4270"; cm[21] = "371C"; cm[49] = "3E07"
throat_line(b, cm); b.off(56, THROAT); shimmer(b, step=1, vol=0x1E, up=1); b.off(52, SHIM)
snare_roll(b, 2, 2, lo=0x14, hi=0x34); fxhit(b, 16, I_RISE, 0x30)
R2 = add(b)
G2_C = add(groove("Groove 2 squelch", riff=RIFF_A, sweep=(0xA0, "3A02"), squelch=True, impact=True, throat_stabs=True))

# ================= outro (12 bars) =================
b = Block("Outro", "outro"); kick(b); hats(b); claps(b); bass(b); acid(b, vol=0x2C, sweep=(0x70, "3B03"))
chant(b, vol=0x30); drone(b, vol=0x28); O_A = add(b)
b = Block("Outro chrome", "outro"); kick(b); hats(b); bass(b, bars=(0, 1), turns=False); drone(b, vol=0x28, vowel="3040")
b.put(0, THROAT, "F#3", I_THROAT, 0x32, "3E00")
throat_line(b, {8: "3E01", 16: "3E02", 17: "3260", 20: "3F80", 24: "3E04", 25: "3240", 32: "3E05",
                33: "3220", 34: "3FE0", 40: "3E06", 44: "371B", 48: "3E07"})
b.off(56, THROAT); stabs(b, bars=(1, 3), rows=(11,), vol=0x26); O_B = add(b)
b = Block("Outro end", "outro"); kick(b, bars=(0, 1)); hats(b, bars=(0, 1, 2), vol=0x1A)
drone(b, vol=0x26, vowel="3040"); chant(b, bars=(0,), vol=0x2C); b.off(48, DRONE)
fxhit(b, 32, I_DOWN, 0x22); O_C = add(b)

ORDER = ([IN_A, IN_B, IN_C, IN_D]                          # intro 16 bars
         + [BU_A, BU_B, BU_A, BU_C]                        # build 16 bars
         + [G_A, G_B, G_C, G_B, G_A, G_B, G_C, G_D]        # groove 32 bars
         + [C1, C2, C3, C4, C5, C6]                        # chrome breakdown 24 bars
         + [D_A, D_B, D_C, D_B, D_A, D_B, D_C, D_D]        # drop 32 bars
         + [G2_A, G2_B, R1, R2, G2_C, G2_B, G2_A, G_D]     # second groove + chrome return 32 bars
         + [O_A, O_B, O_C])                                # outro 12 bars
start = {}
for i, bk in enumerate(ORDER):
    start.setdefault(bk, []).append(i * ROWS)
total = len(ORDER) * ROWS


def at(bk, k=0):
    return start[bk][k]


MIX = os.environ.get("MIX_MASTER_DB", "0.0")
LIM = os.environ.get("MIX_LIM_GAIN", "4.2")
print(f'''# ChipWave demo "Chrome Throat" (generated by scripts/make_chrome_throat.py; edit that, not this)
# Psy / dark trance in F# Phrygian, {BPM} BPM. Single-file text form of a .cwt song (docs/FIRST_RUN.md).
# Kick + three rolling bass 16ths, offbeat hats, 303-style acid, Voice chants / drone /
# whispers / chops, and the "liquid chrome" breakdown: a Throat-mode voice swallowed into a
# metallic gurgle. Arrangement and sound design: docs/research/chrome-throat.md.
format = "chipwave-song-text"
format_version = 2

[song]
title = "Chrome Throat (psy demo)"
bpm = {BPM}
lines_per_beat = 4
speed = 6
tracks = {CH}
key = "F# phrygian"
''')
print('''[[instrument]]
slot = 1
name = "Psy Kick"
patch = "builtin:trance-kick"
drive = 0.0
cutoff = 5000.0
pitch_env = [44.0, 0.009]
decay = 0.24
[[instrument]]
slot = 2
name = "Rolling Bass"
patch = "gen:Psy Rolling Bass"
# fx 1 = HP 35 Hz; the Analog's own cutoff (gen.1) opens in the intro by automation
fx = [{ kind = "Filter", mode = 1.0, cutoff = 35.0, resonance = 0.0 }]
[[instrument]]
slot = 3
name = "Offbeat Hat"
patch = "builtin:hat-closed"
cutoff = 3500.0
resonance = 0.2
metal = 0.45
width = 0.8
fx = [{ kind = "Filter", mode = 1.0, cutoff = 400.0, resonance = 0.2 }, { kind = "Filter", mode = 0.0, cutoff = 13000.0, resonance = 0.0 },
      { kind = "Compressor", threshold = -30.0, ratio = 1.0, attack = 5.0, release = 120.0, makeup = 4.0 }]
[[instrument]]
slot = 4
name = "Clap"
patch = "gen:Tight Clap"
fx = [{ kind = "Filter", mode = 1.0, cutoff = 300.0, resonance = 0.0 }]
[[instrument]]
slot = 5
name = "Roll Snare"
patch = "gen:909 Snare"
fx = [{ kind = "Filter", mode = 1.0, cutoff = 250.0, resonance = 0.0 }]
[[instrument]]
slot = 6
name = "Acid 303"
patch = "builtin:init"
wave = "saw"
filter_model = "vintage"
slope = 24
cutoff = 600.0
resonance = 0.78
env_amount = 2.2
filter_decay = 0.16
key_track = 0.3
vel_cutoff = 1.2
attack = 0.001
decay = 0.25
sustain = 0.35
release = 0.04
drive = 0.4
gain = 0.6
glide_ms = 70.0
legato = true
accent = 0.7
# fx 1 = HP 180 Hz, fx 2 = Phaser (mix automated), fx 3 = drive, fx 4 = dotted 1/8 echo
fx = [{ kind = "Filter", mode = 1.0, cutoff = 180.0, resonance = 0.0 },
      { kind = "Phaser", stages = 2.0, sync = 4.0, depth = 0.7, center = 900.0, feedback = 0.5, mix = 0.0 },
      { kind = "Drive", mode = 0.0, drive = 8.0, tone = 7000.0, output = -6.0, mix = 0.25 },
      { kind = "Delay", sync = 3.0, feedback = 0.3, pingpong = 1.0, hp = 400.0, lp = 5000.0, mix = 0.12 }]
[[instrument]]
slot = 7
name = "Throat Chant"
patch = "voice:Throat Speech"
attack = 0.02
release = 0.5
cutoff = 6000.0
fx = [{ kind = "Filter", mode = 1.0, cutoff = 160.0, resonance = 0.0 },
      { kind = "Chorus", rate = 0.3, depth = 2.5, delay = 12.0, feedback = 0.2, mix = 0.35 },
      { kind = "Compressor", threshold = -30.0, ratio = 1.0, attack = 5.0, release = 120.0, makeup = 7.0 }]
[instrument.voice]
register = "Bass"
words = ["om", "na", "ma", "hu", "ka"]
rate = 0.8
breath = 0.18
[[instrument]]
slot = 8
name = "Vowel Drone"
patch = "voice:Choir Eee"
attack = 1.2
release = 2.5
sustain = 0.9
cutoff = 3200.0
# fx 1 = HP, fx 2 = 8-stage phaser (its centre is LFO-modulated), fx 3 = slow frequency
# shift (+0.4 Hz barber pole: a slow drift)
fx = [{ kind = "Filter", mode = 1.0, cutoff = 120.0, resonance = 0.0 },
      { kind = "Phaser", stages = 2.0, sync = 0.0, rate = 0.08, depth = 0.8, center = 700.0, feedback = 0.6, stereo = 120.0, mix = 0.5 },
      { kind = "FreqShift", coarse = 0.4, fine = 0.0, output = 2.0, feedback = 0.3, mix = 0.35 },
      { kind = "Compressor", threshold = -30.0, ratio = 1.0, attack = 5.0, release = 120.0, makeup = 3.0 }]
[instrument.voice]
register = "Tenor"
vowels = ["A", "O", "U", "O"]
step = 4.0
morph_ms = 900.0
breath = 0.3
[[instrument]]
slot = 9
name = "Whisper Stabs"
patch = "voice:Whisper Ooh"
attack = 0.001
decay = 0.2
sustain = 0.25
release = 0.06
cutoff = 9000.0
fx = [{ kind = "Filter", mode = 1.0, cutoff = 900.0, resonance = 0.3 },
      { kind = "Delay", sync = 3.0, feedback = 0.45, pingpong = 1.0, hp = 800.0, lp = 7000.0, mix = 0.35 },
      { kind = "Compressor", threshold = -30.0, ratio = 1.0, attack = 5.0, release = 120.0, makeup = 13.0 }]
[instrument.voice]
words = ["tsa", "ki", "sha", "pa", "chi"]
rate = 1.4
[[instrument]]
slot = 10
name = "Chop Syllables"
patch = "voice:Talk Ka-Ta"
attack = 0.001
decay = 0.09
sustain = 0.2
release = 0.03
cutoff = 4000.0
resonance = 0.45
fx = [{ kind = "Filter", mode = 1.0, cutoff = 300.0, resonance = 0.0 },
      { kind = "FreqShift", coarse = -35.0, fine = 0.0, output = 0.0, feedback = 0.2, mix = 0.3 },
      { kind = "Delay", sync = 1.0, feedback = 0.35, pingpong = 1.0, hp = 600.0, lp = 6000.0, mix = 0.2 },
      { kind = "Compressor", threshold = -30.0, ratio = 1.0, attack = 5.0, release = 120.0, makeup = 8.0 }]
[instrument.voice]
words = ["da", "ki", "ta", "ka", "ra", "om"]
rate = 1.6
chop_rate = 0.0
[[instrument]]
slot = 11
name = "Chrome Throat"
patch = "voice:Throat Ahh"
attack = 0.03
decay = 1.0
sustain = 0.9
release = 0.6
cutoff = 7000.0
# fx 1 = HP 110 Hz, fx 2 = Metal resonator on the note (comb-like ring, mix automated),
# fx 3 = Frequency shifter (shift and mix automated: slides down into the gurgle),
# fx 4 = 12-stage Phaser (mix automated)
fx = [{ kind = "Filter", mode = 1.0, cutoff = 110.0, resonance = 0.0 },
      { kind = "Resonator", mix = 0.0, ratios = 1.0, pitch = 0.0, tune = 12.0, decay = 900.0, damp = 0.3, spread = 0.7 },
      { kind = "FreqShift", coarse = 0.0, fine = 0.0, output = 2.0, feedback = 0.45, mix = 0.0 },
      { kind = "Phaser", stages = 3.0, sync = 0.0, rate = 0.3, depth = 0.8, center = 900.0, feedback = 0.7, stereo = 120.0, mix = 0.0 },
      { kind = "Compressor", threshold = -30.0, ratio = 1.0, attack = 5.0, release = 120.0, makeup = 5.0 }]
[instrument.voice]
register = "Tenor"
throat = true
vowels = ["A"]
step = 0.0
morph_ms = 260.0
words = ["aa", "o", "u", "gu", "ugu", "ngu", "gugu", "uk"]
rate = 0.7
breath = 0.2
tension = 0.6
jitter = 0.35
[instrument.voice.transform]
blend = 0.0
[instrument.voice.transform.res]
amount = 0.75
ratios = "Metal"
keyed = true
tune = 0.0
decay_ms = 1400.0
damp = 0.25
spread = 0.7
[instrument.voice.transform.dig]
amount = 0.35
drive_db = 10.0
fold = false
bits = 10.0
rate = 16000.0
ring = 0.35
ring_ratio = 1.5
[instrument.voice.transform.sub]
amount = 0.45
mode = "Sine"
formant_drop = 7.0
[[instrument]]
slot = 12
name = "Chrome Shimmer"
patch = "gen:Wave Glass Keys"
# Wave: Pos A (gen.1) and Cutoff (gen.3) rise through the breakdown; fx 1 = HP,
# fx 2 = upward barber-pole frequency shift (feedback), fx 3 = phaser
fx = [{ kind = "Filter", mode = 1.0, cutoff = 900.0, resonance = 0.0 },
      { kind = "FreqShift", coarse = 8.0, fine = 0.0, output = 0.0, feedback = 0.6, mix = 0.4 },
      { kind = "Phaser", stages = 3.0, sync = 3.0, depth = 0.6, center = 2500.0, feedback = 0.5, mix = 0.4 }]
[[instrument]]
slot = 13
name = "Grid Pad"
patch = "gen:Grid Wavetable Pad"
fx = [{ kind = "Filter", mode = 1.0, cutoff = 150.0, resonance = 0.0 },
      { kind = "Filter", mode = 0.0, cutoff = 2500.0, resonance = 0.3 }]
[[instrument]]
slot = 14
name = "Riser"
patch = "gen:White Riser"
fx = [{ kind = "Filter", mode = 1.0, cutoff = 400.0, resonance = 0.0 }]
[[instrument]]
slot = 15
name = "Impact"
patch = "gen:Impact"
[[instrument]]
slot = 16
name = "Downlifter"
patch = "gen:Downlifter"
fx = [{ kind = "Filter", mode = 1.0, cutoff = 300.0, resonance = 0.0 }]
[[instrument]]
slot = 17
name = "Crash"
patch = "gen:Cymbal"
fx = [{ kind = "Filter", mode = 1.0, cutoff = 2000.0, resonance = 0.0 }]
''')
print(f'''[mixer]
master_db = {MIX}
duck_key = 1
duck_release_ms = 110
[mixer.limiter]
enabled = true
gain_db = {LIM}
ceiling_db = -2.4
release_ms = 120.0
target_lufs = -14.0
''')
CHS = [
    ("Kick", -2.0, 0.0, 0.0, 0.0, 0.0),
    ("Bass", -4.0, 0.0, 0.0, 0.0, 0.75),
    ("Hats", 6.0, 0.15, 0.05, 0.12, 0.0),
    ("Clap/Roll", -2.0, -0.05, 0.08, 0.3, 0.1),
    ("Acid", -5.0, -0.12, 0.25, 0.25, 0.4),
    ("Chant", -1.0, 0.08, 0.35, 0.5, 0.25),
    ("Drone", -6.0, 0.0, 0.1, 0.6, 0.4),
    ("Whispers", 3.0, 0.35, 0.4, 0.35, 0.1),
    ("Chops", 2.0, -0.3, 0.3, 0.25, 0.1),
    ("Chrome", 0.0, 0.0, 0.3, 0.45, 0.15),
    ("Shimmer", -6.0, 0.2, 0.3, 0.55, 0.2),
    ("Pad", -9.0, 0.0, 0.05, 0.6, 0.3),
    ("FX", -8.0, -0.2, 0.2, 0.45, 0.0),
]
print("# kick and bass dry and centred (the bass ducks hard off the kick); voices with clearly\n"
      "# audible echo (A: dotted 1/8 ping-pong) and reverb (B: a dark hall)")
for name, vol, pan, sa, sb, duck in CHS:
    print(f'[[mixer.channel]]\nname = "{name}"\nvolume_db = {vol}\npan = {pan}\nsend_a = {sa}\nsend_b = {sb}\nduck = {duck}')
print('''
[mixer.return_a]
sync = 3.0
feedback = 0.42
pingpong = 1.0
hp = 350.0
lp = 4500.0
mix = 1.0
level = 1.3
[mixer.return_b]
size = 0.9
damping = 0.6
predelay = 25.0
width = 1.0
mix = 1.0
lowcut = 250.0
highcut = 9000.0
level = 1.4
''')
for i, bk in enumerate(blocks):
    print(f'[[block]]\nid = {i}\nname = "{bk.name}"\nlines = {ROWS}\nrole = "{bk.role}"')
    print("# row | kick | bass | hat | clap | acid | chant | drone | whisper | chop | chrome | shimmer | pad | fx")
    print('rows = """\n' + bk.text() + '\n"""\n')

secs = []
for bk in ORDER:
    if secs and secs[-1][0] == bk:
        secs[-1][1] += 1
    else:
        secs.append([bk, 1])
print("[sequence]\nsections = [" + ", ".join(f"{{ block = {bk}, repeat = {n} }}" for bk, n in secs) + "]\n")


def lane(target, label, pts, curve="smooth"):
    print(f'[[automation]]\ntarget = "{target}"\nlabel = "{label}"\ncurve = "{curve}"\npoints = [')
    for p in sorted(pts, key=lambda x: x[0]):
        sh = f', shape = "{p[2]}"' if len(p) > 2 else ""
        print(f"  {{ pos = {p[0]}, value = {p[1]} }}," if not sh else f"  {{ pos = {p[0]}, value = {p[1]}{sh} }},")
    print("]\n")


print("# Automation: positions in lines from the song start (64 lines = one block = 4 bars)")
bld, grv, c1, c4, c5, c6, drp = at(BU_A), at(G_A), at(C1), at(C4), at(C5), at(C6), at(D_A)
g2, r1, r2, out = at(G2_A), at(R1), at(R2), at(O_A)
end_c = c6 + ROWS
# bass: the Analog's cutoff opens through the intro
lane("instrument.2.gen.1", "Bass - Cutoff", [
    (0, 120), (at(IN_C), 140), (bld - 1, 330), (grv, 380), (c4, 160), (c5, 260), (drp, 400),
    (out + ROWS, 380), (total, 200)])
# acid: the long cutoff and the phaser
lane("instrument.6.cutoff", "Acid - Cutoff", [
    (0, 300), (bld, 350), (grv, 700), (at(G_A, 1), 1100), (c1, 900), (c5 + 32, 500), (drp, 1300),
    (at(D_A, 1), 1800), (g2, 1000), (r1, 600), (at(G2_C), 1500), (out, 900), (total, 300)])
lane("instrument.6.fx.2.mix", "Acid - Phaser", [
    (0, 0.0), (at(G_C), 0.5), (at(G_C) + ROWS, 0.0), (at(G_C, 1), 0.5), (at(G_C, 1) + ROWS, 0.0),
    (drp, 0.25), (g2, 0.5), (r1, 0.0), (at(G2_C), 0.45), (out, 0.2), (total, 0.2)], curve="step")
# the chrome throat FX: resonator ring, frequency shift sliding down, phaser
lane("instrument.11.fx.2.mix", "Chrome - Resonator", [
    (0, 0.0), (c1, 0.0), (at(C2), 0.15, "linear"), (at(C3), 0.4, "linear"), (c4, 0.6, "linear"),
    (c6 + 48, 0.75, "linear"), (drp, 0.3, "step"), (r1, 0.2, "linear"), (r2 + 48, 0.7, "step"),
    (r2 + ROWS, 0.3, "step"), (out + ROWS, 0.2, "linear"), (total, 0.5)])
lane("instrument.11.fx.3.coarse", "Chrome - Shift Hz", [
    (0, 0.0), (at(C3), 0.0, "linear"), (c4, -40.0, "linear"), (c5, -140.0, "linear"),
    (c6 + 48, -260.0, "step"), (drp, -60.0, "step"), (r1, 0.0, "linear"), (r2 + 48, -220.0, "step"),
    (r2 + ROWS, -60.0, "step"), (out + ROWS, 0.0, "linear"), (total, -120.0)])
lane("instrument.11.fx.3.mix", "Chrome - Shift mix", [
    (0, 0.0), (at(C3), 0.1, "linear"), (c4, 0.35, "linear"), (c5, 0.5, "linear"), (c6 + 48, 0.6, "step"),
    (drp, 0.25, "step"), (r1, 0.1, "linear"), (r2 + 48, 0.55, "step"), (r2 + ROWS, 0.25, "step"),
    (out + ROWS, 0.1, "linear"), (total, 0.4)])
lane("instrument.11.fx.4.mix", "Chrome - Phaser", [
    (0, 0.0), (at(C2), 0.2, "linear"), (c4, 0.5, "linear"), (c6 + 48, 0.6, "step"), (drp, 0.35, "step"),
    (r1, 0.3, "linear"), (r2 + ROWS, 0.35, "step"), (total, 0.4)])
# shimmer: wavetable position, cutoff and the barber-pole shift rise through the breakdown
lane("instrument.12.gen.1", "Shimmer - Pos A", [
    (0, 0.1), (c1, 0.1), (c6 + 40, 0.95), (drp, 0.4, "step"), (r1, 0.2), (r2 + 48, 0.9), (r2 + ROWS, 0.4, "step"),
    (total, 0.4)])
lane("instrument.12.fx.2.coarse", "Shimmer - Rising shift", [
    (0, 4.0), (c1, 4.0, "linear"), (c6 + 48, 60.0, "step"), (drp, 8.0, "step"), (r1, 4.0, "linear"),
    (r2 + 48, 50.0, "step"), (r2 + ROWS, 8.0), (total, 8.0)])
lane("instrument.12.fx.1.cutoff", "Shimmer - HP", [
    (0, 900), (c1, 2500, "linear"), (c5, 1200, "linear"), (c6 + 48, 400, "step"), (drp, 1500), (total, 1500)])
# hats HP opens over the builds
lane("instrument.3.fx.1.cutoff", "Hats - HP sweep", [
    (0, 400), (at(BU_C), 400, "linear"), (at(BU_C) + ROWS - 1, 2500, "step"), (grv, 400),
    (c5, 400, "linear"), (c6 + 31, 3000, "step"), (drp, 400), (r2, 400, "linear"), (r2 + 47, 2500, "step"),
    (r2 + ROWS, 400), (total, 400)], curve="linear")
lane("instrument.14.level", "Riser - Level", [
    (0, 0.6), (bld, 0.6), (at(BU_C), 0.6), (grv - 1, 1.1, "step"), (grv, 0.5), (c5, 0.7), (c6, 1.1, "step"),
    (c6 + 1, 0.6), (total, 0.6)], curve="linear")
# echo and reverb: wetter in the breakdowns, echo throws before the drops
lane("mixer.ch.10.send_a", "Chrome - Echo", [
    (0, 0.3), (c1, 0.45), (c6 + 40, 0.8), (drp, 0.35), (r2 + 40, 0.8), (r2 + ROWS, 0.35), (out + ROWS, 0.6),
    (total, 0.6)], curve="step")
# the swallowed voice gets quieter as the throat closes: the fader rides it back up
lane("mixer.ch.10.volume", "Chrome - Fader", [
    (0, 1.0), (at(C3), 1.0, "linear"), (c4, 1.5, "linear"), (c5, 1.9, "linear"), (c6 + 52, 2.0, "step"),
    (drp, 1.0, "step"), (r1 + 32, 1.0, "linear"), (r2, 1.6, "linear"), (r2 + 56, 1.8, "step"),
    (r2 + ROWS, 1.0, "step"), (at(O_B) + 16, 1.0, "linear"), (at(O_B) + 56, 1.7, "step"), (total, 1.0)])
lane("mixer.ch.6.send_a", "Chant - Echo throws", [
    (0, 0.35), (at(IN_D) + 40, 0.8), (bld, 0.35), (at(G_D) + 40, 0.8), (c1, 0.5), (drp, 0.35),
    (at(G_D, 1) + 40, 0.8), (out, 0.5), (total, 0.6)], curve="step")
lane("mixer.return_a.feedback", "Echo feedback", [
    (0, 0.42), (c1, 0.55), (c6 + 32, 0.75), (drp, 0.42), (r1, 0.5), (r2 + ROWS, 0.42), (out + ROWS, 0.6),
    (total, 0.65)], curve="step")
lane("mixer.return_b.size", "Reverb size", [
    (0, 0.88), (c1, 0.95), (drp, 0.88), (out + ROWS, 0.95), (total, 0.97)], curve="linear")

print('''# Drag-to-modulate LFOs (the same thing as dropping an LFO chip on a knob)
[[modulation.modulator]]
id = 1
owner = 5
name = "Acid wobble"
kind = "Lfo"
shape = "Triangle"
sync = 8.0
[[modulation.modulator]]
id = 2
owner = 7
name = "Drone swirl"
kind = "Lfo"
shape = "Sine"
sync = 16.0
[[modulation.modulator]]
id = 3
owner = 9
name = "Chop squelch"
kind = "Lfo"
shape = "SawDown"
sync = 1.0
[[modulation.modulator]]
id = 4
owner = 10
name = "Chrome wobble"
kind = "Lfo"
shape = "Sine"
sync = 2.0
[[modulation.route]]
src = 1
target = "instrument.6.cutoff"
amount = 0.12
[[modulation.route]]
src = 1
target = "instrument.6.fx.2.p4"
amount = 0.25
[[modulation.route]]
src = 2
target = "instrument.8.fx.2.p4"
amount = 0.3
[[modulation.route]]
src = 3
target = "instrument.10.cutoff"
amount = 0.2
[[modulation.route]]
src = 4
target = "instrument.11.fx.4.p4"
amount = 0.3
''')
