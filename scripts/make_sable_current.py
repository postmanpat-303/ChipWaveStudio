#!/usr/bin/env python3
"""Generate demos/sable_current.cwt.toml, the demo "Sable Current".

An original dark psytrance instrumental in A Phrygian at 145 BPM.
Kick and one short bass note per beat for 32 bars while a soft low-pass opens.
The bleep theme enters at bar 33. A breakdown and the return are still there.
Native instruments, Rack filters and song automation only: no samples.

  python3 scripts/make_sable_current.py > demos/sable_current.cwt.toml
"""
import math

from cwdemo import B, ROWS, Block, Rng, emit_blocks, lane, modulator, route

BPM = 145
(KICK, BASS, HAT, OHAT, PERC, RIFF, PAD, PADR, SWEEP, FX, RIM) = range(11)
CH = 11
(I_KICK, I_BASS, I_HAT, I_OHAT, I_PERC, I_RIFF, I_PAD, I_SWEEP, I_FX, I_RIM) = range(1, 11)
rng = Rng(145)

# StringWave cutoff / reverb, same knob map as scripts/make_tallest_morning.py
KN = {"cutoff": (30.0, 18000.0, True), "rev_mix": (0.0, 1.0, False)}
GEN_CUTOFF = 1
GEN_REV = 6


def u(name, x):
    lo, hi, ex = KN.get(name, (0.0, 1.0, False))
    v = math.log(max(x, lo) / lo) / math.log(hi / lo) if ex else (x - lo) / (hi - lo)
    return round(min(1.0, max(0.0, v)), 4)


def note(b, row, ch, name, ins, vol, ln, fx=None):
    b.put(row, ch, name, ins, vol, fx)
    if ln:
        b.off(min(row + ln, ROWS - 1), ch)


def kick(b, bars=range(4), vol=0x38, stop=None):
    for bar in bars:
        for beat in range(4):
            r = bar * B + beat * 4
            if stop is not None and r >= stop:
                return
            b.put(r, KICK, "A-2", I_KICK, vol)


def roll(b, bars=range(4), vol=0x2A, turn=False, extra=False):
    """One short off-beat note per beat. extra=True adds a second note on the
    last beat of the 4-bar phrase only. Never more than two notes in a beat."""
    for bar in bars:
        for beat in range(4):
            base = bar * B + beat * 4
            if extra and bar == 3 and beat == 3:
                b.put(base + 2, BASS, "A#3", I_BASS, rng.jitter(vol, 1))
                b.put(base + 3, BASS, "G-3", I_BASS, rng.jitter(max(1, vol - 4), 1))
                if base + 4 < ROWS:
                    b.off(base + 4, BASS)
                continue
            n = "A-3"
            if turn and bar == 3 and beat == 3:
                n = "A#3"
            elif turn and bar == 1 and beat == 2:
                n = "E-3"
            b.put(base + 2, BASS, n, I_BASS, rng.jitter(vol, 1))
            if base + 3 < ROWS:
                b.off(base + 3, BASS)


def hats(b, bars=range(4), vol=0x18, sixteenths=False):
    for bar in bars:
        for beat in range(4):
            b.put(bar * B + beat * 4 + 2, HAT, "F#5", I_HAT, rng.jitter(vol, 1), "0814")
            if sixteenths:
                for off in (1, 3):
                    b.put(bar * B + beat * 4 + off, HAT, "F#5", I_HAT,
                          rng.jitter(vol - 8, 1), "080C")


def open_hat(b, bars=range(4), vol=0x1C, beats=(3,)):
    for bar in bars:
        for beat in beats:
            b.put(bar * B + beat * 4 + 2, OHAT, "A-5", I_OHAT, vol, "0820")


def shaker(b, bars=range(4), vol=0x14, step=2):
    for bar in bars:
        r = 0
        while r < B:
            b.put(bar * B + r, PERC, "C-5", I_PERC, rng.jitter(vol, 1))
            r += step


def rim(b, bars=range(4), vol=0x22):
    for bar in bars:
        b.put(bar * B + 4, RIM, "D-4", I_RIM, vol)
        b.put(bar * B + 12, RIM, "D-4", I_RIM, vol - 2)


def fill(b, bar=3, vol=0x28):
    for k, r in enumerate(range(bar * B + 8, bar * B + 16)):
        b.put(r, RIM, "D-4", I_RIM, vol - 8 + k)


def pad(b, low="A-3", high="E-4", vol=0x2A, bars=range(4)):
    for bar in bars:
        b.put(bar * B, PAD, low, I_PAD, vol)
        b.put(bar * B, PADR, high, I_PAD, vol - 2)
        if bar != 3:
            b.off(bar * B + B - 1, PAD)
            b.off(bar * B + B - 1, PADR)


def riff(b, phrase, vol=0x30):
    for row, name, ln in phrase:
        note(b, row, RIFF, name, I_RIFF, rng.jitter(vol, 1), ln)


# short bleeps with space between them, A Phrygian, one row long
RIFF_A = [
    (0, "A-4", 1), (6, "C-5", 1), (12, "A#4", 1),
    (20, "G-4", 1), (26, "F-4", 1),
    (36, "C-5", 1), (44, "A-4", 1),
    (52, "E-4", 1), (60, "A-4", 1),
]
RIFF_B = [
    (2, "C-5", 1), (8, "A#4", 1), (16, "G-4", 1),
    (24, "E-4", 1), (30, "F-4", 1),
    (40, "A-4", 1), (46, "C-5", 1), (54, "A#4", 1),
    (62, "A-4", 1),
]
RIFF_C = [
    (4, "E-4", 1), (10, "F-4", 1), (18, "A-4", 1),
    (26, "C-5", 1), (34, "A#4", 1), (42, "G-4", 1),
    (50, "E-4", 1), (58, "A-4", 1),
]

blocks = []


def blk(name, role):
    b = Block(name, role, CH)
    blocks.append(b)
    return len(blocks) - 1, b


# ---- arrangement ----
# 32 bars of kick + one bass note, then the bleep theme. Not the old 2:39 wait.
A, b = blk("Current, closed", "intro")
kick(b); roll(b, turn=False)
TURN, b = blk("Current, a turn", "intro")
kick(b); roll(b, turn=True)
EARLY, b = blk("Theme, first bleeps", "build")
kick(b); roll(b, turn=False); hats(b, vol=0x12)
riff(b, RIFF_A, 0x20)
ANS, b = blk("Theme, answer", "build")
kick(b); roll(b, turn=True); hats(b, vol=0x16); open_hat(b, vol=0x12, beats=(3,))
riff(b, RIFF_B, 0x24)
E, b = blk("Pad enters", "build")
kick(b); roll(b, vol=0x2C, turn=True); hats(b, vol=0x1A); shaker(b, vol=0x12, step=2)
pad(b, vol=0x28); riff(b, RIFF_A, 0x22)
F, b = blk("Pad leans on the flat two", "build")
kick(b); roll(b, vol=0x2C, turn=True); hats(b, vol=0x1A); open_hat(b, vol=0x16)
pad(b, "A#3", "F-4", vol=0x2A); riff(b, RIFF_B, 0x24)
G, b = blk("First riff", "drop")
kick(b); roll(b, turn=True, extra=True); hats(b, vol=0x1C); shaker(b, vol=0x14, step=2); rim(b, vol=0x1E)
pad(b, vol=0x26); riff(b, RIFF_A, 0x28)
H, b = blk("Riff answers", "drop")
kick(b); roll(b, turn=True, extra=True); hats(b, vol=0x1C, sixteenths=True); open_hat(b, vol=0x18)
rim(b, vol=0x20); pad(b, vol=0x24); riff(b, RIFF_B, 0x2A)
I, b = blk("Full current", "drop")
kick(b); roll(b, vol=0x2C, turn=True, extra=True); hats(b, vol=0x1E, sixteenths=True)
open_hat(b, vol=0x1A, beats=(1, 3)); rim(b, vol=0x22); pad(b, vol=0x24); riff(b, RIFF_A, 0x2C)
J, b = blk("Full current, fill", "drop")
kick(b); roll(b, vol=0x2C, turn=True, extra=True); hats(b, vol=0x1C, sixteenths=True)
open_hat(b, vol=0x18); rim(b, bars=range(3), vol=0x20); fill(b); pad(b, vol=0x22)
riff(b, RIFF_B, 0x2A)
K, b = blk("The room", "breakdown")
pad(b, vol=0x30)
note(b, 0, RIFF, "E-5", I_RIFF, 0x18, 8)
note(b, 32, RIFF, "A-4", I_RIFF, 0x16, 10)
L, b = blk("One sweep", "breakdown")
pad(b, "A-3", "E-4", vol=0x2C)
b.put(0, SWEEP, "E-5", I_SWEEP, 0x34)
M, b = blk("Sweep holds", "breakdown")
pad(b, "A#3", "F-4", vol=0x2A)
N, b = blk("Sweep falls", "breakdown")
pad(b, vol=0x28)
b.off(0, SWEEP)
b.put(0, FX, "C-4", I_FX, 0x28)
O, b = blk("Roll returns", "drop")
kick(b, vol=0x3A); roll(b, vol=0x28, turn=False)
P, b = blk("Hats return", "drop")
kick(b); roll(b, turn=True); hats(b, vol=0x16)
Q, b = blk("Riff returns", "drop")
kick(b); roll(b, turn=True, extra=True); hats(b, vol=0x1C); shaker(b, vol=0x12, step=2)
pad(b, vol=0x26); riff(b, RIFF_C, 0x28)
R, b = blk("Open water", "drop")
kick(b); roll(b, vol=0x2C, turn=True, extra=True); hats(b, vol=0x20, sixteenths=True)
open_hat(b, vol=0x1C, beats=(1, 3)); rim(b); pad(b, vol=0x26); riff(b, RIFF_C, 0x34)
S, b = blk("Peel back", "outro")
kick(b, bars=range(3)); roll(b, bars=range(3), turn=True); hats(b, bars=range(2), vol=0x14)
pad(b, vol=0x24)
T, b = blk("Last current", "outro")
kick(b, bars=range(2), vol=0x30); roll(b, bars=range(2), vol=0x26, turn=False)
pad(b, bars=range(1), vol=0x20)
for ch in (PAD, PADR):
    b.off(16 + 8, ch)

ORDER = (
    [A] * 4 + [TURN] * 4
    + [EARLY] * 2 + [ANS] * 2
    + [E] * 2 + [F] * 2
    + [G] * 2 + [H] * 2 + [I] * 2 + [J] * 2
    + [K] * 2 + [L] + [M] * 3 + [N] * 2
    + [O] * 2 + [P] * 2 + [Q] * 2 + [R] * 2 + [S] * 2 + [T] * 2
)
assert len(ORDER) == 44, len(ORDER)


def at(block, nth=0):
    return [i for i, x in enumerate(ORDER) if x == block][nth] * ROWS


total = len(ORDER) * ROWS

print(f'''# ChipWave demo "Sable Current" (generated by scripts/make_sable_current.py; edit that, not this)
# Original dark psytrance in A Phrygian, {BPM} BPM, 256 bars (about 7:04).
# 32 bars of kick and rolling bass while a resonant filter opens, a StringWave pad,
# then one breakdown sweep before the roll returns. No samples.
format = "chipwave-song-text"
format_version = 2

[song]
title = "Sable Current (dark psy demo)"
artist = "Del (postmanpat)"
bpm = {BPM}
lines_per_beat = 4
speed = 6
tracks = {CH}
key = "A Phrygian"
''')
print('''[[instrument]]
slot = 1
name = "Deep Kick"
patch = "builtin:trance-kick"
drive = 0.12
cutoff = 2800.0
pitch_env = [36.0, 0.012]
decay = 0.36
gain = 0.9
[[instrument]]
slot = 2
name = "Sable Bass"
patch = "builtin:init"
# sine body, a quiet triangle for the harmonics the filter reveals.
# not a detuned saw: the open filter stays soft.
wave = "sine"
octave = -1
osc2 = { wave = "triangle", semi = 0, level = 0.16 }
detune = 0.0
filter_model = "svf"
slope = 12
cutoff = 80.0
resonance = 0.28
env_amount = 0.0
filter_attack = 0.001
filter_decay = 0.04
filter_sustain = 0.0
attack = 0.001
decay = 0.045
sustain = 0.0
release = 0.028
gain = 0.62
# fx 1 = HP so the kick keeps the very bottom
fx = [{ kind = "Filter", mode = 1.0, cutoff = 42.0, resonance = 0.0 }]
[[instrument]]
slot = 3
name = "Closed Hat"
patch = "gen:Closed Hat"
fx = [{ kind = "Filter", mode = 1.0, cutoff = 3500.0, resonance = 0.05 }]
[[instrument]]
slot = 4
name = "Open Hat"
patch = "gen:Open Hat"
fx = [{ kind = "Filter", mode = 1.0, cutoff = 2800.0, resonance = 0.0 }]
[[instrument]]
slot = 5
name = "Dust"
patch = "builtin:shaker"
cutoff = 6000.0
decay = 0.05
gain = 0.7
[[instrument]]
slot = 6
name = "Phrygian Bleep"
patch = "builtin:init"
wave = "triangle"
osc2 = { wave = "pulse", semi = 0, level = 0.18 }
cutoff = 420.0
resonance = 0.2
env_amount = 0.1
filter_attack = 0.001
filter_decay = 0.04
filter_sustain = 0.0
attack = 0.001
decay = 0.05
sustain = 0.0
release = 0.025
glide_ms = 0.0
gain = 0.4
# fx 1 = HP, dark and short; the synth low-pass does the movement
fx = [{ kind = "Filter", mode = 1.0, cutoff = 240.0, resonance = 0.0 }]
''')
PAD_KNOBS = {
    "aa": 0.42, "ad": 0.55, "as": 0.8, "ar": 0.5,
    "cutoff": u("cutoff", 700.0), "reso": 0.18, "f_env": 0.15,
    "rev_mix": 0.55, "echo_mix": 0.08, "voices": 1.0, "master": 0.55,
}
print(f'''[[instrument]]
slot = 7
name = "Night Pad"
patch = "gen:StringWave Slow Swell Orchestra"
[instrument.gen.engine.Strings]
filter_on = true
lfo_on = [true, false]
lfo_wave = ["Sine", "Sine"]
lfo_dest = ["Cutoff", "Pan"]
rev_on = true
[instrument.gen.engine.Strings.knobs]
''' + "\n".join(f"{k} = {v}" for k, v in PAD_KNOBS.items()))
print('''
[[instrument]]
slot = 8
name = "Eerie Sweep"
patch = "builtin:init"
wave = "saw"
noise = 0.7
filter = "lp"
cutoff = 280.0
resonance = 0.72
attack = 0.6
decay = 4.0
sustain = 0.75
release = 1.4
gain = 0.4
[[instrument]]
slot = 9
name = "Fall"
patch = "gen:Downlifter"
[[instrument]]
slot = 10
name = "Rim"
patch = "gen:Rimshot"
gain = 0.6
''')
print('''[mixer]
master_db = 0.0
duck_key = 1
duck_release_ms = 80
[mixer.limiter]
enabled = true
gain_db = 4.0
ceiling_db = -1.5
release_ms = 120.0
target_lufs = -14.0
''')
chans = [
    ("Kick", -1.0, 0.0, 0.0, 0.0, 0.0),
    # quiet in the mix; a little send to the high-passed echo, no reverb
    ("Bass", -9.0, 0.0, 0.07, 0.0, 0.55),
    ("Hat", -14.0, 0.15, 0.0, 0.04, 0.0),
    ("Open Hat", -16.0, -0.25, 0.05, 0.08, 0.1),
    ("Dust", -16.0, 0.35, 0.0, 0.05, 0.15),
    ("Riff", -10.0, -0.2, 0.06, 0.12, 0.4),
    ("Pad L", -10.0, -0.55, 0.05, 0.45, 0.55),
    ("Pad R", -10.0, 0.55, 0.05, 0.45, 0.55),
    ("Sweep", -8.0, 0.0, 0.15, 0.4, 0.0),
    ("Fall", -12.0, 0.0, 0.1, 0.35, 0.0),
    ("Rim", -12.0, -0.1, 0.08, 0.12, 0.25),
]
for name, vol, pan, sa, sb, duck in chans:
    print(f'[[mixer.channel]]\nname = "{name}"\nvolume_db = {vol}\npan = {pan}\n'
          f'send_a = {sa}\nsend_b = {sb}\nduck = {duck}')
print('''
[mixer.return_a]
sync = 3.0
feedback = 0.42
pingpong = 1.0
hp = 350.0
lp = 4500.0
mix = 1.0
level = 1.1
[mixer.return_b]
size = 0.9
damping = 0.62
predelay = 28.0
width = 1.0
mix = 1.0
lowcut = 180.0
highcut = 8000.0
level = 1.2
''')
emit_blocks(blocks, ORDER, "# row | kick | bass | hat | open hat | dust | riff | pad L | pad R | sweep | fall | rim")

intro_end = at(EARLY)      # bar 33: the 32-bar bass sweep has opened
hats_end = at(E)           # pad under the theme
riff_at = at(EARLY)
groove = at(I)
room = at(K)
sweep = at(L)
fall = at(N)
back = at(O)
riff2 = at(Q)
peak = at(R)
outro = at(S)
last = at(T)

# opens across the first 32 bars, and stays under 750 Hz so the mids stay soft
lane("instrument.2.cutoff", "Sable Bass - resonant LP", [
    (0, 70.0), (intro_end, 620.0), (hats_end, 540.0),
    (groove, 640.0), (room, 80.0, "step"), (back, 90.0),
    (back + 8 * B, 500.0), (peak, 680.0), (outro, 220.0), (total, 80.0)])
lane("instrument.2.resonance", "Sable Bass - resonance", [
    (0, 0.42), (intro_end, 0.26), (groove, 0.22), (room, 0.3),
    (back, 0.36), (peak, 0.2), (total, 0.34)], curve="linear")
lane("instrument.3.fx.1.cutoff", "Closed Hat - HP", [
    (0, 7000.0), (at(EARLY), 2800.0), (groove, 4000.0), (room, 5000.0),
    (back, 2400.0), (peak, 3600.0), (total, 5000.0)])
lane(f"instrument.7.gen.{GEN_CUTOFF}", "Night Pad - cutoff", [
    (0, u("cutoff", 400)), (hats_end, u("cutoff", 500)),
    (riff_at, u("cutoff", 1400)), (room, u("cutoff", 800)),
    (sweep, u("cutoff", 1100)), (back, u("cutoff", 900)),
    (peak, u("cutoff", 1600)), (total, u("cutoff", 450))])
lane(f"instrument.7.gen.{GEN_REV}", "Night Pad - reverb", [
    (0, 0.4), (hats_end, 0.55), (room, 0.75), (back, 0.5), (total, 0.7)])
lane("instrument.6.cutoff", "Phrygian Bleep - LP", [
    (0, 280.0), (riff_at, 360.0), (riff_at + 8 * B, 780.0),
    (groove, 640.0), (room, 240.0, "step"), (riff2, 420.0),
    (peak, 980.0), (outro, 360.0), (total, 260.0)])
lane("instrument.8.cutoff", "Eerie Sweep - one arc", [
    (0, 300.0), (sweep, 260.0), (sweep + 4 * B, 220.0),
    (sweep + 12 * B, 7400.0), (fall, 420.0), (back, 300.0)])
lane("instrument.8.resonance", "Eerie Sweep - resonance", [
    (0, 0.7), (sweep, 0.88), (fall, 0.6), (total, 0.5)], curve="linear")
lane("mixer.return_b.size", "Hall size", [
    (0, 0.72), (room, 0.95), (back, 0.8), (total, 0.9)], curve="linear")
lane(f"mod.1.amount", "Pad drift depth", [
    (0, 0.0), (hats_end, 0.04), (room, 0.12), (back, 0.06), (total, 0.02)],
     curve="linear")

print("# Slow cutoff drift on the pad once it is in the song.")
modulator(1, I_PAD, "Pad drift", shape="Sine", sync=32.0)
route(1, f"instrument.7.gen.{GEN_CUTOFF}", 0.0)
print(f"# positions: intro filter ends at line {intro_end}, sweep at {sweep}, "
      f"return at {back}, song {total} lines")
