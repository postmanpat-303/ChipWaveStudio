#!/usr/bin/env python3
"""0.2.3.13 Chip FX demo (145 BPM, 4 rows per beat, 32-row blocks = 3.3 s):
  0:00.0  Pickup     - three rolls of the category button (seeds 11-13)
  0:03.3  Laser      - three rolls
  0:06.6  Explosion  - three rolls
  0:09.9  Power-up   - three rolls
  0:13.2  Hit        - three rolls
  0:16.6  Jump       - three rolls
  0:19.9  Blip       - three rolls
  0:23.2  Randomize  - four rolls
  0:26.5  Mutate     - Laser Shot, then three 25 % mutations (two hits each)
  0:29.8  In tune    - Psy Laser played as a melody (Follow note), Coin arpeggio
  0:33.1  Groove A   - Psy Kit + rolling bass + Laser Shots (43 1y moves Freq), Blips, Power Up
  0:39.7  Groove B   - the same with an Explosion; ends 0:46.3 (+ tail)
Usage: scripts/make_features_02313.py [out.mp3]   (needs ffmpeg, cargo)"""
import os, re, subprocess, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = sys.argv[1] if len(sys.argv) > 1 else "/workspace/chipwave-0.2.3.13-chip-fx.mp3"
TMP = "/tmp/f2313"
BIN = os.path.join(ROOT, "target/release/chipwave")
TRACKS = 8
os.makedirs(TMP, exist_ok=True)
EMPTY = "--- .. .. ...."

t = ('format = "chipwave-song-text"\nformat_version = 2\n\n[song]\ntitle = "0.2.3.13 chip fx"\n'
     f'bpm = 145\nlines_per_beat = 4\nspeed = 6\ntracks = {TRACKS}\n\n')
t += "[mixer]\nmaster_db = 0.0\nduck_key = 1\nduck_release_ms = 110\n"
for name, vol, pan, a, b, duck in [("Kick", 0, 0, 0, 0, 0), ("Hats", -4, 0.2, 0, 0.06, 0),
                                   ("Snare/clap", -2, -0.1, 0, 0.12, 0), ("Perc", -3, -0.25, 0.08, 0.1, 0),
                                   ("Bass", -3, 0, 0, 0, 0.8), ("Chip FX", -2, 0.15, 0.15, 0.12, 0.3),
                                   ("Chip FX 2", -4, -0.2, 0.15, 0.12, 0.3), ("8", 0, 0, 0, 0, 0)]:
    t += (f'[[mixer.channel]]\nname = "{name}"\nvolume_db = {vol}\npan = {pan}\nsend_a = {a}\n'
          f"send_b = {b}\nduck = {duck}\n")
t += "[mixer.return_a]\nsync = 3.0\nfeedback = 0.25\nmix = 1.0\nlevel = 0.5\n[mixer.return_b]\nsize = 0.5\nmix = 1.0\nlevel = 0.6\n"
names = ["Pickup", "Laser", "Explosion", "Power-up", "Hit", "Jump", "Blip", "Randomize",
         "Mutate", "In tune", "Groove A", "Groove B"]
reps = [1] * 10 + [2, 2]
for b, n in enumerate(names):
    t += f'[[block]]\nid = {b}\nname = "{n}"\nlines = 32\nrows = """\n'
    for r in range(32):
        t += f"{r:02X} | " + " | ".join(EMPTY for _ in range(TRACKS)) + "\n"
    t += '"""\n'
t += "\n[sequence]\nsections = [" + ", ".join(f"{{ block = {b}, repeat = {r} }}" for b, r in enumerate(reps)) + "]\n"
base = f"{TMP}/base.cwt.toml"
open(base, "w").write(t)
song = f"{TMP}/chipfx-02313.cwt"
env = dict(os.environ, CW_F2313_IN=base, CW_F2313_OUT=song)
subprocess.run(["cargo", "test", "--release", "-q", "--", "--ignored", "features_02313_demo"],
               cwd=ROOT, env=env, check=True)
subprocess.run(["cargo", "build", "--release", "-q"], cwd=ROOT, check=True)
wav = f"{TMP}/chipfx-02313.wav"
subprocess.run([BIN, "--render", song, wav], check=True)
st = subprocess.run(["ffmpeg", "-i", wav, "-af", "ebur128=peak=true", "-f", "null", "-"],
                    capture_output=True, text=True).stderr
print("integrated", re.findall(r"I:\s+(-?[\d.]+) LUFS", st)[-1], "LUFS, true peak",
      re.findall(r"Peak:\s+(-?[\d.]+) dBFS", st)[-1], "dBFS")
subprocess.run(["ffmpeg", "-loglevel", "error", "-y", "-i", wav, "-codec:a", "libmp3lame", "-q:a", "2", OUT],
               check=True)
print(OUT)
