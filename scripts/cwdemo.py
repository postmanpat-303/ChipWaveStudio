"""Shared helpers for the demo generators (scripts/make_tallest_morning.py): note
names, tracker blocks, automation lanes and modulation in the single-file text
song format (docs/FIRST_RUN.md)."""
import os

ROWS = 64
B = 16  # rows per bar (lines_per_beat = 4)
NAMES = ["C-", "C#", "D-", "D#", "E-", "F-", "F#", "G-", "G#", "A-", "A#", "B-"]
PC = {"C": 0, "C#": 1, "Db": 1, "D": 2, "D#": 3, "Eb": 3, "E": 4, "F": 5, "F#": 6, "Gb": 6,
      "G": 7, "G#": 8, "Ab": 8, "A": 9, "A#": 10, "Bb": 10, "B": 11}


def n(pc, octave):
    """note name: n("F#", 3) -> "F#3"; pc may also be a semitone number"""
    s = PC[pc] if isinstance(pc, str) else pc
    octave += s // 12
    return f"{NAMES[s % 12]}{octave}"


def m(midi):
    """MIDI-style number (C-4 = 48 here: octave * 12 + semitone) to a note name"""
    return f"{NAMES[midi % 12]}{midi // 12}"


def num(name):
    """note name to number (inverse of m)"""
    return int(name[2:]) * 12 + NAMES.index(name[:2])


class Block:
    def __init__(self, name, role, ch):
        self.name, self.role, self.ch = name, role, ch
        self.cells = [[None] * ch for _ in range(ROWS)]

    def put(self, row, ch, note, ins=None, vol=None, fx=None):
        if 0 <= row < ROWS:
            self.cells[row][ch] = (note, ins, vol, fx)

    def off(self, row, ch):
        if 0 <= row < ROWS and self.cells[row][ch] is None:
            self.cells[row][ch] = ("===", None, None, None)

    def fx(self, row, ch, fx):
        c = self.cells[row][ch]
        self.cells[row][ch] = (c[0], c[1], c[2], fx) if c else ("---", None, None, fx)

    def text(self):
        solo = os.environ.get("DEMO_SOLO")
        keep = {int(c) - 1 for c in solo.split(",")} if solo else set(range(self.ch))
        out = []
        for r, row in enumerate(self.cells):
            row = [c if k in keep else None for k, c in enumerate(row)]
            if all(c is None for c in row):
                continue
            parts = []
            for c in row:
                if c is None:
                    parts.append("--- .. .. ....")
                else:
                    nn, i, v, f = c
                    parts.append(f"{nn} {('%02X' % i) if i else '..'} "
                                 f"{('%02X' % v) if v is not None else '..'} {f or '....'}")
            out.append("%02X | " % r + " | ".join(parts))
        return "\n".join(out)


class Rng:
    def __init__(self, seed):
        self.s = seed

    def next(self):
        self.s = (self.s * 1103515245 + 12345) & 0x7FFFFFFF
        return self.s >> 16

    def jitter(self, v, amt=2):
        return max(1, min(0x40, v + self.next() % (2 * amt + 1) - amt))


def emit_blocks(blocks, order, header):
    for i, bk in enumerate(blocks):
        print(f'[[block]]\nid = {i}\nname = "{bk.name}"\nlines = {ROWS}\nrole = "{bk.role}"')
        print(header)
        print('rows = """\n' + bk.text() + '\n"""\n')
    secs = []
    for bk in order:
        if secs and secs[-1][0] == bk:
            secs[-1][1] += 1
        else:
            secs.append([bk, 1])
    print("[sequence]\nsections = [" + ", ".join(
        f"{{ block = {b}, repeat = {k} }}" for b, k in secs) + "]\n")


def lane(target, label, pts, curve="smooth"):
    print(f'[[automation]]\ntarget = "{target}"\nlabel = "{label}"\ncurve = "{curve}"\npoints = [')
    for p in sorted(pts, key=lambda x: x[0]):
        sh = f', shape = "{p[2]}"' if len(p) > 2 else ""
        print(f"  {{ pos = {p[0]}, value = {p[1]}{sh} }},")
    print("]\n")


def modulator(id, owner, name, kind="Lfo", **kw):
    """owner = 1-based instrument slot (written 0-based)"""
    print(f'[[modulation.modulator]]\nid = {id}\nowner = {owner - 1}\nname = "{name}"\nkind = "{kind}"')
    for k, v in kw.items():
        if isinstance(v, bool):
            v = "true" if v else "false"
        elif isinstance(v, str):
            v = f'"{v}"'
        print(f"{k} = {v}")


def route(src, target, amount):
    print(f'[[modulation.route]]\nsrc = {src}\ntarget = "{target}"\namount = {amount}')
