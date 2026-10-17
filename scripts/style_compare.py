#!/usr/bin/env python3
"""Side-by-side table of two style_analyze.py results, plus an optional PNG
(energy over normalised time + band balance). Usage:
  style_compare.py ref.json demo.json [out.png [ref.wav demo.wav]]
With the two audio files the PNG also gets a long-term average spectrum."""
import json
import sys

import numpy as np


def load(p):
    return json.load(open(p))


def section_lufs(r, labels):
    v = [s["lufs"] for s in r["sections"] if s["label"] in labels]
    return round(float(np.mean(v)), 1) if v else float("nan")


def main():
    a, b = load(sys.argv[1]), load(sys.argv[2])
    rows = [
        ("duration s", a["duration_s"], b["duration_s"]),
        ("BPM", a["bpm"], b["bpm"]),
        ("key", a["key"], b["key"]),
        ("LUFS integrated", a["lufs"], b["lufs"]),
        ("peak dBFS", a["peak_dbfs"], b["peak_dbfs"]),
        ("crest dB", a["crest_db"], b["crest_db"]),
        ("drop LUFS", section_lufs(a, ["drop"]), section_lufs(b, ["drop"])),
        ("breakdown LUFS", section_lufs(a, ["breakdown"]), section_lufs(b, ["breakdown"])),
        ("pump depth dB (mid band)", a["pump_depth_db"], b["pump_depth_db"]),
        ("hat hits / bar", a["hat_hits_per_bar"], b["hat_hits_per_bar"]),
        ("breakdown share %", round(100 * a["breakdown_bars"] / len(a["bars"])), round(100 * b["breakdown_bars"] / len(b["bars"]))),
        ("breakdown tail ratio dB", a.get("breakdown_tail_ratio_db"), b.get("breakdown_tail_ratio_db")),
    ]
    for k in a["bands_rel_db"]:
        rows.append((f"band {k} dB rel", a["bands_rel_db"][k], b["bands_rel_db"][k]))
    for k in a["width_side_minus_mid_db"]:
        rows.append((f"width {k} side-mid dB", a["width_side_minus_mid_db"][k], b["width_side_minus_mid_db"][k]))
    ra = [x["centroid_rise_hz"] for x in a["builds"] if x["centroid_rise_hz"] > 0]
    rb = [x["centroid_rise_hz"] for x in b["builds"] if x["centroid_rise_hz"] > 0]
    rows.append(("build centroid rise Hz", round(np.mean(ra)) if ra else 0, round(np.mean(rb)) if rb else 0))
    print(f"| metric | reference | demo | diff |\n|---|---|---|---|")
    for k, x, y in rows:
        d = ""
        if isinstance(x, (int, float)) and isinstance(y, (int, float)):
            d = f"{y - x:+.1f}"
        print(f"| {k} | {x} | {y} | {d} |")
    for k in ("bass_onset_profile", "hat_onset_profile", "lead_onset_profile"):
        fmt = lambda v: " ".join("#" if t > 0.5 else ("+" if t > 0.25 else ".") for t in v)  # noqa: E731
        print(f"\n{k} (16 steps, # strong, + medium)\n  ref : {fmt(a[k])}\n  demo: {fmt(b[k])}")
    print("\npump profile over a beat (dB, 16 steps)\n  ref : " + " ".join(f"{v:5.1f}" for v in a["pump_profile_db"]) +
          "\n  demo: " + " ".join(f"{v:5.1f}" for v in b["pump_profile_db"]))
    print("\nsections ref : " + ", ".join(f"{s['label']} {s['bars']}b {s['lufs']}" for s in a["sections"]))
    print("sections demo: " + ", ".join(f"{s['label']} {s['bars']}b {s['lufs']}" for s in b["sections"]))
    if len(sys.argv) > 3:
        import matplotlib
        matplotlib.use("Agg")
        import matplotlib.pyplot as plt
        audio = sys.argv[4:6] if len(sys.argv) > 5 else None
        n = 4 if audio else 3
        fig, ax = plt.subplots(1, n, figsize=(5 * n + 1, 4.5), gridspec_kw={"width_ratios": [2.2] + [1] * (n - 1)})
        for r, c, n in ((a, "#888", "reference"), (b, "#00a8d8", "demo")):
            e = np.array([x["rms_db"] for x in r["bars"]])
            t = np.linspace(0, 100, len(e))
            ax[0].plot(t, e, color=c, label=f"{n} ({r['duration_s']:.0f} s)", lw=1.6)
        ax[0].set_xlabel("position in track (%)")
        ax[0].set_ylabel("RMS per bar (dB)")
        ax[0].set_title("energy curve")
        ax[0].legend()
        ks = list(a["bands_rel_db"])
        x = np.arange(len(ks))
        ax[1].bar(x - 0.2, [a["bands_rel_db"][k] for k in ks], 0.4, color="#888", label="reference")
        ax[1].bar(x + 0.2, [b["bands_rel_db"][k] for k in ks], 0.4, color="#00a8d8", label="demo")
        ax[1].set_xticks(x, ks, rotation=30)
        ax[1].set_title("band balance (dB rel. total)")
        ax[1].legend()
        st = np.arange(16)
        ax[2].plot(st, a["pump_profile_db"], color="#888", marker="o", label="reference")
        ax[2].plot(st, b["pump_profile_db"], color="#00a8d8", marker="o", label="demo")
        ax[2].set_title("sidechain pump over one beat (mid band)")
        ax[2].set_xlabel("1/16 of a beat")
        ax[2].legend()
        if audio:
            import soundfile as sf
            for path, c, nm in ((audio[0], "#888", "reference"), (audio[1], "#00a8d8", "demo")):
                x, sr = sf.read(path, always_2d=True)
                m = x.mean(1)
                nfft = 16384
                frames = np.lib.stride_tricks.sliding_window_view(m, nfft)[:: nfft * 4]
                ps = (np.abs(np.fft.rfft(frames * np.hanning(nfft), axis=1)) ** 2).mean(0)
                f = np.fft.rfftfreq(nfft, 1 / sr)
                # 1/3-octave smoothing, normalised to total power
                cs = np.geomspace(35, 16000, 60)
                v = [ps[(f >= c / 2 ** (1 / 6)) & (f < c * 2 ** (1 / 6))].mean() for c in cs]
                v = 10 * np.log10(np.array(v) / ps.sum())
                ax[3].semilogx(cs, v, color=c, label=nm, lw=1.6)
            ax[3].set_title("long-term average spectrum (rel.)")
            ax[3].set_xlabel("Hz")
            ax[3].legend()
        fig.suptitle(f"reference: {a['lufs']} LUFS, {a['bpm']} BPM, {a['key']}   |   demo: {b['lufs']} LUFS, peak {b['peak_dbfs']} dBFS, {b['bpm']} BPM, {b['key']}")
        fig.tight_layout()
        fig.savefig(sys.argv[3], dpi=80)
        print(f"wrote {sys.argv[3]}")


main()
