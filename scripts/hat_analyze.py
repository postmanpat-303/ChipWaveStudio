#!/usr/bin/env python3
"""Hi-hat / top-end percussion analysis of a dance track (style reference tool).

  hat_analyze.py track.(wav|mp3) out.json [--bpm 140] [--grid global|local_phase|kick_transient]

Isolates the hats with HPSS (percussive part) + a >6 kHz band, detects onsets
on that band and folds them on a 16th grid anchored on the kick onsets. Reports,
per 16th of a bar: hit rate, level, microtiming vs the local kick grid (ms),
decay (ms to -20 dB); plus brightness, noise colour, 3-5 kHz harshness,
stereo width/pan, level vs kick, top-end sidechain dip, reverb floor and how
the pattern changes between sections (kick vs no-kick bars).
"""
import json
import sys

import librosa
import numpy as np

SR = 44100
FORCE_GRID = None  # --grid global|local_phase|kick_transient
HOP = 128  # 2.9 ms (onset timing uses sample-accurate envelopes)


def db(x):
    return 10 * np.log10(np.maximum(x, 1e-12))


def grid(low_env, t, bpm_hint=None):
    """tempo + phase from the kick onset envelope (autocorrelation + phase coherence)"""
    hop_s = t[1] - t[0]
    e = low_env / (low_env.max() + 1e-12)
    if bpm_hint:
        tempo = bpm_hint
    else:
        n = len(e)
        ch = e[n // 4: n // 4 + int(60 / hop_s)]
        ac = np.correlate(ch - ch.mean(), ch - ch.mean(), "full")[len(ch) - 1:]
        lags = np.arange(len(ac)) * hop_s
        m = (lags > 0.3) & (lags < 0.6)
        tempo = 60.0 / lags[m][np.argmax(ac[m])]
    best = (0, tempo, 0j)
    for bpm in np.arange(tempo - 1.0, tempo + 1.0, 0.002):
        z = np.sum(e * np.exp(-2j * np.pi * t * bpm / 60.0))
        if abs(z) > best[0]:
            best = (abs(z), bpm, z)
    bpm = float(best[1])
    beat = 60.0 / bpm
    ph = (-np.angle(best[2]) / (2 * np.pi) * beat) % beat
    return bpm, beat, ph


def main(path, out, bpm_hint=None):
    y, _ = librosa.load(path, sr=SR, mono=False)
    if y.ndim == 1:
        y = np.stack([y, y])
    L, R = y[0], y[1]
    mono = 0.5 * (L + R)
    n_fft = 1024
    stft = lambda x: librosa.stft(x, n_fft=n_fft, hop_length=HOP)  # noqa: E731
    SM = stft(mono)
    f = librosa.fft_frequencies(sr=SR, n_fft=n_fft)
    t = librosa.frames_to_time(np.arange(SM.shape[1]), sr=SR, hop_length=HOP)
    P = np.abs(SM) ** 2
    # kick: 30-120 Hz energy, onset = steepest rise
    low = P[(f >= 30) & (f < 120)].sum(0)
    low_s = np.convolve(low, np.ones(5) / 5, "same")
    low_on = np.diff(low_s, prepend=low_s[0]).clip(0)
    bpm, beat, ph = grid(low_on, t, bpm_hint)
    six = beat / 4
    # HPSS: percussive part, then bands
    H, Pc = librosa.decompose.hpss(SM, margin=1.0)
    PP = np.abs(Pc) ** 2
    hi = (f >= 6000) & (f < 16000)
    HH = np.abs(H[hi]) ** 2
    del H
    env = PP[hi].sum(0)  # hat band, percussive
    env_all = P[hi].sum(0)  # hat band, everything (for sidechain / reverb floor)
    harm_hi = HH.sum(0)
    del HH
    # stereo hat band (percussive mask applied to L/R)
    mask = np.abs(Pc) / (np.abs(SM) + 1e-9)
    mh = mask[hi].astype(np.float32)
    del mask
    SL = stft(L)[hi] * mh
    SR_ = stft(R)[hi] * mh
    del mh
    eL = (np.abs(SL) ** 2).sum(0)
    eR = (np.abs(SR_) ** 2).sum(0)
    eS = (np.abs(SL - SR_) ** 2).sum(0) / 4
    eM = (np.abs(SL + SR_) ** 2).sum(0) / 4
    del SL, SR_
    fps = 1.0 / (t[1] - t[0])
    # sample-accurate envelopes for onset timing (threshold crossing at -6 dB of the local peak)
    from scipy.signal import butter, sosfiltfilt
    perc = librosa.istft(Pc, hop_length=HOP, length=len(mono))
    hp = sosfiltfilt(butter(6, 6000, "hp", fs=SR, output="sos"), perc)
    win = int(SR * 0.001)
    henv = np.sqrt(np.convolve(hp ** 2, np.ones(win) / win, "same"))

    def cross(e, t0, t1, frac=0.5):
        a, b = int(max(0, t0) * SR), int(t1 * SR)
        seg = e[a:b]
        if len(seg) < 4:
            return None, 0.0, 0.0
        i = int(np.argmax(seg))
        pk = seg[i]
        pre = seg[: i + 1]
        floor = pre.min()
        j = np.where(pre < floor + frac * (pk - floor))[0]
        k = (j[-1] + 1) if len(j) else 0
        return (a + k) / SR, pk, floor
    nbeats = int((t[-1] - ph) / beat) - 1
    # Grid: tempo/phase fit of the kick onsets, refined locally (below). A
    # per-beat "first transient" search is unreliable because the off-16th bass
    # also has sub energy, and the phase fit itself is pulled by the bass, so
    # absolute timing vs the kick is only good to ~+-15 ms; compare against a
    # render that is exactly on the grid (the 0.2.3.2 demo measures -3 ms).
    # local phase (tempo drift / non-exact BPM): phase coherence of the same kick
    # onset envelope over +-8 beats around each beat, unwrapped near the global grid
    e_n = low_on / (low_on.max() + 1e-12)
    gb = ph + np.arange(nbeats) * beat
    for b in range(nbeats):
        m = (t >= gb[b] - 8 * beat) & (t < gb[b] + 8 * beat)
        z = np.sum(e_n[m] * np.exp(-2j * np.pi * t[m] / beat))
        lp = (-np.angle(z) / (2 * np.pi) * beat) % beat
        d = (lp - gb[b]) % beat
        if d > beat / 2:
            d -= beat
        gb[b] += d
    lpk = sosfiltfilt(butter(4, 90, "lp", fs=SR, output="sos"), mono)
    kenv = np.sqrt(np.convolve(lpk ** 2, np.ones(win * 3) / (win * 3), "same"))
    kick_lvl = np.zeros(nbeats)
    for b in range(nbeats):
        g = gb[b]
        a0, a1 = int(g * SR), int((g + 0.06) * SR)
        kick_lvl[b] = kenv[a0:a1].max() if a1 <= len(kenv) and a1 > a0 else 0.0
    kthr = np.percentile(kick_lvl, 90) * 0.25
    has_kick = kick_lvl > kthr
    # choose the grid on which the hat onsets are most consistent: global fit,
    # local phase fit, or per-beat kick transient (LP 150 Hz, 30 % crossing,
    # smoothed over +-8 beats). Different mixes fool different estimators.
    lp150 = sosfiltfilt(butter(4, 150, "lp", fs=SR, output="sos"), mono)
    k150 = np.sqrt(np.convolve(lp150 ** 2, np.ones(win * 3) / (win * 3), "same"))
    gA = ph + np.arange(nbeats) * beat
    devc = np.zeros(nbeats)
    for b in range(nbeats):
        kt, _, _ = cross(k150, gA[b] - 0.045, gA[b] + 0.045, 0.3)
        devc[b] = (kt - gA[b]) if (kt is not None and has_kick[b]) else np.nan
    sm = np.array([np.nanmedian(devc[max(0, i - 8): i + 8]) if np.isfinite(devc[max(0, i - 8): i + 8]).any() else 0.0
                   for i in range(nbeats)])
    gC = gA + np.nan_to_num(sm)
    cands = {"global": gA, "local_phase": gb.copy(), "kick_transient": gC}

    def consistency(g):
        sds = []
        for k in range(4):
            dts = []
            for b in range(0, nbeats, 2):
                if not has_kick[b]:
                    continue
                c = g[b] + k * six
                on_t, hpk, hfl = cross(henv, c - 0.035, c + 0.045, 0.5)
                if on_t is not None and hpk > 6 * (hfl + 1e-12):
                    dts.append((on_t - c) * 1000)
            if len(dts) > 20:
                sds.append((np.std(dts), len(dts)))
        if not sds:
            return 1e9
        # the two most regular positions of the beat (the main hats)
        sds.sort()
        return float(np.mean([x[0] for x in sds[:2]]))

    scores = {k: consistency(v) for k, v in cands.items()}
    grid_used = FORCE_GRID or min(scores, key=scores.get)
    gb = cands[grid_used]
    # hats per 16th
    ehi_s = env
    peak_all = np.percentile(ehi_s, 99.5)
    rows = []
    for b in range(nbeats):
        for k in range(4):
            g = gb[b] + k * six
            m = (t >= g - 0.035) & (t < g + 0.045)
            if m.sum() < 3:
                continue
            seg = ehi_s[m]
            i = int(np.argmax(seg))
            pk = seg[i]
            gi = np.where(m)[0][i]
            on_t, hpk, hfl = cross(henv, g - 0.035, g + 0.045, 0.5)
            if on_t is None:
                continue
            rise = 20 * np.log10((hpk + 1e-9) / (hfl + 1e-9))
            # decay: frames until -20 dB below peak (capped at next-next 16th)
            lim = min(len(ehi_s), gi + int(2 * six * fps))
            after = ehi_s[gi:lim]
            below = np.where(after < pk * 0.01)[0]
            dec = (below[0] if len(below) else len(after)) / fps * 1000
            w = slice(max(0, gi - 2), gi + 6)
            rows.append(dict(beat=b, pos=k, kick=bool(has_kick[b]), lvl=float(db(pk / peak_all)), rise=float(rise),
                             dt=float((on_t - g) * 1000), dec=float(dec),
                             L=float(eL[w].sum()), R=float(eR[w].sum()), S=float(eS[w].sum()),
                             M=float(eM[w].sum()), frame=int(gi)))
    lv = np.array([r["lvl"] for r in rows])
    # hit = a real onset: >= 8 dB rise above the floor just before it, and not buried
    hit = np.array([r["rise"] >= 8.0 for r in rows]) & (lv > -30)
    # downbeat: choose the bar rotation whose beats 2/4 carry the clap/snare (1-4 kHz percussive)
    snare = PP[(f >= 1000) & (f < 4000)].sum(0)
    sc = np.zeros(4)
    for b in range(nbeats):
        i = np.argmin(np.abs(t - gb[b]))
        sc[b % 4] += snare[i: i + 6].sum()
    rot = int(np.argmax(sc[0::2] + sc[1::2] * 0)) if False else None
    # beats with the strongest snare are 2 and 4 -> they are (rot+1)%4 and (rot+3)%4
    pair = [sc[0] + sc[2], sc[1] + sc[3]]
    first_beat = 1 if pair[0] > pair[1] else 0  # beat index (mod 4) that is beat 1
    first_beat = (first_beat) % 4

    def fold(sel):
        res = []
        for p in range(16):
            rs = [r for r, h, s in zip(rows, hit, sel) if s and ((r["beat"] - first_beat) % 4) * 4 + r["pos"] == p]
            hs = [r for r, h in zip(rows, hit) if h] and [r for r, h, s in zip(rows, hit, sel) if s and h and ((r["beat"] - first_beat) % 4) * 4 + r["pos"] == p]
            if not rs:
                res.append(dict(pos=p, n=0))
                continue
            res.append(dict(
                pos=p, n=len(rs), hit_rate=round(len(hs) / len(rs), 2),
                level_db=round(float(np.median([r["lvl"] for r in rs])), 1),
                timing_ms=round(float(np.median([r["dt"] for r in hs])), 1) if hs else None,
                timing_sd_ms=round(float(np.std([r["dt"] for r in hs])), 1) if len(hs) > 2 else None,
                decay_ms=round(float(np.median([r["dec"] for r in hs])), 0) if hs else None,
            ))
        return res

    kick_sel = [r["kick"] for r in rows]
    nokick_sel = [not r["kick"] for r in rows]
    # the "main" hat section: kick bars with strong top end (drops)
    bar_hi = {}
    for r in rows:
        bar_hi.setdefault((r["beat"] - first_beat) // 4, []).append(r["lvl"])
    bar_lv = {k: np.mean(v) for k, v in bar_hi.items()}
    thr = np.percentile(list(bar_lv.values()), 60)
    drop_sel = [r["kick"] and bar_lv[(r["beat"] - first_beat) // 4] >= thr for r in rows]
    groove_sel = [r["kick"] and bar_lv[(r["beat"] - first_beat) // 4] < thr for r in rows]
    res = dict(file=path.split("/")[-1], bpm=round(bpm, 2), sixteenth_ms=round(six * 1000, 1))
    res["pattern_drop"] = fold(drop_sel)
    res["pattern_groove"] = fold(groove_sel)
    res["pattern_nokick"] = fold(nokick_sel)
    res["kick_bars"] = int(sum(has_kick) / 4)
    res["nokick_bars"] = int(sum(~has_kick) / 4)
    # hit-level features (drop hits only)
    hits = [r for r, h, s in zip(rows, hit, drop_sel) if h and s]
    fr = np.array([r["frame"] for r in hits])
    spec = PP[:, fr].mean(1) if len(fr) else PP.mean(1)
    band = (f >= 2000) & (f < 16000)
    res["hat_centroid_hz"] = round(float((f[band] * spec[band]).sum() / spec[band].sum()), 0)
    # noise colour: slope of the hit spectrum 2-16 kHz in dB/octave
    oc = np.log2(f[band] / 1000)
    res["hat_slope_db_per_oct"] = round(float(np.polyfit(oc, db(spec[band]), 1)[0]), 1)
    res["harsh_3to5k_vs_6to16k_db"] = round(float(db(spec[(f >= 3000) & (f < 5000)].sum() / spec[hi].sum())), 1)
    res["hat_spectrum_db"] = {str(c): round(float(db(spec[(f >= c / 1.19) & (f < c * 1.19)].mean() / spec[hi].mean())), 1)
                              for c in [1000, 2000, 3000, 4000, 5000, 6300, 8000, 10000, 12500, 16000]}
    if hits:
        Ls, Rs, Ss, Ms = (sum(r[k] for r in hits) for k in "LRSM")
        res["hat_side_minus_mid_db"] = round(float(db(Ss / Ms)), 1)
        res["hat_pan_L_minus_R_db"] = round(float(db(Ls / Rs)), 1)
        # level vs kick: hat band peak energy vs kick band peak energy (same STFT)
        # hat peak (>6 kHz envelope) vs kick peak (<90 Hz envelope), amplitude dB
        hp_pk = np.median([henv[int(gb[r["beat"]] * SR + (r["pos"] * six + r["dt"] / 1000) * SR):][:int(0.02 * SR)].max() for r in hits])
        res["hat_vs_kick_db"] = round(float(20 * np.log10(hp_pk / np.median(kick_lvl[has_kick]))), 1)
    # top-end sidechain: everything >6 kHz folded over the beat (drop beats), dB vs max
    prof = np.zeros(32)
    cnt = 0
    for b in range(nbeats):
        if not has_kick[b] or bar_lv.get((b - first_beat) // 4, -99) < thr:
            continue
        i0 = np.argmin(np.abs(t - gb[b]))
        idx = i0 + (np.arange(32) * (beat / 32) * fps).astype(int)
        if idx[-1] < len(harm_hi):
            prof += harm_hi[idx]
            cnt += 1
    if cnt:
        prof = db(prof / prof.max())
        res["hf_sustained_beat_profile_db"] = [round(float(v), 1) for v in prof]
        res["hf_sustained_dip_db"] = round(float(-prof[:8].min()), 1)
    # reverb / sustained top end: harmonic-part vs percussive-part energy >6 kHz in drops
    dsel = np.zeros(len(t), bool)
    for b in range(nbeats):
        if has_kick[b] and bar_lv.get((b - first_beat) // 4, -99) >= thr:
            dsel[(t >= gb[b]) & (t < gb[b] + beat)] = True
    res["hf_harmonic_vs_percussive_db"] = round(float(db(harm_hi[dsel].sum() / env[dsel].sum())), 1)
    # floor between hits vs hit peaks (tails / reverb / shaker bed)
    res["hf_floor_vs_peak_db"] = round(float(db(np.percentile(env_all[dsel], 10) / np.percentile(env_all[dsel], 95))), 1)
    res["first_beat_index"] = int(first_beat)
    res["grid_used"] = grid_used
    res["grid_scores_ms"] = {k: round(v, 1) for k, v in scores.items()}
    json.dump(res, open(out, "w"), indent=1)
    show(res)


def show(res):
    print(f"{res['file']}: {res['bpm']} BPM, 16th = {res['sixteenth_ms']} ms, kick bars {res['kick_bars']}, no-kick bars {res['nokick_bars']}")
    for sec in ("pattern_drop", "pattern_groove", "pattern_nokick"):
        p = res[sec]
        print(f"  {sec}:")
        print("    pos    " + " ".join(f"{q['pos'] + 1:>6}" for q in p))
        for k, w in (("hit_rate", "hit"), ("level_db", "lvl dB"), ("timing_ms", "dt ms"), ("timing_sd_ms", "sd ms"), ("decay_ms", "dec ms")):
            print(f"    {w:6} " + " ".join(f"{q.get(k) if q.get(k) is not None else '-':>6}" for q in p))
    for k in res:
        if not k.startswith("pattern") and k not in ("file", "bpm", "sixteenth_ms"):
            print(f"  {k}: {res[k]}")


if __name__ == "__main__":
    bh = float(sys.argv[sys.argv.index("--bpm") + 1]) if "--bpm" in sys.argv else None
    if "--grid" in sys.argv:
        FORCE_GRID = sys.argv[sys.argv.index("--grid") + 1]
    main(sys.argv[1], sys.argv[2], bh)
