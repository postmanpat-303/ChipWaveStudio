#!/usr/bin/env python3
"""Objective listen-check of a rendered song: peak, clipping, integrated
loudness (ITU-R BS.1770-4 / EBU R128 gating), silence gaps, per-section
levels and band balance; optional waveform + spectrogram PNG.
Usage: analyze_audio.py song.wav [out.png] [--block-secs S]"""
import sys
import numpy as np
from scipy.io import wavfile
from scipy.signal import lfilter


def kweight(x, sr):
    # BS.1770 pre-filter (high shelf) + RLB high-pass, coefficients for any rate
    import math
    f0, G, Q = 1681.974450955533, 3.999843853973347, 0.7071752369554196
    K = math.tan(math.pi * f0 / sr)
    Vh, Vb = 10 ** (G / 20), (10 ** (G / 20)) ** 0.4996667741545416
    a0 = 1 + K / Q + K * K
    b1 = [(Vh + Vb * K / Q + K * K) / a0, 2 * (K * K - Vh) / a0, (Vh - Vb * K / Q + K * K) / a0]
    a1 = [1, 2 * (K * K - 1) / a0, (1 - K / Q + K * K) / a0]
    f0, Q = 38.13547087602444, 0.5003270373238773
    K = math.tan(math.pi * f0 / sr)
    a0 = 1 + K / Q + K * K
    b2 = [1, -2, 1]
    a2 = [1, 2 * (K * K - 1) / a0, (1 - K / Q + K * K) / a0]
    return lfilter(b2, a2, lfilter(b1, a1, x, axis=0), axis=0)


def lufs(x, sr):
    y = kweight(x, sr)
    blk, hop = int(0.4 * sr), int(0.1 * sr)
    ms = np.array([np.mean(np.sum(y[i:i + blk] ** 2, axis=1)) for i in range(0, len(y) - blk, hop)])
    l = -0.691 + 10 * np.log10(ms + 1e-12)
    g = ms[l > -70]
    rel = -0.691 + 10 * np.log10(g.mean()) - 10
    g2 = ms[(l > -70) & (l > rel)]
    return -0.691 + 10 * np.log10(g2.mean())


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    bs = 64 * 60 / 140 / 4  # one 64-row block at 140 BPM
    if "--block-secs" in sys.argv:
        bs = float(sys.argv[sys.argv.index("--block-secs") + 1])
    sr, d = wavfile.read(args[0])
    x = d.astype(np.float64)
    if d.dtype == np.int16:
        x /= 32768.0
    if x.ndim == 1:
        x = np.stack([x, x], 1)
    n = len(x)
    pk = np.abs(x).max()
    print(f"length {n / sr:.1f} s, sample rate {sr}")
    print(f"peak {20 * np.log10(pk):.2f} dBFS, samples >= -0.1 dBFS: {(np.abs(x) >= 0.9886).sum()}")
    print(f"integrated loudness {lufs(x, sr):.1f} LUFS")
    # silence gaps: 200 ms windows under -50 dBFS RMS, excluding the final tail
    w = int(0.2 * sr)
    rms = np.array([np.sqrt(np.mean(x[i:i + w] ** 2)) for i in range(0, n - w, w)])
    quiet = np.where(20 * np.log10(rms + 1e-12) < -50)[0]
    music_end = (n / sr) - 3.0
    gaps = [q * 0.2 for q in quiet if q * 0.2 < music_end]
    print(f"silent 200 ms windows before the tail: {len(gaps)} {gaps[:10]}")
    # per block
    mono = x.mean(1)
    print("block  start  rms dBFS  peak dBFS  LUFS-ish  sub<60  low60-250  mid250-2k  high>2k (dB rel)")
    k = 0
    while k * bs < n / sr - 0.5:
        a, b = int(k * bs * sr), min(n, int((k + 1) * bs * sr))
        seg = x[a:b]
        m = mono[a:b]
        spec = np.abs(np.fft.rfft(m * np.hanning(len(m)))) ** 2
        f = np.fft.rfftfreq(len(m), 1 / sr)
        tot = spec.sum() + 1e-20
        bands = [spec[(f < 60)].sum(), spec[(f >= 60) & (f < 250)].sum(), spec[(f >= 250) & (f < 2000)].sum(), spec[f >= 2000].sum()]
        bdb = " ".join(f"{10 * np.log10(v / tot + 1e-12):8.1f}" for v in bands)
        r = np.sqrt(np.mean(seg ** 2))
        try:
            L = lufs(seg, sr)
        except Exception:
            L = float("nan")
        print(f"{k:4d} {k * bs:6.1f}  {20 * np.log10(r + 1e-12):8.1f}  {20 * np.log10(np.abs(seg).max() + 1e-12):9.1f}  {L:8.1f}  {bdb}")
        k += 1
    if len(args) > 1:
        import matplotlib
        matplotlib.use("Agg")
        import matplotlib.pyplot as plt
        fig, (a1, a2) = plt.subplots(2, 1, figsize=(14, 7), sharex=True,
                                     gridspec_kw={"height_ratios": [1, 2]})
        t = np.arange(n) / sr
        step = max(1, n // 20000)
        a1.plot(t[::step], x[::step, 0], lw=0.4, color="#00b8e0")
        a1.plot(t[::step], -np.abs(x[::step, 1]), lw=0.4, color="#e08a00", alpha=0.6)
        a1.axhline(10 ** (-0.5 / 20), color="r", lw=0.6, ls="--")
        a1.axhline(-10 ** (-0.5 / 20), color="r", lw=0.6, ls="--")
        a1.set_ylim(-1.05, 1.05)
        a1.set_title(f"{args[0].split('/')[-1]}: peak {20 * np.log10(pk):.1f} dBFS, {lufs(x, sr):.1f} LUFS")
        for kk in range(int(n / sr / bs) + 1):
            a1.axvline(kk * bs, color="#888", lw=0.3)
        a2.specgram(mono, NFFT=2048, Fs=sr, noverlap=1024, cmap="magma", vmin=-130)
        a2.set_yscale("symlog", linthresh=200)
        a2.set_ylim(30, sr / 2)
        a2.set_ylabel("Hz")
        a2.set_xlabel("seconds (grey lines = 4-bar blocks)")
        fig.tight_layout()
        fig.savefig(args[1], dpi=90)
        print(f"wrote {args[1]}")


main()
