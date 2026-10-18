#!/usr/bin/env python3
# ChipWave Lite - compare a ptplayer tick log with the libxmp reference dump
# Copyright (C) 2026 Del and the ChipWave contributors. GPL-3.0-or-later.
#
#   compare_ref.py TICKLOG XMPREF_TXT [--known FILE] [--show N]
#
# ptplayer (like ProTracker 2.3) processes row 0 on its 6th call, so its
# log is shifted by LEAD = 5 ticks against libxmp, whose frame 0 is row 0.
# Per tick and channel it compares AUDxPER with libxmp's period and AUDxVOL
# with libxmp's volume. One known, harmless difference is allowed: when a
# one-shot sample has played to its end, libxmp reports volume 0, while
# Paula keeps the volume register and plays ptplayer's silent idle loop
# (repeat length 1 word); that is counted as "ended", not as a mismatch.
# Periods may differ by 1 ("round"): ptplayer uses ProTracker's period
# table, libxmp computes periods from note numbers. Cells listed in
# --known FILE (see ref_known_diffs.txt) are places where libxmp itself
# differs from ProTracker 2.3; they are counted as "known".
import sys, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import ticklog

LEAD = 5

def load_ref(path):
    out = []
    for line in open(path):
        parts = line.split("|")
        head = parts[0].split()
        chans = []
        for p in parts[1:]:
            a, b = p.split()
            chans.append((int(a[1:]), int(b[1:])))
        out.append(dict(tick=int(head[0]), pos=int(head[1]), row=int(head[2]),
                        frame=int(head[3]), bpm=int(head[4]), speed=int(head[5]), ch=chans))
    return out

def load_known(path):
    known = set()
    for line in open(path):
        line = line.split("#")[0].split()
        if len(line) == 4:
            known.add((int(line[0]), int(line[1]), int(line[2]), line[3]))
    return known

def main():
    log = ticklog.read(sys.argv[1])
    ref = load_ref(sys.argv[2])
    known = load_known(sys.argv[sys.argv.index("--known") + 1]) if "--known" in sys.argv else set()
    show = int(sys.argv[sys.argv.index("--show") + 1]) if "--show" in sys.argv else 10
    n = min(len(log) - LEAD, len(ref))
    stats = dict(per_ok=0, per_round=0, per_bad=0, known=0, vol_ok=0, vol_bad=0, ended=0, bpm_ok=0, bpm_bad=0)
    bad = []
    for i in range(n):
        L, R = log[i + LEAD], ref[i]
        if L["tempo"] == R["bpm"]:
            stats["bpm_ok"] += 1
        else:
            stats["bpm_bad"] += 1
            bad.append((i, -1, "bpm %d vs %d" % (L["tempo"], R["bpm"])))
        for c in range(4):
            lc, ln, lp, lv = L["ch"][c]
            rp, rv = R["ch"][c]
            if rv == 0 and lv != 0 and L["rep"][c][1] <= 1:
                stats["ended"] += 1
                continue
            if lv == rv:
                stats["vol_ok"] += 1
            elif (R["pos"], R["row"], c, "vol") in known:
                stats["known"] += 1
            else:
                stats["vol_bad"] += 1
                bad.append((i, c, "vol %d vs %d" % (lv, rv)))
            if rv == 0 or rp == 0:
                continue  # silent: the period doesn't matter
            if lp == rp:
                stats["per_ok"] += 1
            elif abs(lp - rp) <= 1:
                stats["per_round"] += 1
            elif (R["pos"], R["row"], c, "per") in known:
                stats["known"] += 1
            else:
                stats["per_bad"] += 1
                bad.append((i, c, "per %d vs %d" % (lp, rp)))
    print("ticks compared: %d (ptplayer log %d incl. %d lead-in, libxmp %d)" % (n, len(log), LEAD, len(ref)))
    for k, v in stats.items():
        print("  %-7s %d" % (k, v))
    for i, c, what in bad[:show]:
        R = ref[i]
        print("  tick %4d pos %d row %2d frame %d ch%d: %s" % (i, R["pos"], R["row"], R["frame"], c, what))
    ok = stats["per_bad"] == 0 and stats["vol_bad"] == 0 and stats["bpm_bad"] == 0
    print("RESULT:", "MATCH" if ok else "DIFFERENCES (%d)" % len(bad))
    return 0 if ok else 1

if __name__ == "__main__":
    sys.exit(main())
