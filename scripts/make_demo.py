#!/usr/bin/env python3
"""Generate demos/first_run.cwt.toml, the first-run demo song.

"Night Signal": psytrance in F# minor, 145 BPM, about 1:40. Blocks have
arrangement roles (intro, verse, build, drop, breakdown, outro) so Fill to
length works on it. Run: python3 scripts/make_demo.py > demos/first_run.cwt.toml
"""

CH = 8
ROWS = 64
KICK, BASS, HAT, CLAP, ACID, LEAD, PAD, FX = range(8)
# instrument slots (keep 01 kick / 02 bass: tests and docs refer to them)
I_KICK, I_BASS, I_CHAT, I_OHAT, I_ACID, I_PLUCK, I_SAW, I_FLUT, I_PAD, I_ZAP, I_CLAP, I_RISE = range(1, 13)

# chords per bar (F# minor: i - VI - VII - i)
DROP_CHORDS = [("F#", "A-", "C#"), ("D-", "F#", "A-"), ("E-", "G#", "B-"), ("F#", "A-", "C#")]


class Block:
    def __init__(self, name, role):
        self.name, self.role = name, role
        self.cells = [[None] * CH for _ in range(ROWS)]

    def put(self, row, ch, note, ins=None, vol=None, fx=None):
        self.cells[row][ch] = (note, ins, vol, fx)

    def text(self):
        import os
        solo = os.environ.get("DEMO_SOLO")  # debug: keep only these channels, e.g. "1,2"
        if solo:
            keep = {int(c) - 1 for c in solo.split(",")}
            self.cells = [[c if k in keep else None for k, c in enumerate(row)] for row in self.cells]
        out = []
        for r, row in enumerate(self.cells):
            if all(c is None for c in row):
                continue
            parts = []
            for c in row:
                if c is None:
                    parts.append("--- .. .. ....")
                else:
                    n, i, v, f = c
                    parts.append(
                        f"{n} {('%02X' % i) if i else '..'} {('%02X' % v) if v is not None else '..'} {f or '....'}"
                    )
            out.append("%02X | " % r + " | ".join(parts))
        return "\n".join(out)


def kick(b, bars=range(4), note="A-2"):
    for bar in bars:
        for beat in range(4):
            b.put(bar * 16 + beat * 4, KICK, note, I_KICK)


def bass(b, bars=range(4), pop=True):
    """rolling offbeat bass: kick-bass-bass-bass, octave pop at the end of bar 4"""
    for bar in bars:
        for beat in range(4):
            for s in (1, 2, 3):
                r = bar * 16 + beat * 4 + s
                n = "F#2"
                if pop and bar == 3 and beat == 3 and s == 2:
                    n = "F#3"
                b.put(r, BASS, n, I_BASS)


def hats(b, bars=range(4), busy=True):
    for bar in bars:
        for r16 in range(16):
            r = bar * 16 + r16
            s = r16 % 4
            if s == 2:
                if busy:
                    b.put(r, HAT, "C-5", I_OHAT, 0x2C)
                else:
                    b.put(r, HAT, "C-5", I_CHAT, 0x30)
            elif busy and s in (1, 3):
                b.put(r, HAT, "C-5", I_CHAT, 0x1C if s == 1 else 0x24)


def claps(b, bars=range(4)):
    for bar in bars:
        for beat in (1, 3):
            b.put(bar * 16 + beat * 4, CLAP, "C-4", I_CLAP, 0x36)


ACID_RIFF = ["F#3", "===", "F#3", "A-3", "F#3", "C#4", "===", "F#3",
             "E-3", "F#3", "===", "A-3", "F#3", "===", "B-3", "C#4"]
ACID_TURN = ["F#3", "===", "F#3", "A-3", "F#3", "C#4", "===", "F#3",
             "E-3", "F#3", "A-3", "C#4", "E-4", "C#4", "B-3", "A-3"]


def acid(b, bars=range(4), vol=0x34):
    for bar in bars:
        riff = ACID_TURN if bar == 3 else ACID_RIFF
        for s, n in enumerate(riff):
            r = bar * 16 + s
            if n == "===":
                b.put(r, ACID, n)
            else:
                accent = s in (0, 5, 11)
                b.put(r, ACID, n, I_ACID, vol if accent else vol - 0x0C)


def pluck_arp(b, chords=DROP_CHORDS, bars=range(4), vol=0x30):
    """3/16 gallop of chord tones, an octave up"""
    steps = [0, 3, 6, 8, 11, 14]
    for bar in bars:
        root, third, fifth = chords[bar]
        tones = [root + "4", fifth + "4", root + "5", third + "5", fifth + "4", third + "4"]
        for s, n in zip(steps, tones):
            b.put(bar * 16 + s, LEAD, n, I_PLUCK, vol)


LEAD_MELODY = [  # (row in block, note, length is implicit until the next)
    (0, "C#5"), (4, "A-4"), (6, "B-4"), (8, "C#5"), (12, "E-5"), (14, "C#5"),
    (16, "D-5"), (22, "C#5"), (24, "A-4"), (28, "F#4"),
    (32, "B-4"), (36, "G#4"), (38, "B-4"), (40, "E-5"), (46, "D-5"),
    (48, "C#5"), (54, "B-4"), (56, "A-4"), (60, "F#4"), (63, "==="),
]


def lead(b, vol=0x30):
    for r, n in LEAD_MELODY:
        if n == "===":
            b.put(r, LEAD, n)
        else:
            b.put(r, LEAD, n, I_SAW, vol)


def pad_line(b, notes=("C#4", "D-4", "E-4", "C#4"), vol=0x2C):
    for bar, n in enumerate(notes):
        b.put(bar * 16, PAD, n, I_PAD, vol)


def zaps(b):
    b.put(22, FX, "F#3", I_ZAP, 0x26)
    b.put(30, FX, "C#4", I_ZAP, 0x20)
    b.put(54, FX, "A-3", I_ZAP, 0x26)
    b.put(59, FX, "F#3", I_ZAP, 0x1E)


def clap_fill(b, start=60):
    for k, r in enumerate(range(start, 64)):
        b.put(r, CLAP, "C-4", I_CLAP, 0x24 + 6 * k)


# ---------------------------------------------------------------- blocks
blocks = []

intro = Block("Intro", "intro")
kick(intro)
hats(intro, busy=False)
intro.put(0, PAD, "A-3", I_PAD, 0x24)
intro.put(32, PAD, "C#4", I_PAD, 0x24)
for r, n in [(8, "F#5"), (14, "C#5"), (40, "E-5"), (46, "C#5"), (56, "A-4")]:
    intro.put(r, FX, n, I_FLUT, 0x20)
intro.put(62, FX, "===")
blocks.append(intro)

groove = Block("Groove", "verse")
groove.put(0, PAD, "===")  # the intro pad releases: the groove is drums, bass and acid
groove.put(0, FX, "===")
kick(groove)
bass(groove)
hats(groove)
claps(groove)
acid(groove, vol=0x30)
blocks.append(groove)

build = Block("Build", "build")
kick(build, bars=range(3))
bass(build, bars=range(3), pop=False)
hats(build, bars=range(2))
for bar in range(4):  # snare roll: quarters, 8ths, 16ths, rising
    step = {0: 8, 1: 4, 2: 2, 3: 1}[bar]
    for s in range(0, 16, step):
        r = bar * 16 + s
        if r < 62:
            b_vol = 0x14 + int(0x2A * r / 62)
            build.put(r, CLAP, "C-4", I_CLAP, b_vol)
acid(build, bars=range(3), vol=0x2C)
build.put(0, FX, "F#4", I_RISE, 0x40)
build.put(63, FX, "===")
build.put(48, PAD, "C#4", I_PAD, 0x28)
blocks.append(build)

drop = Block("Drop", "drop")
kick(drop)
bass(drop)
hats(drop)
claps(drop)
acid(drop)
pluck_arp(drop)
pad_line(drop)
zaps(drop)
blocks.append(drop)

dropb = Block("Drop B", "drop")
kick(dropb)
bass(dropb)
hats(dropb)
claps(dropb, bars=range(3))
dropb.put(48, CLAP, "C-4", I_CLAP, 0x36)
dropb.put(52, CLAP, "C-4", I_CLAP, 0x36)
clap_fill(dropb, 56)
acid(dropb)
lead(dropb)
pad_line(dropb)
dropb.put(22, FX, "F#3", I_ZAP, 0x24)
blocks.append(dropb)

brk = Block("Breakdown", "breakdown")
chords = [("D-4", "F#4", "A-4"), ("E-4", "G#4", "B-4"), ("F#4", "A-4", "C#5"), ("F#4", "A-4", "C#5")]
for bar, (a, b_, c) in enumerate(chords):
    if bar == 3:
        continue  # bar 4 holds bar 3's chord
    brk.put(bar * 16, HAT, a, I_PAD, 0x22)
    brk.put(bar * 16, CLAP, b_, I_PAD, 0x22)
    brk.put(bar * 16, PAD, c, I_PAD, 0x26)
pluck_arp(brk, chords=[("D-", "F#", "A-"), ("E-", "G#", "B-"), ("F#", "A-", "C#"), ("F#", "A-", "C#")], vol=0x2C)
for r, n in [(0, "F#5"), (10, "E-5"), (16, "C#5"), (28, "B-4"), (32, "C#5"), (40, "E-5"), (44, "F#5"), (56, "C#5")]:
    brk.put(r, FX, n, I_FLUT, 0x24)
brk.put(63, FX, "===")
blocks.append(brk)

drop2 = Block("Drop 2", "drop")
kick(drop2)
bass(drop2)
hats(drop2)
claps(drop2)
acid(drop2, vol=0x38)
lead(drop2, vol=0x32)
pad_line(drop2)
zaps(drop2)
blocks.append(drop2)

outro = Block("Outro", "outro")
kick(outro)
bass(outro)
hats(outro)
claps(outro, bars=range(2))
acid(outro, bars=range(3), vol=0x2C)
outro.put(0, PAD, "A-3", I_PAD, 0x22)
outro.put(32, PAD, "C#4", I_PAD, 0x20)
blocks.append(outro)

end = Block("Outro end", "outro")
kick(end, bars=range(3))
end.put(48, KICK, "A-2", I_KICK)
hats(end, bars=range(2), busy=False)
for r, n in [(0, "F#5"), (12, "C#5"), (24, "A-4"), (40, "F#4")]:
    end.put(r, FX, n, I_FLUT, 0x1E)
end.put(0, PAD, "F#3", I_PAD, 0x20)
end.put(56, PAD, "===")
end.put(60, FX, "===")
blocks.append(end)

ORDER = [0, 0, 1, 1, 2, 3, 4, 5, 5, 2, 6, 6, 4, 7, 8]
start = {}
for i, b in enumerate(ORDER):
    start.setdefault(b, []).append(i * ROWS)
B = lambda idx, k=0: start[idx][k]  # noqa: E731 (song line where block idx is played the k-th time)

# ---------------------------------------------------------------- output
print('''# ChipWave first-run demo "Night Signal" (generated by scripts/make_demo.py; edit that, not this)
# Psytrance in F# minor, 145 BPM. Single-file text form of a .cwt song (docs/FIRST_RUN.md).
format = "chipwave-song-text"
format_version = 2

[song]
title = "Night Signal (demo)"
bpm = 145
lines_per_beat = 4
speed = 6
tracks = 8

# instruments: factory presets + built-in percussion; fx = extra devices after the preset's own
[[instrument]]
slot = 1
name = "Punchy Psy Kick"
patch = "factory:psytrance/punchy-psy-kick"
[[instrument]]
slot = 2
name = "Rolling Offbeat Bass"
patch = "factory:psytrance/rolling-offbeat-bass"
[[instrument]]
slot = 3
name = "Closed Hat"
patch = "builtin:hat-closed"
[[instrument]]
slot = 4
name = "Open Hat"
patch = "builtin:hat-open"
[[instrument]]
slot = 5
name = "Acid Squelch"
patch = "factory:psytrance/acid-squelch"
cutoff = 420.0
[[instrument]]
slot = 6
name = "Psy Pluck"
patch = "factory:psytrance/psy-pluck"
[[instrument]]
slot = 7
name = "Supersaw Lead"
patch = "factory:psytrance/supersaw-lead"
fx = [{ kind = "Chorus", rate = 0.4, depth = 3.0, mix = 0.35 }]
[[instrument]]
slot = 8
name = "Flutter Warble"
patch = "factory:alien/flutter-warble"
fx = [{ kind = "Filter", mode = 2, cutoff = 1800.0, resonance = 0.6, lfo_rate = 3.0, lfo_depth = 1.6, mix = 0.8 }]
[[instrument]]
slot = 9
name = "Trance Gate Pad"
patch = "factory:psytrance/trance-gate-pad"
[[instrument]]
slot = 10
name = "Zap"
patch = "factory:psytrance/zap"
[[instrument]]
slot = 11
name = "Clap"
patch = "builtin:clap"
fx = [{ kind = "Reverb", size = 0.35, damping = 0.6, mix = 0.18 }]
[[instrument]]
slot = 12
name = "Noise Riser"
patch = "builtin:riser"
fx = [{ kind = "Bitcrusher", bits = 10.0, rate = 16000.0, mix = 0.25 }]

[mixer]
master_db = 6.0

[[mixer.channel]]
name = "Kick"
volume_db = 4.5
[[mixer.channel]]
name = "Bass"
volume_db = 1.5
[[mixer.channel]]
name = "Hats"
volume_db = -2.5
pan = 0.15
send_b = 0.12
[[mixer.channel]]
name = "Clap"
volume_db = -3.0
pan = -0.05
send_b = 0.2
[[mixer.channel]]
name = "Acid"
volume_db = -3.5
pan = -0.2
send_a = 0.22
[[mixer.channel]]
name = "Lead"
volume_db = -3.0
pan = 0.2
send_a = 0.2
send_b = 0.2
[[mixer.channel]]
name = "Pad"
volume_db = -6.0
send_b = 0.35
[[mixer.channel]]
name = "FX"
volume_db = -5.0
pan = -0.1
send_a = 0.3
send_b = 0.3

[mixer.return_a]
sync = 3.0
feedback = 0.3
hp = 300.0
lp = 5000.0
mix = 1.0
[mixer.return_b]
size = 0.75
damping = 0.5
mix = 1.0
''')
for i, b in enumerate(blocks):
    print(f'[[block]]\nid = {i}\nname = "{b.name}"\nlines = {ROWS}\nrole = "{b.role}"')
    print("# row | kick | bass | hats | clap | acid | lead | pad | fx")
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
        pos, val = p[0], p[1]
        sh = f', shape = "{p[2]}"' if len(p) > 2 else ""
        print(f"  {{ pos = {pos}, value = {val}{sh} }},")
    print("]\n")


print("# Automation: positions in lines from the song start (64 lines = one block = 4 bars)")
bd1, bd2 = B(2, 0), B(2, 1)
dr1, br1, d2 = B(3), B(5), B(6)
o1, o2 = B(7), B(8)
db2 = B(4, 1)
lane("instrument.2.cutoff", "Bass - Filter cutoff", [
    (0, 150), (B(1), 150), (bd1 + 60, 260), (dr1, 190, "linear"), (bd2, 160), (bd2 + 60, 280), (d2, 190), (o1, 190), (o2 + 64, 130)])
lane("instrument.5.cutoff", "Acid - Filter cutoff", [
    (0, 300), (B(1), 300), (bd1, 900), (bd1 + 62, 3200, "step"), (dr1, 1200), (dr1 + 32, 2400), (dr1 + 64, 1300),
    (dr1 + 96, 2800), (br1, 500, "step"), (bd2, 500), (bd2 + 62, 4200, "step"), (d2, 1500), (d2 + 32, 3000),
    (d2 + 64, 1500), (d2 + 96, 3400), (db2, 1600), (db2 + 32, 2600), (o1, 1800), (o1 + 64, 380)])
lane("instrument.6.cutoff", "Pluck - Filter cutoff", [
    (0, 900), (br1, 260), (bd2, 1800), (bd2 + 64, 900)])
lane("instrument.12.level", "Riser - Level", [
    (0, 0.0), (bd1, 0.15), (bd1 + 63, 1.1, "step"), (bd1 + 64, 0.0), (bd2, 0.15), (bd2 + 63, 1.1, "step"), (bd2 + 64, 0.0)], curve="linear")
lane("instrument.12.cutoff", "Riser - Filter sweep", [
    (0, 400), (bd1, 400), (bd1 + 63, 9000, "step"), (bd1 + 64, 400), (bd2, 400), (bd2 + 63, 9000, "step"), (bd2 + 64, 400)])
lane("instrument.8.fx.3.lfo_rate", "Flutter - Alien LFO rate", [
    (0, 1.5), (B(1), 7.0), (br1, 1.2), (bd2, 9.0), (o2, 2.0), (o2 + 64, 7.0)])
lane("mixer.return_a.feedback", "Delay feedback", [
    (0, 0.3), (br1, 0.3), (bd2, 0.6, "step"), (d2, 0.3), (o2, 0.3), (o2 + 64, 0.6)], curve="linear")
