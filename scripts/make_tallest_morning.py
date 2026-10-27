#!/usr/bin/env python3
"""Generate demos/tallest_morning.cwt.toml, the demo "Tallest Morning".

An original, uplifting melodic-house / euphoric dance instrumental: D major,
126 BPM, about 2:20. Built to show off StringWave and the modulation system:

* the chords come from ChipWave's own Chords generator (style "EDM / Trance",
  root D, progression vi-IV-I-V, seed 3), voiced by its Triads and Pad
  voicings: Bm7 - G/B - D/A - A, the same notes the generator writes;
* StringWave "Echo Strings" plays the off-beat chord stabs, its own stereo echo
  set to a dotted 1/8 (0.357 s at 126 BPM) so every stab answers itself, and
  StringWave "Slow Swell Orchestra" plays the pads, both split L / C / R;
* modulation: StringWave's own LFOs (pan sway on the stabs, a cutoff drift on
  the pads) and mod envelope (a pluck on the stab filter); song-level quick
  mods (a "Lift" macro routed to five targets whose depths rise on every build,
  pad and lead LFOs, a lead envelope, a velocity route, random hat drift);
  automation lanes sweep StringWave's cutoff, echo and reverb knobs live; the
  kick ducks bass, stabs and pads (sidechain pumping).

Every note here was written for ChipWave; no melody, hook, bass line or chord
progression is taken from any existing song.

  python3 scripts/make_tallest_morning.py > demos/tallest_morning.cwt.toml
"""
import math

from cwdemo import B, ROWS, Block, Rng, m, num, emit_blocks, lane, modulator, route

BPM = 126
(KICK, CLAP, HAT, OHAT, ROLL, BASS, STAB1, STAB2, STAB3, PAD1, PAD2, PAD3, LEAD, LEAD2, FX,
 CRASH) = range(16)
CH = 16
(I_KICK, I_CLAP, I_HAT, I_OHAT, I_ROLL, I_BASS, I_STAB, I_PAD, I_LEAD, I_RISE, I_DOWN,
 I_IMPACT, I_CYM) = range(1, 14)
rng = Rng(126)

# ---- StringWave knob ranges (src/strings/mod.rs): name -> (lo, hi, exponential) ---------
KN = {
    "cutoff": (30.0, 18000.0, True), "aa": (0.001, 10.0, True), "ar": (0.005, 15.0, True),
    "fa": (0.001, 10.0, True), "fd": (0.005, 10.0, True), "fr": (0.005, 15.0, True),
    "ad": (0.005, 10.0, True), "lfo1_rate": (0.02, 20.0, True), "lfo2_rate": (0.02, 20.0, True),
    "me_a": (0.001, 10.0, True), "me_d": (0.005, 10.0, True), "echo_time": (0.02, 1.5, True),
    "echo_fb": (0.0, 0.95, False), "f_env": (-1.0, 1.0, False), "me_amt": (-1.0, 1.0, False),
    "master": (-48.0, 6.0, False),
}
# 1-based automation index of a knob: instrument.N.gen.K
GEN = {"cutoff": 1, "rev_mix": 6, "echo_fb": 46, "echo_mix": 48}


def u(name, x):
    """a knob value in real units -> the normalized 0..1 value files and lanes store"""
    lo, hi, ex = KN.get(name, (0.0, 1.0, False))
    v = math.log(x / lo) / math.log(hi / lo) if ex else (x - lo) / (hi - lo)
    return round(min(1.0, max(0.0, v)), 4)


BEAT = 60.0 / BPM
ECHO = 0.75 * BEAT  # a dotted 1/8 = 0.357 s

# ---- harmony: the Chords generator's output (EDM / Trance, D, vi-IV-I-V, seed 3) -------
# Triads voicing (top three notes) for the stabs, Pad voicing an octave down for the pads,
# chord roots for the bass.
PROG = ["Bm7", "G/B", "D/A", "A"]
STAB = {"Bm7": ("B-4", "D-5", "F#5"), "G/B": ("B-4", "D-5", "G-5"),
        "D/A": ("A-4", "D-5", "F#5"), "A": ("A-4", "C#5", "E-5"), "D": ("A-4", "D-5", "F#5")}
PAD = {"Bm7": ("D-4", "F#4", "A-4"), "G/B": ("B-3", "D-4", "G-4"),
       "D/A": ("A-3", "D-4", "F#4"), "A": ("A-3", "C#4", "E-4"), "D": ("A-3", "D-4", "F#4")}
ROOT = {"Bm7": "B-1", "G/B": "G-1", "D/A": "D-2", "A": "A-1", "D": "D-2"}

# ---- the hook: a rising, singable 16th phrase per bar (row, note, length) --------------
H_BARS = [
    [(0, "F#5", 3), (3, "F#5", 3), (6, "E-5", 2), (8, "D-5", 2), (10, "E-5", 2), (12, "F#5", 4)],
    [(16, "D-5", 3), (19, "D-5", 3), (22, "B-4", 2), (24, "D-5", 2), (26, "E-5", 2), (28, "G-5", 4)],
    [(32, "A-5", 3), (35, "A-5", 3), (38, "F#5", 2), (40, "E-5", 2), (42, "D-5", 2), (44, "F#5", 4)],
]
HOOK_A = sum(H_BARS, []) + [(48, "E-5", 3), (51, "E-5", 3), (54, "C#5", 2), (56, "E-5", 2),
                            (58, "F#5", 2), (60, "A-5", 4)]
HOOK_B = sum(H_BARS, []) + [(48, "E-5", 3), (51, "F#5", 3), (54, "A-5", 4), (58, "B-5", 2),
                            (60, "C#6", 4)]
HOOK_END = sum(H_BARS, []) + [(48, "E-5", 3), (51, "F#5", 3), (54, "A-5", 2), (56, "C#6", 4),
                              (60, "D-6", 4)]
# the second lead in the final drop: a diatonic third / sixth below the hook (D major)
SCALE = [0, 2, 4, 5, 7, 9, 11]


def third_below(note):
    k = num(note)
    pc = (k - 2) % 12  # degree in D major
    deg = SCALE.index(pc) if pc in SCALE else 0
    d2 = deg - 2
    o = (k - 2) // 12 + (d2 // 7)
    return m(o * 12 + 2 + SCALE[d2 % 7])


# ---- parts -------------------------------------------------------------------------------
def kick(b, bars=range(4), vol=0x3E, stop=None):
    for bar in bars:
        for r in (0, 4, 8, 12):
            if stop is None or bar * B + r < stop:
                b.put(bar * B + r, KICK, "C#4", I_KICK, vol)


def clap(b, bars=range(4), vol=0x34):
    for bar in bars:
        for r in (4, 12):
            b.put(bar * B + r, CLAP, "C-4", I_CLAP, rng.jitter(vol, 1))


def hats(b, bars=range(4), vol=0x16, open_=True):
    for bar in bars:
        for r in range(0, 16):
            if r % 4 == 2:
                if open_:
                    b.put(bar * B + r, OHAT, "C-4", I_OHAT, rng.jitter(vol + 6, 1))
                b.put(bar * B + r, HAT, "C-4", I_HAT, rng.jitter(vol + 4, 1))
            elif r % 2 == 1 or r % 4 == 0:
                b.put(bar * B + r, HAT, "C-4", I_HAT, rng.jitter(vol - (4 if r % 2 else 0), 2))


def roll(b, start_bar, bars=2, lo=0x12, hi=0x38):
    rows = range(start_bar * B, (start_bar + bars) * B)
    for k, r in enumerate(rows):
        f = k / max(1, len(rows) - 1)
        step = 4 if f < 0.4 else (2 if f < 0.75 else 1)
        if r % step == 0:
            b.put(r, ROLL, "C-4", I_ROLL, int(lo + (hi - lo) * f))


def bass(b, prog=PROG, bars=range(4), vol=0x3A, chord=None):
    """the drop bass: off-beat root with a pushed octave, ducked by the kick"""
    for bar in bars:
        root = ROOT[chord or prog[bar % 4]]
        for r in (2, 6, 10, 14):
            b.put(bar * B + r, BASS, root, I_BASS, rng.jitter(vol, 1))
            b.off(min(bar * B + r + 2, ROWS - 1), BASS)
        b.put(bar * B + 15, BASS, m(num(root) + 12), I_BASS, vol - 10)


def bass_pulse(b, prog=PROG, bars=range(4), vol=0x34):
    for bar in bars:
        root = ROOT[prog[bar % 4]]
        b.put(bar * B, BASS, root, I_BASS, vol)
        b.off(bar * B + 12, BASS)


def stabs(b, prog=PROG, bars=range(4), vol=0x34, rows=(2, 6, 10, 14), ln=2):
    """StringWave echo chord stabs: the generator's triads, split over three tracks"""
    for bar in bars:
        for r in rows:
            for j, ch in enumerate((STAB1, STAB2, STAB3)):
                b.put(bar * B + r, ch, STAB[prog[bar % 4]][j], I_STAB,
                      vol if r % 8 == 2 or r == 0 else vol - 5)
                b.off(min(bar * B + r + ln, ROWS - 1), ch)


def pads(b, prog=PROG, bars=range(4), vol=0x30, hold=None):
    for bar in bars:
        for j, ch in enumerate((PAD1, PAD2, PAD3)):
            b.put(bar * B, ch, PAD[prog[bar % 4]][j], I_PAD, vol)
            if hold is not None:
                b.off(bar * B + hold, ch)


def hook(b, phrase, vol=0x36, ch=LEAD, every=1, harm=False):
    for i, (r, note, ln) in enumerate(phrase):
        if i % every:
            continue
        nn = third_below(note) if harm else note
        b.put(r, ch, nn, I_LEAD, rng.jitter(vol if r % 4 == 0 else vol - 4, 1))
        b.off(min(r + ln, ROWS - 1), ch)  # the last note ends inside the block


def drop_hits(b, vol=0x34):
    b.put(0, FX, "C-4", I_IMPACT, vol)
    b.put(0, CRASH, "C-4", I_CYM, vol - 6)


blocks = []


def blk(name, role):
    b = Block(name, role, CH)
    blocks.append(b)
    return len(blocks) - 1, b


IN1, b = blk("First light", "intro")
pads(b, vol=0x2C)
stabs(b, rows=(0, 6), vol=0x2C)
hats(b, range(2, 4), vol=0x10, open_=False)
IN2, b = blk("Waking groove", "intro")
kick(b); hats(b, vol=0x12, open_=False); clap(b, range(2, 4), vol=0x2C)
pads(b, vol=0x2A); stabs(b, rows=(0, 6, 10), vol=0x2E); bass_pulse(b, bars=range(2, 4))
BLD1, b = blk("The climb", "build")
kick(b); clap(b); hats(b, vol=0x14); bass(b, vol=0x34); stabs(b, vol=0x2E)
pads(b, vol=0x26); hook(b, HOOK_A, vol=0x2A, every=2)
roll(b, 2, 2, lo=0x10, hi=0x2C)
b.put(0, FX, "C-4", I_RISE, 0x34)
LIFT, b = blk("Lift-off", "build")
kick(b, range(0, 3), stop=3 * B); hats(b, range(0, 3), vol=0x14)
stabs(b, rows=(0, 4, 8, 12), vol=0x30, ln=2); pads(b, vol=0x28)
hook(b, HOOK_B, vol=0x30)
roll(b, 0, 4, lo=0x16, hi=0x3C)
b.put(0, FX, "C-4", I_RISE, 0x3A)
b.put(3 * B, BASS, "===")
D1, b = blk("Drop: summit hook", "chorus")
drop_hits(b)
kick(b); clap(b); hats(b); bass(b); stabs(b); pads(b, vol=0x22); hook(b, HOOK_A)
D2, b = blk("Drop: answer", "chorus")
kick(b); clap(b); hats(b); bass(b); stabs(b); pads(b, vol=0x22); hook(b, HOOK_B)
roll(b, 3, 1, lo=0x14, hi=0x2C)
BR1, b = blk("Breakdown: echo chords", "breakdown")
b.put(0, FX, "C-4", I_DOWN, 0x34)
b.put(0, CRASH, "C-4", I_CYM, 0x26)
pads(b, vol=0x36)
stabs(b, rows=(0, 6), vol=0x32, ln=3)
b.put(0, BASS, "B-1", I_BASS, 0x30); b.off(2 * B, BASS)
BR2, b = blk("Breakdown: the hook returns", "breakdown")
pads(b, vol=0x36); stabs(b, rows=(0, 6, 10), vol=0x30, ln=3)
hook(b, HOOK_A, vol=0x2C)
b.put(2 * B, FX, "C-4", I_RISE, 0x30)
BLD2, b = blk("Second climb", "build")
kick(b, range(2, 4), vol=0x36); hats(b, range(1, 4), vol=0x12, open_=False)
pads(b, vol=0x30); stabs(b, vol=0x30); bass_pulse(b)
hook(b, HOOK_A, vol=0x30)
roll(b, 0, 4, lo=0x0E, hi=0x28)
F1, b = blk("Final drop", "chorus")
drop_hits(b, 0x38)
kick(b); clap(b); hats(b); bass(b); stabs(b, vol=0x36); pads(b, vol=0x26)
hook(b, HOOK_A, vol=0x38); hook(b, HOOK_A, vol=0x2C, ch=LEAD2, harm=True)
F2, b = blk("Final drop: answer", "chorus")
kick(b); clap(b); hats(b); bass(b); stabs(b, vol=0x36); pads(b, vol=0x26)
hook(b, HOOK_B, vol=0x38); hook(b, HOOK_B, vol=0x2C, ch=LEAD2, harm=True)
roll(b, 3, 1, lo=0x14, hi=0x2E)
FEND, b = blk("Final drop: top of the world", "chorus")
kick(b); clap(b); hats(b); bass(b); stabs(b, vol=0x36); pads(b, vol=0x26)
hook(b, HOOK_END, vol=0x38); hook(b, HOOK_END, vol=0x2C, ch=LEAD2, harm=True)
OUT1, b = blk("Outro: morning", "outro")
b.put(0, CRASH, "C-4", I_CYM, 0x2C)
kick(b); clap(b, range(0, 2)); hats(b, vol=0x12, open_=False); bass(b, bars=range(0, 2), vol=0x30)
stabs(b, rows=(0, 6, 10), vol=0x2E); pads(b, vol=0x2A)
OUT2, b = blk("Outro: last light", "outro")
b.put(0, FX, "C-4", I_DOWN, 0x2C)
kick(b, range(0, 2), vol=0x34); hats(b, range(0, 2), vol=0x10, open_=False)
pads(b, ["D"] * 4, range(1), vol=0x30)
for j, ch in enumerate((STAB1, STAB2, STAB3)):
    b.put(0, ch, STAB["D"][j], I_STAB, 0x30); b.off(3, ch)
    b.put(6, ch, m(num(STAB["D"][j]) + 12) if j == 2 else STAB["D"][j], I_STAB, 0x28); b.off(9, ch)
for ch in (PAD1, PAD2, PAD3):
    b.off(2 * B + 8, ch)
b.put(0, BASS, "D-2", I_BASS, 0x32); b.off(B, BASS)

ORDER = [IN1, IN2, BLD1, LIFT, D1, D2, D1, D2, BR1, BR2, BLD2, LIFT, F1, F2, F1, FEND, OUT1, OUT2]


def at(blk_i, nth=0):
    return [k for k, x in enumerate(ORDER) if x == blk_i][nth] * ROWS


total = len(ORDER) * ROWS

print(f'''# ChipWave demo "Tallest Morning" (generated by scripts/make_tallest_morning.py; edit that, not this)
# Original uplifting melodic-house instrumental, D major, {BPM} BPM. StringWave echo chord stabs and
# pads voiced by the Chords generator (EDM / Trance, vi-IV-I-V), a rising lead hook, sidechain
# pumping, filter-swept builds, a "Lift" macro whose route depths rise on every build, and LFO
# movement on pads, stabs and lead. Single-file text form of a .cwt song (docs/FIRST_RUN.md).
format = "chipwave-song-text"
format_version = 2

[song]
title = "Tallest Morning (uplifting house demo)"
artist = "Del (postmanpat)"
bpm = {BPM}
lines_per_beat = 4
speed = 6
tracks = {CH}
key = "D major"
''')
print('''[[instrument]]
slot = 1
name = "Sunrise Kick"
patch = "gen:909 Kick"
fx = [{ kind = "Filter", mode = 1.0, cutoff = 32.0, resonance = 0.0 }]
[[instrument]]
slot = 2
name = "Hand Clap"
patch = "gen:808 Clap"
fx = [{ kind = "Filter", mode = 1.0, cutoff = 220.0, resonance = 0.0 }]
[[instrument]]
slot = 3
name = "Tick Hat"
patch = "gen:Closed Hat"
# fx 1 = HP (opens up on the builds)
fx = [{ kind = "Filter", mode = 1.0, cutoff = 3000.0, resonance = 0.0 }]
[[instrument]]
slot = 4
name = "Offbeat Hat"
patch = "gen:Open Hat"
fx = [{ kind = "Filter", mode = 1.0, cutoff = 3500.0, resonance = 0.0 }]
[[instrument]]
slot = 5
name = "Climb Snare"
patch = "gen:909 Snare"
fx = [{ kind = "Filter", mode = 1.0, cutoff = 250.0, resonance = 0.0 }]
[[instrument]]
slot = 6
name = "Pulse Bass"
patch = "builtin:init"
wave = "saw"
osc2 = { wave = "square", semi = 0, level = 0.5 }
detune = 4.0
filter_model = "ladder"
slope = 24
cutoff = 700.0
resonance = 0.3
env_amount = 0.45
filter_attack = 0.001
filter_decay = 0.1
filter_sustain = 0.25
attack = 0.002
decay = 0.12
sustain = 0.6
release = 0.04
gain = 1.0
# fx 1 = LP (opens through the intro), fx 2 = HP 30 Hz
fx = [{ kind = "Filter", mode = 0.0, cutoff = 2500.0, resonance = 0.2 },
      { kind = "Filter", mode = 1.0, cutoff = 30.0, resonance = 0.0 }]''')
STAB_KNOBS = {
    "aa": u("aa", 0.003), "ad": u("ad", 0.4), "as": 0.25, "ar": u("ar", 0.35),
    "cutoff": u("cutoff", 3200.0), "reso": 0.2, "f_env": u("f_env", 0.35),
    "fa": u("fa", 0.002), "fd": u("fd", 0.25), "fs": 0.25,
    "echo_time": u("echo_time", ECHO), "echo_fb": u("echo_fb", 0.45), "echo_tone": 0.55,
    "echo_mix": 0.3, "rev_mix": 0.18, "cho_mix": 0.35,
    "lfo1_rate": u("lfo1_rate", BPM / 60 / 4), "lfo1_depth": 0.35,
    "lfo2_rate": u("lfo2_rate", BPM / 60 / 2), "lfo2_depth": 0.12,
    "me_a": u("me_a", 0.001), "me_d": u("me_d", 0.18), "me_amt": u("me_amt", 0.3),
    "voices": 1.0, "master": u("master", 3.0),
}
PAD_KNOBS = {
    "aa": u("aa", 0.35), "ad": u("ad", 1.5), "as": 0.85, "ar": u("ar", 1.1),
    "cutoff": u("cutoff", 1800.0), "f_env": u("f_env", 0.35),
    "fa": u("fa", 0.6), "fd": u("fd", 1.5), "fs": 0.6, "fr": u("fr", 1.2),
    "echo_time": u("echo_time", ECHO), "echo_fb": u("echo_fb", 0.35), "echo_mix": 0.12,
    "rev_mix": 0.35, "lfo1_depth": 0.06,
    "lfo2_rate": u("lfo2_rate", BPM / 60 / 8), "lfo2_depth": 0.22, "voices": 1.0, "master": u("master", 1.0),
}


def knobs(d):
    return "\n".join(f"{k} = {v}" for k, v in d.items())


print(f'''[[instrument]]
slot = 7
name = "Echo Chord Stabs"
patch = "gen:StringWave Echo Strings"
# StringWave: ensemble + plucked layers, echo at a dotted 1/8 ({ECHO:.3f} s), LFO 1 sways the
# pan over a bar, LFO 2 wobbles the cutoff every half bar, the mod envelope plucks the filter
[instrument.gen.engine.Strings]
filter_on = true
lfo_on = [true, true]
lfo_wave = ["Sine", "Triangle"]
lfo_dest = ["Pan", "Cutoff"]
me_on = true
me_dest = "Cutoff"
cho_on = true
echo_on = true
rev_on = true
[instrument.gen.engine.Strings.knobs]
{knobs(STAB_KNOBS)}
[[instrument]]
slot = 8
name = "Sunrise Pad"
patch = "gen:StringWave Slow Swell Orchestra"
# StringWave: ensemble + bowed layers; LFO 1 vibrato, LFO 2 a slow two-bar cutoff drift,
# the same dotted-1/8 echo, quieter, under a big hall
[instrument.gen.engine.Strings]
filter_on = true
lfo_on = [true, true]
lfo_wave = ["Sine", "Sine"]
lfo_dest = ["Pitch", "Cutoff"]
echo_on = true
rev_on = true
[instrument.gen.engine.Strings.knobs]
{knobs(PAD_KNOBS)}''')
print('''[[instrument]]
slot = 9
name = "Summit Lead"
patch = "builtin:init"
wave = "saw"
unison = 3
detune = 14.0
width = 1.0
osc2 = { wave = "pulse", semi = 12, level = 0.25 }
cutoff = 3600.0
resonance = 0.2
env_amount = 0.3
filter_decay = 0.3
attack = 0.004
decay = 0.35
sustain = 0.7
release = 0.2
glide_ms = 15.0
gain = 0.5
lfo = { shape = "sine", rate = 5.2, pitch = 0.04 }
# fx 1 = LP (the build sweeps), fx 2 = HP, fx 3 = chorus
fx = [{ kind = "Filter", mode = 0.0, cutoff = 9000.0, resonance = 0.25 },
      { kind = "Filter", mode = 1.0, cutoff = 280.0, resonance = 0.0 },
      { kind = "Chorus", rate = 0.4, depth = 2.4, delay = 11.0, feedback = 0.1, mix = 0.3 }]
[[instrument]]
slot = 10
name = "Climb Riser"
patch = "gen:White Riser"
fx = [{ kind = "Filter", mode = 1.0, cutoff = 400.0, resonance = 0.0 }]
[[instrument]]
slot = 11
name = "Downlifter"
patch = "gen:Downlifter"
fx = [{ kind = "Filter", mode = 1.0, cutoff = 250.0, resonance = 0.0 }]
[[instrument]]
slot = 12
name = "Drop Impact"
patch = "gen:Impact"
[[instrument]]
slot = 13
name = "Crash"
patch = "gen:Cymbal"
fx = [{ kind = "Filter", mode = 1.0, cutoff = 1500.0, resonance = 0.0 }]
''')
print('''[mixer]
master_db = 0.0
duck_key = 1
duck_release_ms = 140
[mixer.limiter]
enabled = true
gain_db = 5.3
ceiling_db = -1.5
release_ms = 110.0
target_lufs = -14.0
''')
chans = [
    ("Kick", -8.0, 0.0, 0.0, 0.0, 0.0),
    ("Clap", -9.0, 0.0, 0.0, 0.25, 0.0),
    ("Hat", -12.0, 0.25, 0.0, 0.05, 0.0),
    ("Open Hat", -14.0, -0.2, 0.0, 0.08, 0.15),
    ("Climb Snare", -10.0, 0.0, 0.1, 0.3, 0.0),
    ("Bass", -1.0, 0.0, 0.0, 0.0, 0.55),
    ("Stab L", -3.0, -0.45, 0.0, 0.15, 0.45),
    ("Stab C", -3.0, 0.0, 0.0, 0.15, 0.45),
    ("Stab R", -3.0, 0.45, 0.0, 0.15, 0.45),
    ("Pad L", -8.0, -0.6, 0.0, 0.25, 0.55),
    ("Pad C", -8.0, 0.0, 0.0, 0.25, 0.55),
    ("Pad R", -8.0, 0.6, 0.0, 0.25, 0.55),
    ("Lead", -3.0, 0.0, 0.2, 0.25, 0.15),
    ("Lead Harmony", -8.0, 0.3, 0.2, 0.3, 0.15),
    ("FX", -8.0, 0.0, 0.1, 0.4, 0.0),
    ("Crash", -11.0, -0.15, 0.0, 0.2, 0.0),
]
for name, vol, pan, sa, sb, duck in chans:
    print(f'[[mixer.channel]]\nname = "{name}"\nvolume_db = {vol}\npan = {pan}\n'
          f'send_a = {sa}\nsend_b = {sb}\nduck = {duck}')
print('''
[mixer.return_a]
sync = 3.0
feedback = 0.38
pingpong = 1.0
hp = 400.0
lp = 6000.0
mix = 1.0
level = 1.0
[mixer.return_b]
size = 0.75
damping = 0.45
predelay = 15.0
width = 1.0
mix = 1.0
lowcut = 300.0
highcut = 9500.0
level = 1.0
''')
emit_blocks(blocks, ORDER, "# row | kick | clap | hat | open hat | snare | bass | stab L C R | "
            "pad L C R | lead | harmony | fx | crash")

print("# Automation: positions in lines from the song start (64 lines = one block = 4 bars)")
g, b1, lf1, d1, br, br2, b2, lf2, f1, o1, o2 = (
    at(IN2), at(BLD1), at(LIFT), at(D1), at(BR1), at(BR2), at(BLD2), at(LIFT, 1), at(F1),
    at(OUT1), at(OUT2))
c = GEN["cutoff"]
# StringWave cutoff sweeps (normalized knob values): the intro opens, each build climbs,
# the breakdown closes down and opens again
lane(f"instrument.{I_STAB}.gen.{c}", "Echo Stabs - Cutoff", [
    (0, u("cutoff", 900)), (g, u("cutoff", 1200)), (b1, u("cutoff", 1600)),
    (d1 - 1, u("cutoff", 5000)), (d1, u("cutoff", 3200), "step"), (br, u("cutoff", 3200)),
    (br + 1, u("cutoff", 1100), "step"), (b2, u("cutoff", 1500)), (f1 - 1, u("cutoff", 6500)),
    (f1, u("cutoff", 3600), "step"), (o1, u("cutoff", 3200)), (total, u("cutoff", 700))])
lane(f"instrument.{I_PAD}.gen.{c}", "Sunrise Pad - Cutoff", [
    (0, u("cutoff", 700)), (b1, u("cutoff", 1800)), (br, u("cutoff", 1500)),
    (br2, u("cutoff", 2400)), (b2, u("cutoff", 2000)), (f1, u("cutoff", 2200)),
    (o2, u("cutoff", 1600)), (total, u("cutoff", 500))])
# echo and reverb: the breakdown lets the echo chords ring out; throws at phrase ends
pts = [(0, 0.3)]
for blk_i, k in ((D2, 0), (D2, 1), (F2, 0)):
    e = at(blk_i, k) + 4 * B
    pts += [(e - 6, 0.6), (e, 0.3)]
pts += [(br, 0.55), (b2, 0.35), (f1, 0.3), (o1, 0.45), (o2, 0.6)]
lane(f"instrument.{I_STAB}.gen.{GEN['echo_mix']}", "Echo Stabs - Echo Mix", pts, curve="step")
lane(f"instrument.{I_STAB}.gen.{GEN['echo_fb']}", "Echo Stabs - Echo Feedback", [
    (0, u("echo_fb", 0.45)), (br, u("echo_fb", 0.68)), (b2, u("echo_fb", 0.5)),
    (f1, u("echo_fb", 0.45)), (o2, u("echo_fb", 0.62))], curve="step")
lane(f"instrument.{I_PAD}.gen.{GEN['rev_mix']}", "Sunrise Pad - Reverb", [
    (0, 0.4), (d1, 0.3), (br, 0.55), (b2, 0.4), (f1, 0.3), (o1, 0.45), (total, 0.6)])
# the lead and bass filters; the hats open up on the builds
lane(f"instrument.{I_LEAD}.fx.1.p1", "Lead - LP sweep", [
    (0, 900), (b1, 900), (d1 - 1, 9000), (d1, 9000), (br2, 1200), (b2, 1500), (f1 - 1, 12000),
    (f1, 10000), (o1, 6000), (total, 800)])
lane(f"instrument.{I_BASS}.fx.1.p1", "Bass - LP sweep", [
    (0, 200), (g + 2 * B, 300), (b1, 900), (d1, 2500), (br, 2500), (br + 1, 500, "step"),
    (b2, 500), (f1, 2800), (o2, 1200), (total, 200)])
lane(f"instrument.{I_HAT}.fx.1.p1", "Hat - HP sweep", [
    (0, 3000), (b1, 3000), (d1 - 1, 9000), (d1, 3000, "step"), (b2, 4000), (f1 - 1, 9500),
    (f1, 3000, "step"), (total, 3000)])
lane(f"instrument.{I_RISE}.level", "Riser - Level", [
    (0, 0.6), (b1, 0.6), (d1 - 1, 1.1), (d1, 0.6, "step"), (br2 + 2 * B, 0.6), (f1 - 1, 1.2),
    (f1, 0.6, "step")], curve="linear")
lane(f"instrument.{I_RISE}.cutoff", "Riser - Filter", [
    (0, 600), (b1, 600), (d1 - 1, 11000), (d1, 600, "step"), (br2 + 2 * B, 600),
    (f1 - 1, 13000), (f1, 600, "step")])
pts = [(0, 0.15)]
for blk_i, k in ((D1, 0), (D1, 1), (F1, 0), (F1, 1)):
    e = at(blk_i, k) + 4 * B
    pts += [(e - 4, 0.7), (e, 0.15)]
lane(f"mixer.ch.{LEAD + 1}.send_a", "Lead - Echo throws", pts + [(o1, 0.5), (total, 0.5)],
     curve="step")
lane("mixer.return_b.size", "Reverb size", [
    (0, 0.8), (d1, 0.72), (br, 0.9), (b2, 0.8), (f1, 0.72), (o1, 0.85), (total, 0.95)],
    curve="linear")
# the macro rise: the "Lift" macro's five route depths climb through every build and snap
# back on the drop (mod.K.amount = route K's depth)
for k, peak in ((1, 0.22), (2, 0.18), (3, 0.3), (4, 0.25), (5, 0.25)):
    lane(f"mod.{k}.amount", f"Lift macro - route {k} depth", [
        (0, 0.0), (b1, 0.0), (d1 - 1, peak), (d1, 0.0, "step"), (br2, 0.0), (b2, peak * 0.4),
        (f1 - 1, peak), (f1, 0.0, "step"), (total, 0.0)], curve="linear")

print("# Quick mods: the Lift macro (depths automated above), LFO drift on the pads, a pan")
print("# sway on the stabs, LFO and envelope on the lead, velocity on the stab filter and a")
print("# random drift on the hats")
modulator(1, I_STAB, "Lift", kind="Macro", value=1.0)
modulator(2, I_PAD, "Pad drift", shape="Sine", sync=32.0)
modulator(3, I_STAB, "Stab sway", shape="Triangle", sync=16.0)
modulator(4, I_LEAD, "Lead wobble", shape="Triangle", sync=2.0)
modulator(5, I_LEAD, "Lead pluck", kind="Envelope", a=0.005, d=0.35, s=0.35, r=0.2)
modulator(6, I_STAB, "Stab velocity", kind="Velocity")
modulator(7, I_HAT, "Hat drift", kind="Random", rate=2.0, smooth=0.6)
route(1, f"instrument.{I_STAB}.gen.{c}", 0.0)
route(1, f"instrument.{I_PAD}.gen.{c}", 0.0)
route(1, f"instrument.{I_STAB}.gen.{GEN['echo_mix']}", 0.0)
route(1, f"instrument.{I_LEAD}.cutoff", 0.0)
route(1, f"instrument.{I_PAD}.gen.{GEN['rev_mix']}", 0.0)
route(2, f"instrument.{I_PAD}.gen.{c}", 0.08)
route(3, f"mixer.ch.{STAB1 + 1}.pan", 0.25)
route(3, f"mixer.ch.{STAB3 + 1}.pan", -0.25)
route(4, f"instrument.{I_LEAD}.cutoff", 0.1)
route(5, f"instrument.{I_LEAD}.cutoff", 0.25)
route(6, f"instrument.{I_STAB}.gen.{c}", 0.08)
route(7, f"mixer.ch.{HAT + 1}.pan", 0.15)
