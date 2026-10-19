#!/usr/bin/env python3
"""Unreleased FX demo (120 BPM, 4 rows per beat, 32-row blocks = 4 s):
  0:00  Dark Pad, dry
  0:04  + Phaser (8 stages, lane fades the mix in)
  0:08  Phaser feedback rises to a jet (lane)
  0:12  Frequency shifter: +3 Hz shimmer sweeping to +240 Hz metal (lanes)
  0:16  Shifter -80 -> -500 Hz with feedback: barber pole
  0:20  Fat Moog Bass (Analog), a lane opens the ladder cutoff
  0:24  + kick, the lane closes it again
  0:28  everything: pad with phaser + shifter, cutoff wobble; ends 0:32 (+ tail)
Usage: scripts/make_fx_demo.py [out.mp3]   (needs ffmpeg, cargo)"""
import os, re, subprocess, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = sys.argv[1] if len(sys.argv) > 1 else "/workspace/ChipWave-fx-demo.mp3"
TMP = "/tmp/cw-fxdemo"
TARGET = os.environ.get("CARGO_TARGET_DIR", os.path.join(ROOT, "target"))
BIN = os.path.join(TARGET, "release/chipwave")
TRACKS = 8
os.makedirs(TMP, exist_ok=True)
EMPTY = "--- .. .. ...."

t = ('format = "chipwave-song-text"\nformat_version = 2\n\n[song]\ntitle = "ChipWave FX demo"\n'
     f'bpm = 120\nlines_per_beat = 4\nspeed = 6\ntracks = {TRACKS}\n\n')
t += "[mixer]\nmaster_db = 0.0\nduck_key = 1\nduck_release_ms = 120\n"
for name, vol, pan, a, b, duck in [("Kick", -1, 0, 0, 0, 0), ("-", -6, 0, 0, 0, 0), ("-", -6, 0, 0, 0, 0),
                                   ("Bass", -3, 0, 0, 0.05, 0.6),
                                   ("Pad 1", -6, -0.35, 0.1, 0.3, 0.3), ("Pad 2", -6, 0.35, 0.1, 0.3, 0.3),
                                   ("Pad 3", -6, -0.15, 0.1, 0.3, 0.3), ("Pad 4", -6, 0.15, 0.1, 0.3, 0.3)]:
    t += (f'[[mixer.channel]]\nname = "{name}"\nvolume_db = {vol}\npan = {pan}\nsend_a = {a}\n'
          f"send_b = {b}\nduck = {duck}\n")
t += "[mixer.return_a]\nsync = 3.0\nfeedback = 0.3\nmix = 1.0\nlevel = 0.4\n[mixer.return_b]\nsize = 0.6\nmix = 1.0\nlevel = 0.6\n"
names = ["Pad dry", "Pad + Phaser", "Phaser jet", "Freq shifter up", "Barber pole",
         "Analog bass sweep", "Bass + kick", "All together"]
for b, n in enumerate(names):
    t += f'[[block]]\nid = {b}\nname = "{n}"\nlines = 32\nrows = """\n'
    for r in range(32):
        t += f"{r:02X} | " + " | ".join(EMPTY for _ in range(TRACKS)) + "\n"
    t += '"""\n'
t += "\n[sequence]\nsections = [" + ", ".join(f"{{ block = {b}, repeat = 1 }}" for b in range(len(names))) + "]\n"
base = f"{TMP}/base.cwt.toml"
open(base, "w").write(t)
song = f"{TMP}/ChipWave-fx-demo.cwt"
env = dict(os.environ, CW_FXDEMO_IN=base, CW_FXDEMO_OUT=song)
subprocess.run(["cargo", "test", "--release", "-q", "--", "--ignored", "fx_demo_song"],
               cwd=ROOT, env=env, check=True)
subprocess.run(["cargo", "build", "--release", "-q"], cwd=ROOT, check=True)
wav = f"{TMP}/ChipWave-fx-demo.wav"
subprocess.run([BIN, "--render", song, wav], check=True)
st = subprocess.run(["ffmpeg", "-i", wav, "-af", "ebur128=peak=true", "-f", "null", "-"],
                    capture_output=True, text=True).stderr
print("integrated", re.findall(r"I:\s+(-?[\d.]+) LUFS", st)[-1], "LUFS, true peak",
      re.findall(r"Peak:\s+(-?[\d.]+) dBFS", st)[-1], "dBFS")
subprocess.run(["ffmpeg", "-loglevel", "error", "-y", "-i", wav, "-codec:a", "libmp3lame", "-q:a", "2", OUT],
               check=True)
print(OUT, song)
