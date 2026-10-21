#!/usr/bin/env python3
"""Generate demos/moonlit_cartridge.cwt.toml, the melodic chiptune demo "Moonlit Cartridge".

A melodic chiptune in G minor at 125 BPM, about 3:27, made only from Chip FX instruments
(the sfxr-style engine, tuned so C-4 plays 261.6 Hz), the built-in effects and one Drum Kit
("Chip Kit"). Modern touches come from the effects: gentle kick-keyed ducking on the bass,
pads and arp, a dotted-1/8 ping-pong echo, a big reverb, chorus and filter sweeps on the
pads, light drive on the triangle bass, and noise risers / impacts made with Chip FX.
The melodies, chords and bass line follow a reference track Del supplied (transcribed by
pitch tracking); the drums and the effects are new.

Arrangement (4-bar blocks; in timeline mode each block is a named clip):
  Intro (theme A, soft square) > Theme B (lead) > Build (G pedal, trill, riser) >
  Groove x4 (Bb F Ab C) > Chorus x4 > Bridge x2 > Chorus (lead + music box) x2 >
  Groove x2 > Breakdown x4 (music-box reprise) > Interlude x2 > Outro (theme A) x2

  python3 scripts/make_moonlit_cartridge.py > demos/moonlit_cartridge.cwt.toml
  DEMO_SOLO=4,5 python3 scripts/make_moonlit_cartridge.py > /tmp/x.toml   (debug: only these channels)
"""
import os

ROWS = 64
B = 16  # rows per bar
BPM = 125
(KICK, SNR, HAT, BASS, LEAD, HARM, ARP, PAD1, PAD2, PAD3, FX) = range(11)
CH = 11
(I_KIT, I_LEAD, I_SOFT, I_ARP, I_BASS, I_PAD, I_BELL, I_RISER, I_IMPACT, I_SPARK,
 I_HARM) = range(1, 12)

# Drum Kit ("Chip Kit", 16-pad layout) trigger notes
K_KICK, K_SNARE, K_CLAP, K_CH, K_OH, K_CRASH = "C-4", "D-4", "D#4", "F#4", "A#4", "C-5"
K_SHAKER, K_RIM, K_TOM_L, K_TOM_M, K_TOM_H = "D#5", "D-5", "E-4", "G-4", "A-4"

PC = {"C": 0, "C#": 1, "Db": 1, "D": 2, "D#": 3, "Eb": 3, "E": 4, "F": 5, "F#": 6, "Gb": 6,
      "G": 7, "G#": 8, "Ab": 8, "A": 9, "A#": 10, "Bb": 10, "B": 11}
NAMES = ["C-", "C#", "D-", "D#", "E-", "F-", "F#", "G-", "G#", "A-", "A#", "B-"]


def midi(n):
    """'G3' / 'Bb4' / 'F#2' -> semitone number (C0 = 0)"""
    return PC[n[:-1]] + 12 * int(n[-1])


def tn(m):
    """semitone number -> tracker note 'G-3'"""
    return NAMES[m % 12] + str(m // 12)


# ------------------------------------------------------------------ song grid
TOTAL_BARS = 108
grid = [[None] * CH for _ in range(TOTAL_BARS * B)]


def put(row, ch, note, ins=None, vol=None, fx=None):
    if 0 <= row < len(grid):
        grid[row][ch] = (note, ins, vol, fx)


def note(bar, step, ch, n, ins, vol, length=None):
    """a note (name or semitone number) at bar/step, with a note-off after `length` steps
    unless something else is already there"""
    r = bar * B + step
    put(r, ch, tn(n) if isinstance(n, int) else tn(midi(n)), ins, vol)
    if length:
        e = r + length
        if e < len(grid) and grid[e][ch] is None:
            put(e, ch, "===")


def phrase(bar0, ch, ins, vol, notes, octave=0, vol_fn=None):
    """notes: [(bar, step, name, length)]; ties are just long lengths"""
    for (b, s, n, ln) in notes:
        v = vol_fn(b, s) if vol_fn else vol
        note(bar0 + b, s, ch, midi(n) + 12 * octave, ins, v, ln)


_rng = [125]


def rnd(n):
    _rng[0] = (_rng[0] * 1103515245 + 12345) & 0x7FFFFFFF
    return (_rng[0] >> 16) % n


def jit(v, a=2):
    return max(1, min(0x40, v + rnd(2 * a + 1) - a))


# ------------------------------------------------------------------ harmony
CHORD = {  # chord tones (pitch classes), root first
    "Gm": [7, 10, 2], "G": [7, 11, 2], "Eb": [3, 7, 10], "Cm": [0, 3, 7], "C": [0, 4, 7],
    "D": [2, 6, 9], "D7": [2, 6, 9, 0], "Bb": [10, 2, 5], "F": [5, 9, 0], "Ab": [8, 0, 3],
    "Fm": [5, 8, 0], "E": [4, 8, 11], "Dm": [2, 5, 9], "Am": [9, 0, 4], "A": [9, 1, 4],
}
# pad voicings (three voices, smooth voice leading; the top line of the groove
# falls Bb > A > Ab > G as in the reference)
VOICING = {
    "Gm": ["D4", "G4", "Bb4"], "G": ["D4", "G4", "B4"], "Eb": ["Eb4", "G4", "Bb4"],
    "Cm": ["C4", "Eb4", "G4"], "C": ["C4", "E4", "G4"], "D": ["D4", "F#4", "A4"],
    "D7": ["C4", "F#4", "A4"], "Bb": ["D4", "F4", "Bb4"], "F": ["C4", "F4", "A4"],
    "Ab": ["C4", "Eb4", "Ab4"], "Fm": ["C4", "F4", "Ab4"], "E": ["B3", "E4", "G#4"],
    "Dm": ["D4", "F4", "A4"], "Am": ["C4", "E4", "A4"], "A": ["C#4", "E4", "A4"],
}
BASSROOT = {"Gm": "G2", "G": "G2", "Eb": "Eb2", "Cm": "C3", "C": "C3", "D": "D2", "D7": "D2",
            "Bb": "Bb2", "F": "F2", "Ab": "Ab2", "Fm": "F2", "E": "E2", "Dm": "D2", "Am": "A2",
            "A": "A2"}


def pads(bar0, chords, vol=0x1C, bars_each=1, attack_off=False):
    """one chord per `bars_each` bars on the three pad channels"""
    for k, c in enumerate(chords):
        b = bar0 + k * bars_each
        for i, n in enumerate(VOICING[c]):
            note(b, 0, PAD1 + i, n, I_PAD, min(0x40, int(vol * 1.35)) - (2 if i == 1 else 0))
    end = bar0 + len(chords) * bars_each
    if not attack_off and end < TOTAL_BARS:
        for i in range(3):
            if grid[end * B][PAD1 + i] is None:
                put(end * B, PAD1 + i, "===")


def bass_groove(bar0, chords, vol=0x34, bars_each=2):
    """the reference's groove bass: root, octave drops on 8/11, a leading note into
    the next root on the last 16th of each chord"""
    for k, c in enumerate(chords):
        root = midi(BASSROOT[c])
        nxt = midi(BASSROOT[chords[k + 1]]) if k + 1 < len(chords) else None
        for j in range(bars_each):
            b = bar0 + k * bars_each + j
            last = j == bars_each - 1
            for s, o, ln, dv in ((0, 0, 3, 0), (4, 0, 3, -3), (7, 0, 1, -6), (8, -12, 3, -1),
                                 (11, -12, 2, -3), (14, 0, 2, -5)):
                note(b, s, BASS, root + o, I_BASS, jit(vol + dv, 1), ln)
            if last and nxt is not None and nxt != root:
                lead_in = nxt - 1 if nxt > root else nxt + 1
                note(b, 13, BASS, lead_in + (12 if lead_in < midi("E2") else 0), I_BASS, vol - 4, 1)


def bass_eighths(bar0, chords, vol=0x32, bars_each=1):
    """driving octave 8ths (build / bridge)"""
    for k, c in enumerate(chords):
        root = midi(BASSROOT[c])
        for j in range(bars_each):
            b = bar0 + k * bars_each + j
            for s in range(0, 16, 2):
                o = 0 if s % 4 == 0 else 12
                note(b, s, BASS, root + o, I_BASS, jit(vol - (4 if o else 0), 1), 2)


def bass_long(bar0, chords, vol=0x2C, bars_each=1):
    for k, c in enumerate(chords):
        b = bar0 + k * bars_each
        note(b, 0, BASS, BASSROOT[c], I_BASS, vol, bars_each * B - 1)


ARP_PAT = [0, 1, 2, 3, 2, 1, 2, 3, 0, 1, 2, 3, 2, 1, 3, 2]


def arp(bar0, chords, vol=0x18, bars_each=1, base="C5", every=1, pat=ARP_PAT, accent=True):
    """16th arpeggio of the chord (root, 3rd, 5th, octave) around `base`"""
    lo = midi(base) - 7
    for k, c in enumerate(chords):
        tones = CHORD[c][:3]
        ns = []
        for pcl in tones:
            m = lo + ((pcl - lo) % 12)
            ns.append(m)
        ns.sort()
        ns.append(ns[0] + 12)
        for j in range(bars_each):
            b = bar0 + k * bars_each + j
            for s in range(0, 16, every):
                v = int(vol * 1.8) + (3 if accent and s % 4 == 0 else 0)
                note(b, s, ARP, ns[pat[s]], I_ARP, jit(min(v, 0x3C), 1))


# ------------------------------------------------------------------ drums
def kick(bar, steps=(0, 4, 8, 12), vol=0x3A):
    for s in steps:
        put(bar * B + s, KICK, K_KICK, I_KIT, vol if s % 8 == 0 else vol - 3)


def clap(bar, steps=(4, 12), vol=0x2A, pad=K_CLAP):
    for s in steps:
        put(bar * B + s, SNR, pad, I_KIT, jit(vol, 1))


def hats(bar, kind="off", vol=0x20):
    """off = offbeat 8ths (open on the last), eighths, sixteenths, shaker"""
    for s in range(16):
        r = bar * B + s
        if kind == "off" and s % 4 == 2:
            put(r, HAT, K_OH if s == 14 else K_CH, I_KIT, jit(vol - (4 if s == 14 else 0), 2))
        elif kind == "eighths" and s % 2 == 0:
            put(r, HAT, K_CH, I_KIT, jit(vol + (2 if s % 4 == 2 else -3), 2))
        elif kind == "sixteenths":
            put(r, HAT, K_OH if s == 14 else K_CH, I_KIT, jit(vol + (2 if s % 4 == 2 else -5), 2))
        elif kind == "shaker" and s % 2 == 0:
            put(r, HAT, K_SHAKER, I_KIT, jit(vol - (5 if s % 4 == 0 else 0), 2))


def crash(bar, vol=0x2C):
    put(bar * B, HAT, K_CRASH, I_KIT, vol)


def snare_roll(bar, bars=2, lo=0x10, hi=0x30):
    n = bars * B
    for k in range(n):
        f = k / (n - 1)
        step = 4 if f < 0.5 else (2 if f < 0.75 else 1)
        if k % step == 0:
            put(bar * B + k, SNR, K_SNARE, I_KIT, int(lo + (hi - lo) * f))


def tom_fill(bar, vol=0x2C):
    for s, p in ((8, K_TOM_H), (10, K_TOM_H), (11, K_TOM_M), (12, K_TOM_M), (14, K_TOM_L), (15, K_TOM_L)):
        put(bar * B + s, SNR, p, I_KIT, jit(vol, 1))


def beat(bar, kind="four", hat="off", clap_on=True, hv=0x20, kv=0x3A):
    if kind == "four":
        kick(bar, vol=kv)
    elif kind == "half":
        kick(bar, (0, 10), vol=kv - 4)
    elif kind == "break":
        kick(bar, (0, 6, 10), vol=kv - 2)
    if clap_on:
        clap(bar)
    if hat:
        hats(bar, hat, hv)


def riser(bar, bars=2, vol=0x26):
    note(bar, 0, FX, "C4", I_RISER, vol)


def impact(bar, vol=0x2E):
    note(bar, 0, FX, "C4", I_IMPACT, vol)


def sparkle(bar, step, n="C6", vol=0x1C):
    note(bar, step, FX, n, I_SPARK, vol)


# ------------------------------------------------------------------ melodies
# All transcribed from the reference by pitch tracking, quantized to 16ths.
# (bar, step, note, length in 16ths)
THEME_A = [  # Gm | Eb | Cm | D || Gm | Eb | Cm | D7 : a long note, then a three-note turn into the next
    (0, 0, "G3", 10), (0, 10, "D3", 1), (0, 11, "F3", 1), (0, 12, "D3", 1), (0, 13, "G3", 9),
    (1, 6, "D3", 1), (1, 7, "F3", 1), (1, 8, "D3", 1), (1, 9, "Eb3", 9),
    (2, 2, "B3", 1), (2, 3, "C#4", 1), (2, 4, "Bb3", 1), (2, 5, "C4", 9), (2, 14, "Bb3", 2),
    (3, 0, "D3", 10), (3, 10, "A3", 1), (3, 11, "C4", 1), (3, 12, "A3", 1), (3, 13, "D4", 9),
    (4, 6, "Bb3", 1), (4, 7, "G3", 1), (4, 8, "Bb3", 8),
    (5, 0, "Bb3", 2), (5, 2, "G3", 1), (5, 3, "Eb3", 1), (5, 4, "G3", 8), (5, 12, "Eb3", 1),
    (5, 13, "G3", 1), (5, 14, "Bb3", 2),
    (6, 0, "Bb3", 11), (6, 11, "A3", 5),
    (7, 0, "A3", 7), (7, 7, "C4", 8),
]
THEME_A_CH = ["Gm", "Eb", "Cm", "D", "Gm", "Eb", "Cm", "D7"]

THEME_B = [  # Gm | Eb | D | D || Gm | Fm | E | Bb
    (0, 0, "C4", 1), (0, 1, "G3", 3), (0, 4, "G4", 4), (0, 8, "G3", 2), (0, 10, "D4", 2),
    (0, 12, "C4", 1), (0, 13, "Bb3", 3),
    (1, 0, "Bb3", 6), (1, 6, "C4", 1), (1, 7, "Bb3", 1), (1, 8, "A3", 1), (1, 9, "G3", 7),
    (2, 0, "G3", 2), (2, 2, "F#3", 1), (2, 3, "G3", 1), (2, 4, "A3", 1), (2, 5, "Bb3", 9),
    (2, 14, "A3", 1), (2, 15, "Bb3", 1),
    (3, 0, "A3", 13), (3, 13, "C4", 3),
    (4, 0, "C4", 6), (4, 6, "D4", 1), (4, 7, "C4", 1), (4, 8, "A3", 1), (4, 9, "Bb3", 1),
    (4, 10, "G3", 6),
    (5, 0, "G3", 2), (5, 2, "Bb3", 3), (5, 5, "F3", 9), (5, 14, "Ab3", 2),
    (6, 0, "Ab3", 1), (6, 1, "E3", 12), (6, 13, "E4", 2), (6, 15, "F4", 1),
    (7, 0, "F4", 9), (7, 9, "Eb4", 7),
]
THEME_B_CH = ["Gm", "Eb", "D", "D", "Gm", "Fm", "E", "Bb"]

CHORUS = [  # Bb | Bb | F | F | Ab | Ab | Cm | Cm
    (0, 0, "D5", 8), (0, 8, "Bb4", 8),
    (1, 0, "F4", 11), (1, 11, "Bb4", 1), (1, 12, "D5", 4),
    (2, 0, "C5", 6), (2, 6, "Bb4", 1), (2, 7, "A4", 5), (2, 12, "A4", 2), (2, 14, "Bb4", 1),
    (2, 15, "A4", 1),
    (3, 0, "C5", 12), (3, 12, "D5", 4),
    (4, 0, "Eb5", 10), (4, 10, "D5", 1), (4, 11, "Ab4", 5),
    (5, 0, "Eb5", 9), (5, 9, "Ab4", 7),
    (6, 0, "Eb5", 10), (6, 10, "G4", 1), (6, 11, "C4", 5),
    (7, 0, "C5", 11), (7, 11, "C4", 5),
]
CHORUS_CH = ["Bb", "Bb", "F", "F", "Ab", "Ab", "Cm", "Cm"]

BRIDGE = [  # Bb | Dm | Am | F | Ab | Ab | Eb | Cm
    (0, 0, "D5", 9), (0, 9, "Bb4", 7),
    (1, 0, "F4", 12), (1, 12, "A4", 1), (1, 13, "D5", 3),
    (2, 0, "A4", 1), (2, 1, "C5", 4), (2, 5, "A4", 6), (2, 11, "G#4", 1), (2, 12, "A4", 4),
    (3, 0, "F4", 13), (3, 13, "D5", 3),
    (4, 0, "Ab4", 10),
    (5, 1, "Ab4", 9), (5, 10, "C5", 1), (5, 11, "Bb4", 1), (5, 12, "Ab4", 1), (5, 13, "G4", 3),
    (6, 0, "Ab4", 10), (6, 10, "F4", 2), (6, 12, "Eb4", 2), (6, 14, "D4", 1), (6, 15, "D5", 1),
    (7, 0, "C5", 8), (7, 8, "B4", 1), (7, 9, "C5", 7),
]
BRIDGE_CH = ["Bb", "Dm", "Am", "F", "Ab", "Ab", "Eb", "Cm"]

INTERLUDE = [  # Gm | Fm | E | A | Eb | Cm | Gm | Bb : the slow chromatic descent
    (0, 0, "G4", 2), (0, 2, "Bb3", 14),
    (1, 0, "Bb3", 2), (1, 2, "A3", 1), (1, 3, "Ab3", 13),
    (2, 0, "G#3", 16),
    (3, 0, "G#3", 3), (3, 3, "C#4", 1), (3, 4, "B3", 1), (3, 5, "D4", 2), (3, 7, "C#4", 5),
    (4, 2, "G3", 14),
    (5, 0, "G3", 3), (5, 3, "Eb3", 13),
    (6, 14, "Bb3", 2),
    (7, 0, "Bb3", 10), (7, 10, "B3", 1), (7, 11, "C4", 5),
]
INTERLUDE_CH = ["Gm", "Fm", "E", "A", "Eb", "Cm", "Gm", "Bb"]

BUILD_TRILL = [  # over the G pedal: a held G, then the reference's Eb-F trill and C turn
    (0, 0, "G4", 14), (0, 14, "F#4", 1), (0, 15, "G4", 1),
    (1, 0, "G4", 8), (1, 8, "G#4", 1), (1, 9, "A4", 1), (1, 10, "G4", 6),
    (2, 0, "G#4", 1), (2, 1, "Eb5", 1), (2, 2, "E5", 1), (2, 3, "Eb5", 2), (2, 5, "F5", 1),
    (2, 6, "Eb5", 1), (2, 7, "F5", 1), (2, 8, "Eb5", 1), (2, 9, "F5", 1), (2, 10, "Eb5", 1),
    (2, 11, "F5", 1), (2, 12, "Eb5", 1), (2, 13, "F5", 1), (2, 14, "Eb5", 1), (2, 15, "F5", 1),
    (3, 0, "Eb5", 2), (3, 2, "C5", 2), (3, 4, "C#5", 2), (3, 6, "C5", 3), (3, 9, "B4", 2),
    (3, 11, "C5", 3), (3, 14, "D5", 2),
]


def harmony_of(mel, chords, below=True):
    """a chord tone under each melody note (the closest one at least a minor 3rd down)"""
    out = []
    for (b, s, n, ln) in mel:
        m = midi(n)
        tones = CHORD[chords[b]]
        h = m - 3
        while h % 12 not in tones:
            h -= 1
        out.append((b, s, tn(h)[:-1].replace("-", "") + str(h // 12), ln))
    return out


# ================================================================== arrangement
sections = []  # (name, role, first bar, bars)


def section(name, role, bar, bars):
    sections.append((name, role, bar, bars))


bar = 0
# ---- Intro: theme A on the soft square, 16th pulse arp, pads filtered (bars 0-7)
section("Intro", "intro", bar, 8)
phrase(bar, LEAD, I_SOFT, 0x22, THEME_A)
pads(bar, THEME_A_CH, vol=0x12)
arp(bar, THEME_A_CH, vol=0x0C, base="G4")
bass_long(bar + 4, THEME_A_CH[4:], vol=0x1C)
for b in range(bar + 4, bar + 8):
    hats(b, "shaker", 0x14)
sparkle(bar + 7, 12, "G5", 0x16)
bar += 8

# ---- Theme B: the lead takes the second theme, the beat starts half-time (bars 8-15)
section("Theme B", "verse", bar, 8)
phrase(bar, LEAD, I_LEAD, 0x2E, THEME_B, octave=1)
pads(bar, THEME_B_CH, vol=0x18)
arp(bar, THEME_B_CH, vol=0x12, base="G4")
for k, c in enumerate(THEME_B_CH):
    note(bar + k, 0, BASS, BASSROOT[c], I_BASS, 0x2E, 10)
    note(bar + k, 10, BASS, midi(BASSROOT[c]) + 12, I_BASS, 0x26, 2)
    note(bar + k, 12, BASS, BASSROOT[c], I_BASS, 0x2A, 3)
for b in range(bar, bar + 8):
    beat(b, "half", "eighths", clap_on=b >= bar + 4, hv=0x18, kv=0x34)
crash(bar, 0x22)
bar += 8

# ---- Build: G pedal, the trill, snare roll and a Chip FX riser (bars 16-19)
section("Build", "build", bar, 4)
phrase(bar, LEAD, I_LEAD, 0x2E, BUILD_TRILL)
pads(bar, ["Gm", "Gm", "Cm", "D"], vol=0x1A)
bass_eighths(bar, ["Gm", "Gm", "Cm", "D"], vol=0x30)
arp(bar, ["Gm", "Gm", "Cm", "D"], vol=0x14, base="G4")
for b in range(bar, bar + 2):
    beat(b, "four", "eighths", clap_on=True, hv=0x1A, kv=0x36)
snare_roll(bar + 2, 2, 0x10, 0x32)
for b in range(bar + 2, bar + 4):
    kick(b, vol=0x34)
    hats(b, "sixteenths", 0x16)
riser(bar + 1, 3, 0x2A)
bar += 4

# ---- Groove: bass, pads, arp, the groove's chords (bars 20-35)
GROOVE_CH = ["Bb", "F", "Ab", "C", "Bb", "F", "Ab", "Cm"]
section("Groove", "verse", bar, 16)
impact(bar, 0x2A)
crash(bar)
bass_groove(bar, GROOVE_CH)
pads(bar, GROOVE_CH, vol=0x1C, bars_each=2)
arp(bar, GROOVE_CH, vol=0x14, bars_each=2, base="Bb4")
for b in range(bar, bar + 16):
    first = b < bar + 8
    beat(b, "half" if first else "four", "off", clap_on=True, hv=0x1E, kv=0x38)
# the reference's broken-chord ornament on the second pass
for k, n in enumerate(["C4", "F4", "Bb4", "D5", "F4", "Bb4", "D5"]):
    note(bar + 8, k, HARM, n, I_HARM, 0x1C, 1)
tom_fill(bar + 15)
sparkle(bar + 15, 14, "D6", 0x18)
bar += 16

# ---- Chorus: the main melody twice, harmony and full beat on the second pass (bars 36-51)
section("Chorus", "drop", bar, 16)
for rep in range(2):
    b0 = bar + rep * 8
    phrase(b0, LEAD, I_LEAD, 0x32, CHORUS)
    if rep == 1:
        phrase(b0, HARM, I_HARM, 0x22, harmony_of(CHORUS, CHORUS_CH))
    bass_groove(b0, CHORUS_CH, vol=0x36)
    pads(b0, CHORUS_CH, vol=0x1A, bars_each=1)
    arp(b0, CHORUS_CH, vol=0x14, base="F5" if rep else "C5")
    for b in range(b0, b0 + 8):
        beat(b, "four", "off" if rep == 0 else "sixteenths", hv=0x1E if rep == 0 else 0x16, kv=0x3A)
    crash(b0)
impact(bar, 0x2E)
snare_roll(bar + 15, 1, 0x14, 0x2C)
bar += 16

# ---- Bridge: the second melody over driving octave 8ths (bars 52-59)
section("Bridge", "bridge", bar, 8)
phrase(bar, LEAD, I_LEAD, 0x36, BRIDGE)
phrase(bar, HARM, I_HARM, 0x20, harmony_of(BRIDGE, BRIDGE_CH))
bass_eighths(bar, BRIDGE_CH, vol=0x34)
pads(bar, BRIDGE_CH, vol=0x1A)
arp(bar, BRIDGE_CH, vol=0x12, base="A4", every=2)
for b in range(bar, bar + 8):
    beat(b, "four", "eighths", hv=0x1E, kv=0x3C)
crash(bar)
tom_fill(bar + 7, 0x2E)
riser(bar + 6, 2, 0x26)
bar += 8

# ---- Chorus 2: lead + music box an octave up + 16th arps (bars 60-67)
section("Chorus 2", "drop", bar, 8)
impact(bar, 0x2E)
phrase(bar, LEAD, I_LEAD, 0x36, CHORUS)
phrase(bar, HARM, I_BELL, 0x26, CHORUS, octave=1)
bass_groove(bar, CHORUS_CH, vol=0x36)
pads(bar, CHORUS_CH, vol=0x1C)
arp(bar, CHORUS_CH, vol=0x16, base="F5")
for b in range(bar, bar + 8):
    beat(b, "four", "sixteenths", hv=0x18, kv=0x3C)
crash(bar)
crash(bar + 4, 0x26)
bar += 8

# ---- Groove 2: back down, Gm into the groove chords (bars 68-75)
G2_CH = ["Gm", "Bb", "F", "F", "Ab", "Ab", "Cm", "Cm"]
section("Groove 2", "verse", bar, 8)
bass_groove(bar, ["Gm", "Bb", "F", "F", "Ab", "Ab", "Cm", "Cm"], vol=0x32, bars_each=1)
pads(bar, G2_CH, vol=0x1A)
arp(bar, G2_CH, vol=0x12, base="Bb4", every=2)
for b in range(bar, bar + 8):
    beat(b, "half" if b >= bar + 4 else "four", "off", hv=0x1A, kv=0x34)
crash(bar, 0x24)
sparkle(bar + 7, 12, "G5", 0x16)
bar += 8

# ---- Breakdown: held chords; then a music-box reprise of the chorus (bars 76-91)
BD_CH = ["Bb", "F", "Ab", "C", "Bb", "F", "Ab", "Cm"]
section("Breakdown", "breakdown", bar, 16)
pads(bar, BD_CH, vol=0x1E, bars_each=2)
bass_long(bar, BD_CH, vol=0x1E, bars_each=2)
phrase(bar + 8, HARM, I_BELL, 0x26, CHORUS)
for k, c in enumerate(BD_CH[4:]):
    for j in range(2):
        b = bar + 8 + k * 2 + j
        hats(b, "shaker", 0x12)
arp(bar + 4, ["Ab", "Ab", "C", "C"], vol=0x0E, base="C5", every=4)
bar += 16

# ---- Interlude: the slow chromatic melody (bars 92-99)
section("Interlude", "bridge", bar, 8)
phrase(bar, LEAD, I_SOFT, 0x24, INTERLUDE)
pads(bar, INTERLUDE_CH, vol=0x1A)
bass_long(bar, INTERLUDE_CH, vol=0x20)
arp(bar, INTERLUDE_CH, vol=0x0E, base="G4", every=2)
for b in range(bar, bar + 8):
    kick(b, (0, 10), vol=0x2A)
    hats(b, "shaker", 0x12)
    if b >= bar + 4:
        clap(b, (12,), 0x1E, K_RIM)
bar += 8

# ---- Outro: theme A comes home, the music box echoes it, everything fades (bars 100-107)
section("Outro", "outro", bar, 8)
phrase(bar, LEAD, I_SOFT, 0x24, THEME_A)
phrase(bar + 4, HARM, I_BELL, 0x1A, THEME_A[18:], octave=1)
pads(bar, THEME_A_CH[:7] + ["Gm"], vol=0x18, attack_off=True)
arp(bar, THEME_A_CH[:7] + ["Gm"], vol=0x0E, base="G4")
for k, c in enumerate(THEME_A_CH[:7] + ["Gm"]):
    note(bar + k, 0, BASS, BASSROOT[c], I_BASS, max(0x14, 0x28 - k * 2), 14)
for b in range(bar, bar + 4):
    hats(b, "shaker", 0x12)
sparkle(bar + 7, 8, "G5", 0x14)
# the fade: every note of the outro gets quieter bar by bar (to about -11 dB)
for r in range((bar - 8) * B, bar * B):
    k = (r - (bar - 8) * B) / (8 * B)
    g = 1.0 - 0.72 * k
    for ch in range(CH):
        c = grid[r][ch]
        if c and c[2] is not None:
            grid[r][ch] = (c[0], c[1], max(1, int(round(c[2] * g))), c[3])
bar += 8
assert bar == TOTAL_BARS, bar

# final notes ring out: last bar, release the arp after beat 2
for s in range(8, 16):
    grid[(TOTAL_BARS - 1) * B + s][ARP] = None
put((TOTAL_BARS - 1) * B + 8, ARP, "===")

# ================================================================== output
MIX = 0.0
LIM = 0.0
P_C4 = 0.27048  # sfxr base frequency for C-4 = 261.6 Hz (follow note on)


def sfx(wave, **kw):
    """an sfxr parameter array (sfxr order), defaults: C-4 pitch, instant attack"""
    order = ["base", "limit", "slide", "dslide", "duty", "duty_ramp", "vib", "vib_speed", "vib_delay",
             "attack", "sustain", "decay", "punch", "lp_res", "lp", "lp_ramp", "hp", "hp_ramp",
             "pha", "pha_ramp", "repeat", "arp_speed", "arp"]
    d = dict(base=P_C4, limit=0.0, slide=0.0, dslide=0.0, duty=0.0, duty_ramp=0.0, vib=0.0,
             vib_speed=0.0, vib_delay=0.0, attack=0.0, sustain=1.0, decay=1.0, punch=0.0,
             lp_res=0.0, lp=1.0, lp_ramp=0.0, hp=0.0, hp_ramp=0.0, pha=0.0, pha_ramp=0.0,
             repeat=0.0, arp_speed=1.0, arp=0.0)
    d.update(kw)
    return wave, [round(float(d[k]), 5) for k in order]


def chip(slot, name, preset, wave_p, level=0.8, follow=True, fx=(), base="", gen=""):
    wave, p = wave_p
    out = [f'[[instrument]]\nslot = {slot}\nname = "{name}"\npatch = "gen:{preset}"']
    if base:
        out.append(base.strip())
    if fx:
        out.append("fx = [" + ",\n      ".join(fx) + "]")
    out.append(f"[instrument.gen]\n{gen.strip()}" if gen else "[instrument.gen]")
    out.append(f'[instrument.gen.engine.ChipFx]\nwave = "{wave}"\nlevel = {level}\n'
               f'follow_note = {"true" if follow else "false"}\np = {p}')
    return "\n".join(out)


blocks = []
for (name, role, b0, nb) in sections:
    for k in range(nb // 4):
        blocks.append((f"{name} {k + 1}" if nb > 4 else name, role, b0 + k * 4))

hdr = ['''# ChipWave demo "Moonlit Cartridge" (generated by scripts/make_moonlit_cartridge.py; edit that, not this)
# Melodic chiptune in G minor, 125 BPM, about 3:27. Only Chip FX instruments (the sfxr-style
# engine), the built-in effects and one Drum Kit. Notes: docs/research/moonlit-cartridge.md.
format = "chipwave-song-text"
format_version = 2

[song]
title = "Moonlit Cartridge (chiptune demo)"
bpm = 125
lines_per_beat = 4
speed = 6
tracks = 11
key = "G minor"
''']
print("\n".join(hdr))
print('''[[instrument]]
slot = 1
name = "Chip Kit"
patch = "drumkit:Chip Kit"
fx = [{ kind = "Filter", mode = 1.0, cutoff = 30.0, resonance = 0.0 },
      { kind = "Compressor", threshold = -18.0, ratio = 3.0, attack = 3.0, release = 90.0, makeup = 2.0 }]''')
FILT = '{ kind = "Filter", mode = 1.0, cutoff = %s, resonance = 0.0 }'
print(chip(2, "Moon Lead", "Blip",
           sfx("Square", duty=0.5, vib=0.012, vib_speed=0.28, attack=0.0, sustain=1.0, decay=1.0, punch=0.15),
           level=0.7,
           base="attack = 0.004\ndecay = 0.6\nsustain = 0.75\nrelease = 0.18\ncutoff = 7000.0\nresonance = 0.15",
           gen="filter_on = true",
           fx=[FILT % "200.0",
               '{ kind = "Chorus", rate = 0.4, depth = 1.5, delay = 9.0, feedback = 0.1, mix = 0.1 }',
               '{ kind = "Delay", sync = 3.0, feedback = 0.32, pingpong = 1.0, hp = 500.0, lp = 5000.0, mix = 0.16 }']))
print(chip(3, "Soft Square", "Blip",
           sfx("Square", duty=0.0, vib=0.008, vib_speed=0.25, sustain=1.0, decay=1.0, lp=0.55, lp_res=0.2),
           level=0.75,
           base="attack = 0.012\ndecay = 0.8\nsustain = 0.7\nrelease = 0.3\ncutoff = 2400.0\nresonance = 0.1",
           gen="filter_on = true",
           fx=[FILT % "150.0",
               '{ kind = "Delay", sync = 3.0, feedback = 0.3, pingpong = 1.0, hp = 400.0, lp = 4000.0, mix = 0.14 }']))
print(chip(4, "Pulse Arp", "Blip",
           sfx("Square", duty=0.75, sustain=0.05, decay=0.32, punch=0.3),
           level=0.6,
           base="attack = 0.001\ndecay = 0.2\nsustain = 0.15\nrelease = 0.06\ncutoff = 4200.0\nresonance = 0.2",
           gen="filter_on = true",
           fx=[FILT % "350.0",
               '{ kind = "Delay", sync = 2.0, feedback = 0.25, pingpong = 1.0, hp = 800.0, lp = 6000.0, mix = 0.18 }']))
print(chip(5, "Tri Bass", "Blip",
           sfx("Triangle", sustain=1.0, decay=0.8, punch=0.2),
           level=0.95,
           base="attack = 0.002\ndecay = 0.3\nsustain = 0.85\nrelease = 0.06",
           fx=['{ kind = "Drive", mode = 0.0, drive = 4.0, tone = 3500.0, output = -3.0, mix = 0.3 }',
               FILT % "50.0"]))
print(chip(6, "Phase Pad", "Blip",
           sfx("Saw", attack=0.35, sustain=1.0, decay=0.95, pha=0.12, pha_ramp=0.02, lp=0.7, lp_res=0.1),
           level=0.5,
           base="attack = 0.25\ndecay = 1.5\nsustain = 0.8\nrelease = 0.9\ncutoff = 3000.0\nresonance = 0.12",
           gen="filter_on = true",
           fx=[FILT % "220.0",
               '{ kind = "Filter", mode = 0.0, cutoff = 4500.0, resonance = 0.1 }',
               '{ kind = "Chorus", rate = 0.25, depth = 2.5, delay = 14.0, feedback = 0.1, mix = 0.3 }']))
print(chip(7, "Music Box", "Blip",
           sfx("Sine", sustain=0.08, decay=0.55, punch=0.5, arp_speed=0.98, arp=0.0),
           level=0.75,
           base="attack = 0.001\ndecay = 0.9\nsustain = 0.0\nrelease = 0.6",
           fx=[FILT % "300.0",
               '{ kind = "Delay", sync = 3.0, feedback = 0.4, pingpong = 1.0, hp = 700.0, lp = 7000.0, mix = 0.28 }']))
print(chip(8, "Noise Riser", "Blip",
           sfx("Noise", base=0.18, slide=0.11, attack=0.85, sustain=0.95, decay=0.35, lp=0.55, lp_ramp=0.2, lp_res=0.3),
           level=0.55, follow=False,
           base="attack = 0.0\ndecay = 1.0\nsustain = 1.0\nrelease = 0.4",
           fx=[FILT % "300.0"]))
print(chip(9, "Impact", "Explosion",
           sfx("Noise", base=0.12, slide=-0.05, attack=0.0, sustain=0.2, decay=0.75, punch=0.6, lp=0.6, lp_ramp=-0.1),
           level=0.7, follow=False,
           base="attack = 0.0\ndecay = 1.0\nsustain = 1.0\nrelease = 0.6",
           fx=[FILT % "40.0"]))
print(chip(10, "Coin Sparkle", "Coin",
           sfx("Square", duty=0.5, sustain=0.06, decay=0.4, punch=0.4, arp_speed=0.6, arp=0.45),
           level=0.5,
           base="attack = 0.0\ndecay = 1.0\nsustain = 1.0\nrelease = 0.3",
           fx=[FILT % "600.0",
               '{ kind = "Delay", sync = 3.0, feedback = 0.45, pingpong = 1.0, hp = 900.0, lp = 8000.0, mix = 0.35 }']))
print(chip(11, "Echo Square", "Blip",
           sfx("Square", duty=0.25, vib=0.01, vib_speed=0.3, sustain=1.0, decay=0.9, punch=0.1),
           level=0.6,
           base="attack = 0.006\ndecay = 0.5\nsustain = 0.65\nrelease = 0.2\ncutoff = 3800.0\nresonance = 0.1",
           gen="filter_on = true",
           fx=[FILT % "300.0"]))

print(f'''
[mixer]
master_db = {MIX}
duck_key = 1
duck_release_ms = 160
[mixer.limiter]
enabled = true
gain_db = {LIM}
ceiling_db = -1.2
release_ms = 120.0
target_lufs = -14.0
''')
CHS = [  # name, volume dB, pan, send A (echo), send B (reverb), duck
    ("Kick", 1.5, 0.0, 0.0, 0.03, 0.0),
    ("Snare/Clap", 0.0, 0.05, 0.05, 0.22, 0.0),
    ("Hats", 2.0, 0.15, 0.0, 0.08, 0.0),
    ("Bass", -8.5, 0.0, 0.0, 0.0, 0.25),
    ("Lead", 0.0, 0.0, 0.18, 0.22, 0.0),
    ("Harmony / Bell", -2.5, -0.12, 0.22, 0.3, 0.05),
    ("Arp", 1.5, 0.12, 0.1, 0.2, 0.1),
    ("Pad 1", -3.0, -0.15, 0.0, 0.28, 0.15),
    ("Pad 2", -4.0, 0.0, 0.0, 0.28, 0.15),
    ("Pad 3", -3.0, 0.15, 0.0, 0.28, 0.15),
    ("FX", -14.0, 0.0, 0.2, 0.5, 0.0),
]
print("# kick and bass centred and dry (bass, pads and arp duck off the kick); lead and\n"
      "# bell through the dotted-1/8 ping-pong echo (A) and a big plate-like reverb (B)")
for name, vol, pan, sa, sb, duck in CHS:
    print(f'[[mixer.channel]]\nname = "{name}"\nvolume_db = {vol}\npan = {pan}\nsend_a = {sa}\nsend_b = {sb}\nduck = {duck}')
print('''
[mixer.return_a]
sync = 3.0
feedback = 0.38
pingpong = 1.0
hp = 400.0
lp = 5000.0
mix = 1.0
level = 1.0
[mixer.return_b]
size = 0.82
damping = 0.45
predelay = 20.0
width = 0.8
mix = 1.0
lowcut = 350.0
highcut = 10000.0
level = 1.1
''')

solo = os.environ.get("DEMO_SOLO")
keep = {int(c) - 1 for c in solo.split(",")} if solo else set(range(CH))
for i, (name, role, b0) in enumerate(blocks):
    print(f'[[block]]\nid = {i}\nname = "{name}"\nlines = {ROWS}\nrole = "{role}"')
    print("# row | kick | snare | hats | bass | lead | harmony | arp | pad 1 | pad 2 | pad 3 | fx")
    out = []
    for r in range(ROWS):
        row = grid[b0 * B + r]
        row = [c if k in keep else None for k, c in enumerate(row)]
        if all(c is None for c in row):
            continue
        parts = []
        for c in row:
            if c is None:
                parts.append("--- .. .. ....")
            else:
                n, ins, v, f = c
                parts.append(f"{n} {('%02X' % ins) if ins else '..'} {('%02X' % v) if v is not None else '..'} {f or '....'}")
        out.append("%02X | " % r + " | ".join(parts))
    print('rows = """\n' + "\n".join(out) + '\n"""\n')

print("[sequence]\nsections = [" + ", ".join(f"{{ block = {i}, repeat = 1 }}" for i in range(len(blocks))) + "]\n")


def at(name, k=0):
    for (n, _r, b0, _nb) in sections:
        if n == name:
            return (b0 + k) * B
    raise KeyError(name)


def lane(target, label, pts, curve="smooth"):
    print(f'[[automation]]\ntarget = "{target}"\nlabel = "{label}"\ncurve = "{curve}"\npoints = [')
    for p in sorted(pts, key=lambda x: x[0]):
        print(f"  {{ pos = {p[0]}, value = {p[1]} }},")
    print("]\n")


total = TOTAL_BARS * B
print("# Automation: positions in lines from the song start (16 lines = one bar)")
# pad low-pass (fx 2) opens through the intro and the build, closes for the breakdown
lane("instrument.6.fx.2.cutoff", "Pad - Filter sweep", [
    (0, 900), (at("Theme B"), 1800), (at("Build"), 2200), (at("Groove") - 1, 6000), (at("Groove"), 3500),
    (at("Chorus"), 6500), (at("Groove 2"), 3000), (at("Breakdown"), 1200), (at("Breakdown", 8), 2500),
    (at("Interlude"), 2000), (at("Outro"), 1600), (total, 600)])
# arp low-pass (patch filter): darker in the quiet parts, open in the choruses
lane("instrument.4.cutoff", "Arp - Cutoff", [
    (0, 2200), (at("Theme B"), 3000), (at("Build", 3), 6000), (at("Groove"), 4000), (at("Chorus"), 6500),
    (at("Groove 2"), 3500), (at("Breakdown"), 2200), (at("Interlude"), 2500), (total, 1600)])
# the riser's high-pass lifts before each drop
lane("instrument.8.fx.1.cutoff", "Riser - HP", [
    (0, 300), (at("Build", 1), 300), (at("Groove") - 1, 2500), (at("Groove"), 300), (at("Bridge", 6), 300),
    (at("Chorus 2") - 1, 2500), (at("Chorus 2"), 300), (total, 300)], curve="linear")
# lead echo throws at the ends of phrases
lane("mixer.ch.5.send_a", "Lead - Echo throws", [
    (0, 0.2), (at("Build", 3), 0.45), (at("Groove"), 0.18), (at("Chorus", 7), 0.4), (at("Chorus", 8), 0.18),
    (at("Chorus", 15), 0.4), (at("Bridge"), 0.18), (at("Chorus 2", 7), 0.45), (at("Groove 2"), 0.2),
    (at("Interlude"), 0.3), (at("Outro"), 0.35), (total, 0.5)], curve="step")
lane("mixer.return_b.size", "Reverb size", [
    (0, 0.85), (at("Groove"), 0.78), (at("Breakdown"), 0.9), (at("Interlude"), 0.86), (total, 0.92)],
    curve="linear")
