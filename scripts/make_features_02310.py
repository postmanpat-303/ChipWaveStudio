#!/usr/bin/env python3
"""0.2.3.10 feature demo (120 BPM, 4 bars per block):
  1 Talk     - Voice "Talk Ka-Ta" speaks ka, ta, ka, da (formant voice + consonants)
  2 Throat   - "Throat Speech" (simulated vocal tract): ma, na, ya, sha; second
               half picks words with 3Exx; rolling bass
  3 Demon    - "Demon Words" growls gha, dum, osh, kar; 3E01 re-speaks a word
  4 Break    - a synthesised breakbeat sliced at its transients into
               instruments + notes (the sample editor's "To notes"), played as is
  5 Re-cut   - the same slices re-sequenced, Demon Words on top
Usage: scripts/make_features_02310.py [out.mp3]   (needs ffmpeg, cargo)"""
import os, re, subprocess, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = sys.argv[1] if len(sys.argv) > 1 else "/workspace/chipwave-0.2.3.10-features.mp3"
TMP = "/tmp/f2310"
BIN = os.path.join(ROOT, "target/release/chipwave")
TRACKS = 5
os.makedirs(TMP, exist_ok=True)
EMPTY = "--- .. .. ...."


def cell(note="---", ins=None, vol=None):
    return f"{note} {('%02X' % ins) if ins else '..'} {vol or '..'} ...."


t = ('format = "chipwave-song-text"\nformat_version = 2\n\n[song]\ntitle = "0.2.3.10 words + slices"\n'
     f'bpm = 120\nlines_per_beat = 4\nspeed = 6\ntracks = {TRACKS}\n\n'
     '[[instrument]]\nslot = 1\nname = "Kick"\npatch = "builtin:trance-kick"\n'
     '[[instrument]]\nslot = 2\nname = "Hat"\npatch = "builtin:hat-open"\n')
t += "[mixer]\nmaster_db = 0.0\nduck_key = 1\nduck_release_ms = 140\n"
for name, vol, pan, a, b, duck in [("Kick", 0, 0, 0, 0, 0), ("Hat", -11, 0.2, 0, 0.05, 0),
                                   ("Voice", -2, 0, 0.12, 0.18, 0.2), ("Bass", -3, 0, 0, 0, 0.7),
                                   ("Break", 0, 0, 0, 0.08, 0)]:
    t += (f'[[mixer.channel]]\nname = "{name}"\nvolume_db = {vol}\npan = {pan}\nsend_a = {a}\n'
          f"send_b = {b}\nduck = {duck}\n")
t += "[mixer.return_a]\nsync = 3.0\nfeedback = 0.25\nmix = 1.0\nlevel = 0.6\n[mixer.return_b]\nsize = 0.6\nmix = 1.0\nlevel = 0.7\n"
names = ["Talk", "Throat", "Demon", "Break", "Re-cut"]
for b in range(5):
    t += f'[[block]]\nid = {b}\nname = "{names[b]}"\nlines = 64\nrows = """\n'
    for r in range(64):
        ev = {}
        if b < 3 and r % 4 == 0:
            ev[0] = cell("A-2", 1)
        if b < 3 and r % 4 == 2:
            ev[1] = cell("C-5", 2, "30")
        t += f"{r:02X} | " + " | ".join(ev.get(c, EMPTY) for c in range(TRACKS)) + "\n"
    t += '"""\n'
t += "\n[sequence]\nsections = [" + ", ".join(f"{{ block = {b}, repeat = 1 }}" for b in range(5)) + "]\n"
base = f"{TMP}/base.cwt.toml"
open(base, "w").write(t)
song = f"{TMP}/words-slices-02310.cwt"
env = dict(os.environ, CW_F2310_IN=base, CW_F2310_OUT=song)
subprocess.run(["cargo", "test", "--release", "-q", "--", "--ignored", "--nocapture", "features_02310_demo"],
               cwd=ROOT, env=env, check=True)
wav = f"{TMP}/words-slices-02310.wav"
subprocess.run([BIN, "--render", song, wav], check=True)
st = subprocess.run(["ffmpeg", "-i", wav, "-af", "ebur128=peak=true", "-f", "null", "-"],
                    capture_output=True, text=True).stderr
print("integrated", re.findall(r"I:\s+(-?[\d.]+) LUFS", st)[-1], "LUFS, true peak",
      re.findall(r"Peak:\s+(-?[\d.]+) dBFS", st)[-1], "dBFS")
subprocess.run(["ffmpeg", "-loglevel", "error", "-y", "-i", wav, "-codec:a", "libmp3lame", "-q:a", "2", OUT],
               check=True)
print(OUT)
