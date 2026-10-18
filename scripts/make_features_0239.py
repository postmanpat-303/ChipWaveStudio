#!/usr/bin/env python3
"""0.2.3.9 feature demo: a drums-only A minor song; the chord helper code
(src/chords.rs) writes i-VI-III-VII as a gated pad (block 1), adds a rolling
bass (block 2) and an up-down arp (block 3); block 4 swaps the pad and arp
for other factory sounds with "use this instead of" (this block only).
Sounds come from the instrument library's factory list; the pad goes through
a .cwi file. Every block plays twice (4 bars each, 140 BPM).
Usage: scripts/make_features_0239.py [out.mp3]   (needs ffmpeg, cargo)"""
import os, re, subprocess, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = sys.argv[1] if len(sys.argv) > 1 else "/workspace/chipwave-0.2.3.9-features.mp3"
TMP = "/tmp/f239"
BIN = os.path.join(ROOT, "target/release/chipwave")
TRACKS = 7
os.makedirs(TMP, exist_ok=True)
EMPTY = "--- .. .. ...."


def cell(note="---", ins=None, vol=None):
    return f"{note} {('%02X' % ins) if ins else '..'} {vol or '..'} ...."


t = ('format = "chipwave-song-text"\nformat_version = 2\n\n[song]\ntitle = "0.2.3.9 chord helper"\n'
     f'key = "A minor"\nbpm = 140\nlines_per_beat = 4\nspeed = 6\ntracks = {TRACKS}\n\n'
     '[[instrument]]\nslot = 1\nname = "Kick"\npatch = "builtin:trance-kick"\n'
     '[[instrument]]\nslot = 2\nname = "Hat"\npatch = "builtin:hat-open"\n')
t += "[mixer]\nmaster_db = 0.0\nduck_key = 1\nduck_release_ms = 140\n"
for name, vol, pan, a, b, duck in [("Kick", 0, 0, 0, 0, 0), ("Hat", -10, 0.2, 0, 0.05, 0),
                                   ("Bass", 0, 0, 0, 0, 0.7), ("Pad 1", -7, -0.5, 0.1, 0.3, 0.6),
                                   ("Pad 2", -7, 0, 0.1, 0.3, 0.6), ("Pad 3", -7, 0.5, 0.1, 0.3, 0.6),
                                   ("Arp", -6, 0.25, 0.3, 0.2, 0.4)]:
    t += (f'[[mixer.channel]]\nname = "{name}"\nvolume_db = {vol}\npan = {pan}\nsend_a = {a}\n'
          f"send_b = {b}\nduck = {duck}\n")
t += "[mixer.return_a]\nsync = 3.0\nfeedback = 0.3\nmix = 1.0\nlevel = 0.7\n[mixer.return_b]\nsize = 0.75\nmix = 1.0\nlevel = 0.8\n"
names = ["Chords", "+ Bass", "+ Arp", "Swap"]
for b in range(4):
    t += f'[[block]]\nid = {b}\nname = "{names[b]}"\nlines = 64\nrows = """\n'
    for r in range(64):
        ev = {}
        if r % 4 == 0:
            ev[0] = cell("A-2", 1)
        if r % 4 == 2:
            ev[1] = cell("C-5", 2, "30")
        t += f"{r:02X} | " + " | ".join(ev.get(c, EMPTY) for c in range(TRACKS)) + "\n"
    t += '"""\n'
t += "\n[sequence]\nsections = [" + ", ".join(f"{{ block = {b}, repeat = 2 }}" for b in range(4)) + "]\n"
base = f"{TMP}/base.cwt.toml"
open(base, "w").write(t)
song = f"{TMP}/chords-0239.cwt"
env = dict(os.environ, CW_F239_IN=base, CW_F239_OUT=song)
subprocess.run(["cargo", "test", "--release", "-q", "--", "--ignored", "features_0239_demo"],
               cwd=ROOT, env=env, check=True)
wav = f"{TMP}/chords-0239.wav"
subprocess.run([BIN, "--render", song, wav], check=True)
st = subprocess.run(["ffmpeg", "-i", wav, "-af", "ebur128=peak=true", "-f", "null", "-"],
                    capture_output=True, text=True).stderr
print("integrated", re.findall(r"I:\s+(-?[\d.]+) LUFS", st)[-1], "LUFS, true peak",
      re.findall(r"Peak:\s+(-?[\d.]+) dBFS", st)[-1], "dBFS")
subprocess.run(["ffmpeg", "-loglevel", "error", "-y", "-i", wav, "-codec:a", "libmp3lame", "-q:a", "2", OUT],
               check=True)
print(OUT)
