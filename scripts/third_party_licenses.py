#!/usr/bin/env python3
"""Write THIRD_PARTY_LICENSES.txt for a release build: every crate linked into
the Windows exe, its licence expression, and the licence files it ships.
Usage: python3 scripts/third_party_licenses.py [target] > THIRD_PARTY_LICENSES.txt"""
import glob, json, os, subprocess, sys

target = sys.argv[1] if len(sys.argv) > 1 else "x86_64-pc-windows-gnu"
meta = json.loads(subprocess.check_output(
    ["cargo", "metadata", "--format-version", "1", "--filter-platform", target]))
nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
root = meta["resolve"]["root"]
# normal (non-dev, non-build) dependency closure of the app
keep, todo = set(), [root]
while todo:
    i = todo.pop()
    if i in keep:
        continue
    keep.add(i)
    for d in nodes[i]["deps"]:
        if any(k["kind"] is None for k in d["dep_kinds"]):
            todo.append(d["pkg"])
pkgs = sorted((p for p in meta["packages"] if p["id"] in keep and p["id"] != root),
              key=lambda p: (p["name"], p["version"]))
out = sys.stdout
out.write("ChipWave Tracker - third-party software\n")
out.write("=" * 60 + "\n\n")
out.write("ChipWave Tracker is GPL-3.0-or-later (see LICENSE.txt). It includes the\n"
          "crates below. MP3 export uses LAME 3.100 (LGPL), statically linked through\n"
          "mp3lame-sys; LAME's source is at https://lame.sourceforge.io and the\n"
          "complete ChipWave source (needed to relink) is at\n"
          "https://github.com/postmanpat-303/ChipWaveTracker\n\n")
for p in pkgs:
    out.write(f"- {p['name']} {p['version']}: {p.get('license') or 'see files'}\n")
out.write("\n\n")
seen = set()
for p in pkgs:
    d = os.path.dirname(p["manifest_path"])
    files = sorted(f for pat in ("LICEN[CS]E*", "COPYING*", "NOTICE*", "lame*/COPYING", "*/COPYING")
                   for f in glob.glob(os.path.join(d, pat)) if os.path.isfile(f))
    for f in files:
        try:
            txt = open(f, encoding="utf-8", errors="replace").read().strip()
        except OSError:
            continue
        key = (p["name"], txt)
        if key in seen:
            continue
        seen.add(key)
        out.write("=" * 60 + "\n")
        out.write(f"{p['name']} {p['version']} - {os.path.relpath(f, d)}\n")
        out.write("=" * 60 + "\n")
        out.write(txt + "\n\n")
