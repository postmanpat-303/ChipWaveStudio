#!/usr/bin/env python3
"""How audible are the reverb / echo returns? Renders a text song twice with ChipWave (as is, and
"dry": every channel send 0, send automation removed, return levels 0) and compares.

  wet_check.py song.cwt.toml [--chipwave target/release/chipwave] [--holes LINE:LEN ...]

Prints the overall wet/dry ratio (wet = full - dry), the ratio per 4 bars, and the wet level in the
given holes (song positions in lines where the dry mix is meant to be silent, e.g. the gap before
a drop), so "is the reverb there?" becomes a number.
"""
import argparse
import os
import re
import subprocess
import tempfile

import numpy as np
from scipy.io import wavfile


def render(cw, toml_text, out):
    with tempfile.NamedTemporaryFile("w", suffix=".cwt.toml", delete=False) as f:
        f.write(toml_text)
    r = subprocess.run([cw, "--render", f.name, out], capture_output=True, text=True)
    if r.returncode:
        raise SystemExit(f"render failed: {r.stderr.strip()}")
    os.unlink(f.name)
    sr, x = wavfile.read(out)
    return sr, x.astype(np.float64) / 32768.0


def dry_version(t):
    t = re.sub(r"^(send_[ab]) = [\d.]+$", r"\1 = 0.0", t, flags=re.M)
    t = re.sub(r'\[\[automation\]\]\ntarget = "mixer\.ch\.\d+\.send_[ab]".*?\n\]\n', "", t, flags=re.S)
    t = re.sub(r"(\[mixer\.return_[ab]\][^\[]*?)level = [\d.]+", r"\1level = 0.0", t, flags=re.S)
    return t


def db(x):
    return 10 * np.log10(np.mean(x ** 2) + 1e-20)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("song")
    ap.add_argument("--chipwave", default="target/release/chipwave")
    ap.add_argument("--holes", nargs="*", default=[])
    a = ap.parse_args()
    text = open(a.song).read()
    bpm = float(re.search(r"^bpm = ([\d.]+)", text, re.M).group(1))
    line = 60.0 / (bpm * 4)
    tmp = tempfile.mkdtemp()
    sr, full = render(a.chipwave, text, f"{tmp}/full.wav")
    _, dry = render(a.chipwave, dry_version(text), f"{tmp}/dry.wav")
    n = min(len(full), len(dry))
    full, dry = full[:n], dry[:n]
    wet = full - dry
    print(f"{a.song}: wet/dry {db(wet) - db(dry):+.1f} dB overall (wet RMS {db(wet):.1f} dBFS)")
    win = int(64 * line * sr)
    row = [f"{db(wet[k:k+win]) - db(dry[k:k+win]):+.0f}" for k in range(0, n - win, win)]
    print("  wet/dry per 4 bars (dB): " + " ".join(row))
    for h in a.holes:
        p, ln = (float(v) for v in h.split(":"))
        s, e = int(p * line * sr), int((p + ln) * line * sr)
        print(f"  hole at line {p:.0f} (+{ln:.0f}): dry {db(dry[s:e]):.1f} dBFS, wet {db(wet[s:e]):.1f} dBFS")
    from scipy.signal import butter, sosfilt
    lo = butter(4, 150, "lowpass", fs=sr, output="sos")
    print(f"  below 150 Hz: wet {db(sosfilt(lo, wet.mean(1))):.1f} dBFS vs dry {db(sosfilt(lo, dry.mean(1))):.1f} dBFS"
          " (low end stays dry)")
    tail = int(2.0 * sr)
    print(f"  export tail (last 2 s): dry {db(dry[-tail:]):.1f} dBFS, wet {db(wet[-tail:]):.1f} dBFS")


main()
