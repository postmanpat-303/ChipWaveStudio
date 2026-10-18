#!/usr/bin/env python3
"""Generate demos/dark_psy.cwt.toml, the extra dark psytrance demo "Hollow Ritual".

Dark psy / night-time in E Phrygian, 147 BPM, about 3:40. Kick-dominant
mix, a KBBB rolling bass (three 16ths after every kick, gated off the kick by
the sidechain duck), a resonant acid riff whose filter is swept by automation
across the whole track, a metallic call-and-response stab, a dark pad that
moves E -> F (the Phrygian b2), offbeat-only hats (one hit on the "and" of
each beat, nothing else), cuts (kick drop-outs, bass high-pass cuts, a
silence hole before the drop) and delay throws. Research notes:
docs/research/dark-psytrance.md.

  python3 scripts/make_dark_psy.py > demos/dark_psy.cwt.toml
  DEMO_SOLO=1,2 python3 scripts/make_dark_psy.py > /tmp/x.toml   (debug: only these channels)
"""
import os

CH, ROWS = 8, 64
KICK, BASS, HAT, CLAP, ACID, METAL, PAD, FX = range(8)
(I_KICK, I_BASS, I_HAT, I_CLAP, I_ACID, I_METAL, I_PAD,
 I_RISE, I_DOWN, I_ZAP, I_CRASH, I_SWEEP) = range(1, 13)
BPM = 147


class Block:
    def __init__(self, name, role):
        self.name, self.role = name, role
        self.cells = [[None] * CH for _ in range(ROWS)]

    def put(self, row, ch, note, ins=None, vol=None, fx=None):
        self.cells[row][ch] = (note, ins, vol, fx)

    def clear(self, row, ch):
        self.cells[row][ch] = None

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


_rng = [4242]


def jitter(v, amt=2):
    _rng[0] = (_rng[0] * 1103515245 + 12345) & 0x7FFFFFFF
    return max(1, min(0x40, v + (_rng[0] >> 16) % (2 * amt + 1) - amt))


# ---- drums -------------------------------------------------------------
def kick(b, bars=range(4), drop_last_beat=False, vol=None):
    for bar in bars:
        for beat in range(4):
            if drop_last_beat and bar == bars[-1] and beat == 3:
                continue
            b.put(bar * 16 + beat * 4, KICK, "A-2", I_KICK, vol)


def hats(b, bars=range(4), soft=False, skip_last=False):
    """offbeat hat only: one hit on the 'and' of each beat (rows 2, 6, 10, 14)"""
    for bar in bars:
        for beat in range(4):
            if skip_last and bar == bars[-1] and beat == 3:
                continue
            b.put(bar * 16 + beat * 4 + 2, HAT, "C-5", I_HAT, jitter(0x2A if soft else 0x36), "0814")


def snare(b, bars=range(4)):
    for bar in bars:
        for beat in (1, 3):
            b.put(bar * 16 + beat * 4, CLAP, "D-4", I_CLAP, 0x34)


def roll(b, start_bar, bars=1, top=0x3C):
    """accelerating roll: 8ths, then 16ths, rising"""
    rows = list(range(start_bar * 16, (start_bar + bars) * 16))
    for k, r in enumerate(rows):
        frac = k / max(1, len(rows) - 1)
        if frac < 0.5 and r % 2:
            continue
        b.put(r, CLAP, "D-4", I_CLAP, int(0x12 + (top - 0x12) * frac))


# ---- bass: KBBB, root E with Phrygian turns --------------------------------
# bass instrument plays octave -1: "E-3" sounds E2 (82 Hz), above the kick's
# 55 Hz A fundamental; the kick owns the sub
BASS_TURN = {  # (bar in 4-bar block, beat) -> notes for the three 16ths
    (1, 3): ("E-3", "E-3", "G-3"),
    (3, 2): ("E-3", "E-3", "F-3"),
    (3, 3): ("D-3", "E-3", "F-3"),
}


def bass(b, bars=range(4), turns=True, cut_last_half_beat=False, vol=0x3A):
    for bar in bars:
        for beat in range(4):
            notes = BASS_TURN.get((bar % 4, beat), ("E-3",) * 3) if turns else ("E-3",) * 3
            for k, n in enumerate(notes):
                r = bar * 16 + beat * 4 + 1 + k
                if cut_last_half_beat and bar == bars[-1] and beat == 3 and k >= 1:
                    b.put(r, BASS, "===")
                    break
                # first 16th (in the kick tail) softer, like a -6 dB "kick-tail" note
                v = vol - 8 if k == 0 else (vol if k == 1 else vol - 2)
                b.put(r, BASS, n, I_BASS, v)


# ---- acid riff: 2-bar hypnotic loop in E Phrygian ---------------------------
# (step, note, accent, fx) ; '===' = note off (303 gate)
ACID_A = [
    (1, "E-3", 0, None), (2, "E-3", 1, None), (3, "E-4", 0, None), (4, "===", 0, None),
    (5, "E-3", 0, None), (6, "F-3", 1, None), (7, "E-3", 0, None), (8, "===", 0, None),
    (9, "E-3", 0, None), (10, "E-4", 1, None), (11, "E-3", 0, None), (12, "D-3", 0, None),
    (13, "E-3", 0, None), (14, "G-3", 1, None), (15, "F-3", 0, None),
    (17, "E-3", 0, None), (18, "E-3", 1, None), (19, "E-4", 0, None), (20, "===", 0, None),
    (21, "E-3", 0, None), (22, "F-3", 1, None), (23, "E-3", 0, None), (24, "===", 0, None),
    (25, "B-3", 1, None), (26, "A-3", 0, None), (27, "E-3", 0, None), (28, "===", 0, None),
    (29, "E-4", 1, None), (30, "D-4", 0, "0206"), (31, "===", 0, None),
]
# variation: one turn changed (C instead of B/A: the b6 makes it darker)
ACID_B = [x for x in ACID_A if x[0] not in (25, 26)] + [(25, "C-4", 1, None), (26, "B-3", 0, None)]
# sparse version for the breakdown: just the accents (the filter sweep is the show)
ACID_SPARSE = [x for x in ACID_A if x[2] or x[1] == "==="]


def acid(b, phrase=ACID_A, bars=range(4), vol=0x2E, acc=0x40):
    for bar in bars:
        half = (bar % 2) * 16
        for step, n, a, f in phrase:
            if half <= step < half + 16:
                r = bar * 16 + step - half
                if n == "===":
                    b.put(r, ACID, n)
                else:
                    b.put(r, ACID, n, I_ACID, acc if a else jitter(vol), f)


# ---- metallic call (answers the acid in the second bar of each pair) --------
METAL_CALL = [(16 + 3, "B-4", None), (16 + 6, "C-5", None), (16 + 11, "B-4", "0204")]
METAL_CALL2 = [(16 + 3, "B-4", None), (16 + 6, "C-5", None), (16 + 9, "F-5", None), (16 + 11, "E-5", "0206")]


def metal(b, call=METAL_CALL, bars=(0, 2), vol=0x34):
    for bar in bars:
        for r, n, f in call:
            b.put(bar * 16 + r, METAL, n, I_METAL, vol, f)


# ---- dark pad: E -> F (Phrygian b2) ---------------------------------------
def pad(b, notes=("E-3", "E-3", "F-3", "F-3"), vol=0x36):
    last = None
    for bar, n in enumerate(notes):
        if n != last:
            b.put(bar * 16, PAD, n, I_PAD, vol)
        last = n


blocks = []


def add(b):
    blocks.append(b)
    return len(blocks) - 1


# 0 intro A: kick + bass (bass filter closed, opens by automation)
b = Block("Intro", "intro"); kick(b); bass(b, turns=False, vol=0x34); I0 = add(b)
# 1 intro B: + offbeat hats, pad drone, kick drop on the last beat
b = Block("Intro hats", "intro"); kick(b, drop_last_beat=True); bass(b, turns=False, vol=0x36, cut_last_half_beat=True)
hats(b, soft=True, skip_last=True); pad(b, ("E-3",) * 4, vol=0x2C); b.put(56, FX, "E-5", I_ZAP, 0x30); I1 = add(b)
# 2 groove A: acid enters (filter low), bass turns
b = Block("Groove A", "verse"); kick(b); bass(b); hats(b); acid(b); I2 = add(b)
# 3 groove A end: kick drop + bass cut on the last beat, delay throw (automation)
b = Block("Groove A cut", "verse"); kick(b, drop_last_beat=True); bass(b, cut_last_half_beat=True); hats(b, skip_last=True)
acid(b); b.put(60, FX, "B-4", I_ZAP, 0x34); I3 = add(b)
# 4 groove B: + snare on 2 & 4, metallic call and response
b = Block("Groove B", "verse"); kick(b); bass(b); hats(b); snare(b); acid(b, ACID_B); metal(b); I4 = add(b)
# 5 groove B end: roll into the breakdown, kick out in the last bar
b = Block("Groove B roll", "build"); kick(b, bars=range(3)); bass(b, bars=range(3)); hats(b, bars=range(3)); snare(b, bars=range(3))
acid(b, ACID_B); metal(b, bars=(0,)); roll(b, 3, top=0x34); b.put(48, FX, "E-5", I_SWEEP, 0x34); I5 = add(b)
# 6 breakdown: no kick, no bass: pad E -> F, sparse acid with the resonance up, downlifter
b = Block("Breakdown", "breakdown"); pad(b, vol=0x3C); acid(b, ACID_SPARSE, vol=0x30); b.put(0, FX, "E-4", I_DOWN, 0x34)
b.put(0, CLAP, "C-5", I_CRASH, 0x2C); I6 = add(b)
# 7 breakdown 2: full acid comes back under the pad, metallic calls echo (delay)
b = Block("Breakdown 2", "breakdown"); pad(b, ("E-3", "E-3", "F-3", "G-3"), vol=0x3C); acid(b, vol=0x2A, acc=0x3A)
metal(b, METAL_CALL2, bars=(2,), vol=0x30); hats(b, bars=(2, 3), soft=True); I7 = add(b)
# 8 build: riser, kick for 2 bars, accelerating roll, then a 2-beat silence hole before the drop
b = Block("Build", "build"); pad(b, ("F-3", "F-3", "F-3", "F-3"), vol=0x34); acid(b, ACID_B, bars=range(3))
kick(b, bars=range(2)); bass(b, bars=range(2), turns=False, vol=0x34); b.put(32, BASS, "==="); roll(b, 2, bars=2, top=0x3C)
for r in range(56, 64):
    b.clear(r, CLAP)
b.put(56, CLAP, "===")
b.put(0, FX, "E-4", I_RISE, 0x40); b.put(56, FX, "===")   # riser stops: 2-beat silence hole
b.put(56, PAD, "===")
b.put(48, ACID, "===")
I8 = add(b)


def drop(name, phrase, call, last=False, crash=True):
    d = Block(name, "drop")
    kick(d, drop_last_beat=last); bass(d, cut_last_half_beat=last); hats(d, skip_last=last); snare(d)
    acid(d, phrase); metal(d, call)
    if crash:
        d.put(0, CLAP, "C-5", I_CRASH, 0x2E)
    if last:
        d.put(60, FX, "E-5", I_ZAP, 0x34)
    return d


# 9-11 drop (32 bars = 8 blocks via the order list)
I9 = add(drop("Drop", ACID_A, METAL_CALL))
I10 = add(drop("Drop var", ACID_B, METAL_CALL2, crash=False))
I11 = add(drop("Drop cut", ACID_B, METAL_CALL2, last=True, crash=False))
# 12 reset: dry kick + bass + hats, acid only (the "dry kick and bass" reset)
b = Block("Reset", "verse"); kick(b); bass(b, turns=False); hats(b); acid(b, ACID_SPARSE, vol=0x2C)
b.put(48, FX, "E-5", I_SWEEP, 0x30); I12 = add(b)
# 13 reset end: kick-only last bar, bass high-pass cut by automation, roll
b = Block("Reset roll", "build"); kick(b); bass(b, bars=range(3)); hats(b); acid(b, ACID_SPARSE, bars=range(3))
roll(b, 3, top=0x38); I13 = add(b)
# 14-15 final drop (pad doubles the darkness)
d = drop("Final drop", ACID_B, METAL_CALL2); pad(d, ("E-3", "E-3", "F-3", "F-3"), vol=0x28); I14 = add(d)
d = drop("Final drop end", ACID_A, METAL_CALL, last=True, crash=False); pad(d, ("E-3", "E-3", "F-3", "E-3"), vol=0x28); I15 = add(d)
# 16 outro: kick, bass, hats, acid closing
b = Block("Outro", "outro"); kick(b); bass(b, turns=False, vol=0x34); hats(b, soft=True); acid(b, ACID_SPARSE, vol=0x28); I16 = add(b)
# 17 outro end: kick and bass thin out, pad tail
b = Block("Outro end", "outro"); kick(b, bars=range(3)); bass(b, bars=range(2), turns=False, vol=0x30); b.put(32, BASS, "===")
hats(b, bars=range(3), soft=True); pad(b, ("E-3",) * 4, vol=0x2C); b.put(56, PAD, "==="); b.put(48, FX, "E-4", I_DOWN, 0x2C)
I17 = add(b)

ORDER = ([I0] * 3 + [I1]                      # intro 16 bars
         + [I2] * 3 + [I3]                    # groove A 16 bars
         + [I4] * 3 + [I5]                    # groove B 16 bars
         + [I6, I6, I7, I8]                   # breakdown + build 16 bars
         + [I9, I10, I9, I11, I9, I10, I9, I11]  # drop 32 bars
         + [I12, I13]                         # reset 8 bars
         + [I14, I14, I14, I15]               # final drop 16 bars
         + [I16, I17])                        # outro 8 bars
start = {}
for i, bk in enumerate(ORDER):
    start.setdefault(bk, []).append(i * ROWS)
total = len(ORDER) * ROWS


def at(bk, k=0):
    return start[bk][k]


print(f'''# ChipWave extra demo "Hollow Ritual" (generated by scripts/make_dark_psy.py; edit that, not this)
# Dark psytrance in E Phrygian, {BPM} BPM. Single-file text form of a .cwt song (docs/FIRST_RUN.md).
# Mix: the kick leads (about 6 LU above the bass solo), the bass is gated off the kick by the duck,
# everything except kick and bass is high-passed, riffs and FX sit under the kick and bass, the delay
# return is high-passed at 400 Hz, low end mono. See docs/research/dark-psytrance.md.
format = "chipwave-song-text"
format_version = 2

[song]
title = "Hollow Ritual (dark psy demo)"
bpm = {BPM}
lines_per_beat = 4
speed = 6
tracks = 8
''')
print('''# patch = factory preset or built-in; the other keys override synth parameters
[[instrument]]
slot = 1
name = "Ritual Kick"
patch = "builtin:trance-kick"
drive = 0.0
cutoff = 5000.0
pitch_env = [44.0, 0.009]
decay = 0.24
[[instrument]]
slot = 2
name = "Rolling Bass"
patch = "builtin:init"
wave = "saw"
octave = -1
sub = 0.0
unison = 1
cutoff = 260.0
resonance = 0.35
env_amount = 0.22
filter_decay = 0.055
attack = 0.0005
decay = 0.085
sustain = 0.0
release = 0.02
drive = 0.35
gain = 0.95
fx = [{ kind = "Filter", mode = 1.0, cutoff = 70.0, resonance = 0.0 }]
[[instrument]]
slot = 3
name = "Offbeat Hat"
patch = "builtin:hat-closed"
cutoff = 3500.0
resonance = 0.2
metal = 0.45
width = 0.8
# fx 1 = LP 10 kHz (tames the noise peaks), fx 2 = HP sweep (automated)
fx = [{ kind = "Filter", mode = 0.0, cutoff = 10000.0, resonance = 0.0 }, { kind = "Filter", mode = 1.0, cutoff = 300.0, resonance = 0.2 }]
[[instrument]]
slot = 4
name = "Dark Snare"
patch = "builtin:clap"
cutoff = 2600.0
resonance = 0.4
attack = 0.003
fx = [{ kind = "Filter", mode = 1.0, cutoff = 220.0, resonance = 0.0 }, { kind = "Filter", mode = 0.0, cutoff = 8000.0, resonance = 0.0 }]
[[instrument]]
slot = 5
name = "Acid Riff"
patch = "builtin:init"
wave = "saw"
cutoff = 420.0
resonance = 0.72
env_amount = 0.42
filter_decay = 0.13
key_track = 0.5
vel_cutoff = 1.6
attack = 0.001
decay = 0.2
sustain = 0.25
release = 0.03
drive = 0.45
gain = 0.7
# fx 1 = HP (sweeps into breakdown / drops), fx 2 = band-pass section filter with its own LFO
# (wet mix automated: 0 = off), fx 3 = drive (amount of grit per section), fx 4 = chorus (breakdown)
fx = [{ kind = "Filter", mode = 1.0, cutoff = 250.0, resonance = 0.0 },
      { kind = "Filter", mode = 2.0, cutoff = 1100.0, resonance = 0.65, lfo_rate = 0.6, lfo_depth = 1.6, mix = 0.0 },
      { kind = "Drive", mode = 0.0, drive = 9.0, tone = 6500.0, output = -7.0, mix = 0.15 },
      { kind = "Chorus", rate = 0.25, depth = 4.0, delay = 14.0, feedback = 0.3, mix = 0.0 }]
[[instrument]]
slot = 6
name = "Metal Call"
patch = "builtin:init"
wave = "square"
osc2 = { wave = "square", semi = 1, level = 0.6 }
metal = 0.35
filter = "bp"
cutoff = 1500.0
resonance = 0.5
env_amount = 0.25
filter_decay = 0.1
attack = 0.001
decay = 0.16
sustain = 0.0
release = 0.06
drive = 0.3
gain = 0.55
# fx 1 = bitcrusher (mix automated), fx 2 = foldback distortion (100 % wet: the fold flips polarity,
# so a dry/wet blend cancels), fx 3 = slow-LFO band-pass,
# fx 4 = HP sweep, fx 5 = its own 1/4 ping-pong echo
fx = [{ kind = "Bitcrusher", bits = 8.0, rate = 12000.0, mix = 0.3 },
      { kind = "Drive", mode = 2.0, drive = 12.0, tone = 6000.0, output = -6.0, mix = 1.0 },
      { kind = "Filter", mode = 2.0, cutoff = 1700.0, resonance = 0.55, lfo_rate = 0.12, lfo_depth = 1.4, mix = 0.8 },
      { kind = "Filter", mode = 1.0, cutoff = 400.0, resonance = 0.0 },
      { kind = "Delay", sync = 4.0, feedback = 0.45, pingpong = 1.0, hp = 350.0, lp = 5000.0, mix = 0.3 }]
[[instrument]]
slot = 7
name = "Hollow Pad"
patch = "builtin:init"
wave = "saw"
unison = 6
detune = 18.0
width = 1.0
osc2 = { wave = "saw", semi = 7, level = 0.35 }
cutoff = 900.0
resonance = 0.25
attack = 1.2
decay = 1.0
sustain = 0.85
release = 2.0
gain = 0.55
lfo = { shape = "sine", sync = 8.0, cutoff = 0.7 }
# fx 1 = HP (sweeps), fx 2 = resonant LP with a very slow LFO (about 12 s), fx 3 = wide chorus
fx = [{ kind = "Filter", mode = 1.0, cutoff = 300.0, resonance = 0.1 },
      { kind = "Filter", mode = 0.0, cutoff = 1300.0, resonance = 0.4, lfo_rate = 0.08, lfo_depth = 1.8, mix = 1.0 },
      { kind = "Chorus", rate = 0.2, depth = 4.0, mix = 0.45 }]
[[instrument]]
slot = 8
name = "Noise Riser"
patch = "builtin:riser"
fx = [{ kind = "Filter", mode = 1.0, cutoff = 300.0, resonance = 0.0 }]
[[instrument]]
slot = 9
name = "Downlifter"
patch = "builtin:init"
wave = "saw"
unison = 3
detune = 15.0
width = 0.8
noise = 0.35
cutoff = 5000.0
resonance = 0.3
pitch_env = [-24.0, 1.6]
attack = 0.005
decay = 2.6
sustain = 0.0
release = 0.3
gain = 0.45
fx = [{ kind = "Filter", mode = 1.0, cutoff = 300.0, resonance = 0.0 }]
[[instrument]]
slot = 10
name = "Zap"
patch = "builtin:init"
wave = "square"
filter = "bp"
cutoff = 2500.0
resonance = 0.6
key_track = 1.0
pitch_env = [30.0, 0.06]
attack = 0.0
decay = 0.12
sustain = 0.0
release = 0.02
gain = 0.6
fx = [{ kind = "Filter", mode = 1.0, cutoff = 400.0, resonance = 0.0 }, { kind = "Bitcrusher", bits = 6.0, rate = 9000.0, mix = 0.5 }]
[[instrument]]
slot = 11
name = "Crash"
patch = "builtin:hat-open"
decay = 1.6
release = 0.8
cutoff = 4500.0
[[instrument]]
slot = 12
name = "Noise Sweep"
patch = "builtin:riser"
attack = 1.4
decay = 0.6
sustain = 0.0
release = 0.4
fx = [{ kind = "Filter", mode = 1.0, cutoff = 800.0, resonance = 0.0 }]
''')
MIX = os.environ.get("MIX_MASTER_DB", "6.0")
print(f'''[mixer]
master_db = {MIX}
duck_key = 1
duck_release_ms = 90
''')
print('''# gain staging (faders): kick +3 dB centre dry (it hits the master soft clipper a little, like a
# clipped psy kick); bass +1.5 dB centre dry, gated off the kick (duck 80 %, 90 ms, so it is silent
# under the kick and back for the next 16th); riffs / pad / FX below kick and bass, panned off
# centre, light sidechain. Sends: big dark reverb (B) on acid / metal / pad / snare / FX, dotted-1/8
# ping-pong echo (A) on the riffs; the bass sends a little to both (its insert HP is 70 Hz, the echo
# return loop is high-passed at 300 Hz; the reverb has no filter, so the bass reverb send stays small).
# The kick sends nothing.
[[mixer.channel]]
name = "Kick"
volume_db = 3.0
[[mixer.channel]]
name = "Bass"
volume_db = 1.5
send_a = 0.15
send_b = 0.06
duck = 0.8
[[mixer.channel]]
name = "Hats"
volume_db = -8.0
pan = 0.12
send_a = 0.06
send_b = 0.09
[[mixer.channel]]
name = "Snare"
volume_db = -4.0
pan = -0.05
send_a = 0.12
send_b = 0.4
duck = 0.2
[[mixer.channel]]
name = "Acid"
volume_db = -6.0
pan = -0.15
send_a = 0.3
send_b = 0.3
duck = 0.45
[[mixer.channel]]
name = "Metal"
volume_db = 3.0
pan = 0.35
send_a = 0.35
send_b = 0.45
duck = 0.45
[[mixer.channel]]
name = "Pad"
volume_db = -7.0
send_a = 0.1
send_b = 0.5
duck = 0.6
[[mixer.channel]]
name = "FX"
volume_db = -5.0
pan = -0.3
send_a = 0.3
send_b = 0.45
duck = 0.4

# A = dotted-1/8 ping-pong echo, loop high-passed at 300 Hz (no sub in the echoes), dark loop LP;
# B = big dark hall (long size, heavy damping, 30 ms pre-delay so the dry attack stays clear)
[mixer.return_a]
sync = 3.0
feedback = 0.45
pingpong = 1.0
hp = 300.0
lp = 4000.0
mix = 1.0
level = 1.5
[mixer.return_b]
size = 0.93
damping = 0.7
predelay = 30.0
width = 1.0
mix = 1.0
level = 1.5
''')
for i, bk in enumerate(blocks):
    print(f'[[block]]\nid = {i}\nname = "{bk.name}"\nlines = {ROWS}\nrole = "{bk.role}"')
    print("# row | kick | bass | hats | snare | acid | metal | pad | fx")
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
    for p in pts:
        sh = f', shape = "{p[2]}"' if len(p) > 2 else ""
        print(f"  {{ pos = {p[0]}, value = {p[1]}{sh} }},")
    print("]\n")


BAR = B = 16
print("# Automation: positions in lines from the song start (64 lines = one block = 4 bars)")
grA, grB, brk, bld = at(I2), at(I4), at(I6), at(I8)
drp, rst, fin, out = at(I9), at(I12), at(I14), at(I16)
cutA, rollB, cut1, cut2, rroll, cutF = at(I3), at(I5), at(I11, 0), at(I11, 1), at(I13), at(I15)
hole = bld + 14 * B          # 2-beat silence hole before the drop


def sweep(base, spans, top):
    """HP sweep lanes: rise linearly over each (start, end) span, snap back to base at the end"""
    pts = [(0, base)]
    for a, e in spans:
        pts += [(a, base, "linear"), (e - 1, top, "step"), (e, base, "linear")]
    pts.append((total, base))
    return pts


# --- acid: the main event --------------------------------------------------------------
lane("instrument.5.cutoff", "Acid - Filter cutoff", [
    (0, 200), (grA, 200), (grA + 12 * B, 900), (cutA + 4 * B - 2, 500, "step"), (grB, 600),
    (grB + 8 * B, 1800), (grB + 12 * B, 900), (rollB + 3 * B, 2200), (brk - 1, 2200, "step"),
    (brk, 200), (brk + 8 * B, 1400), (brk + 16 * B, 400), (bld, 600), (hole, 5000, "step"),
    (drp, 1000), (drp + 8 * B, 2600), (drp + 12 * B, 700), (drp + 16 * B, 1200), (drp + 24 * B, 4200),
    (drp + 32 * B - 1, 4200, "step"), (rst, 500), (rst + 8 * B - 1, 1500, "step"), (fin, 1600),
    (fin + 12 * B, 5000), (out, 2000), (total, 220)])
lane("instrument.5.resonance", "Acid - Resonance", [
    (0, 0.6), (grA, 0.6), (grB, 0.7), (brk, 0.88), (bld + 8 * B, 0.92), (drp, 0.74),
    (drp + 24 * B, 0.85), (rst, 0.9), (fin, 0.78), (out, 0.7), (total, 0.6)], curve="linear")
lane("instrument.5.fx.1.cutoff", "Acid - HP sweep",
     sweep(250, [(rollB + B, brk), (bld + 8 * B, drp), (rroll, fin), (cut1 + 3 * B, cut1 + 4 * B),
                 (cut2 + 3 * B, cut2 + 4 * B)], 2600))
lane("instrument.5.fx.2.mix", "Acid - Band-pass section", [
    (0, 0.0), (grA + 8 * B, 0.8), (grB, 0.0), (brk + 8 * B, 0.6), (bld, 0.0), (rst, 1.0), (fin, 0.0),
    (total, 0.0)], curve="step")
lane("instrument.5.fx.3.mix", "Acid - Drive amount", [
    (0, 0.1), (grB, 0.35), (brk, 0.1), (drp, 0.45), (rst, 0.25), (fin, 0.65), (out, 0.2), (total, 0.2)],
    curve="step")
lane("instrument.5.fx.4.mix", "Acid - Chorus", [
    (0, 0.0), (brk, 0.55), (bld, 0.3), (drp, 0.0), (total, 0.0)], curve="step")
# --- metal, hats, snare, pad: grit per section and HP sweep builds -----------------------
lane("instrument.6.fx.1.mix", "Metal - Bitcrush amount", [
    (0, 0.3), (drp, 0.5), (drp + 16 * B, 0.65), (rst, 0.4), (fin, 0.75), (out, 0.4), (total, 0.4)],
    curve="step")
lane("instrument.6.fx.4.cutoff", "Metal - HP sweep", sweep(400, [(rollB + B, brk), (bld + 8 * B, drp)], 3000))
lane("instrument.3.fx.2.cutoff", "Hats - HP sweep",
     sweep(300, [(rollB, rollB + 3 * B), (rroll, fin), (cut1 + 2 * B, cut1 + 4 * B)], 5000)[:-1]
     + [(out, 300, "linear"), (total, 3500)])
lane("instrument.4.fx.1.cutoff", "Snare - HP sweep",
     sweep(220, [(rollB + 2 * B, brk), (bld + 8 * B, hole), (rroll + 2 * B, fin)], 1800))
lane("instrument.7.fx.1.cutoff", "Pad - HP sweep", sweep(300, [(bld + 6 * B, hole), (out + 4 * B, total)], 2500))
lane("instrument.7.cutoff", "Pad - Filter cutoff", [
    (0, 300), (16 * B, 500), (brk, 500), (bld, 1800), (hole, 3000, "step"), (drp, 700), (fin, 900),
    (total, 300)])
# --- bass: the intro filter opens; LP and HP cuts before every return -------------------
bass_lp = [(0, 110), (12 * B, 260), (cutA + 3 * B, 260), (cutA + 4 * B - 1, 110, "step"), (grB, 270),
           (bld, 260), (bld + 2 * B - 1, 60, "step"), (drp, 290)]
for c in (cut1, cut2, cutF):
    bass_lp += [(c + 3 * B, 290), (c + 4 * B - 1, 130, "step"), (c + 4 * B, 290)]
bass_lp += [(rst, 250), (rroll, 250), (rroll + 3 * B - 1, 70, "step"), (fin, 300), (out, 260), (total, 140)]
lane("instrument.2.cutoff", "Bass - LP filter cuts", sorted(bass_lp), curve="linear")
hp_pts = [(0, 70)]
for c in (cutA, cut1, cut2, cutF):   # last 2 bars of these blocks: HP sweep up, snap back
    hp_pts += [(c + 2 * B, 70, "linear"), (c + 4 * B - 1, 450, "step"), (c + 4 * B, 70)]
hp_pts += [(rroll + 2 * B, 70, "linear"), (rroll + 3 * B - 1, 300, "step"), (rroll + 3 * B, 70), (total, 70)]
lane("instrument.2.fx.1.cutoff", "Bass - HP cut", sorted(hp_pts), curve="linear")
lane("instrument.8.level", "Riser - Level", [
    (0, 0.0), (bld, 0.1), (hole - 1, 1.1, "step"), (hole, 0.0)], curve="linear")
lane("instrument.8.cutoff", "Riser - Filter sweep", [
    (0, 400), (bld, 400), (hole - 1, 9000, "step"), (hole, 400)])
# --- echo throws: the last notes before each cut fly into the echo ----------------------
ends = [cutA + 4 * B, rollB + 4 * B, cut1 + 4 * B, cut2 + 4 * B, cutF + 4 * B, rroll + 3 * B]


def throws(base, top, extra=()):
    pts = [(0, base)]
    for e in ends:
        pts += [(e - 6, top), (e + 2, base)]
    for a, e, v in extra:
        pts += [(a, v), (e, base)]
    pts.append((total, base))
    return sorted(pts)


lane("mixer.ch.5.send_a", "Acid - Echo throws", throws(0.3, 0.9, [(hole - 8, drp, 1.0)]), curve="step")
lane("mixer.ch.6.send_a", "Metal - Echo throws", throws(0.35, 0.9, [(bld, drp, 0.8)]), curve="step")
lane("mixer.ch.4.send_a", "Snare - Echo on rolls", [
    (0, 0.12), (rollB + 3 * B, 0.6), (brk, 0.12), (bld + 8 * B, 0.6), (drp, 0.12), (rroll + 3 * B, 0.6),
    (fin, 0.12), (total, 0.12)], curve="step")
lane("mixer.ch.3.send_a", "Hats - Echo in breakdown", [
    (0, 0.06), (brk + 8 * B, 0.35), (bld, 0.06), (out, 0.2), (total, 0.2)], curve="step")
lane("mixer.ch.7.send_b", "Pad - Reverb", [
    (0, 0.5), (brk, 0.7), (bld, 0.6), (drp, 0.45), (out, 0.6), (total, 0.7)], curve="linear")
lane("mixer.ch.5.send_b", "Acid - Reverb", [
    (0, 0.3), (brk, 0.5), (drp, 0.3), (out, 0.4), (total, 0.5)], curve="step")
lane("mixer.return_a.feedback", "Echo feedback", [
    (0, 0.45), (brk, 0.6), (bld, 0.45), (hole - 8, 0.72), (drp, 0.45), (rst, 0.55), (fin, 0.45),
    (out, 0.5), (total, 0.7)], curve="step")
lane("mixer.return_b.size", "Reverb size", [
    (0, 0.9), (brk, 0.97), (drp, 0.93), (out, 0.97), (total, 0.97)], curve="linear")
