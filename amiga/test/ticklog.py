#!/usr/bin/env python3
# ChipWave Lite - read/compare tick logs written by "ChipWaveLite x.mod TICKLOG file"
# Copyright (C) 2026 Del and the ChipWave contributors. GPL-3.0-or-later.
#
# File format (big-endian): 16-byte header "CWTL", u16 version (1),
# u16 record size (74), u32 module size, u32 0; then one 74-byte record per tick:
#   0 u32 tick
#   4 u16 DMACON written by mt_music (0xffff = not written; else the channels stopped)
#   6 4 x {u32 AUDxLC (offset into the module; ffffffff = 0, fffffffe = elsewhere),
#          u16 AUDxLEN, u16 AUDxPER, u16 AUDxVOL}   state after mt_music
#  46 u16 DMACON written by mt_dmaon (channels (re)started this tick, bit 15 = set)
#  48 u16 INTREQ written by mt_setrep
#  50 4 x {u24 repeat LC offset, u16 repeat LEN}    state after mt_setrep
#  70 u8 CIA-A PRA bit 1 (the "LED" audio filter, E0x)
#  71 u8 _mt_E8Trigger
#  72 u8 tempo (BPM, last Fxx >= $20; 125 at start)
#  73 u8 0
import struct, sys

def read(path):
    b = open(path, "rb").read()
    assert b[:4] == b"CWTL", "not a tick log"
    ver, rs, size = struct.unpack(">HHI", b[4:12])
    recs = []
    for o in range(16, len(b) - rs + 1, rs):
        r = b[o:o + rs]
        t, dmac = struct.unpack(">IH", r[:6])
        ch = [struct.unpack(">IHHH", r[6 + c * 10:16 + c * 10]) for c in range(4)]
        dmaon, intreq = struct.unpack(">HH", r[46:50])
        rep = []
        for c in range(4):
            q = r[50 + c * 5:55 + c * 5]
            rep.append(((q[0] << 16) | (q[1] << 8) | q[2], (q[3] << 8) | q[4]))
        recs.append(dict(t=t, dmacon=dmac, ch=ch, dmaon=dmaon, intreq=intreq, rep=rep,
                         led=r[70], e8=r[71], tempo=r[72]))
    return recs

def lc(v):
    return "null" if v == 0xffffffff else ("ext" if v == 0xfffffffe else "%05x" % v)

def text(recs):
    for r in recs:
        stop = "" if r["dmacon"] == 0xffff else "stop=%x" % (r["dmacon"] & 15)
        start = "" if r["dmaon"] == 0xffff else "start=%x" % (r["dmaon"] & 15)
        chs = " | ".join("%s %04x p%03d v%02d" % (lc(a), l, p, v) for a, l, p, v in r["ch"])
        yield "%05d %-6s %-7s | %s | led%d e8=%d bpm%d" % (r["t"], stop, start, chs, r["led"] >> 1, r["e8"], r["tempo"])

if __name__ == "__main__":
    if len(sys.argv) == 2:
        for line in text(read(sys.argv[1])):
            print(line)
    elif len(sys.argv) == 3:
        a, b = read(sys.argv[1]), read(sys.argv[2])
        n = min(len(a), len(b))
        diff = [i for i in range(n) if a[i] != b[i]]
        print("%s: %d ticks, %s: %d ticks, %d differing ticks" % (sys.argv[1], len(a), sys.argv[2], len(b), len(diff)))
        sys.exit(0 if not diff and len(a) == len(b) else 1)
    else:
        print("usage: ticklog.py LOG  |  ticklog.py LOG_A LOG_B")
