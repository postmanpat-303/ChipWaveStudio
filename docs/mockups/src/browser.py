"""Browser / file manager mockups (3 designs), docked on the right of the real Timeline.

python3 docs/mockups/src/browser.py  ->  docs/mockups/src/browser_{a,b,c}.html
Render: google-chrome --headless --screenshot=out.png --window-size=1600,900 file://…/browser_a.html
Mockups only: nothing here is app code.
"""
import os, math, random

HERE = os.path.dirname(os.path.abspath(__file__))
BG = "../../screenshots/dark-timeline.png"  # the real Timeline (1920x1080, shown at 1600x900)

# colour per format (the legend in every design)
T = {
    "inst": ("Instrument", ".cwi · presets", "#5CC07A"),
    "vst3": ("VST3 plugin", ".vst3", "#A98BE0"),
    "clap": ("CLAP plugin", ".clap", "#E07BB5"),
    "wav": ("Sample", "WAV", "#4FB8C9"),
    "flac": ("Sample", "FLAC", "#4F9FD9"),
    "mp3": ("Sample", "MP3", "#62C9B2"),
    "ogg": ("Sample", "OGG", "#7FC4E0"),
    "song": ("Song", ".cwt · .cwt.toml", "#E0B45C"),
    "mod": ("Tracker module", "MOD · MED (planned)", "#E08A4F"),
    "wt": ("Wavetable", "WAV (clm)", "#C9CF52"),
    "grid": ("Grid patch", "patch", "#E06C6C"),
    "mid": ("MIDI file", ".mid (planned)", "#6C8FE8"),
}
LEG = ["inst", "vst3", "clap", "wav", "flac", "mp3", "ogg", "song", "mod", "wt", "grid", "mid"]
SHORT = {"inst": "instrument", "vst3": "plugin", "clap": "plugin", "wav": "sample", "flac": "sample", "mp3": "sample",
         "ogg": "sample", "song": "song", "mod": "MOD/MED*", "wt": "wavetable", "grid": "Grid patch", "mid": "MIDI*"}
BADGE = {"inst": "CWI", "vst3": "VST3", "clap": "CLAP", "wav": "WAV", "flac": "FLAC", "mp3": "MP3",
         "ogg": "OGG", "song": "CWT", "mod": "MOD", "wt": "WT", "grid": "GRID", "mid": "MID"}

CSS = """
@font-face{font-family:HackM;src:local('Hack'),url(file:///workspace/mock/Hack-Regular.ttf),local('DejaVu Sans Mono')}
*{box-sizing:border-box}
body{margin:0;width:1600px;height:900px;position:relative;overflow:hidden;background:#1d2023 url(%(bg)s) 0 0/1600px 900px no-repeat;
 font-family:'Ubuntu','DejaVu Sans',sans-serif;color:#D4D7DB;font-size:13px}
.abs{position:absolute}
.mono{font-family:HackM,'DejaVu Sans Mono',monospace}
#cover{left:1234px;top:62px;width:366px;height:818px;background:#26292d}
#main{left:0;top:62px;width:1234px;height:818px;background:url(%(bg)s) -273px -62px/1600px 900px no-repeat}
#panel{left:1238px;top:66px;width:358px;height:810px;background:#2b2f33;border:1px solid #444a50;border-right:3px solid #4CAF6A;border-radius:6px 0 0 6px;overflow:hidden}
.ph{height:30px;display:flex;align-items:center;gap:6px;padding:0 8px;background:linear-gradient(#2f3337,#272a2e);border-bottom:1px solid #1d2023}
.chip{font-size:11px;font-weight:bold;letter-spacing:.5px;color:#1A1C1F;background:#5CC07A;border-radius:3px;padding:1px 6px}
.search{flex:1;height:22px;background:#1f2225;border:1px solid #4a5056;border-radius:11px;display:flex;align-items:center;padding:0 8px;color:#7d848c;font-size:12px}
.ib{width:22px;height:22px;border-radius:5px;background:#34383D;border:1px solid #3d4247;display:flex;align-items:center;justify-content:center;color:#9AA0A8;font-size:12px}
.ib.on{background:#2E4A37;color:#8FD9A3;border-color:#4CAF6A}
.badge{display:inline-block;min-width:36px;text-align:center;font-size:9.5px;font-weight:bold;letter-spacing:.3px;line-height:15px;border-radius:3px;padding:0 4px;color:#15171a}
.dot{display:inline-block;width:9px;height:9px;border-radius:2px;margin-right:5px;vertical-align:-1px}
.sec{font-size:10.5px;letter-spacing:1px;color:#7d848c;text-transform:uppercase;padding:6px 10px 3px}
.leg{position:absolute;left:0;right:0;bottom:0;background:#232629;border-top:1px solid #1d2023;padding:5px 8px 6px;display:grid;grid-template-columns:repeat(3,1fr);row-gap:2px;column-gap:4px;font-size:10.5px;color:#9AA0A8}
.leg b{color:#D4D7DB;font-weight:normal}
.hint{font-size:11px;color:#7d848c}
.btn{display:inline-block;background:#34383D;border-radius:5px;padding:1px 8px;font-size:12px;color:#D4D7DB;border:1px solid #3d4247}
.star{color:#E0C46C}
"""

def legend(title="Colour = format"):
    cells = "".join(
        f'<div style="white-space:nowrap"><span class="dot" style="background:{T[k][2]}"></span><b>{BADGE[k]}</b> {SHORT[k]}</div>'
        for k in LEG)
    return f'<div class="leg"><div style="grid-column:1/4;font-size:10px;letter-spacing:1px;color:#7d848c;text-transform:uppercase">{title} <span style="text-transform:none;letter-spacing:0">· * import planned</span></div>{cells}</div>'

def badge(k):
    return f'<span class="badge" style="background:{T[k][2]}">{BADGE[k]}</span>'

def wave(w, h, seed=3, col="#4FB8C9", n=None, env="perc"):
    random.seed(seed)
    n = n or w // 2
    pts = []
    for i in range(n):
        x = i * w / n
        t = i / n
        e = math.exp(-t * 5) if env == "perc" else (0.35 + 0.65 * math.sin(t * math.pi) ** 0.5)
        a = (0.15 + 0.85 * random.random()) * e * h / 2
        pts.append(f'<rect x="{x:.1f}" y="{h/2-a:.1f}" width="{max(w/n-0.6,0.8):.1f}" height="{2*a+0.5:.1f}" fill="{col}"/>')
    return f'<svg width="{w}" height="{h}">' + "".join(pts) + "</svg>"

def page(name, panel, extra=""):
    html = f"""<!doctype html><html><head><meta charset="utf-8"><title>ChipWave browser mockup {name}</title>
<style>{CSS % {'bg': BG}}</style></head><body>
<div id="cover" class="abs"></div><div id="main" class="abs"></div>
<div id="panel" class="abs">{panel}</div>{extra}
</body></html>"""
    with open(os.path.join(HERE, f"browser_{name}.html"), "w") as f:
        f.write(html)

# ---------------------------------------------------------------- A
def design_a():
    tree = [
        (0, "▾", "Library", None, ""), (1, "▸", "Instruments", "inst", "214"), (1, "▾", "Samples", "wav", "1 382"),
        (2, "▾", "Drums", "wav", ""), (3, "", "Kicks", "wav", "64"), (3, "", "Snares &amp; claps", "wav", "88"),
        (3, "", "Breaks", "flac", "31"), (2, "▸", "Loops", "mp3", ""), (2, "▸", "Field recordings", "ogg", ""),
        (1, "▸", "Wavetables", "wt", "10"), (1, "▸", "Grid patches", "grid", "9"), (1, "▸", "Plugins", "vst3", "37"),
        (1, "▸", "Songs", "song", "26"), (1, "▸", "MIDI", "mid", "12"),
        (0, "▸", "~/Music/ChipWave", None, ""), (0, "▸", "D:\\Samples", None, ""),
    ]
    trows = ""
    for d, ar, nm, k, cnt in tree:
        col = T[k][2] if k else "#9AA0A8"
        sel = nm == "Breaks"
        trows += (f'<div style="height:19px;line-height:19px;padding-left:{8+d*14}px;display:flex;align-items:center;'
                  f'{"background:#2E4A37;color:#E6E8EB;" if sel else ""}"><span style="width:12px;color:#8FC7A0;font-size:10px">{ar}</span>'
                  f'<span class="dot" style="background:{col};{"" if k else "border-radius:1px;opacity:.6"}"></span>{nm}'
                  f'<span style="margin-left:auto;margin-right:8px;color:#5d636a;font-size:11px">{cnt}</span></div>')
    files = [
        ("flac", "Amen 136 full.flac", "44.1k st · 7.06 s", "2.1 MB", 0),
        ("flac", "Amen 136 ride.flac", "44.1k st · 1.76 s", "540 kB", 0),
        ("wav", "Apache break 112.wav", "48k st · 8.57 s", "3.0 MB", 1),
        ("flac", "Funky drummer 101.flac", "44.1k st · 9.50 s", "2.8 MB", 0),
        ("mp3", "Hot pants loop.mp3", "44.1k st · 4.20 s", "168 kB", 0),
        ("wav", "Impeach 98.wav", "44.1k m · 4.90 s", "430 kB", 0),
        ("ogg", "Think break 108.ogg", "48k st · 8.88 s", "290 kB", 0),
        ("wav", "Synthetic sub-stab.wav", "44.1k m · 0.62 s", "54 kB", 0),
    ]
    frows = ""
    for i, (k, nm, info, sz, sel) in enumerate(files):
        bg = "background:#2E4A37;color:#E6E8EB;box-shadow:inset 2px 0 #5CC07A;" if sel else ("background:#2e3236;" if i % 2 else "")
        frows += (f'<div style="height:22px;display:flex;align-items:center;gap:7px;padding:0 8px;{bg}">{badge(k)}'
                  f'<span style="flex:1;white-space:nowrap;overflow:hidden;text-overflow:ellipsis">{nm}</span>'
                  f'<span class="mono" style="font-size:10.5px;color:#8FC7A0">{info}</span>'
                  f'<span style="width:46px;text-align:right;font-size:11px;color:#7d848c">{sz}</span></div>')
    panel = f"""
<div class="ph"><span class="chip">BROWSER</span><div class="search">⌕&nbsp; Search all files…<span style="margin-left:auto;font-size:10px;color:#5d636a">Ctrl+B</span></div>
<div class="ib on">☰</div><div class="ib">▦</div></div>
<div style="display:flex;gap:4px;padding:5px 8px;border-bottom:1px solid #1d2023;font-size:11.5px">
<span class="btn" style="background:#2E4A37;color:#8FD9A3;border-color:#4CAF6A">All types</span><span class="btn">Audio</span><span class="btn">Instruments</span><span class="btn">Plugins</span><span class="btn">Songs</span></div>
<div class="sec">Places &amp; folders</div>
<div style="height:288px;overflow:hidden">{trows}</div>
<div style="height:1px;background:#1d2023"></div>
<div style="display:flex;align-items:center;padding:4px 8px;font-size:11px;color:#7d848c;background:#26292d">
<span style="width:43px">Type</span><span style="flex:1">Name ▴</span><span>Format · length</span><span style="width:52px;text-align:right">Size</span></div>
<div style="height:220px">{frows}</div>
<div class="abs" style="left:0;right:0;bottom:80px;height:118px;background:#212427;border-top:1px solid #1d2023;padding:6px 8px">
 <div style="display:flex;align-items:center;gap:6px;font-size:12px">{badge('wav')}<span style="color:#E6E8EB">Apache break 112.wav</span>
 <span style="margin-left:auto" class="hint">48 kHz · stereo · 24-bit · 112 BPM</span></div>
 <div style="margin-top:6px;background:#17191C;border:1px solid #3a3f44;border-radius:5px;height:52px;position:relative;overflow:hidden">
 <div class="abs" style="left:4px;top:3px">{wave(332,46,7,'#4FB8C9',150,'loop')}</div>
 <div class="abs" style="left:118px;top:0;width:2px;height:52px;background:#E8833A"></div></div>
 <div style="display:flex;align-items:center;gap:6px;margin-top:6px">
 <span class="ib on" style="width:26px">▶</span><span class="ib">■</span>
 <span class="btn" style="font-size:11px">⟳ Auto-play</span><span class="btn" style="font-size:11px;background:#2E4A37;color:#8FD9A3;border-color:#4CAF6A">♩ Sync 104 BPM</span>
 <span style="margin-left:auto" class="hint">−6 dB</span></div>
</div>
{legend()}
"""
    note = '<div class="abs hint" style="left:300px;top:884px;font-size:11px;color:#9AA0A8">Mockup A · tree + file list · double-click or drag a file onto a track; Enter loads into the selected track</div>'
    page("a", panel, note)

# ---------------------------------------------------------------- B
def design_b():
    cats = [("Instruments", "inst", "♪", True), ("Plugins", "vst3", "⧉", False), ("Samples", "wav", "∿", False),
            ("Songs", "song", "♫", False), ("Presets", "inst", "★", False), ("Grid", "grid", "⊞", False),
            ("Wavetables", "wt", "◠", False), ("MIDI", "mid", "▤", False)]
    tabs = ""
    for nm, k, ic, on in cats:
        c = T[k][2]
        tabs += (f'<div style="width:87px;height:46px;display:flex;flex-direction:column;align-items:center;justify-content:center;'
                 f'border-radius:6px;{"background:#383c41;box-shadow:inset 0 -2px "+c+";" if on else "background:#2a2d31;"}">'
                 f'<span style="font-size:17px;color:{c}">{ic}</span><span style="font-size:11px;color:{"#E6E8EB" if on else "#9AA0A8"}">{nm}</span></div>')
    tags_on = {"Bass", "Chip"}
    tags = "".join(
        f'<span style="display:inline-block;margin:0 4px 4px 0;padding:1px 8px;border-radius:9px;font-size:11px;'
        f'{"background:#2E4A37;color:#8FD9A3;border:1px solid #4CAF6A" if t in tags_on else "background:#34383D;color:#9AA0A8;border:1px solid #3d4247"}">{t}</span>'
        for t in ["Bass", "Lead", "Pad", "Keys", "Drums", "FX", "Chip", "Analog", "FM", "Voice", "Warm", "Bright", "Plucky", "Evolving"])
    def colm(title, items, w):
        rows = "".join(
            f'<div style="height:18px;line-height:18px;padding:0 6px;display:flex;{"background:#2E4A37;color:#E6E8EB;" if s else ""}">'
            f'<span>{n}</span><span style="margin-left:auto;color:#5d636a;font-size:10.5px">{c}</span></div>'
            for n, c, s in items)
        return f'<div style="width:{w}px"><div class="sec" style="padding:4px 6px 2px">{title}</div>{rows}</div>'
    filt = (colm("Category", [("All", 214, 0), ("Bass", 38, 1), ("Lead", 41, 0), ("Pad", 27, 0), ("Drums", 52, 0), ("FX", 19, 0)], 112)
            + colm("Creator", [("All", 214, 1), ("ChipWave", 120, 0), ("Del", 61, 0), ("Community", 33, 0)], 112)
            + colm("Location", [("All", 214, 1), ("Library", 160, 0), ("This song", 14, 0), ("Favourites", 40, 0)], 112))
    res = [
        ("inst", "Acid Moog", "Analog · Bass · Chip", 3, 1),
        ("inst", "Octave Bass", "Analog · Bass", 2, 0),
        ("inst", "Pulse 12.5% Sub", "Chip · Bass", 3, 0),
        ("clap", "Surge XT › Reese Gnarl", "CLAP · Bass", 2, 0),
        ("vst3", "Vital › Wobble Mono", "VST3 · Bass", 1, 0),
        ("grid", "Grid › FM Growl", "Grid patch · Bass", 2, 0),
        ("inst", "SID Saw Bass", "Chip · Bass · Bright", 3, 0),
        ("wt", "Wave › Formant Bass", "Wavetable · Bass", 1, 0),
        ("inst", "Square Pluck Bass", "Chip · Bass · Plucky", 2, 0),
        ("inst", "Warm FM Bass", "FM · Bass · Warm", 2, 0),
    ]
    rows = ""
    for i, (k, nm, sub, stars, sel) in enumerate(res):
        c = T[k][2]
        bg = "background:#2E4A37;" if sel else ("background:#2e3236;" if i % 2 else "")
        rows += (f'<div style="height:30px;display:flex;align-items:center;gap:7px;padding:0 8px 0 0;{bg}">'
                 f'<span style="width:4px;height:30px;background:{c}"></span>{badge(k)}'
                 f'<div style="flex:1;line-height:13px"><div style="color:{"#E6E8EB" if sel else "#D4D7DB"}">{nm}</div>'
                 f'<div style="font-size:10.5px;color:#7d848c">{sub}</div></div>'
                 f'<span class="star" style="font-size:11px">{"★"*stars}<span style="color:#4a4f55">{"★"*(3-stars)}</span></span>'
                 f'<span style="color:{"#E0C46C" if i in (0,2,6) else "#4a4f55"};font-size:13px">♥</span></div>')
    panel = f"""
<div class="ph"><span class="chip">BROWSER</span><span class="hint" style="margin-left:4px">Instruments · 214</span>
<span class="ib" style="margin-left:auto">♥</span><span class="ib">⟳</span><span class="ib">⚙</span></div>
<div style="display:grid;grid-template-columns:repeat(4,1fr);gap:3px;padding:5px 5px 3px">{tabs}</div>
<div style="padding:4px 8px"><div class="search" style="height:24px">⌕&nbsp; <span style="color:#D4D7DB">bass</span><span style="margin-left:auto;font-size:10px;color:#5d636a">tags · name · creator</span></div></div>
<div style="padding:2px 8px 2px">{tags}</div>
<div style="display:flex;gap:4px;padding:0 4px;border-top:1px solid #1d2023;border-bottom:1px solid #1d2023;background:#26292d;height:136px">{filt}</div>
<div style="display:flex;align-items:center;padding:4px 8px;font-size:11px;color:#7d848c"><span>12 results · Bass + Chip</span><span style="margin-left:auto">Sort: Rating ▾</span></div>
<div>{rows}</div>
<div class="abs" style="left:0;right:0;bottom:80px;height:30px;background:#212427;border-top:1px solid #1d2023;display:flex;align-items:center;gap:6px;padding:0 8px">
<span class="ib on">▶</span><span style="font-size:11.5px">Preview on <b style="color:#8FD9A3;font-weight:normal">C-3</b> · plays through track 02 Bass</span>
<span class="hint" style="margin-left:auto">Enter: load</span></div>
{legend()}
"""
    note = '<div class="abs hint" style="left:300px;top:884px;font-size:11px;color:#9AA0A8">Mockup B · Bitwig-style category tabs, tag chips, Category/Creator/Location columns, rated results</div>'
    page("b", panel, note)

# ---------------------------------------------------------------- C
def design_c():
    places = [("★", "Favourites", True), ("⏱", "Recent", False), ("▣", "Library", False), ("♫", "Projects", False),
              ("⧉", "Plugins", False), ("⌂", "Home", False), ("⛁", "D:", False), ("+", "Add", False)]
    rail = "".join(
        f'<div style="height:52px;display:flex;flex-direction:column;align-items:center;justify-content:center;'
        f'{"background:#2E4A37;box-shadow:inset 3px 0 #5CC07A;" if on else ""}">'
        f'<span style="font-size:17px;color:{"#E0C46C" if ic=="★" else "#9AA0A8"}">{ic}</span>'
        f'<span style="font-size:9.5px;color:{"#E6E8EB" if on else "#7d848c"}">{nm}</span></div>' for ic, nm, on in places)
    tiles = [
        ("wav", "Kick 909 tight", "perc"), ("wav", "Snare gated", "perc"), ("flac", "Amen 136", "loop"),
        ("inst", "Acid Moog", None), ("inst", "Disco Strings", None), ("clap", "Surge XT", None),
        ("vst3", "Vital", None), ("song", "Velvet Mirrorball", None), ("song", "Night Drive", None),
        ("wt", "Formant sweep", None), ("grid", "FM Growl", None), ("mid", "Chords 8 bars", None),
        ("mp3", "Vox chop", "perc"), ("ogg", "Rain field", "loop"), ("mod", "space_debris.mod", None),
    ]
    ICON = {"inst": "♪", "clap": "⧉", "vst3": "⧉", "song": "♫", "wt": "◠", "grid": "⊞", "mid": "▤", "mod": "▦"}
    tl = ""
    for i, (k, nm, w) in enumerate(tiles):
        c = T[k][2]
        sel = i == 2
        if w:
            body = f'<div style="margin-top:6px">{wave(78,30,i+2,c,34,w)}</div>'
        elif k == "wt":
            body = ('<svg width="78" height="34" style="margin-top:4px">' + "".join(
                f'<path d="M{4+j*3} {28-j*2} ' + " ".join(f'L{4+j*3+x*0.62} {28-j*2-10*math.sin(x/100*6.28)*(0.3+j*0.12)}' for x in range(0,101,5)) + f'" stroke="{c}" stroke-opacity="{0.25+j*0.15}" fill="none"/>' for j in range(5)) + '</svg>')
        elif k == "mid":
            body = '<svg width="78" height="34" style="margin-top:4px">' + "".join(
                f'<rect x="{4+j*9}" y="{4+((j*5)%4)*7}" width="8" height="5" rx="1" fill="{c}"/>' for j in range(8)) + '</svg>'
        else:
            body = f'<div style="font-size:24px;color:{c};margin-top:4px;line-height:34px">{ICON[k]}</div>'
        tl += (f'<div style="width:94px;height:92px;border-radius:7px;background:{"#2E4A37" if sel else "#33373c"};'
               f'border:1px solid {"#5CC07A" if sel else "#41464c"};position:relative;text-align:center;overflow:hidden">'
               f'<div style="height:3px;background:{c}"></div>{body}'
               f'<div style="font-size:10.5px;color:#D4D7DB;margin-top:3px;padding:0 4px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis">{nm}</div>'
               f'<div class="abs" style="right:4px;top:6px">{badge(k).replace("min-width:36px","")}</div>'
               f'{"<div class=abs style=left:5px;top:5px;color:#E0C46C;font-size:11px>★</div>" if i in (0,3,7) else ""}</div>')
    cols = f"""
<div style="display:flex;height:100%">
 <div style="width:58px;background:#25282c;border-right:1px solid #1d2023">{rail}</div>
 <div style="flex:1;position:relative">
  <div style="display:flex;align-items:center;gap:4px;padding:6px 8px;font-size:11.5px;color:#9AA0A8;border-bottom:1px solid #1d2023">
   <span class="star">★</span><span style="color:#E6E8EB">Favourites</span><span>›</span><span>All</span>
   <span class="ib on" style="margin-left:auto;width:20px;height:20px">▦</span><span class="ib" style="width:20px;height:20px">☰</span><span class="ib" style="width:20px;height:20px">▥</span></div>
  <div style="display:flex;gap:4px;flex-wrap:wrap;padding:6px 8px;font-size:10.5px">
   {''.join(f'<span style="padding:0 7px;border-radius:8px;border:1px solid {T[k][2]};color:{T[k][2]}">{n}</span>' for n,k in [("Audio","wav"),("Instr.","inst"),("Plugins","vst3"),("Songs","song"),("Grid","grid"),("MIDI","mid")])}
  </div>
  <div style="display:grid;grid-template-columns:repeat(3,94px);gap:6px;padding:2px 8px">{tl}</div>
  <div class="sec" style="padding-top:8px">Recently used</div><div style="height:20px;display:flex;align-items:center;gap:6px;padding:0 8px;font-size:11.5px">{badge("song")}<span>Chip Odyssey.cwt.toml</span><span class="hint" style="margin-left:auto">today</span></div><div style="height:20px;display:flex;align-items:center;gap:6px;padding:0 8px;font-size:11.5px">{badge("inst")}<span>0B Cyber Choir.cwi</span><span class="hint" style="margin-left:auto">today</span></div>
 </div>
</div>"""
    panel = f"""
<div class="ph"><span class="chip">FILES</span><div class="search">⌕&nbsp; Filter favourites…</div><div class="ib">⚙</div></div>
<div style="height:582px">{cols}</div>
<div class="abs" style="left:58px;right:0;bottom:80px;height:72px;background:#212427;border-top:1px solid #1d2023;padding:6px 8px">
 <div style="display:flex;align-items:center;gap:6px;font-size:11.5px">{badge('flac')}<span style="color:#E6E8EB">Amen 136.flac</span><span class="hint" style="margin-left:auto">7.06 s · 136 BPM</span></div>
 <div style="margin-top:5px;background:#17191C;border:1px solid #3a3f44;border-radius:5px;height:34px;overflow:hidden;padding:2px 4px">{wave(270,28,11,'#4F9FD9',120,'loop')}</div>
</div>
{legend()}
"""
    # the drag in progress: a ghost tile over the Hats track and a drop marker
    ghost = f"""
<div class="abs" style="left:641px;top:205px;width:2px;height:38px;background:#E8833A;box-shadow:0 0 6px #E8833A"></div>
<div class="abs" style="left:642px;top:207px;width:260px;height:34px;border:2px dashed #4F9FD9;border-radius:4px;background:rgba(79,159,217,.18)"></div>
<div class="abs" style="left:658px;top:222px;width:96px;height:44px;border-radius:6px;background:#33373c;border:1px solid #4F9FD9;opacity:.92;box-shadow:0 6px 16px rgba(0,0,0,.5);padding:4px;text-align:center">
 <div style="font-size:10px;color:#E6E8EB">Amen 136.flac</div>{wave(84,22,11,'#4F9FD9',40,'loop')}</div>
<div class="abs" style="left:760px;top:248px;background:#1f2225;border:1px solid #4a5056;border-radius:5px;padding:3px 8px;font-size:11.5px;color:#D4D7DB;box-shadow:0 4px 10px rgba(0,0,0,.5)">
 Drop on <b style="color:#E0B45C">03 Hats</b> at bar 41 · new audio clip<br><span class="hint">Shift: new track · Alt: replace instrument / sample</span></div>
<svg class="abs" style="left:740px;top:150px" width="830" height="140"><path d="M810 50 C 560 10, 260 40, 14 80" stroke="#4F9FD9" stroke-width="1.5" stroke-dasharray="4 4" fill="none"/></svg>
<div class="abs hint" style="left:300px;top:884px;font-size:11px;color:#9AA0A8">Mockup C · places rail + favourites, icon tiles with previews, drag a file onto a track</div>"""
    page("c", panel, ghost)

design_a(); design_b(); design_c()
