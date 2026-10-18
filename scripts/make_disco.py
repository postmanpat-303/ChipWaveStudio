#!/usr/bin/env python3
"""Generate demos/disco.cwt.toml, the disco demo "Velvet Mirrorball".

Disco in F minor (Dorian colour), 104 BPM (the tempo of the reference track),
about 3:23. Four-on-the-floor 909 kick, quiet open hats on the offbeat only
(one per beat), 808 claps on 2 and 4, cowbell / claves / conga percussion
(Drum Synth generators), an octave-jumping Fat Moog Bass (Analog generator),
string stabs and swells (unison saw synth with a swirling short-delay
"phaser" chorus), E-Piano chord stabs (FM-4) in min9 / dom9 / maj7 voicings
from the chord helper, a Warm Lead hook, a Cyber Choir voice singing word
hooks with the Voice Transform blended in by 3Fxx, a robot "dis-co" voice
with formant sweeps (32xx) and vowel morphs (30xy), Noise riser /
downlifter / impact, filter sweeps by 38xx-3Bxx and automation lanes,
sidechain pumping off the kick, audible reverb and echo, master limiter.
Research notes and the reference analysis: docs/research/disco.md.
No melody is taken from the reference; it was used for tempo, groove and mix.

  python3 scripts/make_disco.py > demos/disco.cwt.toml
  DEMO_SOLO=1,2 python3 scripts/make_disco.py > /tmp/x.toml   (debug: only these channels)
"""
import os

ROWS = 64
(KICK, BASS, HAT, CLAP, PERC, STR1, STR2, STR3, EP1, EP2, EP3, LEAD, VOX, ROBO, FX) = range(15)
CH = 15
(I_KICK, I_BASS, I_HAT, I_CLAP, I_COW, I_CLAV, I_CONGA, I_STR, I_EP, I_LEAD, I_VOX, I_ROBO,
 I_RISE, I_DOWN, I_IMPACT, I_CYM) = range(1, 17)
BPM = 104
B = 16  # rows per bar


class Block:
    def __init__(self, name, role):
        self.name, self.role = name, role
        self.cells = [[None] * CH for _ in range(ROWS)]

    def put(self, row, ch, note, ins=None, vol=None, fx=None):
        if 0 <= row < ROWS:
            self.cells[row][ch] = (note, ins, vol, fx)

    def fx(self, row, ch, fx):
        """effect only (keeps a note already there)"""
        c = self.cells[row][ch]
        self.cells[row][ch] = (c[0], c[1], c[2], fx) if c else ("---", None, None, fx)

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


_rng = [104]


def jitter(v, amt=2):
    _rng[0] = (_rng[0] * 1103515245 + 12345) & 0x7FFFFFFF
    return max(1, min(0x40, v + (_rng[0] >> 16) % (2 * amt + 1) - amt))


# ---- harmony (F minor, Dorian IV) -----------------------------------------------------
# chord helper voicings without the root (the bass plays it): 3 notes per chord
CHORDS = {
    "Fm9":    {"root": "F", "ep": ("G#3", "D#4", "G-4"), "str": ("C-4", "D#4", "G#4"), "vox": "G#4"},
    "Bb9":    {"root": "A#", "ep": ("G#3", "D-4", "C-5"), "str": ("D-4", "F-4", "G#4"), "vox": "D-5"},
    "Dbmaj7": {"root": "C#", "ep": ("F-3", "G#3", "C-4"), "str": ("C#4", "F-4", "C-5"), "vox": "C-5"},
    "Eb9":    {"root": "D#", "ep": ("G-3", "C#4", "F-4"), "str": ("D#4", "G-4", "A#4"), "vox": "A#4"},
    "Cm7":    {"root": "C", "ep": ("D#3", "G-3", "A#3"), "str": ("D#4", "G-4", "A#4"), "vox": "G-4"},
    "C7":     {"root": "C", "ep": ("E-3", "G-3", "A#3"), "str": ("E-4", "G-4", "A#4"), "vox": "E-5"},
    "Bbm7":   {"root": "A#", "ep": ("C#3", "F-3", "G#3"), "str": ("C#4", "F-4", "G#4"), "vox": "F-4"},
}
VERSE = ["Fm9", "Fm9", "Bb9", "Bb9"]          # one chord per bar
CHORUS = ["Dbmaj7", "Eb9", "Cm7", "Fm9"]
BREAK = ["Dbmaj7", "Cm7", "Bbm7", "C7"]


# ---- drums ----------------------------------------------------------------------------
def kick(b, bars=range(4), skip=(), vol=0x3C):
    for bar in bars:
        for beat in range(4):
            if (bar, beat) in skip:
                continue
            b.put(bar * B + beat * 4, KICK, "D#4", I_KICK, vol)


def hats(b, bars=range(4), vol=0x1E, skip=()):
    """Del's rule: offbeat open hat only, once per beat (rows 2, 6, 10, 14), quiet"""
    for bar in bars:
        for beat in range(4):
            if (bar, beat) in skip:
                continue
            b.put(bar * B + beat * 4 + 2, HAT, "C-4", I_HAT, jitter(vol, 2))


def claps(b, bars=range(4), vol=0x32):
    for bar in bars:
        for beat in (1, 3):
            b.put(bar * B + beat * 4, CLAP, "C-4", I_CLAP, vol)


def clap_roll(b, start_bar, bars=2):
    rows = range(start_bar * B, (start_bar + bars) * B)
    for k, r in enumerate(rows):
        f = k / max(1, len(rows) - 1)
        step = 4 if f < 0.5 else (2 if f < 0.85 else 1)
        if r % step == 0:
            b.put(r, CLAP, "C-4", I_CLAP, int(0x14 + 0x24 * f))


def perc(b, bars=range(4), cow=True, clave=True, conga=False):
    """cowbell on the 'a' of beat 4 every 2nd bar; claves in a 3-3-2 figure; conga answers"""
    for bar in bars:
        if clave:
            for r in (0, 6, 12):
                b.put(bar * B + r, PERC, "C-4", I_CLAV, jitter(0x1A, 2))
        if conga:
            for r, n in ((3, "G-4"), (7, "C-4"), (10, "G-4"), (11, "C-4")):
                if b.cells[bar * B + r][PERC] is None:
                    b.put(bar * B + r, PERC, n, I_CONGA, jitter(0x22, 3))
        if cow and bar % 2 == 1:
            b.put(bar * B + 14, PERC, "C-4", I_COW, 0x20)


# ---- bass: octave jumps on 8ths, syncopated pickups ------------------------------------
def bass(b, chords, bars=range(4), vol=0x3A, fill=True, cut_last=False):
    for bar in bars:
        root = CHORDS[chords[bar % len(chords)]]["root"]
        lo, hi = f"{root}-2" if len(root) == 1 else f"{root}2", f"{root}-3" if len(root) == 1 else f"{root}3"
        # 8th notes alternating low / high octave (disco octave bass), accent on the beat
        for e in range(8):
            r = bar * B + e * 2
            if cut_last and bar == bars[-1] and e >= 6:
                b.put(r, BASS, "===")
                break
            n = lo if e % 2 == 0 else hi
            b.put(r, BASS, n, I_BASS, vol if e % 2 == 0 else vol - 6)
        # 16th pickup into the next bar every 2nd bar (b7 -> root)
        if fill and bar % 2 == 1 and not cut_last:
            b.put(bar * B + 15, BASS, "D#3", I_BASS, vol - 8)


# ---- strings: stabs on the "and" of 2 and 4 (chorus) or swells (intro / break) ---------
def strings(b, chords, bars=range(4), stabs=True, vol=0x30, swell_vol=0x2A):
    for bar in bars:
        ch = CHORDS[chords[bar % len(chords)]]["str"]
        if stabs:
            for r in (6, 14):
                for k, n in enumerate(ch):
                    b.put(bar * B + r, STR1 + k, n, I_STR, vol)
                    b.put(bar * B + r + 2, STR1 + k, "===")
        else:
            for k, n in enumerate(ch):
                b.put(bar * B, STR1 + k, n, I_STR, swell_vol)


# ---- E-Piano: syncopated chord stabs ----------------------------------------------------
EP_RHYTHM = [(0, 3), (3, 2), (8, 2), (10, 3)]   # (row in bar, length in rows)


def epiano(b, chords, bars=range(4), vol=0x2C, rhythm=EP_RHYTHM):
    for bar in bars:
        ch = CHORDS[chords[bar % len(chords)]]["ep"]
        for r, ln in rhythm:
            for k, n in enumerate(ch):
                b.put(bar * B + r, EP1 + k, n, I_EP, jitter(vol, 2))
                b.put(bar * B + r + ln, EP1 + k, "===")


# ---- lead hook (original; F minor pentatonic + the 9th) ---------------------------------
# (row in the 2-bar phrase, note, fx)
HOOK_A = [(0, "C-5", None), (3, "D#5", None), (6, "F-5", None), (10, "D#5", None), (12, "C-5", None),
          (16 + 2, "G#4", None), (16 + 4, "A#4", None), (16 + 8, "C-5", "0103"), (16 + 14, "===", None)]
HOOK_B = [(0, "C-5", None), (3, "D#5", None), (6, "G-5", None), (8, "F-5", None), (10, "D#5", None),
          (12, "F-5", None), (16, "G-5", "0302"), (16 + 6, "F-5", None), (16 + 10, "D#5", None),
          (16 + 12, "C-5", None), (16 + 15, "===", None)]


def lead(b, phrases=(HOOK_A, HOOK_B), bars=range(4), vol=0x34):
    for bar in bars:
        ph = phrases[(bar // 2) % len(phrases)]
        half = (bar % 2) * 16
        for r, n, f in ph:
            if half <= r < half + 16:
                if n == "===":
                    b.put(bar * B + r - half, LEAD, n)
                else:
                    b.put(bar * B + r - half, LEAD, n, I_LEAD, jitter(vol, 1), f)


# ---- voices -----------------------------------------------------------------------------
def choir(b, chords, bars=range(4), vol=0x34, blend="3F60", every=2, words=True):
    """Cyber Choir: one sung word per chord change, Transform blend by 3Fxx; vowel glide"""
    for bar in bars:
        if bar % every:
            continue
        n = CHORDS[chords[bar % len(chords)]]["vox"]
        r = bar * B
        b.put(r, VOX, n, I_VOX, vol, blend)
        b.fx(r + 8, VOX, "3022" if words else "3042")         # glide to I
        b.fx(r + 16 * every - 4, VOX, "3034")                 # O at the end of the phrase


def choir_hook(b, bars=(0, 2), vol=0x36):
    """chorus hook: 'oo-ah' calls answered by the lead"""
    for bar in bars:
        r = bar * B
        b.put(r + 0, VOX, "C-5", I_VOX, vol, "3E00")     # word 0
        b.put(r + 4, VOX, "D#5", I_VOX, vol, "3E01")     # word 1
        b.put(r + 8, VOX, "F-5", I_VOX, vol, "3E02")
        b.fx(r + 12, VOX, "3043")
        b.put(r + 14, VOX, "===")


def robo(b, bars=(1, 3), vol=0x30, sweep=True):
    """robot 'dis-co' answer with a formant sweep (32xx) and stutter on the tail"""
    for bar in bars:
        r = bar * B
        b.put(r + 10, ROBO, "F-4", I_ROBO, vol, "3E00")
        b.put(r + 12, ROBO, "F-4", I_ROBO, vol, "3E01")
        if sweep:
            b.fx(r + 13, ROBO, "3260")
            b.fx(r + 14, ROBO, "32A0")
            b.fx(r + 15, ROBO, "3332")
        b.put(r + 16 if bar < 3 else r + 15, ROBO, "===") if bar < 3 else None


blocks = []


def add(b):
    blocks.append(b)
    return len(blocks) - 1


# 0 intro A: kick + strings swell (filter closed, opens by automation), choir "ooh" far away
b = Block("Intro", "intro"); kick(b); strings(b, CHORUS, stabs=False, swell_vol=0x2C)
b.put(0, FX, "C-4", I_CYM, 0x20); I0 = add(b)
# 1 intro B: + hats, claves, bass enters with filter opening, riser into the groove
b = Block("Intro bass", "intro"); kick(b); hats(b, vol=0x1A); perc(b, cow=False)
strings(b, CHORUS, stabs=False, swell_vol=0x2C); bass(b, CHORUS, bars=range(2, 4), vol=0x34)
b.put(32, FX, "C-4", I_RISE, 0x2C); I1 = add(b)
# 2 groove: full drums, bass, EP stabs, robot "dis-co"
b = Block("Groove", "verse"); kick(b); hats(b); claps(b); perc(b); bass(b, VERSE); epiano(b, VERSE)
robo(b); b.put(0, FX, "C-4", I_IMPACT, 0x30); I2 = add(b)
# 3 groove end: + conga, string pickup stab, kick drop on the last beat
b = Block("Groove turn", "verse"); kick(b, skip={(3, 3)}); hats(b); claps(b); perc(b, conga=True)
bass(b, VERSE); epiano(b, VERSE); robo(b, bars=(1,)); strings(b, ["Bb9"], bars=(3,), vol=0x2C); I3 = add(b)
# 4-5 verse: choir sings the chord tones (light Transform), EP, bass; lead silent
b = Block("Verse", "verse"); kick(b); hats(b); claps(b); perc(b, cow=False); bass(b, VERSE)
epiano(b, VERSE, vol=0x28); choir(b, VERSE, blend="3F50"); I4 = add(b)
b = Block("Verse 2", "verse"); kick(b); hats(b); claps(b); perc(b, conga=True); bass(b, VERSE)
epiano(b, VERSE, vol=0x28); choir(b, VERSE, blend="3F70"); robo(b, bars=(3,))
strings(b, VERSE, stabs=False, swell_vol=0x22); I5 = add(b)


# 6-7 chorus: string stabs, lead hook, choir hook, claps, cowbell
def chorus(name, phrases=(HOOK_A, HOOK_B), last=False, vox_vol=0x36, crash=True):
    d = Block(name, "chorus")
    kick(d, skip={(3, 3)} if last else ()); hats(d); claps(d); perc(d, conga=True)
    bass(d, CHORUS, cut_last=last); strings(d, CHORUS); epiano(d, CHORUS, vol=0x26)
    lead(d, phrases); choir_hook(d, vol=vox_vol)
    if crash:
        d.put(0, FX, "C-4", I_CYM, 0x28)
    return d


I6 = add(chorus("Chorus"))
I7 = add(chorus("Chorus end", (HOOK_A, HOOK_A), last=True, crash=False))
# 8 breakdown: no kick; strings swell + choir with big reverb, flanger sweep, downlifter
b = Block("Breakdown", "breakdown"); strings(b, BREAK, stabs=False, swell_vol=0x34)
choir(b, BREAK, vol=0x38, blend="3FB0", every=1); b.put(0, FX, "C-4", I_DOWN, 0x34)
epiano(b, BREAK, vol=0x20, rhythm=[(0, 6), (10, 4)]); I8 = add(b)
# 9 breakdown 2: filtered bass back (octaves), robot voice formant sweep, claves
b = Block("Breakdown 2", "breakdown"); strings(b, BREAK, stabs=False, swell_vol=0x34)
choir(b, BREAK, vol=0x36, blend="3FC0", every=1); bass(b, BREAK, vol=0x30, fill=False)
perc(b, cow=False); robo(b, bars=(1, 3), vol=0x2C); I9 = add(b)
# 10-11 build: kick returns (half time -> four), clap roll, riser, filters opening (3Axx)
b = Block("Build", "build"); kick(b, bars=range(4), skip={(0, 1), (0, 3), (1, 1), (1, 3)}); hats(b, vol=0x1A)
bass(b, VERSE, vol=0x32); strings(b, VERSE, stabs=False, swell_vol=0x2E); choir(b, VERSE, blend="3F80")
b.put(0, FX, "C-4", I_RISE, 0x34); I10 = add(b)
b = Block("Build roll", "build"); kick(b, bars=range(3)); hats(b, bars=range(3), vol=0x1C)
bass(b, VERSE, bars=range(3), vol=0x34); clap_roll(b, 2, 2)
strings(b, ["Bb9"] * 4, stabs=False, swell_vol=0x32)
for k in range(3):
    b.fx(32, STR1 + k, "3A08")     # cutoff slides up through the roll
b.put(32, VOX, "D-5", I_VOX, 0x34, "3FD0"); b.fx(40, VOX, "3210"); b.fx(48, VOX, "32F0")   # formant sweep up
b.put(60, VOX, "==="); b.put(60, STR1, "==="); b.put(60, STR2, "==="); b.put(60, STR3, "===")
I11 = add(b)
# 12-14 final chorus (+ robot answers, choir doubled by the lead an octave up)
d = chorus("Final chorus", vox_vol=0x38); robo(d, bars=(1, 3)); I12 = add(d)
d = chorus("Final chorus 2", (HOOK_B, HOOK_A), crash=False, vox_vol=0x38); robo(d, bars=(3,)); I13 = add(d)
I14 = add(chorus("Final chorus end", (HOOK_A, HOOK_B), last=True, crash=False, vox_vol=0x38))
# 15-16 outro: groove thins out, strings and choir tail into the reverb, echo throws
b = Block("Outro", "outro"); kick(b); hats(b, vol=0x1A); claps(b); perc(b, conga=True)
bass(b, VERSE); epiano(b, VERSE, vol=0x24); choir(b, VERSE, blend="3F90"); I15 = add(b)
b = Block("Outro end", "outro"); kick(b, bars=range(3)); hats(b, bars=range(3), vol=0x18)
bass(b, ["Fm9"] * 4, bars=range(2), vol=0x30, fill=False); b.put(32, BASS, "===")
strings(b, ["Fm9"], bars=(0,), stabs=False, swell_vol=0x2C); b.put(48, STR1, "==="); b.put(48, STR2, "==="); b.put(48, STR3, "===")
b.put(32, VOX, "G#4", I_VOX, 0x30, "3FA0"); b.fx(40, VOX, "3034"); b.put(52, VOX, "===")
b.put(40, FX, "C-4", I_DOWN, 0x2A); I16 = add(b)

ORDER = ([I0, I1]                 # intro 8 bars
         + [I2, I3]               # groove 8 bars
         + [I4, I5, I4, I5]       # verse 16 bars
         + [I6, I7, I6, I7]       # chorus 16 bars
         + [I8, I9]               # breakdown 8 bars
         + [I10, I11]             # build 8 bars
         + [I12, I13, I12, I14]   # final chorus 16 bars
         + [I15, I16])            # outro 8 bars
start = {}
for i, bk in enumerate(ORDER):
    start.setdefault(bk, []).append(i * ROWS)
total = len(ORDER) * ROWS


def at(bk, k=0):
    return start[bk][k]


print(f'''# ChipWave demo "Velvet Mirrorball" (generated by scripts/make_disco.py; edit that, not this)
# Disco in F minor, {BPM} BPM. Single-file text form of a .cwt song (docs/FIRST_RUN.md).
# Four-on-the-floor 909 kick, offbeat open hat (quiet, once per beat), claps on 2 and 4,
# octave bass, string stabs, E-Piano chords, Warm Lead hook, Cyber Choir + robot voice.
# Reference analysis and mix notes: docs/research/disco.md.
format = "chipwave-song-text"
format_version = 2

[song]
title = "Velvet Mirrorball (disco demo)"
bpm = {BPM}
lines_per_beat = 4
speed = 6
tracks = {CH}
key = "F minor"
''')
print('''[[instrument]]
slot = 1
name = "909 Kick"
patch = "gen:909 Kick"
# played at D#4: the 909 body tuned up to about 62 Hz (disco kicks sit above the sub), HP 35 Hz
fx = [{ kind = "Filter", mode = 1.0, cutoff = 35.0, resonance = 0.0 }]
[[instrument]]
slot = 2
name = "Octave Bass"
patch = "gen:Fat Moog Bass"
glide_ms = 0.0
legato = false
# fx 1 = LP (opens in the intro, closes in the breakdown), fx 2 = HP 40 Hz (no rumble)
fx = [{ kind = "Filter", mode = 0.0, cutoff = 6000.0, resonance = 0.25 },
      { kind = "Filter", mode = 1.0, cutoff = 40.0, resonance = 0.0 }]
[[instrument]]
slot = 3
name = "Open Hat"
patch = "gen:Open Hat"
fx = [{ kind = "Filter", mode = 1.0, cutoff = 600.0, resonance = 0.0 }, { kind = "Filter", mode = 0.0, cutoff = 11000.0, resonance = 0.0 }]
[[instrument]]
slot = 4
name = "808 Clap"
patch = "gen:808 Clap"
fx = [{ kind = "Filter", mode = 1.0, cutoff = 250.0, resonance = 0.0 }]
[[instrument]]
slot = 5
name = "Cowbell"
patch = "gen:Cowbell"
[[instrument]]
slot = 6
name = "Claves"
patch = "gen:Claves"
[[instrument]]
slot = 7
name = "Conga"
patch = "gen:Conga"
[[instrument]]
slot = 8
name = "Disco Strings"
patch = "builtin:init"
wave = "saw"
unison = 5
detune = 14.0
width = 1.0
osc2 = { wave = "saw", semi = 12, level = 0.3 }
cutoff = 6000.0
resonance = 0.15
env_amount = 0.2
filter_decay = 0.25
attack = 0.006
decay = 0.35
sustain = 0.7
release = 0.35
gain = 0.42
# fx 1 = HP 250 Hz, fx 2 = LP sweep (automated: closed in the intro and breakdown),
# fx 3 = "phaser": a short swirling delay with negative feedback (ChipWave has no phaser device),
# fx 4 = flanger (mix automated: only in the breakdown and on the build)
fx = [{ kind = "Filter", mode = 1.0, cutoff = 380.0, resonance = 0.0 },
      { kind = "Filter", mode = 0.0, cutoff = 6000.0, resonance = 0.2 },
      { kind = "Chorus", rate = 0.22, depth = 2.2, delay = 2.5, feedback = -0.55, mix = 0.5 },
      { kind = "Chorus", rate = 0.12, depth = 3.5, delay = 3.0, feedback = 0.75, mix = 0.0 }]
[[instrument]]
slot = 9
name = "E-Piano"
patch = "gen:E-Piano"
fx = [{ kind = "Filter", mode = 1.0, cutoff = 260.0, resonance = 0.0 },
      { kind = "Chorus", rate = 0.6, depth = 2.0, delay = 10.0, feedback = 0.0, mix = 0.35 }]
[[instrument]]
slot = 10
name = "Warm Lead"
patch = "gen:Warm Lead"
fx = [{ kind = "Filter", mode = 1.0, cutoff = 300.0, resonance = 0.0 },
      { kind = "Delay", sync = 3.0, feedback = 0.35, pingpong = 1.0, hp = 400.0, lp = 6000.0, mix = 0.22 }]
[[instrument]]
slot = 11
name = "Cyber Choir"
patch = "voice:Cyber Choir"
attack = 0.06
release = 0.7
fx = [{ kind = "Filter", mode = 1.0, cutoff = 220.0, resonance = 0.0 },
      { kind = "Chorus", rate = 0.35, depth = 3.0, delay = 14.0, feedback = 0.2, mix = 0.4 }]
[instrument.voice]
words = ["oo", "ah", "yeah"]
rate = 0.9
[instrument.voice.transform]
blend = 0.5
[[instrument]]
slot = 12
name = "Robot Disco"
patch = "voice:Robot Err"
fx = [{ kind = "Filter", mode = 1.0, cutoff = 300.0, resonance = 0.0 },
      { kind = "Chorus", rate = 0.15, depth = 1.5, delay = 2.0, feedback = 0.6, mix = 0.3 }]
[instrument.voice]
words = ["dis", "ko"]
rate = 1.1
[[instrument]]
slot = 13
name = "Riser"
patch = "gen:White Riser"
fx = [{ kind = "Filter", mode = 1.0, cutoff = 400.0, resonance = 0.0 }]
[[instrument]]
slot = 14
name = "Downlifter"
patch = "gen:Downlifter"
fx = [{ kind = "Filter", mode = 1.0, cutoff = 300.0, resonance = 0.0 }]
[[instrument]]
slot = 15
name = "Impact"
patch = "gen:Impact"
[[instrument]]
slot = 16
name = "Cymbal"
patch = "gen:Cymbal"
fx = [{ kind = "Filter", mode = 1.0, cutoff = 2000.0, resonance = 0.0 }]
''')
MIX = os.environ.get("MIX_MASTER_DB", "0.0")
LIM = os.environ.get("MIX_LIM_GAIN", "3.2")
print(f'''[mixer]
master_db = {MIX}
duck_key = 1
duck_release_ms = 160
[mixer.limiter]
enabled = true
gain_db = {LIM}
ceiling_db = -1.8
release_ms = 120.0
target_lufs = -14.0
''')
print('''# gain staging: kick and bass lead the low end (dry, centre); hats quiet and right; claps a
# little reverb; strings wide with lots of reverb; EP, lead and voices with clearly audible echo
# (A, dotted 1/8 ping-pong) and reverb (B, a warm plate-like room). Sidechain: the bass, strings
# and EP duck a little off the kick (the reference pumps about 4 dB).
[[mixer.channel]]
name = "Kick"
volume_db = -4.0
[[mixer.channel]]
name = "Bass"
volume_db = -6.0
duck = 0.3
[[mixer.channel]]
name = "Hats"
volume_db = 3.0
pan = 0.25
send_b = 0.08
[[mixer.channel]]
name = "Clap"
volume_db = -1.0
pan = -0.05
send_b = 0.22
[[mixer.channel]]
name = "Perc"
volume_db = -4.0
pan = -0.35
send_a = 0.08
send_b = 0.15
[[mixer.channel]]
name = "Strings L"
volume_db = -5.0
pan = -0.45
send_a = 0.05
send_b = 0.45
duck = 0.35
[[mixer.channel]]
name = "Strings C"
volume_db = -5.0
pan = 0.0
send_a = 0.05
send_b = 0.45
duck = 0.35
[[mixer.channel]]
name = "Strings R"
volume_db = -5.0
pan = 0.45
send_a = 0.05
send_b = 0.45
duck = 0.35
[[mixer.channel]]
name = "Keys 1"
volume_db = -9.0
pan = -0.25
send_a = 0.12
send_b = 0.25
duck = 0.25
[[mixer.channel]]
name = "Keys 2"
volume_db = -9.0
pan = 0.05
send_a = 0.12
send_b = 0.25
duck = 0.25
[[mixer.channel]]
name = "Keys 3"
volume_db = -9.0
pan = 0.3
send_a = 0.12
send_b = 0.25
duck = 0.25
[[mixer.channel]]
name = "Lead"
volume_db = -11.5
pan = 0.15
send_a = 0.35
send_b = 0.35
duck = 0.15
[[mixer.channel]]
name = "Choir"
volume_db = 0.0
pan = -0.1
send_a = 0.3
send_b = 0.5
duck = 0.15
[[mixer.channel]]
name = "Robot"
volume_db = 6.0
pan = 0.3
send_a = 0.45
send_b = 0.3
[[mixer.channel]]
name = "FX"
volume_db = -9.0
pan = -0.2
send_a = 0.2
send_b = 0.4

[mixer.return_a]
sync = 3.0
feedback = 0.4
pingpong = 1.0
hp = 350.0
lp = 5500.0
mix = 1.0
level = 1.0
[mixer.return_b]
size = 0.8
damping = 0.45
predelay = 20.0
width = 1.0
mix = 1.0
lowcut = 250.0
highcut = 9000.0
level = 1.0
''')
for i, bk in enumerate(blocks):
    print(f'[[block]]\nid = {i}\nname = "{bk.name}"\nlines = {ROWS}\nrole = "{bk.role}"')
    print("# row | kick | bass | hat | clap | perc | str L C R | keys 1 2 3 | lead | choir | robot | fx")
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
        print(f"  {{ pos = {p[0]}, value = {p[1]}{sh} }},")
    print("]\n")


print("# Automation: positions in lines from the song start (64 lines = one block = 4 bars)")
grv, vrs, cho, brk, bld, fin, out = at(I2), at(I4), at(I6), at(I8), at(I10), at(I12), at(I15)
roll = at(I11)
# strings LP: closed in the intro, opens to the groove; dark in the breakdown, opens over the build
lane("instrument.8.fx.2.cutoff", "Strings - LP sweep", [
    (0, 500), (2 * B * 4 - 1, 3000), (grv, 6000), (brk - 1, 8000, "step"), (brk, 900),
    (brk + 8 * B, 1500), (bld, 1200), (roll + 3 * B, 9000), (fin, 7000), (out, 6000), (total, 1200)])
lane("instrument.8.fx.4.mix", "Strings - Flanger", [
    (0, 0.0), (brk, 0.55), (bld, 0.3), (fin, 0.0), (out + 4 * B, 0.35), (total, 0.35)], curve="step")
lane("instrument.8.fx.4.delay", "Strings - Flanger sweep", [
    (0, 3.0), (brk, 1.0), (brk + 4 * B, 8.0), (brk + 8 * B, 1.0), (bld, 8.0), (fin, 3.0),
    (out + 4 * B, 1.0), (total, 8.0)], curve="smooth")
# bass LP: opens in the intro, filtered breakdown bass, HP-ish thin on the build
lane("instrument.2.fx.1.cutoff", "Bass - LP filter", [
    (0, 200), (at(I1) + 2 * B, 200), (grv, 3000), (brk, 3000), (brk + 1, 350, "step"),
    (bld, 500), (roll + 3 * B, 4000), (fin, 6000), (total, 1500)])
# choir: Transform blend comes from 3Fxx; this lane opens the voice filter in the build
lane("instrument.11.fx.1.cutoff", "Choir - HP sweep", [
    (0, 220), (roll, 220, "linear"), (roll + 4 * B - 1, 1200, "step"), (fin, 220), (total, 220)],
    curve="linear")
lane("instrument.3.fx.1.cutoff", "Hats - HP sweep", [
    (0, 600), (roll, 600, "linear"), (roll + 3 * B, 3000, "step"), (fin, 600), (total, 600)],
    curve="linear")
lane("instrument.13.level", "Riser - Level", [
    (0, 0.0), (at(I1) + 2 * B, 0.2), (grv - 1, 1.0, "step"), (grv, 0.0), (bld, 0.15),
    (fin - 1, 1.1, "step"), (fin, 0.0)], curve="linear")
lane("instrument.13.cutoff", "Riser - Filter", [
    (0, 500), (bld, 500), (fin - 1, 9000, "step"), (fin, 500)])
# echo throws at the end of every chorus and before the drops
ends = [at(I7, 0) + 4 * B, at(I7, 1) + 4 * B, at(I3) + 4 * B, fin, at(I14) + 4 * B]
pts = [(0, 0.35)]
for e in ends:
    pts += [(e - 6, 0.85), (e + 2, 0.35)]
lane("mixer.ch.12.send_a", "Lead - Echo throws", pts + [(total, 0.5)], curve="step")
lane("mixer.ch.13.send_b", "Choir - Reverb", [
    (0, 0.5), (brk, 0.75), (bld, 0.6), (fin, 0.5), (out, 0.7), (total, 0.8)], curve="linear")
lane("mixer.ch.13.send_a", "Choir - Echo", [
    (0, 0.3), (brk, 0.5), (bld, 0.35), (out, 0.55), (total, 0.6)], curve="step")
lane("mixer.return_a.feedback", "Echo feedback", [
    (0, 0.4), (brk, 0.55), (bld, 0.45), (roll + 3 * B, 0.7), (fin, 0.4), (out, 0.55), (total, 0.65)],
    curve="step")
lane("mixer.return_b.size", "Reverb size", [
    (0, 0.8), (brk, 0.92), (bld, 0.85), (fin, 0.8), (out, 0.9), (total, 0.95)], curve="linear")
