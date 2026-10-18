#!/usr/bin/env python3
"""0.2.3.11 Voice Transform demo (145 BPM, 4 rows per beat, every block is
2 bars and plays twice = 6.6 s):
  0:00.0  Dry       - the voice phrase, Metal Demon's voice with 3F01 (blend 0)
  0:06.6  Metal Demon
  0:13.2  Digital Ghost
  0:19.9  Sub Beast
  0:26.5  Cyber Choir
  0:33.1  Ritual Growl
  0:39.7  Groove 1  - kick, rolling bass, hats, Sub Beast hits, Metal Demon stabs (40xx)
  0:46.3  Groove 2  - + Digital Ghost (41xx) and the Cyber Choir pad
  0:53.0  Groove 3  - + Ritual Growl words, the blend sweeping up with 3Fxx
  0:59.6  Groove 4  - 42FF: the Sub Beast's sub at full level; ends 1:06.2 (+ tail)
Usage: scripts/make_features_02311.py [out.mp3]   (needs ffmpeg, cargo)"""
import os, re, subprocess, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = sys.argv[1] if len(sys.argv) > 1 else "/workspace/chipwave-0.2.3.11-voice-transform.mp3"
TMP = "/tmp/f2311"
BIN = os.path.join(ROOT, "target/release/chipwave")
TRACKS = 8
os.makedirs(TMP, exist_ok=True)
EMPTY = "--- .. .. ...."

t = ('format = "chipwave-song-text"\nformat_version = 2\n\n[song]\ntitle = "0.2.3.11 voice transform"\n'
     f'bpm = 145\nlines_per_beat = 4\nspeed = 6\ntracks = {TRACKS}\n\n'
     '[[instrument]]\nslot = 1\nname = "Kick"\npatch = "builtin:trance-kick"\n'
     '[[instrument]]\nslot = 2\nname = "Hat"\npatch = "builtin:hat-open"\n')
t += "[mixer]\nmaster_db = 0.0\nduck_key = 1\nduck_release_ms = 120\n"
for name, vol, pan, a, b, duck in [("Kick", 0, 0, 0, 0, 0), ("Bass", -4, 0, 0, 0, 0.75),
                                   ("Hat", -12, 0.25, 0, 0.05, 0), ("Voice", 5, 0, 0.1, 0.15, 0.3),
                                   ("Stab", -4, -0.2, 0.2, 0.2, 0.4), ("Ghost", -7, 0.35, 0.25, 0.3, 0.4),
                                   ("Choir", -9, -0.35, 0.1, 0.4, 0.5), ("Growl", -3, 0.1, 0.15, 0.15, 0.3)]:
    t += (f'[[mixer.channel]]\nname = "{name}"\nvolume_db = {vol}\npan = {pan}\nsend_a = {a}\n'
          f"send_b = {b}\nduck = {duck}\n")
t += "[mixer.return_a]\nsync = 3.0\nfeedback = 0.3\nmix = 1.0\nlevel = 0.6\n[mixer.return_b]\nsize = 0.7\nmix = 1.0\nlevel = 0.7\n"
names = ["Dry", "Metal Demon", "Digital Ghost", "Sub Beast", "Cyber Choir", "Ritual Growl",
         "Groove 1", "Groove 2", "Groove 3", "Groove 4"]
for b, n in enumerate(names):
    t += f'[[block]]\nid = {b}\nname = "{n}"\nlines = 32\nrows = """\n'
    for r in range(32):
        t += f"{r:02X} | " + " | ".join(EMPTY for _ in range(TRACKS)) + "\n"
    t += '"""\n'
t += "\n[sequence]\nsections = [" + ", ".join(f"{{ block = {b}, repeat = 2 }}" for b in range(len(names))) + "]\n"
base = f"{TMP}/base.cwt.toml"
open(base, "w").write(t)
song = f"{TMP}/voice-transform-02311.cwt"
env = dict(os.environ, CW_F2311_IN=base, CW_F2311_OUT=song)
subprocess.run(["cargo", "test", "--release", "-q", "--", "--ignored", "features_02311_demo"],
               cwd=ROOT, env=env, check=True)
wav = f"{TMP}/voice-transform-02311.wav"
subprocess.run([BIN, "--render", song, wav], check=True)
st = subprocess.run(["ffmpeg", "-i", wav, "-af", "ebur128=peak=true", "-f", "null", "-"],
                    capture_output=True, text=True).stderr
print("integrated", re.findall(r"I:\s+(-?[\d.]+) LUFS", st)[-1], "LUFS, true peak",
      re.findall(r"Peak:\s+(-?[\d.]+) dBFS", st)[-1], "dBFS")
subprocess.run(["ffmpeg", "-loglevel", "error", "-y", "-i", wav, "-codec:a", "libmp3lame", "-q:a", "2", OUT],
               check=True)
print(OUT)
