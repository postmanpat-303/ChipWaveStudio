#!/usr/bin/env python3
"""Plot per-16th hi-hat level / timing / decay of several hat_analyze.py results.
  hat_compare.py out.png label1=a.json label2=b.json ..."""
import json
import sys

import matplotlib
import numpy as np

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402

out = sys.argv[1]
sets = [(a.split("=")[0], json.load(open(a.split("=", 1)[1]))) for a in sys.argv[2:]]
cols = ["#888888", "#b07030", "#d04040", "#00a8d8"]
fig, ax = plt.subplots(4, 1, figsize=(15, 13))
x = np.arange(16)
w = 0.8 / len(sets)


def val(p, k, hit_only=False):
    v = p.get(k)
    if v is None or (hit_only and p.get("hit_rate", 0) < 0.5):
        return np.nan
    return v


for i, (nm, r) in enumerate(sets):
    p = r["pattern_drop"]
    o = (i - (len(sets) - 1) / 2) * w
    ax[0].bar(x + o, [val(q, "level_db") for q in p], w, color=cols[i % 4], label=nm, bottom=0)
    ax[1].bar(x + o, [val(q, "timing_ms", True) for q in p], w, color=cols[i % 4], label=nm)
    ax[2].bar(x + o, [val(q, "decay_ms", True) for q in p], w, color=cols[i % 4], label=nm)
labels = ["1", "e", "+", "a"] * 4
for a, t, yl in zip(ax[:3], ["hat level per 16th in the drop (dB, >6 kHz percussive, 0 = loudest)",
                             "onset vs the kick grid (ms; hits only; the analyser reads an exactly on-grid render as about -2 ms)",
                             "decay to -20 dB (ms; hits only)"], ["dB", "ms", "ms"]):
    a.set_xticks(x, [f"{k // 4 + 1}.{labels[k]}" for k in x])
    a.set_title(t)
    a.set_ylabel(yl)
    a.axhline(0, color="k", lw=0.5)
    a.legend(ncol=len(sets), fontsize=8)
ax[0].set_ylim(-30, 3)
# timbre / space summary
keys = [("hat_centroid_hz", "centroid kHz", 1e-3), ("hat_slope_db_per_oct", "slope dB/oct", 1),
        ("harsh_3to5k_vs_6to16k_db", "3-5k vs 6-16k dB", 1), ("hat_side_minus_mid_db", "side-mid dB", 1),
        ("hat_vs_kick_db", "hat vs kick dB", 1), ("hf_floor_vs_peak_db", "HF floor vs peak dB", 1)]
xx = np.arange(len(keys))
for i, (nm, r) in enumerate(sets):
    o = (i - (len(sets) - 1) / 2) * w
    vals = [r.get(k, np.nan) * s for k, _, s in keys]
    ax[3].bar(xx + o, vals, w, color=cols[i % 4], label=nm)
    for j, v in enumerate(vals):
        ax[3].text(xx[j] + o, v, f"{v:.1f}", ha="center", va="bottom" if v >= 0 else "top", fontsize=7)
ax[3].set_xticks(xx, [n for _, n, _ in keys])
ax[3].axhline(0, color="k", lw=0.5)
ax[3].set_title("timbre, width, level and space of the hat hits")
ax[3].legend(ncol=len(sets), fontsize=8)
fig.tight_layout()
fig.savefig(out, dpi=75)
print("wrote", out)
