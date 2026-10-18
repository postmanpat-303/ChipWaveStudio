#!/usr/bin/env python3
"""Check the Windows icon: small frames must be BMP (DIB), not PNG.

Windows Explorer, the taskbar and Edge's download list often can't draw PNG-compressed
frames below 256 px and fall back to the generic program icon. A standard .ico keeps
256 px as PNG and every smaller size (128, 64, 48, 32, 24, 16) as 32-bit BMP.

Usage:
  scripts/check-ico.py assets/icon/chipwave_tracker.ico      # an .ico file
  scripts/check-ico.py dist/pkg/ChipWave.exe                  # every icon group in a PE (.exe)
  scripts/check-ico.py --extract OUTDIR file.exe              # also write the groups as .ico files
Exits 1 if a frame below 256 px is PNG, or 16, 32 or 48 px is missing.
Standard library only.
"""
import struct
import sys
from pathlib import Path

PNG = b"\x89PNG\r\n\x1a\n"
NEEDED = {16, 32, 48}


def frame_kind(data):
    if data[:8] == PNG:
        return "PNG"
    if len(data) >= 40 and struct.unpack_from("<I", data)[0] == 40:
        return "BMP %d-bit" % struct.unpack_from("<H", data, 14)[0]
    return "unknown"


def parse_ico(blob):
    """-> list of (size, bpp, frame bytes) from an .ico file."""
    _, typ, n = struct.unpack_from("<HHH", blob)
    if typ != 1:
        raise ValueError("not an icon file")
    out = []
    for i in range(n):
        w, _h, _c, _r, _p, bpp, size, off = struct.unpack_from("<BBBBHHII", blob, 6 + 16 * i)
        out.append((w or 256, bpp, blob[off:off + size]))
    return out


class PE:
    def __init__(self, blob):
        self.b = blob
        pe = struct.unpack_from("<I", blob, 0x3C)[0]
        if blob[pe:pe + 4] != b"PE\0\0":
            raise ValueError("not a PE file")
        nsec, = struct.unpack_from("<H", blob, pe + 6)
        optsz, = struct.unpack_from("<H", blob, pe + 20)
        opt = pe + 24
        magic, = struct.unpack_from("<H", blob, opt)
        dd = opt + (112 if magic == 0x20B else 96)
        self.rsrc_rva, _ = struct.unpack_from("<II", blob, dd + 2 * 8)
        self.secs = []
        for i in range(nsec):
            s = opt + optsz + 40 * i
            vsize, va, rsize, rptr = struct.unpack_from("<IIII", blob, s + 8)
            self.secs.append((va, max(vsize, rsize), rptr))
        self.root = self.off(self.rsrc_rva)

    def off(self, rva):
        for va, size, ptr in self.secs:
            if va <= rva < va + size:
                return rva - va + ptr
        raise ValueError("rva %x not in a section" % rva)

    def entries(self, d):
        named, ids = struct.unpack_from("<HH", self.b, d + 12)
        for i in range(named + ids):
            name, target = struct.unpack_from("<II", self.b, d + 16 + 8 * i)
            yield name, target

    def leaves(self, typ):
        """-> {id: data bytes} for one resource type (first language of each)."""
        out = {}
        for name, t in self.entries(self.root):
            if name != typ:
                continue
            for rid, t2 in self.entries(self.root + (t & 0x7FFFFFFF)):
                lang = self.root + (t2 & 0x7FFFFFFF)
                for _, t3 in self.entries(lang):
                    rva, size = struct.unpack_from("<II", self.b, self.root + t3)
                    out[rid] = self.b[self.off(rva):self.off(rva) + size]
                    break
        return out


def pe_groups(blob):
    """-> list of (group id, [(size, bpp, frame bytes)]) from RT_GROUP_ICON / RT_ICON."""
    pe = PE(blob)
    icons = pe.leaves(3)
    groups = []
    for gid, g in sorted(pe.leaves(14).items()):
        _, _, n = struct.unpack_from("<HHH", g)
        frames = []
        for i in range(n):
            w, _h, _c, _r, _p, bpp, _size, nid = struct.unpack_from("<BBBBHHIH", g, 6 + 14 * i)
            frames.append((w or 256, bpp, icons.get(nid, b"")))
        groups.append((gid, frames))
    return groups


def to_ico(frames):
    head = struct.pack("<HHH", 0, 1, len(frames))
    off = 6 + 16 * len(frames)
    body = b""
    for size, bpp, data in frames:
        s = size if size < 256 else 0
        head += struct.pack("<BBBBHHII", s, s, 0, 0, 1, bpp, len(data), off + len(body))
        body += data
    return head + body


def check(label, frames):
    ok = True
    print(label)
    for size, bpp, data in sorted(frames, key=lambda f: -f[0]):
        kind = frame_kind(data)
        bad = size < 256 and kind != "BMP 32-bit"
        ok &= not bad
        print("  %3d px  %2d bpp  %6d bytes  %s%s" % (size, bpp, len(data), kind, "  <-- should be BMP" if bad else ""))
    missing = NEEDED - {f[0] for f in frames}
    if missing:
        ok = False
        print("  missing sizes:", sorted(missing))
    return ok


def main(argv):
    extract = None
    if argv[:1] == ["--extract"]:
        extract, argv = Path(argv[1]), argv[2:]
    if not argv:
        print(__doc__)
        return 2
    ok = True
    for p in argv:
        blob = Path(p).read_bytes()
        if blob[:2] == b"MZ":
            groups = pe_groups(blob)
            if not groups:
                print(p + ": no icon resource")
                ok = False
            for gid, frames in groups:
                ok &= check("%s: icon group %s" % (p, gid), frames)
                if extract:
                    extract.mkdir(parents=True, exist_ok=True)
                    (extract / ("%s-%s.ico" % (Path(p).stem, gid))).write_bytes(to_ico(frames))
        else:
            ok &= check(p, parse_ico(blob))
    print("OK" if ok else "FAIL: small icon frames must be 32-bit BMP")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
