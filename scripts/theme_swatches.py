#!/usr/bin/env python3
"""Palette swatches of the built-in themes, read straight from src/theme.rs
(the real colour values, labelled with their field names and hex codes).
Usage: python3 scripts/theme_swatches.py docs/screenshots/theme-palettes.png"""
import os, re, sys
from PIL import Image, ImageDraw, ImageFont

SRC = os.path.join(os.path.dirname(__file__), "..", "src", "theme.rs")
NAMED = {"WHITE": 0xFFFFFF, "BLACK": 0x000000}


def themes():
    text = open(SRC, encoding="utf-8").read()
    out = []
    for m in re.finditer(r"pub const (\w+): Theme = Theme \{(.*?)\n\};", text, re.S):
        body, cols, name = m.group(2), [], m.group(1)
        for line in body.splitlines():
            line = line.strip()
            n = re.match(r'name: "(.*)"', line)
            if n:
                name = n.group(1)
            c = re.match(r"(\w+): hex\(0x([0-9A-Fa-f]{6})\)", line)
            w = re.match(r"(\w+): Color32::(\w+)", line)
            if c:
                cols.append((c.group(1), int(c.group(2), 16)))
            elif w and w.group(2) in NAMED:
                cols.append((w.group(1), NAMED[w.group(2)]))
        out.append((name, cols))
    return out


def font(size, bold=False):
    for f in (f"/usr/share/fonts/truetype/dejavu/DejaVuSans{'-Bold' if bold else ''}.ttf",
              "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"):
        if os.path.exists(f):
            return ImageFont.truetype(f, size)
    return ImageFont.load_default()


def main(path):
    ts = themes()
    cols_per_row, sw, sh, gap, pad = 10, 128, 92, 10, 24
    rows = [((len(c) + cols_per_row - 1) // cols_per_row) for _, c in ts]
    W = pad * 2 + cols_per_row * sw + (cols_per_row - 1) * gap
    H = pad + sum(46 + r * (sh + gap) + 16 for r in rows) + pad
    img = Image.new("RGB", (W, H), (30, 32, 36))
    d = ImageDraw.Draw(img)
    f_t, f_n, f_h = font(22, True), font(13), font(14, True)
    y = pad
    for (name, cols), r in zip(ts, rows):
        d.text((pad, y), f"{name}  ·  src/theme.rs", font=f_t, fill=(230, 232, 235))
        y += 46
        for i, (field, c) in enumerate(cols):
            x = pad + (i % cols_per_row) * (sw + gap)
            yy = y + (i // cols_per_row) * (sh + gap)
            rgb = ((c >> 16) & 255, (c >> 8) & 255, c & 255)
            d.rounded_rectangle([x, yy, x + sw, yy + sh], radius=8, fill=rgb, outline=(90, 94, 100))
            lum = 0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2]
            ink = (20, 22, 26) if lum > 140 else (240, 242, 245)
            d.text((x + 8, yy + sh - 42), field, font=f_n, fill=ink)
            d.text((x + 8, yy + sh - 22), f"#{c:06X}", font=f_h, fill=ink)
        y += r * (sh + gap) + 16
    img.save(path, optimize=True)
    print(f"{path}: {len(ts)} themes, {W}x{H}")


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else "theme-palettes.png")
