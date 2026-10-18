#!/usr/bin/env python3
"""0.2.3.14 Generators stage 2 demo (140 BPM, 4 rows per beat, 32-row blocks = 3.43 s):
  0:00.0  Fat Moog Bass     - legato E-minor line, overlaps glide
  0:03.4  Psy Rolling Bass  - with the kick, E then F#
  0:06.9  Warm Lead         - phrase with glide
  0:10.3  Brass             - three stabs
  0:13.7  Dark Pad          - Em(add9)
  0:17.1  Acid Squelch      - 16ths, 43 1y opens the cutoff
  0:20.6  E-Piano           - Em7, Cmaj7
  0:24.0  Metallic Bell     - four bells
  0:27.4  Psy Zap Bass      - rolling, with the kick
  0:30.9  Growl             - FM growl line
  0:34.3  Dark Pluck        - arpeggio
  0:37.7  White Riser
  0:41.1  Downlifter, Impact (0:43.1)
  0:44.6  Wind + Vinyl Crackle
  0:48.0  Groove: kick + Psy Rolling Bass + offbeat open hats, wind (x2)
  0:54.9  + clap on 2/4, Dark Pad, bell, F# turn (x2)
  1:01.7  + Acid Squelch line, White Riser
  1:05.1  Impact, pad, Dark Pluck answers (x2); ends 1:12.0 (+ tail)
Usage: scripts/make_features_02314.py [out.mp3]   (needs ffmpeg, cargo)"""
import os, re, subprocess, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = sys.argv[1] if len(sys.argv) > 1 else "/workspace/chipwave-0.2.3.14-generators-2.mp3"
TMP = "/tmp/f2314"
BIN = os.path.join(ROOT, "target/release/chipwave")
TRACKS = 8
os.makedirs(TMP, exist_ok=True)
EMPTY = "--- .. .. ...."

t = ('format = "chipwave-song-text"\nformat_version = 2\n\n[song]\ntitle = "0.2.3.14 generators 2"\n'
     f'bpm = 140\nlines_per_beat = 4\nspeed = 6\ntracks = {TRACKS}\n\n')
t += "[mixer]\nmaster_db = 0.0\nduck_key = 1\nduck_release_ms = 110\n"
for name, vol, pan, a, b, duck in [("Kick", 0, 0, 0, 0, 0), ("Hats", -7, 0.25, 0, 0.08, 0),
                                   ("Clap", -6, -0.15, 0.05, 0.2, 0), ("Bass", -2, 0, 0, 0, 0.75),
                                   ("Synth A", -4, -0.1, 0.12, 0.2, 0.3), ("Synth B", -5, 0.15, 0.12, 0.2, 0.3),
                                   ("Synth C", -5, -0.3, 0.15, 0.25, 0.3), ("FX", -4, 0.3, 0.1, 0.25, 0.2)]:
    t += (f'[[mixer.channel]]\nname = "{name}"\nvolume_db = {vol}\npan = {pan}\nsend_a = {a}\n'
          f"send_b = {b}\nduck = {duck}\n")
t += "[mixer.return_a]\nsync = 3.0\nfeedback = 0.3\nmix = 1.0\nlevel = 0.5\n[mixer.return_b]\nsize = 0.5\nmix = 1.0\nlevel = 0.6\n"
names = ["Fat Moog Bass", "Psy Rolling Bass", "Warm Lead", "Brass", "Dark Pad", "Acid Squelch",
         "E-Piano", "Metallic Bell", "Psy Zap Bass", "Growl", "Dark Pluck", "White Riser",
         "Downlifter + Impact", "Wind + Vinyl", "Groove A", "Groove B", "Groove C", "Groove D"]
reps = [1] * 14 + [2, 2, 1, 2]
for b, n in enumerate(names):
    t += f'[[block]]\nid = {b}\nname = "{n}"\nlines = 32\nrows = """\n'
    for r in range(32):
        t += f"{r:02X} | " + " | ".join(EMPTY for _ in range(TRACKS)) + "\n"
    t += '"""\n'
t += "\n[sequence]\nsections = [" + ", ".join(f"{{ block = {b}, repeat = {r} }}" for b, r in enumerate(reps)) + "]\n"
base = f"{TMP}/base.cwt.toml"
open(base, "w").write(t)
song = f"{TMP}/gen2-02314.cwt"
env = dict(os.environ, CW_F2314_IN=base, CW_F2314_OUT=song)
subprocess.run(["cargo", "test", "--release", "-q", "--", "--ignored", "features_02314_demo"],
               cwd=ROOT, env=env, check=True)
subprocess.run(["cargo", "build", "--release", "-q"], cwd=ROOT, check=True)
wav = f"{TMP}/gen2-02314.wav"
subprocess.run([BIN, "--render", song, wav], check=True)
st = subprocess.run(["ffmpeg", "-i", wav, "-af", "ebur128=peak=true", "-f", "null", "-"],
                    capture_output=True, text=True).stderr
print("integrated", re.findall(r"I:\s+(-?[\d.]+) LUFS", st)[-1], "LUFS, true peak",
      re.findall(r"Peak:\s+(-?[\d.]+) dBFS", st)[-1], "dBFS")
subprocess.run(["ffmpeg", "-loglevel", "error", "-y", "-i", wav, "-codec:a", "libmp3lame", "-q:a", "2", OUT],
               check=True)
print(OUT)
