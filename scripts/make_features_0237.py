#!/usr/bin/env python3
"""0.2.3.7 feature demo: an acid line using per-row filter commands (38/39/
3A/3B), 303 glide (3C) and accents (velocity threshold), then the same
groove through the master limiter at 0 dB and +10 dB gain (quiet vs loud,
both clean). Renders with the release binary and joins into one MP3.
Usage: scripts/make_features_0237.py [out.mp3]   (needs ffmpeg)"""
import os, re, subprocess, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = sys.argv[1] if len(sys.argv) > 1 else "/workspace/chipwave-0.2.3.7-features.mp3"
TMP = "/tmp/f237"
BIN = os.path.join(ROOT, "target/release/chipwave")
BPM = 138
os.makedirs(TMP, exist_ok=True)
EMPTY = "--- .. .. ...."


def cell(note="---", ins=None, vol=None, fx=None):
    return f"{note} {('%02X' % ins) if ins else '..'} {vol or '..'} {fx or '....'}"


def block(bid, chans, ev, lines=64):
    out = [f'[[block]]\nid = {bid}\nname = "B{bid}"\nlines = {lines}\nrows = """']
    for r in range(lines):
        out.append(f"{r:02X} | " + " | ".join(ev.get((r, c), EMPTY) for c in range(chans)))
    out.append('"""\n')
    return "\n".join(out)


src = open(os.path.join(ROOT, "demos/dark_psy.cwt.toml")).read()
chunks = src.split("[[instrument]]")[1:4]  # kick, bass, hat (slots 1-3)
DRUMS = "".join("[[instrument]]" + re.split(r"\n# ", c)[0].rstrip() + "\n" for c in chunks)
ACID = '''[[instrument]]
slot = 4
name = "Acid 303"
patch = "builtin:init"
wave = "saw"
cutoff = 300.0
resonance = 0.7
env_amount = 0.45
filter_decay = 0.2
attack = 0.002
decay = 0.3
sustain = 0.5
release = 0.04
drive = 0.45
gain = 0.6
glide_ms = 75
accent = 0.9
accent_vel = 0.9
fx = [{ kind = "Delay", sync = 3.0, feedback = 0.35, pingpong = 1.0, hp = 400.0, lp = 4000.0, mix = 0.18 }]
'''
RETURNS = """[mixer.return_a]
sync = 3.0
feedback = 0.4
mix = 1.0
level = 1.0
[mixer.return_b]
size = 0.85
mix = 1.0
level = 1.0
"""


def header(title, tracks):
    return (f'format = "chipwave-song-text"\nformat_version = 2\n\n[song]\ntitle = "{title}"\n'
            f"bpm = {BPM}\nlines_per_beat = 4\nspeed = 6\ntracks = {tracks}\n\n")


# two-bar acid phrase, A minor: (note, glide, accent)
PHRASE = [("A-2", 0, 1), None, ("A-2", 0, 0), ("A-3", 1, 0), None, ("G-2", 0, 0), ("A-2", 0, 1), None,
          ("C-3", 0, 0), None, ("A-2", 0, 0), ("D#3", 1, 1), ("E-3", 1, 0), None, ("A-2", 0, 0), ("G-2", 0, 0),
          ("A-2", 0, 1), ("A-2", 0, 0), None, ("C-3", 1, 0), ("D-3", 1, 0), None, ("A-2", 0, 1), None,
          ("E-3", 0, 0), ("E-2", 0, 1), None, ("G-2", 0, 0), ("A-2", 1, 0), None, ("C-3", 0, 1), ("B-2", 1, 0)]

# per phrase (2 bars): filter action on the first row, then on free rows
MOVES = [("3850", "3A05"),   # closed, slowly opening
         ("39D8", "3A06"),   # resonance up, keeps opening
         ("39F4", "3B07"),   # screaming resonance, closing again
         ("3880", None)]     # mid, wobble with alternating slides


def acid_events(ch, ins, phrases=4, with_moves=True):
    ev = {}
    for k in range(phrases):
        first, free = MOVES[k % 4]
        for i, st in enumerate(PHRASE):
            r = k * 32 + i
            fx = None
            if with_moves and i == 0:
                fx = first
            if st:
                note, glide, acc = st
                if glide:
                    fx = "3C01"
                elif with_moves and i != 0 and free is None:
                    fx = "3A0C" if (i // 4) % 2 == 0 else "3B0C"
                ev[(r % 64, ch, r // 64)] = cell(note, ins, "40" if acc else "30", fx)
            elif with_moves and i != 0:
                f = free if free else ("3A0C" if (i // 4) % 2 == 0 else "3B0C")
                ev[(r % 64, ch, r // 64)] = cell(fx=f)
        if with_moves and k == phrases - 1:
            ev[(63, ch, (k * 32 + 31) // 64)] = cell(fx="3800")
    return ev


def drums(ev, blk, kick=0, hat=2, bass=None):
    for r in range(64):
        if r % 4 == 0:
            ev[(r, kick, blk)] = cell("A-2", 1)
        if r % 4 == 2:
            ev[(r, hat, blk)] = cell("C-5", 3, "34", "0814")
        if bass is not None and r % 4:
            ev[(r, bass, blk)] = cell("A-2", 2, ("2C", "34", "32")[r % 4 - 1])


def song_text(title, tracks, ev, nblocks, mixer_extra, chans_mix):
    t = header(title, tracks) + DRUMS + ACID
    t += "[mixer]\nmaster_db = 0.0\nduck_key = 1\nduck_release_ms = 100\n" + mixer_extra
    for name, vol, a, b, duck in chans_mix:
        t += (f'[[mixer.channel]]\nname = "{name}"\nvolume_db = {vol}\nsend_a = {a}\n'
              f"send_b = {b}\nduck = {duck}\n")
    t += RETURNS
    for bi in range(nblocks):
        t += block(bi, tracks, {(r, c): v for (r, c, b), v in ev.items() if b == bi})
    t += "\n[sequence]\nsections = [" + ", ".join(f"{{ block = {b}, repeat = 1 }}" for b in range(nblocks)) + "]\n"
    return t


files = []
# 1) acid line: kick, hat, acid (8 bars)
ev = acid_events(2, 4)
for b in range(2):
    drums(ev, b, kick=0, hat=1)
t = song_text("0.2.3.7 acid line", 3, ev, 2, "",
              [("Kick", 0.0, 0, 0, 0), ("Hats", -9.0, 0.05, 0.1, 0), ("Acid", -2.0, 0.2, 0.12, 0.35)])
open(f"{TMP}/1-acid.cwt.toml", "w").write(t)
files.append(("acid line: per-row filter sweeps + glide + accents", f"{TMP}/1-acid.cwt.toml", 128))

# 2/3) limiter before / after: kick, bass, hat, acid (4 bars), same mix
for i, (lab, gain) in enumerate([("limiter 0 dB gain (quiet)", 0.0), ("limiter +10 dB gain (loud)", 10.0)]):
    ev = {}
    drums(ev, 0, kick=0, hat=2, bass=1)
    ev.update({k: v for k, v in acid_events(3, 4, phrases=2).items()})
    lim = f"[mixer.limiter]\nenabled = true\ngain_db = {gain}\nceiling_db = -1.0\nrelease_ms = 80.0\n"
    t = song_text(f"0.2.3.7 {lab}", 4, ev, 1, lim,
                  [("Kick", 0.0, 0, 0, 0), ("Bass", -1.0, 0, 0.02, 0.8),
                   ("Hats", -9.0, 0.05, 0.1, 0), ("Acid", -3.0, 0.2, 0.12, 0.35)])
    p = f"{TMP}/{i + 2}-limiter-{int(gain)}.cwt.toml"
    open(p, "w").write(t)
    files.append((lab, p, 64))

row = 60.0 / BPM / 4
parts = []
for name, p, lines in files:
    wav = p.replace(".cwt.toml", ".wav")
    subprocess.run([BIN, "--render", p, wav], check=True, stdout=subprocess.DEVNULL)
    keep = lines * row + 1.0
    tw = wav.replace(".wav", "-t.wav")
    subprocess.run(["ffmpeg", "-loglevel", "error", "-y", "-i", wav, "-t", f"{keep:.3f}",
                    "-af", f"afade=t=out:st={keep - 0.5:.3f}:d=0.5", "-ar", "44100", tw], check=True)
    st = subprocess.run(["ffmpeg", "-i", tw, "-af", "ebur128=peak=true", "-f", "null", "-"],
                        capture_output=True, text=True).stderr
    i_lufs = re.findall(r"I:\s+(-?[\d.]+) LUFS", st)[-1]
    tp = re.findall(r"Peak:\s+(-?[\d.]+) dBFS", st)[-1]
    print(f"{name}: {keep:.1f} s, {i_lufs} LUFS, true peak {tp} dBFS")
    parts.append(tw)
gap = f"{TMP}/gap.wav"
subprocess.run(["ffmpeg", "-loglevel", "error", "-y", "-f", "lavfi", "-i", "anullsrc=r=44100:cl=stereo",
                "-t", "0.8", gap], check=True)
with open(f"{TMP}/list.txt", "w") as f:
    for i, w in enumerate(parts):
        if i:
            f.write(f"file '{gap}'\n")
        f.write(f"file '{w}'\n")
subprocess.run(["ffmpeg", "-loglevel", "error", "-y", "-f", "concat", "-safe", "0", "-i", f"{TMP}/list.txt",
                "-codec:a", "libmp3lame", "-q:a", "2", OUT], check=True)
print(OUT)
