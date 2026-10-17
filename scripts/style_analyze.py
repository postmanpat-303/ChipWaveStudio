#!/usr/bin/env python3
"""Objective style / production analysis of a rendered track (needs librosa).

  style_analyze.py track.wav out.json

Metrics: tempo, key/mode, duration, LUFS / peak / crest, band balance,
per-bar energy + sections (energy + novelty), kick-bass relation (sidechain
pump depth, bass 1/16 position profile), hat density, risers (centroid /
flatness rise before drops), stereo width, tail ratio, chord estimate per
bar (triad templates), lead rhythm (16-step onset profile).
Only numbers are extracted: no audio or melodies are kept.
"""
import json
import sys

import librosa
import numpy as np
from scipy.signal import lfilter

NAMES = "C C# D D# E F F# G G# A A# B".split()
BANDS = [("sub", 20, 60), ("bass", 60, 250), ("lowmid", 250, 500), ("mid", 500, 2000),
         ("highmid", 2000, 6000), ("high", 6000, 20000)]


def kweight(x, sr):
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
    a2 = [1, 2 * (K * K - 1) / a0, (1 - K / Q + K * K) / a0]
    return lfilter([1, -2, 1], a2, lfilter(b1, a1, x, axis=0), axis=0)


def lufs(x, sr):
    y = kweight(x, sr)
    blk, hop = int(0.4 * sr), int(0.1 * sr)
    if len(y) <= blk:
        return float("nan")
    ms = np.array([np.mean(np.sum(y[i:i + blk] ** 2, axis=1)) for i in range(0, len(y) - blk, hop)])
    l = -0.691 + 10 * np.log10(ms + 1e-12)
    g = ms[l > -70]
    if not len(g):
        return -70.0
    rel = -0.691 + 10 * np.log10(g.mean()) - 10
    g2 = ms[(l > -70) & (l > rel)]
    return float(-0.691 + 10 * np.log10(g2.mean()))


def db(v):
    return float(10 * np.log10(v + 1e-20))


def key_of(chroma_mean):
    # Krumhansl-Schmuckler profiles
    maj = np.array([6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88])
    mnr = np.array([6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17])
    best = None
    for k in range(12):
        for name, prof in (("major", maj), ("minor", mnr)):
            r = np.corrcoef(np.roll(prof, k), chroma_mean)[0, 1]
            if best is None or r > best[0]:
                best = (r, f"{NAMES[k]} {name}", k, name)
    return best


def chord_of(c):
    best = (-1, "N")
    for k in range(12):
        for q, iv in (("", (0, 4, 7)), ("m", (0, 3, 7))):
            t = np.zeros(12)
            t[[(k + i) % 12 for i in iv]] = 1
            r = np.dot(t, c) / (np.linalg.norm(c) * np.sqrt(3) + 1e-9)
            if r > best[0]:
                best = (r, NAMES[k] + q)
    return best[1]


def main(path, out):
    y, sr = librosa.load(path, sr=22050, mono=False)
    if y.ndim == 1:
        y = np.stack([y, y])
    st = y.T.astype(np.float64)
    mono = y.mean(0)
    # loudness on the full-rate file
    import soundfile as sf
    full, fsr = sf.read(path, always_2d=True)
    res = {"file": path.split("/")[-1], "duration_s": round(len(mono) / sr, 1)}
    pk = float(np.abs(full).max())
    rms = float(np.sqrt(np.mean(full ** 2)))
    res["peak_dbfs"] = round(20 * np.log10(pk), 2)
    res["lufs"] = round(lufs(full, fsr), 1)
    res["crest_db"] = round(20 * np.log10(pk / rms), 1)

    tempo, beats = librosa.beat.beat_track(y=mono, sr=sr, units="frames", hop_length=512)
    tempo = float(np.atleast_1d(tempo)[0])
    if tempo < 100:
        tempo *= 2
    res["bpm"] = round(tempo, 1)
    beat_s = 60.0 / tempo
    bar_s = 4 * beat_s
    # beat grid anchored on the strongest low-band onset phase
    S = np.abs(librosa.stft(mono, n_fft=2048, hop_length=256)) ** 2
    f = librosa.fft_frequencies(sr=sr, n_fft=2048)
    hop_s = 256 / sr
    low = S[(f >= 30) & (f < 120)].sum(0)
    env = np.diff(low, prepend=low[0]).clip(0)
    t = np.arange(len(low)) * hop_s
    # refine tempo + phase: phase coherence of the kick onsets (a 0.1 % tempo
    # error smears beat-folded profiles over minutes)
    envn = env / (env.max() + 1e-12)
    # coarse: autocorrelation of the kick onsets (beat 0.3..0.6 s); librosa's
    # tracker can lock to 2/3 or 3/2 of the tempo on rolling bass lines
    mid_chunk = envn[len(envn) // 4: len(envn) // 4 + int(90 / hop_s)]
    ac = np.correlate(mid_chunk - mid_chunk.mean(), mid_chunk - mid_chunk.mean(), "full")[len(mid_chunk) - 1:]
    lags = np.arange(len(ac)) * hop_s
    mm = (lags > 0.3) & (lags < 0.6)
    tempo = 60.0 / lags[mm][np.argmax(ac[mm])]
    best = (0, tempo, 0.0)
    for bpm in np.arange(tempo - 1.0, tempo + 1.0, 0.002):
        z = np.sum(envn * np.exp(-2j * np.pi * t * bpm / 60.0))
        if abs(z) > best[0]:
            best = (abs(z), bpm, z)
    tempo = float(best[1])
    res["bpm"] = round(tempo, 2)
    beat_s = 60.0 / tempo
    bar_s = 4 * beat_s
    best_ph = (-np.angle(best[2]) / (2 * np.pi) * beat_s) % beat_s
    res["grid_offset_s"] = round(best_ph, 3)

    # key / chroma
    chroma = librosa.feature.chroma_cqt(y=mono, sr=sr, hop_length=512)
    r, kname, kidx, mode = key_of(chroma.mean(1))
    res["key"] = kname
    res["key_confidence"] = round(float(r), 2)

    # band balance (whole track, relative to total)
    tot = S.sum()
    res["bands_rel_db"] = {n: round(db(S[(f >= a) & (f < b)].sum() / tot), 1) for n, a, b in BANDS}

    # per-bar metrics
    nb = int((t[-1] - best_ph) / bar_s)
    bars = []
    cfr = librosa.frames_to_time(np.arange(chroma.shape[1]), sr=sr, hop_length=512)
    flat = librosa.feature.spectral_flatness(S=np.sqrt(S))[0]
    cent = librosa.feature.spectral_centroid(S=np.sqrt(S), sr=sr)[0]
    for b in range(nb):
        a0, a1 = best_ph + b * bar_s, best_ph + (b + 1) * bar_s
        m = (t >= a0) & (t < a1)
        seg = st[int(a0 * sr):int(a1 * sr)]
        e = {"t": round(a0, 2)}
        e["rms_db"] = round(20 * np.log10(np.sqrt(np.mean(seg ** 2)) + 1e-9), 1)
        for n, lo, hi in BANDS:
            e[n] = round(db(S[(f >= lo) & (f < hi)][:, m].sum() / m.sum()), 1)
        cm = chroma[:, (cfr >= a0) & (cfr < a1)].mean(1)
        e["chord"] = chord_of(cm)
        e["centroid"] = round(float(cent[m].mean()), 0)
        e["flatness"] = round(float(flat[m].mean()), 4)
        mid_s = seg.mean(1)
        side = (seg[:, 0] - seg[:, 1]) / 2
        e["width"] = round(float(np.sqrt(np.mean(side ** 2)) / (np.sqrt(np.mean(mid_s ** 2)) + 1e-9)), 3)
        bars.append(e)
    res["bars"] = bars

    # sections: kick presence (sub+bass onset periodicity) + energy jumps
    subE = np.array([b["sub"] for b in bars])
    allE = np.array([b["rms_db"] for b in bars])
    ref_sub = np.percentile(subE, 90)
    kick = subE > ref_sub - 9
    labels = []
    for i, b in enumerate(bars):
        if allE[i] < np.percentile(allE, 90) - 18:
            lab = "quiet"
        elif not kick[i]:
            lab = "breakdown"
        elif allE[i] > np.percentile(allE, 75) - 1.5:
            lab = "drop"
        else:
            lab = "groove"
        labels.append(lab)
    # smooth to 4-bar phrases (majority)
    secs = []
    for i in range(0, nb, 4):
        ph = labels[i:i + 4]
        lab = max(set(ph), key=ph.count)
        if secs and secs[-1]["label"] == lab:
            secs[-1]["bars"] += len(ph)
        else:
            secs.append({"label": lab, "start_s": bars[i]["t"], "bars": len(ph)})
    for s in secs:
        i0 = int(round((s["start_s"] - best_ph) / bar_s))
        sl = bars[i0:i0 + s["bars"]]
        s["rms_db"] = round(float(np.mean([b["rms_db"] for b in sl])), 1)
        seg = full[int(s["start_s"] * fsr):int((s["start_s"] + s["bars"] * bar_s) * fsr)]
        s["lufs"] = round(lufs(seg, fsr), 1)
        s["chords"] = " ".join(b["chord"] for b in sl[:8])
    res["sections"] = secs
    # novelty boundaries (for reference): MFCC + chroma self-similarity
    mf = librosa.feature.mfcc(y=mono, sr=sr, hop_length=512, n_mfcc=13)
    feat = np.vstack([librosa.util.normalize(mf, axis=1), chroma])
    bound = librosa.segment.agglomerative(feat, k=max(4, min(14, nb // 12)))
    res["novelty_boundaries_s"] = [round(float(x), 1) for x in librosa.frames_to_time(bound, sr=sr, hop_length=512)]

    # beat-phase profiles in loud kick sections
    loud = [i for i, l in enumerate(labels) if l == "drop"] or [i for i, l in enumerate(labels) if l == "groove"]

    def phase_profile(band, steps, per="beat"):
        lo, hi = band
        e = S[(f >= lo) & (f < hi)].sum(0)
        period = beat_s if per == "beat" else bar_s
        prof = np.zeros(steps)
        cnt = np.zeros(steps)
        for i in loud:
            a0 = best_ph + i * bar_s
            for k in range(int(bar_s / hop_s)):
                tt = a0 + k * hop_s
                fi = int(tt / hop_s)
                if fi >= len(e):
                    break
                ph = ((tt - best_ph) % period) / period
                j = int(ph * steps) % steps
                prof[j] += e[fi]
                cnt[j] += 1
        return prof / np.maximum(cnt, 1)

    # sidechain: mid-band (pads/leads 500-4k) level over the beat
    pm = phase_profile((500, 4000), 32)
    res["pump_depth_db"] = round(db(pm.max()) - db(pm.min()), 1)
    res["pump_profile_db"] = [round(db(v) - db(pm.max()), 1) for v in pm[::2]]
    # bass envelope 80-250 Hz over the beat (16 steps): rolling vs offbeat
    pb = phase_profile((80, 250), 16)
    res["bass_beat_profile_db"] = [round(db(v) - db(pb.max()), 1) for v in pb]
    # onset profiles (16th grid over one bar)

    def onset_hist(lo, hi):
        e = S[(f >= lo) & (f < hi)].sum(0)
        o = np.diff(np.log(e + 1e-9), prepend=0).clip(0)
        h = np.zeros(16)
        for i in loud:
            a0 = best_ph + i * bar_s
            for s16 in range(16):
                tt = a0 + s16 * bar_s / 16
                fi = int(tt / hop_s)
                w = o[max(0, fi - 2):fi + 3]
                if len(w):
                    h[s16] += w.max()
        return h / max(len(loud), 1)

    hh = onset_hist(7000, 11000)
    res["hat_onset_profile"] = [round(float(v), 2) for v in hh / (hh.max() + 1e-9)]
    res["hat_hits_per_bar"] = int((hh / (hh.max() + 1e-9) > 0.35).sum())
    lead = onset_hist(1000, 5000)
    res["lead_onset_profile"] = [round(float(v), 2) for v in lead / (lead.max() + 1e-9)]
    bassh = onset_hist(60, 250)
    res["bass_onset_profile"] = [round(float(v), 2) for v in bassh / (bassh.max() + 1e-9)]
    # stereo width by band (side/mid rms) over loud sections
    Sl = np.abs(librosa.stft(y[0], n_fft=2048, hop_length=1024)) ** 2
    Sr = np.abs(librosa.stft(y[1], n_fft=2048, hop_length=1024)) ** 2
    Sm = np.abs(librosa.stft((y[0] + y[1]) / 2, n_fft=2048, hop_length=1024)) ** 2
    Ss = np.abs(librosa.stft((y[0] - y[1]) / 2, n_fft=2048, hop_length=1024)) ** 2
    res["width_side_minus_mid_db"] = {n: round(db(Ss[(f >= a) & (f < b)].sum()) - db(Sm[(f >= a) & (f < b)].sum()), 1) for n, a, b in BANDS}
    # risers: centroid/flatness slope over the 4 bars before each breakdown->drop/groove transition
    risers = []
    for i in range(4, nb):
        if labels[i] in ("drop", "groove") and labels[i - 1] == "breakdown":
            c = [bars[j]["centroid"] for j in range(i - 4, i)]
            fl = [bars[j]["flatness"] for j in range(i - 4, i)]
            risers.append({"drop_at_s": bars[i]["t"], "centroid_rise_hz": round(c[-1] - c[0]),
                           "flatness_rise": round(fl[-1] - fl[0], 4)})
    res["builds"] = risers
    # tails: energy in the second half of each beat vs first quarter, mid band, breakdown bars
    brk = [i for i, l in enumerate(labels) if l == "breakdown"]
    if brk:
        e = S[(f >= 500) & (f < 6000)].sum(0)
        on, off = [], []
        for i in brk:
            a0 = best_ph + i * bar_s
            for k in range(16):
                fi = int((a0 + k * bar_s / 16) / hop_s)
                on.append(e[fi:fi + 3].mean())
                fi2 = int((a0 + (k + 0.8) * bar_s / 16) / hop_s)
                off.append(e[fi2:fi2 + 2].mean())
        res["breakdown_tail_ratio_db"] = round(db(np.mean(off)) - db(np.mean(on)), 1)
    res["breakdown_bars"] = len(brk)
    json.dump(res, open(out, "w"), indent=1)
    print(json.dumps({k: v for k, v in res.items() if k not in ("bars",)}, indent=1)[:6000])


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
