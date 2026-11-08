#!/usr/bin/env python3
"""Native sketch of a 4:12 reference, not a bundled demo and not that recording.

Measured from the file (DSP, not a listening pass): about 140.6 BPM on the low
band, duration 252.44 s, chroma closest to C minor, low end steady near 55 Hz
(A1) and near 52 Hz (G#1) in one loud stretch. The repeating figure below uses
those pitch classes. It is not a note-for-note transcription; the mid-band
pitch track was too unstable to recover a melody.

  python3 scripts/make_hypnotic_matrix.py > sketches/hypnotic_matrix.cwt.toml
"""
from cwdemo import B, ROWS, Block, Rng, emit_blocks, lane

BPM = 140.7  # 148 bars at this tempo is 252.5 s; the file is 252.44 s
(KICK, BASS, HAT, CLAP, LEAD, PAD) = range(6)
CH = 6
(I_KICK, I_BASS, I_HAT, I_CLAP, I_LEAD, I_PAD) = range(1, 7)
rng = Rng(1407)

CELL = [
    (0, "C-5", 1), (4, "D#5", 1), (8, "G-4", 1), (12, "D#5", 1),
]
CELL_B = [
    (2, "C-5", 1), (6, "G-4", 1), (10, "G#4", 1), (14, "D#5", 1),
]
# four-bar phrase: the cell, then the same cell with the flat sixth
PHRASE = []
for bar in range(4):
    src = CELL_B if bar == 2 else CELL
    for row, name, ln in src:
        PHRASE.append((bar * B + row, name, ln))


def pulse(b, row, ch, name, ins, vol):
    b.put(row, ch, name, ins, vol)
    if row + 1 < ROWS:
        b.off(row + 1, ch)


def kick(b, bars=range(4), vol=0x36, beats=(0, 1, 2, 3)):
    for bar in bars:
        for beat in beats:
            b.put(bar * B + beat * 4, KICK, "C-3", I_KICK, vol)


def hats(b, bars=range(4), vol=0x16):
    for bar in bars:
        for beat in range(4):
            b.put(bar * B + beat * 4 + 2, HAT, "F#5", I_HAT, rng.jitter(vol, 1), "0810")


def claps(b, bars=range(4), vol=0x28):
    for bar in bars:
        b.put(bar * B + 4, CLAP, "D-4", I_CLAP, vol)
        b.put(bar * B + 12, CLAP, "D-4", I_CLAP, vol - 2)


def bass(b, note="A-2", bars=range(4), vol=0x2C):
    """one short pulse per beat, off the kick"""
    for bar in bars:
        for beat in range(4):
            pulse(b, bar * B + beat * 4 + 2, BASS, note, I_BASS, rng.jitter(vol, 1))


def lead(b, vol=0x2A):
    for row, name, ln in PHRASE:
        pulse(b, row, LEAD, name, I_LEAD, rng.jitter(vol, 1))


def pad(b, vol=0x22):
    for bar, (low, high) in enumerate((("C-4", "G-4"), ("C-4", "G-4"), ("G#3", "D#4"), ("C-4", "G-4"))):
        b.put(bar * B, PAD, low, I_PAD, vol)
        b.off(bar * B + 14, PAD)


blocks = []


def blk(name, role):
    b = Block(name, role, CH)
    blocks.append(b)
    return len(blocks) - 1, b


SP, b = blk("Cold open", "intro")
kick(b, beats=(0, 2), vol=0x28)
bass(b, "A-2", vol=0x24)
GR, b = blk("Grid", "intro")
kick(b); hats(b, vol=0x12); bass(b, "A-2", vol=0x28); lead(b, 0x18)
DR, b = blk("Matrix", "drop")
kick(b, vol=0x38); hats(b, vol=0x1A); claps(b); bass(b, "G#2", vol=0x2A); lead(b, 0x2C); pad(b, 0x20)
BR, b = blk("Open glass", "breakdown")
hats(b, vol=0x14); lead(b, 0x26); pad(b, 0x28)
RT, b = blk("Back on A", "drop")
kick(b); hats(b, vol=0x18); claps(b, vol=0x24); bass(b, "A-2", vol=0x28); lead(b, 0x2A); pad(b, 0x1E)
VA, b = blk("Turn the cell", "drop")
kick(b); hats(b, vol=0x16); claps(b, bars=range(0, 4, 2), vol=0x20)
bass(b, "A-2", vol=0x26); lead(b, 0x24); pad(b, 0x1C)
PK, b = blk("Bright matrix", "drop")
kick(b, vol=0x38); hats(b, vol=0x1C); claps(b, vol=0x2A)
bass(b, "A-2", vol=0x2A); lead(b, 0x2E); pad(b, 0x22)
EN, b = blk("Last grid", "outro")
kick(b, bars=range(3), vol=0x30); hats(b, bars=range(2), vol=0x10)
bass(b, "A-2", bars=range(3), vol=0x22); lead(b, 0x18); pad(b, 0x16)

# 2+4+6+2+4+6+4+6+3 = 37 blocks = 148 bars
ORDER = (
    [SP] * 2 + [GR] * 4 + [DR] * 6 + [BR] * 2 + [RT] * 4
    + [VA] * 8 + [PK] * 8 + [EN] * 3
)
# PK*4 + PK*2 is the peak (24 bars). Split only so the comment stays honest.
assert len(ORDER) == 37, len(ORDER)


def at(block, nth=0):
    return [i for i, x in enumerate(ORDER) if x == block][nth] * ROWS


total = len(ORDER) * ROWS
bars = len(ORDER) * 4
secs = bars * 4 * 60 / BPM

print(f'''# Native sketch "Hypnotic Matrix" (scripts/make_hypnotic_matrix.py). Not the original recording.
# Tempo {BPM} so {bars} bars land on {secs:.2f} s. Low end and chroma were measured; the
# repeating figure is an inference from C, Eb, G and Ab, not a transcribed melody.
format = "chipwave-song-text"
format_version = 2

[song]
title = "Hypnotic Matrix (native sketch)"
artist = "Del (postmanpat)"
comment = "Native sketch. Reference title Hypnotic Matrix, artist craftyfox. This is not that recording."
bpm = {BPM}
lines_per_beat = 4
speed = 6
tracks = {CH}
key = "C minor"
''')
print('''[[instrument]]
slot = 1
name = "Pulse Kick"
patch = "builtin:trance-kick"
drive = 0.08
cutoff = 2400.0
decay = 0.28
gain = 0.85
[[instrument]]
slot = 2
name = "Low A"
patch = "builtin:init"
wave = "sine"
octave = -1
osc2 = { wave = "triangle", semi = 0, level = 0.08 }
cutoff = 140.0
resonance = 0.12
env_amount = 0.0
attack = 0.002
decay = 0.09
sustain = 0.0
release = 0.04
gain = 0.7
fx = [{ kind = "Filter", mode = 1.0, cutoff = 40.0, resonance = 0.0 }]
[[instrument]]
slot = 3
name = "Tick"
patch = "gen:Closed Hat"
fx = [{ kind = "Filter", mode = 1.0, cutoff = 4000.0, resonance = 0.0 }]
[[instrument]]
slot = 4
name = "Clap"
patch = "gen:808 Clap"
[[instrument]]
slot = 5
name = "Minor Cell"
patch = "builtin:init"
wave = "triangle"
osc2 = { wave = "square", semi = 12, level = 0.12 }
cutoff = 700.0
resonance = 0.18
env_amount = 0.08
filter_decay = 0.06
filter_sustain = 0.0
attack = 0.002
decay = 0.07
sustain = 0.15
release = 0.05
gain = 0.45
fx = [{ kind = "Filter", mode = 1.0, cutoff = 200.0, resonance = 0.0 }]
[[instrument]]
slot = 6
name = "Minor Pad"
patch = "builtin:init"
wave = "saw"
unison = 2
detune = 8.0
cutoff = 900.0
resonance = 0.08
attack = 0.4
decay = 0.8
sustain = 0.7
release = 0.6
gain = 0.28
''')
print('''[mixer]
master_db = -1.0
duck_key = 1
duck_release_ms = 110
[mixer.limiter]
enabled = true
gain_db = 9.0
ceiling_db = -1.5
release_ms = 140.0
target_lufs = -14.0
''')
chans = [
    ("Kick", -2.0, 0.0, 0.0, 0.0, 0.0),
    ("Bass", -6.0, 0.0, 0.0, 0.0, 0.45),
    ("Hat", -14.0, 0.2, 0.0, 0.05, 0.0),
    ("Clap", -10.0, -0.05, 0.08, 0.15, 0.1),
    ("Lead", -8.0, -0.15, 0.08, 0.18, 0.25),
    ("Pad", -12.0, 0.0, 0.04, 0.35, 0.4),
]
for name, vol, pan, sa, sb, duck in chans:
    print(f'[[mixer.channel]]\nname = "{name}"\nvolume_db = {vol}\npan = {pan}\n'
          f'send_a = {sa}\nsend_b = {sb}\nduck = {duck}')
print('''
[mixer.return_a]
sync = 3.0
feedback = 0.28
pingpong = 1.0
hp = 400.0
lp = 5000.0
mix = 1.0
level = 0.8
[mixer.return_b]
size = 0.7
damping = 0.55
predelay = 20.0
width = 1.0
mix = 1.0
lowcut = 200.0
highcut = 8000.0
level = 0.9
''')
emit_blocks(blocks, ORDER, "# row | kick | bass | hat | clap | lead | pad")

drop = at(DR)
brk = at(BR)
back = at(RT)
vary = at(VA)
peak = at(PK)
end = at(EN)
# brightness follows the measured spectral centroid: darker body, a bright
# hole around the breakdown, brighter again in the last peak
lane("instrument.5.cutoff", "Minor Cell - LP", [
    (0, 500.0), (drop, 900.0), (brk, 1400.0), (brk + 4 * B, 3200.0),
    (back, 1200.0), (vary, 1000.0), (peak, 1600.0), (peak + 16 * B, 2600.0),
    (end, 900.0), (total, 400.0)])
lane("instrument.2.cutoff", "Low A - body", [
    (0, 90.0), (drop, 160.0), (brk, 70.0), (back, 150.0), (peak, 180.0), (total, 80.0)])
print(f"# {bars} bars, about {secs:.2f} s, theme block starts at line {drop}")
