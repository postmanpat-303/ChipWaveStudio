#!/usr/bin/env python3
"""0.2.3.12 Generators demo (145 BPM, 4 rows per beat, 32-row blocks = 3.3 s):
  0:00.0  Kicks        - Psy Zap Kick, 808 Boom, 909 Kick, v0 Punch Kick (2 hits each)
  0:03.3  Snares/claps - 808 Snare, 909 Snare, Shell Snare, 808 Clap, Tight Clap
  0:06.6  Hats         - Closed Hat, Open Hat, Ring Hat, Comb Hat
  0:09.9  Toms/perc    - 808 Tom, Conga, Cowbell, Claves, Rimshot, Cymbal, Zap Perc
  0:13.2  Dice         - 8 dice rolls of the Psy Zap Kick, diced closed hats
  0:16.6  Mutate       - 8 mutations (25 %) of the Psy Zap Kick
  0:19.9  43xy         - kick decay growing (434x), punch dropping (432x), hat metal (433x)
  0:23.2  SVF 12 dB    - an acid line with a 38xx cutoff sweep, resonance 0.82
  0:26.5  Ladder clean - the same line through the Moog ladder (24 dB)
  0:29.8  Ladder vintage - saturated, 2x oversampled
  0:33.1  Groove A     - Psy Kit (kick, hats, clap, perc from one instrument), vintage rolling bass
  0:39.7  Groove B     - + 16th hats, 43xy shortening the kick; ends 0:46.3 (+ tail)
Usage: scripts/make_features_02312.py [out.mp3]   (needs ffmpeg, cargo)"""
import os, re, subprocess, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = sys.argv[1] if len(sys.argv) > 1 else "/workspace/chipwave-0.2.3.12-generators.mp3"
TMP = "/tmp/f2312"
BIN = os.path.join(ROOT, "target/release/chipwave")
TRACKS = 8
os.makedirs(TMP, exist_ok=True)
EMPTY = "--- .. .. ...."

t = ('format = "chipwave-song-text"\nformat_version = 2\n\n[song]\ntitle = "0.2.3.12 generators"\n'
     f'bpm = 145\nlines_per_beat = 4\nspeed = 6\ntracks = {TRACKS}\n\n')
t += "[mixer]\nmaster_db = 0.0\nduck_key = 1\nduck_release_ms = 110\n"
for name, vol, pan, a, b, duck in [("Kick", 0, 0, 0, 0, 0), ("Hats", -4, 0.2, 0, 0.06, 0),
                                   ("Snare/clap", -2, -0.1, 0, 0.12, 0), ("Perc", -3, -0.25, 0.08, 0.1, 0),
                                   ("Bass", -3, 0, 0, 0, 0.8), ("6", 0, 0, 0, 0, 0),
                                   ("7", 0, 0, 0, 0, 0), ("8", 0, 0, 0, 0, 0)]:
    t += (f'[[mixer.channel]]\nname = "{name}"\nvolume_db = {vol}\npan = {pan}\nsend_a = {a}\n'
          f"send_b = {b}\nduck = {duck}\n")
t += "[mixer.return_a]\nsync = 3.0\nfeedback = 0.25\nmix = 1.0\nlevel = 0.5\n[mixer.return_b]\nsize = 0.5\nmix = 1.0\nlevel = 0.6\n"
names = ["Kicks", "Snares + claps", "Hats", "Toms + perc", "Dice", "Mutate", "43xy",
         "SVF", "Ladder clean", "Ladder vintage", "Groove A", "Groove B"]
reps = [1] * 10 + [2, 2]
for b, n in enumerate(names):
    t += f'[[block]]\nid = {b}\nname = "{n}"\nlines = 32\nrows = """\n'
    for r in range(32):
        t += f"{r:02X} | " + " | ".join(EMPTY for _ in range(TRACKS)) + "\n"
    t += '"""\n'
t += "\n[sequence]\nsections = [" + ", ".join(f"{{ block = {b}, repeat = {r} }}" for b, r in enumerate(reps)) + "]\n"
base = f"{TMP}/base.cwt.toml"
open(base, "w").write(t)
song = f"{TMP}/generators-02312.cwt"
env = dict(os.environ, CW_F2312_IN=base, CW_F2312_OUT=song)
subprocess.run(["cargo", "test", "--release", "-q", "--", "--ignored", "features_02312_demo"],
               cwd=ROOT, env=env, check=True)
subprocess.run(["cargo", "build", "--release", "-q"], cwd=ROOT, check=True)
wav = f"{TMP}/generators-02312.wav"
subprocess.run([BIN, "--render", song, wav], check=True)
st = subprocess.run(["ffmpeg", "-i", wav, "-af", "ebur128=peak=true", "-f", "null", "-"],
                    capture_output=True, text=True).stderr
print("integrated", re.findall(r"I:\s+(-?[\d.]+) LUFS", st)[-1], "LUFS, true peak",
      re.findall(r"Peak:\s+(-?[\d.]+) dBFS", st)[-1], "dBFS")
subprocess.run(["ffmpeg", "-loglevel", "error", "-y", "-i", wav, "-codec:a", "libmp3lame", "-q:a", "2", OUT],
               check=True)
print(OUT)
