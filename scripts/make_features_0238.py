#!/usr/bin/env python3
"""0.2.3.8 feature demo: one song that grows from 8 to 16 to 32 channels
(drums + bass + an interlocking 4-channel arp, then 8 held 'string' voices,
then 16 panned sparkle voices). Every voice is its own tracker channel.
Renders with the release binary and writes one MP3.
Usage: scripts/make_features_0238.py [out.mp3]   (needs ffmpeg)"""
import os, re, subprocess, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = sys.argv[1] if len(sys.argv) > 1 else "/workspace/chipwave-0.2.3.8-features.mp3"
TMP = "/tmp/f238"
BIN = os.path.join(ROOT, "target/release/chipwave")
BPM, TRACKS = 128, 32
os.makedirs(TMP, exist_ok=True)
EMPTY = "--- .. .. ...."
NAMES = ["C-", "C#", "D-", "D#", "E-", "F-", "F#", "G-", "G#", "A-", "A#", "B-"]


def nn(semi, octv):
    """note name for a semitone (0 = C) in an octave"""
    octv += semi // 12
    return f"{NAMES[semi % 12]}{octv}"


def cell(note="---", ins=None, vol=None, fx=None):
    return f"{note} {('%02X' % ins) if ins else '..'} {vol or '..'} {fx or '....'}"


INSTR = '''[[instrument]]
slot = 1
name = "Kick"
patch = "builtin:trance-kick"
[[instrument]]
slot = 2
name = "Hat"
patch = "builtin:hat-closed"
[[instrument]]
slot = 3
name = "Clap"
patch = "builtin:clap"
[[instrument]]
slot = 4
name = "Bass"
patch = "builtin:init"
wave = "saw"
octave = -1
cutoff = 500.0
resonance = 0.3
env_amount = 0.3
filter_decay = 0.08
decay = 0.12
sustain = 0.0
release = 0.03
gain = 0.8
[[instrument]]
slot = 5
name = "Arp"
patch = "builtin:init"
wave = "pulse"
cutoff = 2600.0
decay = 0.12
sustain = 0.0
release = 0.05
gain = 0.45
[[instrument]]
slot = 6
name = "String"
patch = "builtin:init"
wave = "triangle"
attack = 0.25
decay = 0.4
sustain = 0.8
release = 0.4
cutoff = 3000.0
gain = 0.28
[[instrument]]
slot = 7
name = "Sparkle"
patch = "builtin:init"
wave = "square"
cutoff = 6000.0
decay = 0.12
sustain = 0.0
release = 0.1
gain = 0.55
'''
# A minor: Am F C G, one chord per bar (16 rows), 4 bars per block
CHORDS = [(9, [9, 12, 16]), (5, [5, 9, 12]), (0, [0, 4, 7]), (7, [7, 11, 14])]


def events(layers):
    ev = {}
    for r in range(64):
        bar, step = divmod(r, 16)
        root, tones = CHORDS[bar]
        if step % 4 == 0:
            ev[(r, 0)] = cell("A-2", 1)
        if step % 4 == 2:
            ev[(r, 1)] = cell("C-5", 2, "28")
        if step in (4, 12):
            ev[(r, 2)] = cell("C-4", 3, "30")
        if step % 4 == 2 or step % 4 == 3:
            ev[(r, 3)] = cell(nn(root, 2), 4, "38" if step % 4 == 2 else "28")
        # interlocking arp: channel 4+k plays every 4th 16th, rotated
        arp = [tones[0], tones[1], tones[2], tones[1] + 12]
        ev[(r, 4 + step % 4)] = cell(nn(arp[(step + bar) % 4], 4), 5, "30" if step % 4 == 0 else "22")
        if layers >= 2 and step == 0:
            # 8 held string voices: chord tones over three octaves
            vs = [tones[0], tones[1], tones[2], tones[0] + 12, tones[1] + 12, tones[2] + 12, tones[0] + 24, tones[2] - 12]
            for k, t in enumerate(vs):
                ev[(r, 8 + k)] = cell(nn(t, 3), 6, "38")
        if layers >= 3:
            # 16 sparkle voices, each a one-shot blip on its own row of the bar
            k = (step * 5 + bar * 3) % 16
            t = tones[step % 3] + 24 + (12 if step % 5 == 0 else 0)
            ev[(r, 16 + k)] = cell(nn(t, 3), 7, "%02X" % (0x30 + (step % 3) * 8))
    return ev


def block(bid, ev, lines=64):
    out = [f'[[block]]\nid = {bid}\nname = "{8 << bid} ch"\nlines = {lines}\nrows = """']
    for r in range(lines):
        out.append(f"{r:02X} | " + " | ".join(ev.get((r, c), EMPTY) for c in range(TRACKS)))
    out.append('"""\n')
    return "\n".join(out)


t = (f'format = "chipwave-song-text"\nformat_version = 2\n\n[song]\ntitle = "0.2.3.8 thirty-two channels"\n'
     f"bpm = {BPM}\nlines_per_beat = 4\nspeed = 6\ntracks = {TRACKS}\n\n" + INSTR)
t += "[mixer]\nmaster_db = 0.0\nduck_key = 1\nduck_release_ms = 120\n"
for c in range(TRACKS):
    if c < 4:
        name, vol, pan, a, b, duck = [("Kick", 0, 0, 0, 0, 0), ("Hat", -8, 0.3, 0, 0.05, 0),
                                      ("Clap", -6, -0.1, 0, 0.15, 0), ("Bass", -2, 0, 0, 0, 0.7)][c]
    elif c < 8:
        name, vol, pan, a, b, duck = f"Arp {c - 3}", -4, (-0.6, -0.2, 0.2, 0.6)[c - 4], 0.15, 0.1, 0.3
    elif c < 16:
        name, vol, pan, a, b, duck = f"Str {c - 7}", -6, -0.8 + (c - 8) * 0.23, 0, 0.3, 0.5
    else:
        name, vol, pan, a, b, duck = f"Spk {c - 15}", 0, -0.9 + (c - 16) * 0.12, 0.25, 0.25, 0.2
    t += (f'[[mixer.channel]]\nname = "{name}"\nvolume_db = {vol}\npan = {pan:.2f}\nsend_a = {a}\n'
          f"send_b = {b}\nduck = {duck}\n")
t += "[mixer.return_a]\nsync = 3.0\nfeedback = 0.35\nmix = 1.0\nlevel = 0.8\n[mixer.return_b]\nsize = 0.8\nmix = 1.0\nlevel = 0.9\n"
for b in range(3):
    t += block(b, events(b + 1))
t += "\n[sequence]\nsections = [" + ", ".join(f"{{ block = {b}, repeat = 2 }}" for b in range(3)) + "]\n"
song = f"{TMP}/thirty-two.cwt.toml"
open(song, "w").write(t)
wav = f"{TMP}/thirty-two.wav"
subprocess.run([BIN, "--render", song, wav], check=True)
st = subprocess.run(["ffmpeg", "-i", wav, "-af", "ebur128=peak=true", "-f", "null", "-"],
                    capture_output=True, text=True).stderr
print("integrated", re.findall(r"I:\s+(-?[\d.]+) LUFS", st)[-1], "LUFS, true peak",
      re.findall(r"Peak:\s+(-?[\d.]+) dBFS", st)[-1], "dBFS")
subprocess.run(["ffmpeg", "-loglevel", "error", "-y", "-i", wav, "-codec:a", "libmp3lame", "-q:a", "2", OUT],
               check=True)
print(OUT)
