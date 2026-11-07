#!/usr/bin/env python3
"""Generate demos/sable_current.cwt.toml, the demo "Sable Current".

An original dark psytrance instrumental in A Phrygian at 145 BPM, about 7:04.
Kick and a rolling bass alone for the first 32 bars, with a resonant low-pass
opening across that whole span. A dark StringWave pad enters later. The middle
drops the kick and the roll for one long resonant sweep, then the roll returns.
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


def roll(b, bars=range(4), vol=0x36, turn=False):
    """three 16ths after every kick. A Phrygian colour on the last beat when turn."""
    for bar in bars:
        for beat in range(4):
            if turn and bar == 3 and beat == 3:
                notes = ("A#3", "A-3", "G-3")
            elif turn and bar == 1 and beat == 2:
                notes = ("A-3", "A-3", "E-3")
            else:
                notes = ("A-3", "A-3", "A-3")
            for k, n in enumerate(notes):
                # the 16th inside the kick is quieter; the duck does the rest
                v = vol - 8 if k == 0 else vol
                b.put(bar * B + beat * 4 + 1 + k, BASS, n, I_BASS, rng.jitter(v, 1))


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


# two answering phrases, A Phrygian, written for this song
RIFF_A = [
    (0, "A-4", 3), (4, "C-5", 2), (8, "A#4", 3), (12, "A-4", 2),
    (16, "G-4", 3), (20, "A-4", 2), (24, "E-4", 3), (28, "F-4", 2),
    (36, "C-5", 2), (40, "A#4", 2), (44, "A-4", 3), (48, "E-4", 4),
    (56, "A-4", 2), (60, "A#4", 3),
]
RIFF_B = [
    (0, "E-5", 2), (4, "C-5", 2), (8, "A#4", 4), (14, "A-4", 2),
    (18, "G-4", 2), (22, "F-4", 2), (26, "E-4", 4),
    (32, "A-4", 2), (36, "C-5", 2), (40, "D-5", 2), (44, "C-5", 2), (48, "A#4", 4),
    (56, "G-4", 2), (60, "A-4", 3),
]
RIFF_C = [
    (2, "C-5", 2), (6, "A#4", 2), (10, "A-4", 4),
    (18, "F-4", 2), (22, "E-4", 2), (26, "F-4", 2), (30, "A-4", 2),
    (34, "C-5", 3), (40, "E-5", 2), (44, "C-5", 2), (48, "A#4", 6),
    (58, "A-4", 4),
]

blocks = []


def blk(name, role):
    b = Block(name, role, CH)
    blocks.append(b)
    return len(blocks) - 1, b


# ---- arrangement: 64 plays of a 4-bar block = 256 bars, 7:04 at 145 BPM ----
A, b = blk("Current, closed", "intro")
kick(b); roll(b, turn=False)
TURN, b = blk("Current, a turn", "intro")
kick(b); roll(b, turn=True)
C, b = blk("Offbeat dust", "intro")
kick(b); roll(b, turn=True); hats(b, vol=0x14)
D, b = blk("Shaker under the roll", "intro")
kick(b); roll(b, turn=True); hats(b, vol=0x18); shaker(b, vol=0x12, step=2)
E, b = blk("Pad enters", "build")
kick(b); roll(b, vol=0x34, turn=True); hats(b, vol=0x1A); shaker(b, vol=0x12, step=2)
pad(b, vol=0x28)
F, b = blk("Pad leans on the flat two", "build")
kick(b); roll(b, vol=0x34, turn=True); hats(b, vol=0x1A); open_hat(b, vol=0x16)
pad(b, "A#3", "F-4", vol=0x2A)
G, b = blk("First riff", "drop")
kick(b); roll(b, turn=True); hats(b, vol=0x1C); shaker(b, vol=0x14, step=2); rim(b, vol=0x1E)
pad(b, vol=0x26); riff(b, RIFF_A, 0x2E)
H, b = blk("Riff answers", "drop")
kick(b); roll(b, turn=True); hats(b, vol=0x1C, sixteenths=True); open_hat(b, vol=0x18)
rim(b, vol=0x20); pad(b, vol=0x24); riff(b, RIFF_B, 0x30)
I, b = blk("Full current", "drop")
kick(b); roll(b, vol=0x38, turn=True); hats(b, vol=0x1E, sixteenths=True)
open_hat(b, vol=0x1A, beats=(1, 3)); rim(b, vol=0x22); pad(b, vol=0x24); riff(b, RIFF_A, 0x32)
J, b = blk("Full current, fill", "drop")
kick(b); roll(b, vol=0x38, turn=True); hats(b, vol=0x1C, sixteenths=True)
open_hat(b, vol=0x18); rim(b, bars=range(3), vol=0x20); fill(b); pad(b, vol=0x22)
riff(b, RIFF_B, 0x30)
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
kick(b, vol=0x3A); roll(b, vol=0x34, turn=False)
P, b = blk("Hats return", "drop")
kick(b); roll(b, turn=True); hats(b, vol=0x16)
Q, b = blk("Riff returns", "drop")
kick(b); roll(b, turn=True); hats(b, vol=0x1C); shaker(b, vol=0x12, step=2)
pad(b, vol=0x26); riff(b, RIFF_C, 0x30)
R, b = blk("Open water", "drop")
kick(b); roll(b, vol=0x38, turn=True); hats(b, vol=0x20, sixteenths=True)
open_hat(b, vol=0x1C, beats=(1, 3)); rim(b); pad(b, vol=0x26); riff(b, RIFF_C, 0x34)
S, b = blk("Peel back", "outro")
kick(b, bars=range(3)); roll(b, bars=range(3), turn=True); hats(b, bars=range(2), vol=0x14)
pad(b, vol=0x24)
T, b = blk("Last current", "outro")
kick(b, bars=range(2), vol=0x30); roll(b, bars=range(2), vol=0x2C, turn=False)
pad(b, bars=range(1), vol=0x20)
for ch in (PAD, PADR):
    b.off(16 + 8, ch)

ORDER = (
    [A] * 4 + [TURN] * 4 + [C] * 4 + [D] * 4 + [E] * 4 + [F] * 4
    + [G] * 4 + [H] * 4 + [I] * 4 + [J] * 4
    + [K] * 2 + [L] + [M] * 3 + [N] * 2
    + [O] * 2 + [P] * 2 + [Q] * 4 + [R] * 4 + [S] * 2 + [T] * 2
)
assert len(ORDER) == 64, len(ORDER)


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
wave = "saw"
octave = -1
osc2 = { wave = "square", semi = 0, level = 0.35 }
detune = 6.0
filter_model = "ladder"
slope = 24
cutoff = 140.0
resonance = 0.62
env_amount = 0.08
filter_attack = 0.001
filter_decay = 0.12
filter_sustain = 0.55
attack = 0.001
decay = 0.16
sustain = 0.7
release = 0.05
gain = 0.85
# fx 1 = HP so the kick keeps the bottom
fx = [{ kind = "Filter", mode = 1.0, cutoff = 48.0, resonance = 0.0 }]
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
name = "Phrygian Riff"
patch = "builtin:init"
wave = "square"
osc2 = { wave = "saw", semi = -12, level = 0.2 }
cutoff = 900.0
resonance = 0.45
env_amount = 0.35
filter_decay = 0.12
attack = 0.002
decay = 0.14
sustain = 0.35
release = 0.08
glide_ms = 40.0
gain = 0.55
fx = [{ kind = "Filter", mode = 1.0, cutoff = 180.0, resonance = 0.05 }]
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
    ("Bass", -2.5, 0.0, 0.0, 0.0, 0.78),
    ("Hat", -14.0, 0.15, 0.0, 0.04, 0.0),
    ("Open Hat", -16.0, -0.25, 0.05, 0.08, 0.1),
    ("Dust", -16.0, 0.35, 0.0, 0.05, 0.15),
    ("Riff", -8.0, -0.2, 0.22, 0.2, 0.4),
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

intro_end = at(C)          # 32 bars: filter has opened
hats_end = at(E)           # pad enters
riff_at = at(G)
groove = at(I)
room = at(K)
sweep = at(L)
fall = at(N)
back = at(O)
riff2 = at(Q)
peak = at(R)
outro = at(S)
last = at(T)

lane("instrument.2.cutoff", "Sable Bass - resonant LP", [
    (0, 90.0), (intro_end, 4200.0), (hats_end, 1800.0), (riff_at, 1400.0),
    (groove, 1450.0), (room, 180.0, "step"), (back, 170.0),
    (back + 8 * B, 880.0), (peak, 1600.0), (outro, 700.0), (total, 160.0)])
lane("instrument.2.resonance", "Sable Bass - resonance", [
    (0, 0.78), (intro_end, 0.5), (groove, 0.38), (room, 0.7),
    (back, 0.66), (peak, 0.34), (total, 0.72)], curve="linear")
lane("instrument.3.fx.1.cutoff", "Closed Hat - HP", [
    (0, 7000.0), (at(C), 2200.0), (groove, 4000.0), (room, 5000.0),
    (back, 2400.0), (peak, 3600.0), (total, 5000.0)])
lane(f"instrument.7.gen.{GEN_CUTOFF}", "Night Pad - cutoff", [
    (0, u("cutoff", 400)), (hats_end, u("cutoff", 500)),
    (riff_at, u("cutoff", 1400)), (room, u("cutoff", 800)),
    (sweep, u("cutoff", 1100)), (back, u("cutoff", 900)),
    (peak, u("cutoff", 1600)), (total, u("cutoff", 450))])
lane(f"instrument.7.gen.{GEN_REV}", "Night Pad - reverb", [
    (0, 0.4), (hats_end, 0.55), (room, 0.75), (back, 0.5), (total, 0.7)])
lane("instrument.6.cutoff", "Phrygian Riff - LP", [
    (0, 400.0), (riff_at, 500.0), (riff_at + 16 * B, 1800.0),
    (groove, 1400.0), (room, 300.0, "step"), (riff2, 600.0),
    (peak, 2200.0), (outro, 800.0), (total, 400.0)])
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
