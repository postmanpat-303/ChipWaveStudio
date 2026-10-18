#!/usr/bin/env python3
"""Mix sanity check of a rendered demo (no librosa needed).

  mix_check.py full.wav [stem1.wav ...]

Full mix: integrated LUFS + true peak (ffmpeg ebur128), sample peak, clipped
samples, band balance (sub/bass/low-mid/mid/high-mid/high, dB relative to
total), low-end mono correlation (<150 Hz) and the loudest-section LUFS.
Stems (solo renders): RMS and peak in dBFS so channel levels can be compared.
"""
import re
import subprocess
import sys

import numpy as np
from scipy.io import wavfile
from scipy.signal import butter, sosfilt, resample_poly

BANDS = [("sub", 20, 60), ("bass", 60, 250), ("lowmid", 250, 500), ("mid", 500, 2000),
         ("highmid", 2000, 6000), ("high", 6000, 20000)]


def load(p):
    sr, x = wavfile.read(p)
    x = x.astype(np.float64)
    if x.dtype.kind == "i" or np.abs(x).max() > 2:
        x /= 32768.0
    if x.ndim == 1:
        x = np.stack([x, x], 1)
    return sr, x


def ebur(p):
    o = subprocess.run(["ffmpeg", "-hide_banner", "-nostats", "-i", p, "-af",
                        "ebur128=peak=true:framelog=quiet", "-f", "null", "-"],
                       capture_output=True, text=True).stderr
    i = float(re.findall(r"I:\s+(-?[\d.]+) LUFS", o)[-1])
    tp = float(re.findall(r"Peak:\s+(-?[\d.]+) dBFS", o)[-1])
    lra = float(re.findall(r"LRA:\s+(-?[\d.]+) LU", o)[-1])
    return i, tp, lra


def db(v):
    return 20 * np.log10(max(v, 1e-12))


def main():
    full = sys.argv[1]
    sr, x = load(full)
    i, tp, lra = ebur(full)
    m = x.mean(1)
    print(f"{full}: {len(x)/sr:.1f} s")
    print(f"  integrated {i:.1f} LUFS, true peak {tp:.1f} dBTP, LRA {lra:.1f} LU")
    print(f"  sample peak {db(np.abs(x).max()):.2f} dBFS, samples >= -0.1 dBFS: {(np.abs(x) >= 0.9886).sum()}")
    tot = np.mean(m ** 2)
    row = []
    for n, lo, hi in BANDS:
        sos = butter(4, [lo, min(hi, sr / 2 - 100)], "bandpass", fs=sr, output="sos")
        row.append(f"{n} {10*np.log10(np.mean(sosfilt(sos, m)**2)/tot):+.1f}")
    print("  bands (dB re total): " + ", ".join(row))
    sos = butter(4, 150, "lowpass", fs=sr, output="sos")
    lo = sosfilt(sos, x, axis=0)
    c = np.corrcoef(lo[:, 0], lo[:, 1])[0, 1]
    print(f"  low end (<150 Hz) L/R correlation {c:.3f} (1 = mono)")
    # short-term loudness proxy per 4 bars: RMS of k-weight-free mono, top section
    win = int(sr * 6.53)
    rms = [db(np.sqrt(np.mean(m[k:k+win]**2))) for k in range(0, len(m) - win, win)]
    print("  4-bar RMS dBFS: " + " ".join(f"{r:.0f}" for r in rms))
    for p in sys.argv[2:]:
        sr2, s = load(p)
        sm = s.mean(1)
        act = sm[np.abs(sm) > 1e-4]
        si, stp, _ = ebur(p)
        bands = []
        for n, lo, hi in BANDS:
            sos = butter(4, [lo, min(hi, sr / 2 - 100)], "bandpass", fs=sr, output="sos")
            bands.append(f"{n[:4]} {10*np.log10(np.mean(sosfilt(sos, sm)**2)/tot + 1e-20):+.0f}")
        print(f"  stem {p.split('/')[-1]}: {si:.1f} LUFS, peak {db(np.abs(s).max()):.1f} dBFS, TP {stp:.1f}, "
              f"RMS(active) {db(np.sqrt(np.mean(act**2))) if len(act) else -120:.1f} | " + " ".join(bands))


main()
