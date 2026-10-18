#!/usr/bin/env python3
"""Voice (glitch) audio examples: one short phrase per built-in Voice preset,
then a 16-bar dark psy groove (kick / bass / hat from Hollow Ritual) with
voice glitches. Writes text songs to OUT, renders them with the release
binary (--render) and joins them into one MP3 with short gaps.
Usage: scripts/make_voice_examples.py [out.mp3]   (needs ffmpeg)"""
import os, re, subprocess, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT_MP3 = sys.argv[1] if len(sys.argv) > 1 else "/workspace/voice-glitch-examples.mp3"
TMP = "/tmp/vx"
BIN = os.path.join(ROOT, "target/release/chipwave")
BPM = 147
os.makedirs(TMP, exist_ok=True)

EMPTY = "--- .. .. ...."


def cell(note="---", ins=None, vol=None, fx=None):
    return f"{note} {('%02X' % ins) if ins else '..'} {vol or '..'} {fx or '....'}"


def block(bid, name, chans, lines, events):
    """events: {(row, ch): cell text}"""
    out = [f"[[block]]\nid = {bid}\nname = \"{name}\"\nlines = {lines}\nrows = \"\"\""]
    for r in range(lines):
        out.append(f"{r:02X} | " + " | ".join(events.get((r, c), EMPTY) for c in range(chans)))
    out.append('"""\n')
    return "\n".join(out)


RETURNS = """[mixer.return_a]
sync = 3.0
feedback = 0.45
pingpong = 1.0
hp = 300.0
lp = 4000.0
mix = 1.0
level = 1.5
[mixer.return_b]
size = 0.93
damping = 0.7
predelay = 30.0
width = 1.0
mix = 1.0
level = 1.5
"""


def header(title, tracks):
    return (f'format = "chipwave-song-text"\nformat_version = 2\n\n[song]\ntitle = "{title}"\n'
            f"bpm = {BPM}\nlines_per_beat = 4\nspeed = 6\ntracks = {tracks}\n\n")


def voice_inst(slot, preset, extra=""):
    return (f'[[instrument]]\nslot = {slot}\nname = "{preset}"\npatch = "voice:{preset}"\n'
            f'fx = [{{ kind = "Filter", mode = 1.0, cutoff = 120.0, resonance = 0.0 }}]\n{extra}\n')


# ---- one phrase per preset (2 bars = 32 lines), dry-ish with reverb + echo
PRESETS = {
    "Demon Ahh": {(0, 0): cell("E-2", 1, "38", "3000"), (12, 0): cell(fx="3036"),
                  (20, 0): cell(fx="3046"), (28, 0): cell("===")},
    "Choir Eee": {(0, 0): cell("B-3", 1, "34"), (16, 0): cell("E-4", 1, "34"), (30, 0): cell("===")},
    "Glitch Babble": {**{(r, 0): cell("E-3", 1, "38") for r in range(0, 32, 4)},
                      (20, 0): cell("G-3", 1, "38", "3332")},
    "Robot Err": {**{(r, 0): cell("E-2" if r % 8 == 0 else "F-2", 1, "38", "3050" if r % 8 == 0 else "3022")
                     for r in (0, 3, 6, 8, 11, 14, 16, 19, 22, 24, 27, 30)}},
    "Stutter Ohh": {(r, 0): cell(n, 1, "38") for r, n in ((0, "E-3"), (8, "G-3"), (16, "F-3"), (24, "E-3"))},
    "Whisper Ooh": {(0, 0): cell("E-3", 1, "3C"), (24, 0): cell("===")},
}
files = []
for i, (name, ev) in enumerate(PRESETS.items()):
    t = header(f"Voice example {name}", 1) + voice_inst(1, name)
    t += ('[mixer]\nmaster_db = 0.0\n[[mixer.channel]]\nname = "Voice"\nvolume_db = 0.0\n'
          "send_a = 0.15\nsend_b = 0.25\n" + RETURNS)
    t += block(0, name, 1, 32, ev)
    p = os.path.join(TMP, f"{i + 1:02d}-{name.replace(' ', '-')}.cwt.toml")
    open(p, "w").write(t)
    files.append((p, name, 32))

# ---- 16-bar dark psy groove with voice glitches
src = open(os.path.join(ROOT, "demos/dark_psy.cwt.toml")).read()
chunks = src.split("[[instrument]]")[1:4]  # kick, bass, hat
insts = "".join("[[instrument]]" + re.split(r"\n# ", c)[0].rstrip() + "\n" for c in chunks)
insts += voice_inst(4, "Demon Ahh", "cutoff = 3500.0")
insts += voice_inst(5, "Glitch Babble")
insts += voice_inst(6, "Robot Err")
mix = """[mixer]
master_db = 6.0
duck_key = 1
duck_release_ms = 90
[[mixer.channel]]
name = "Kick"
volume_db = 3.0
[[mixer.channel]]
name = "Bass"
volume_db = 1.5
send_a = 0.15
send_b = 0.06
duck = 0.8
[[mixer.channel]]
name = "Hats"
volume_db = -8.0
pan = 0.12
send_a = 0.06
send_b = 0.09
[[mixer.channel]]
name = "Demon"
volume_db = -3.0
pan = -0.1
send_a = 0.25
send_b = 0.45
duck = 0.5
[[mixer.channel]]
name = "Babble"
volume_db = -5.0
pan = 0.3
send_a = 0.4
send_b = 0.35
duck = 0.4
[[mixer.channel]]
name = "Robot"
volume_db = -6.0
pan = -0.35
send_a = 0.35
send_b = 0.3
duck = 0.4
""" + RETURNS
blocks = []
for b in range(4):
    ev = {}
    cut = b in (1, 3)  # kick + bass drop out on the last beat of bars 8 and 16
    for r in range(64):
        last_beat = cut and r >= 60
        if r % 4 == 0 and not last_beat:
            ev[(r, 0)] = cell("A-2", 1)
        if r % 4 and not last_beat:
            n = "F-3" if (r // 16 == 3 and b % 2 == 1) else "E-3"
            ev[(r, 1)] = cell(n, 2, ("2C", "34", "32")[r % 4 - 1])
        if r % 4 == 2:
            ev[(r, 2)] = cell("C-5", 3, "34", "0814")
    if b == 0:  # demon breath: ahh -> ohh -> ooh
        ev.update({(0, 3): cell("E-2", 4, "38", "3000"), (16, 3): cell(fx="3036"),
                   (32, 3): cell(fx="3046"), (48, 3): cell(fx="3268"), (60, 3): cell("===")})
    if b == 1:  # babble stabs on the offbeats, a stutter roll into the cut
        for r in (6, 14, 22, 30, 38, 46):
            ev[(r, 4)] = cell("G-3" if r % 16 == 6 else "E-3", 5, "36")
        ev[(54, 4)] = cell("A#3", 5, "38", "3402")
        ev[(56, 4)] = cell(fx="3243")
        ev[(60, 3)] = cell("E-2", 4, "3C", "3050")
        ev[(62, 3)] = cell(fx="3332")
    if b == 2:  # robot call and response, demon underneath
        ev.update({(0, 3): cell("E-2", 4, "34", "3040"), (32, 3): cell(fx="3014"), (56, 3): cell("===")})
        for r, n, f in ((8, "E-2", "3050"), (10, "E-2", "3022"), (24, "F-2", "3050"), (26, "E-2", "3002"),
                        (40, "E-2", "3050"), (42, "G-2", "3022"), (56, "F-2", "3050"), (58, "E-2", "3302")):
            ev[(r, 5)] = cell(n, 6, "38", f)
    if b == 3:  # everything: babble chatter, robot, demon stutter throw
        for r in range(4, 48, 8):
            ev[(r, 4)] = cell("E-3", 5, "34", "3638")
        for r, n, f in ((16, "E-2", "3050"), (18, "E-2", "3002"), (48, "F-2", "3053")):
            ev[(r, 5)] = cell(n, 6, "38", f)
        ev.update({(0, 3): cell("E-2", 4, "38", "3010"), (32, 3): cell(fx="3268"),
                   (52, 3): cell("B-1", 4, "3C", "3000"), (53, 3): cell(fx="3403"),
                   (56, 3): cell(fx="3343"), (62, 3): cell("===")})
    blocks.append(block(b, f"Groove {b + 1}", 6, 64, ev))
t = header("Voice glitch groove (dark psy)", 6) + insts + mix + "\n".join(blocks)
t += "\n[sequence]\nsections = [" + ", ".join(f"{{ block = {b}, repeat = 1 }}" for b in range(4)) + "]\n"
p = os.path.join(TMP, "07-Groove.cwt.toml")
open(p, "w").write(t)
files.append((p, "16-bar dark psy groove", 256))

# ---- render, trim, join
row = 60.0 / BPM / 4
parts = []
for p, name, lines in files:
    wav = p.replace(".cwt.toml", ".wav")
    subprocess.run([BIN, "--render", p, wav], check=True, stdout=subprocess.DEVNULL)
    keep = lines * row + 1.2  # phrase + a short tail
    trimmed = wav.replace(".wav", "-t.wav")
    subprocess.run(["ffmpeg", "-loglevel", "error", "-y", "-i", wav, "-t", f"{keep:.3f}",
                    "-af", f"afade=t=out:st={keep - 0.6:.3f}:d=0.6", trimmed], check=True)
    # level-match the parts: peak-normalise to -1.5 dBFS
    vd = subprocess.run(["ffmpeg", "-i", trimmed, "-af", "volumedetect", "-f", "null", "-"],
                        capture_output=True, text=True).stderr
    peak = float(re.search(r"max_volume: (\S+) dB", vd).group(1))
    norm = trimmed.replace("-t.wav", "-n.wav")
    subprocess.run(["ffmpeg", "-loglevel", "error", "-y", "-i", trimmed, "-af",
                    f"volume={-1.5 - peak:.2f}dB", norm], check=True)
    parts.append(norm)
    print(f"{name}: {keep:.1f} s")
gap = os.path.join(TMP, "gap.wav")
subprocess.run(["ffmpeg", "-loglevel", "error", "-y", "-f", "lavfi", "-i",
                "anullsrc=r=44100:cl=stereo", "-t", "0.7", gap], check=True)
lst = os.path.join(TMP, "list.txt")
with open(lst, "w") as f:
    for i, w in enumerate(parts):
        if i:
            f.write(f"file '{gap}'\n")
        f.write(f"file '{w}'\n")
subprocess.run(["ffmpeg", "-loglevel", "error", "-y", "-f", "concat", "-safe", "0", "-i", lst,
                "-ar", "44100", "-codec:a", "libmp3lame", "-q:a", "2", OUT_MP3], check=True)
print(OUT_MP3)
