#!/usr/bin/env python3
"""Generate demos/first_run.cwt.toml, the first-run demo song.

"Lumen Drift": uplifting trance in A minor, 140 BPM, about 2:24. Galloping
sidechained bass, off-beat 16th hats and arp, pumping supersaw pads, a long
breakdown, a riser + snare-roll build and a big second drop with a lead.
Blocks carry arrangement roles so Fill to length works on it.

  python3 scripts/make_demo.py > demos/first_run.cwt.toml
  DEMO_SOLO=2,5 python3 scripts/make_demo.py > /tmp/x.toml   (debug: only these channels)
"""
import os

CH, ROWS = 8, 64
KICK, BASS, HAT, CLAP, ARP, PADA, PADB, FX = range(8)
I_KICK, I_BASS, I_CHAT, I_SHAKER, I_CLAP, I_ARP, I_PAD, I_LEAD, I_PLUCK, I_RISE, I_CRASH = range(1, 12)

# A minor: i - VI - III - VII (Am F C G); pad voices whose +7 osc2 stays in key
PROG = ["Am", "F", "C", "G"]
PAD_VOICES = {"Am": ("A-4", "C-5"), "F": ("F-4", "A-4"), "C": ("C-5", "E-5"), "G": ("G-4", "D-5"),
              "Em": ("E-4", "G-4"), "Dm": ("D-4", "F-4")}
ARP_NOTES = {  # 8 notes on the off 16ths of a bar
    "Am": ["E-5", "A-4", "C-5", "E-5", "A-5", "E-5", "C-5", "A-4"],
    "F": ["C-5", "F-4", "A-4", "C-5", "F-5", "C-5", "A-4", "F-4"],
    "C": ["E-5", "G-4", "C-5", "E-5", "G-5", "E-5", "C-5", "G-4"],
    "G": ["D-5", "G-4", "B-4", "D-5", "G-5", "D-5", "B-4", "G-4"],
}
BASS_ROOT = {"Am": "A-2", "F": "F-2", "C": "C-3", "G": "G-2", "Em": "E-2", "Dm": "D-2"}


class Block:
    def __init__(self, name, role):
        self.name, self.role = name, role
        self.cells = [[None] * CH for _ in range(ROWS)]

    def put(self, row, ch, note, ins=None, vol=None, fx=None):
        self.cells[row][ch] = (note, ins, vol, fx)

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


def kick(b, bars=range(4), vol=None):
    for bar in bars:
        for beat in range(4):
            b.put(bar * 16 + beat * 4, KICK, "A-2", I_KICK, vol)


def bass(b, prog=PROG, bars=range(4), vol=0x38):
    """galloping bass: kick | bass on the 2nd 16th | rest | bass on the 4th 16th"""
    for bar in bars:
        root = BASS_ROOT[prog[bar]]
        for beat in range(4):
            r = bar * 16 + beat * 4
            b.put(r + 1, BASS, root, I_BASS, vol)
            b.put(r + 3, BASS, root, I_BASS, vol - 4)


_rng = [12345]


def jitter(v, amt=2):
    """small deterministic velocity variation (no robotic flat velocity)"""
    _rng[0] = (_rng[0] * 1103515245 + 12345) & 0x7FFFFFFF
    return max(1, min(0x40, v + (_rng[0] >> 16) % (2 * amt + 1) - amt))


# offbeat hat: one crisp, slightly open hit on the "and" of every beat (exactly
# halfway between kicks), on the grid; per-note decay 08 14 = 200 ms envelope
# (about 90 ms to -20 dB), consistent accent with a small velocity variation
HAT_OFF = "0814"


def hats(b, bars=range(4), soft=False):
    """classic trance offbeat hat: rows 2, 6, 10, 14 of each bar, nothing else"""
    for bar in bars:
        for beat in range(4):
            vol = 0x2C if soft else 0x3A
            b.put(bar * 16 + beat * 4 + 2, HAT, "C-5", I_CHAT, jitter(vol), HAT_OFF)


def claps(b, bars=range(4)):
    for bar in bars:
        for beat in (1, 3):
            b.put(bar * 16 + beat * 4, CLAP, "C-4", I_CLAP, 0x38)


def arp(b, prog=PROG, bars=range(4), vol=0x30, octave_up=False):
    for bar in bars:
        notes = ARP_NOTES[prog[bar]]
        for k, n in enumerate(notes):
            if octave_up and k % 4 == 3:
                n = n[:2] + str(int(n[2]) + 1)
            b.put(bar * 16 + 2 * k + 1, ARP, n, I_ARP, vol if k % 2 else vol - 6)


def pads(b, prog=PROG, bars=range(4), vol=0x30, inst=I_PAD):
    for bar in bars:
        a, c = PAD_VOICES[prog[bar]]
        b.put(bar * 16, PADA, a, inst, vol)
        b.put(bar * 16, PADB, c, inst, vol - 4)


def roll(b, start_bar, bars=1, top=0x40):
    """snare/clap roll: 8ths then 16ths, rising"""
    rows = list(range(start_bar * 16, (start_bar + bars) * 16))
    for k, r in enumerate(rows):
        frac = k / max(1, len(rows) - 1)
        if frac < 0.5 and r % 2:
            continue
        b.put(r, CLAP, "C-4", I_CLAP, int(0x14 + (top - 0x14) * frac))


def fill(b):
    """last beat of the block: 16th clap fill"""
    for k, r in enumerate(range(60, 64)):
        b.put(r, CLAP, "C-4", I_CLAP, 0x28 + 6 * k)


LEAD_A = [(0, "E-5"), (6, "D-5"), (8, "C-5"), (12, "D-5"),
          (16, "C-5"), (22, "A-4"), (24, "C-5"), (28, "F-5"),
          (32, "G-5"), (38, "E-5"), (40, "C-5"), (44, "E-5"),
          (48, "D-5"), (56, "B-4"), (60, "D-5"), (62, "===")]
LEAD_B = LEAD_A[:12] + [(48, "D-5"), (52, "G-5"), (56, "E-5"), (62, "===")]


def lead(b, phrase, vol=0x34):
    for r, n in phrase:
        if n == "===":
            b.put(r, FX, n)
        else:
            b.put(r, FX, n, I_LEAD, vol)


blocks = []
# 0 intro: kick, bass, hats; pads under a closed filter (automation opens it)
intro = Block("Intro", "intro")
kick(intro, vol=0x26)
bass(intro, vol=0x24)
hats(intro, soft=True)
pads(intro, vol=0x26)
blocks.append(intro)

# 1 pre-break: no kick; pads, filtered arp, riser and a roll into the drop
pre = Block("Rise", "build")
pads(pre)
arp(pre, vol=0x2A)
pre.put(0, FX, "A-4", I_RISE, 0x40)
pre.put(63, FX, "===")
roll(pre, 3)
blocks.append(pre)


def drop_block(name, fill_end=False, octave_up=False, with_lead=None, crash=True):
    d = Block(name, "drop")
    kick(d)
    bass(d)
    hats(d)
    claps(d, bars=range(3) if fill_end else range(4))
    if fill_end:
        d.put(52, CLAP, "C-4", I_CLAP, 0x38)
        fill(d)
    arp(d, octave_up=octave_up)
    pads(d)
    if crash:
        d.put(0, CLAP, "C-5", I_CRASH, 0x30)  # crash on the downbeat (clap channel)
    if with_lead:
        lead(d, with_lead)
    return d


blocks.append(drop_block("Drop A"))                                  # 2
blocks.append(drop_block("Drop B", fill_end=True, octave_up=True, crash=False))  # 3

# 4 groove: drums and bass only (a breather)
groove = Block("Groove", "verse")
kick(groove)
bass(groove)
hats(groove)
claps(groove)
groove.put(0, ARP, "===")
blocks.append(groove)

# 5 breakdown: no drums; pads (VI - VII - i - i) and a slow pluck melody
brk = Block("Breakdown", "breakdown")
BRK = ["F", "G", "Am", "Am"]
pads(brk, prog=BRK, vol=0x40)
for r, n in [(0, "A-4"), (16, "B-4"), (24, "D-5"), (32, "C-5"), (48, "E-5"), (56, "C-5"), (60, "B-4")]:
    brk.put(r, ARP, n, I_PLUCK, 0x40)
    brk.put(r, FX, n[:2] + str(int(n[2]) - 1), I_LEAD, 0x3C)
brk.put(0, BASS, "===")
brk.put(63, FX, "===")
blocks.append(brk)

# 6 breakdown 2: the melody climbs, the lead doubles it an octave up
brk2 = Block("Breakdown 2", "breakdown")
pads(brk2, prog=BRK, vol=0x40)
for r, n in [(0, "C-5"), (8, "A-4"), (16, "D-5"), (24, "G-5"), (32, "E-5"), (40, "C-5"), (48, "E-5"), (56, "A-5")]:
    brk2.put(r, ARP, n, I_PLUCK, 0x40)
    brk2.put(r, FX, n, I_LEAD, 0x38)
brk2.put(63, FX, "===")
blocks.append(brk2)

# 7 build: arp back (filter rising), riser, accelerating roll, kick drops out
build = Block("Build", "build")
pads(build, prog=BRK, vol=0x30)
arp(build, prog=["F", "G", "Am", "Am"], vol=0x2C)
kick(build, bars=range(2))
build.put(0, FX, "A-4", I_RISE, 0x40)
build.put(63, FX, "===")
roll(build, 2, bars=2)
blocks.append(build)

blocks.append(drop_block("Drop 2", with_lead=LEAD_A))                 # 8
blocks.append(drop_block("Drop 2B", fill_end=True, octave_up=True, with_lead=LEAD_B, crash=False))  # 9

# 10 outro: kick, bass, hats, arp (filter closing)
outro = Block("Outro", "outro")
kick(outro, vol=0x2C)
bass(outro, vol=0x2A)
hats(outro, soft=True)
arp(outro, vol=0x28)
outro.put(0, FX, "===")
blocks.append(outro)

# 11 outro end: pads and a last arp phrase, no drums
end = Block("Outro end", "outro")
pads(end, prog=["Am", "F", "Am", "Am"], vol=0x3C)
arp(end, prog=["Am", "F"], bars=range(2), vol=0x24)
end.put(48, PADA, "===")
end.put(48, PADB, "===")
end.put(0, BASS, "===")
blocks.append(end)

ORDER = [0, 0, 1, 2, 3, 2, 3, 2, 3, 4, 5, 5, 6, 7, 8, 9, 8, 9, 3, 10, 11]
start = {}
for i, b in enumerate(ORDER):
    start.setdefault(b, []).append(i * ROWS)


def at(block, k=0):
    return start[block][k]


print('''# ChipWave first-run demo "Lumen Drift" (generated by scripts/make_demo.py; edit that, not this)
# Uplifting trance in A minor, 140 BPM. Single-file text form of a .cwt song (docs/FIRST_RUN.md).
format = "chipwave-song-text"
format_version = 2

[song]
title = "Lumen Drift (demo)"
bpm = 140
lines_per_beat = 4
speed = 6
tracks = 8

# patch = factory preset or built-in; the other keys override synth parameters
[[instrument]]
slot = 1
name = "Trance Kick"
patch = "builtin:trance-kick"
drive = 0.0
cutoff = 2500.0
pitch_env = [36.0, 0.010]
[[instrument]]
slot = 2
name = "Gallop Bass"
patch = "builtin:init"
wave = "saw"
octave = -1
sub = 0.25
unison = 2
detune = 6.0
cutoff = 300.0
resonance = 0.25
env_amount = 0.45
filter_decay = 0.12
attack = 0.001
decay = 0.16
sustain = 0.35
release = 0.03
drive = 0.25
gain = 0.9
[[instrument]]
slot = 3
name = "Offbeat Hat"
patch = "builtin:hat-closed"
cutoff = 3500.0
resonance = 0.2
metal = 0.45
width = 0.8
fx = [{ kind = "Filter", mode = 0.0, cutoff = 9500.0, resonance = 0.0 }]
[[instrument]]
slot = 4
name = "Shaker"
patch = "builtin:shaker"
start_delay_ms = 18
[[instrument]]
slot = 5
name = "Clap"
patch = "builtin:clap"
cutoff = 2200.0
fx = [{ kind = "Reverb", size = 0.45, damping = 0.5, mix = 0.22 }]
[[instrument]]
slot = 6
name = "Offbeat Arp"
patch = "builtin:init"
wave = "saw"
unison = 3
detune = 14.0
width = 0.6
osc2 = { wave = "square", semi = 12, level = 0.25 }
cutoff = 2000.0
resonance = 0.3
env_amount = 0.4
filter_decay = 0.09
attack = 0.001
decay = 0.13
sustain = 0.0
release = 0.08
gain = 0.75
fx = [{ kind = "Filter", mode = 1.0, cutoff = 280.0, resonance = 0.1 }]
[[instrument]]
slot = 7
name = "Supersaw Pad"
patch = "builtin:init"
wave = "saw"
unison = 7
detune = 24.0
width = 1.0
osc2 = { wave = "saw", semi = 7, level = 0.45 }
cutoff = 3200.0
resonance = 0.12
attack = 0.25
decay = 0.6
sustain = 0.85
release = 1.1
gain = 0.6
lfo = { shape = "sine", sync = 8.0, cutoff = 0.6 }
fx = [{ kind = "Filter", mode = 1.0, cutoff = 320.0, resonance = 0.1 }, { kind = "Chorus", rate = 0.3, depth = 3.0, mix = 0.3 }]
[[instrument]]
slot = 8
name = "Supersaw Lead"
patch = "factory:psytrance/supersaw-lead"
width = 0.8
lfo = { shape = "sine", rate = 5.5, pitch = 0.12 }
[[instrument]]
slot = 9
name = "Breakdown Pluck"
patch = "factory:psytrance/psy-pluck"
release = 0.4
[[instrument]]
slot = 10
name = "Noise Riser"
patch = "builtin:riser"
fx = [{ kind = "Reverb", size = 0.6, damping = 0.4, mix = 0.3 }]
[[instrument]]
slot = 11
name = "Crash"
patch = "builtin:hat-open"
decay = 1.4
release = 0.8
cutoff = 4500.0

[mixer]
master_db = 4.3
duck_key = 1
duck_release_ms = 300

[[mixer.channel]]
name = "Kick"
volume_db = 4.0
[[mixer.channel]]
name = "Bass"
volume_db = 0.0
duck = 0.55
[[mixer.channel]]
name = "Hats"
volume_db = 4.0
send_b = 0.03
[[mixer.channel]]
name = "Clap"
volume_db = -3.0
send_b = 0.15
[[mixer.channel]]
name = "Arp"
volume_db = -6.0
pan = -0.15
send_a = 0.25
send_b = 0.25
duck = 0.75
[[mixer.channel]]
name = "Pad 1"
volume_db = -9.0
pan = -0.5
send_b = 0.35
duck = 1.0
[[mixer.channel]]
name = "Pad 2"
volume_db = -9.0
pan = 0.5
send_b = 0.35
duck = 1.0
[[mixer.channel]]
name = "Lead / FX"
volume_db = -3.0
send_a = 0.3
send_b = 0.3
duck = 0.8

[mixer.return_a]
sync = 3.0
feedback = 0.35
hp = 350.0
lp = 6000.0
mix = 1.0
[mixer.return_b]
size = 0.88
damping = 0.45
predelay = 25.0
width = 1.0
mix = 1.0
''')
for i, b in enumerate(blocks):
    print(f'[[block]]\nid = {i}\nname = "{b.name}"\nlines = {ROWS}\nrole = "{b.role}"')
    print("# row | kick | bass | hats | clap | arp | pad 1 | pad 2 | lead/fx")
    print('rows = """\n' + b.text() + '\n"""\n')

secs = []
for b in ORDER:
    if secs and secs[-1][0] == b:
        secs[-1][1] += 1
    else:
        secs.append([b, 1])
print("[sequence]\nsections = [" + ", ".join(f"{{ block = {b}, repeat = {n} }}" for b, n in secs) + "]\n")


def lane(target, label, pts, curve="smooth"):
    print(f'[[automation]]\ntarget = "{target}"\nlabel = "{label}"\ncurve = "{curve}"\npoints = [')
    for p in pts:
        sh = f', shape = "{p[2]}"' if len(p) > 2 else ""
        print(f"  {{ pos = {p[0]}, value = {p[1]}{sh} }},")
    print("]\n")


print("# Automation: positions in lines from the song start (64 lines = one block = 4 bars)")
pre, d1, grv, brk0, bld, d2, out, end_ = at(1), at(2), at(4), at(5), at(7), at(8), at(10), at(11)
total = len(ORDER) * ROWS
lane("instrument.7.cutoff", "Pad - Filter cutoff", [
    (0, 350), (pre, 1800), (d1, 3200), (grv, 3200), (brk0, 1500), (bld, 2200), (bld + 63, 5000, "step"),
    (d2, 3600), (out, 3600), (end_, 1200), (total, 500)])
lane("instrument.6.cutoff", "Arp - Filter cutoff", [
    (0, 500), (pre, 500), (pre + 62, 2600, "step"), (d1, 1100), (d1 + 128, 2200), (d1 + 256, 1400), (d1 + 384, 2800),
    (grv, 1200), (bld, 400), (bld + 62, 3600, "step"), (d2, 1800), (d2 + 128, 3000), (out, 2400), (end_, 900), (total, 400)])
lane("instrument.2.cutoff", "Bass - Filter cutoff", [
    (0, 180), (pre, 260), (d1, 260), (grv, 320), (d2, 300), (out, 260), (total, 180)])
lane("instrument.10.level", "Riser - Level", [
    (0, 0.0), (pre, 0.1), (pre + 63, 1.2, "step"), (pre + 64, 0.0), (bld, 0.1), (bld + 63, 1.2, "step"), (bld + 64, 0.0)], curve="linear")
lane("instrument.10.cutoff", "Riser - Filter sweep", [
    (0, 300), (pre, 300), (pre + 63, 12000, "step"), (pre + 64, 300), (bld, 300), (bld + 63, 12000, "step"), (bld + 64, 300)])
lane("mixer.return_a.feedback", "Delay feedback", [
    (0, 0.3), (grv, 0.3), (grv + 60, 0.6, "step"), (brk0, 0.4), (d2, 0.3), (end_, 0.3), (total, 0.55)], curve="linear")
