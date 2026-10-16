exec(open(__import__('os').path.join(__import__('os').path.dirname(__file__),'common.py')).read())
M='font-family="DejaVu Sans Mono, monospace"'
a('<text x="18" y="29" font-size="19" font-weight="bold" fill="#fff">ChipWave · Pattern Editor — Block 03 "Chorus"</text>')
a('<text x="1262" y="29" font-size="13" fill="#fff" text-anchor="end">EDIT ● · Oct 4 · Step 1 · Instr 05 · 140 BPM · Spd 6 · Follow ✔</text>')
cart(12,56,1256,44,"")
x=26
for t,w,c in [("▶ Play song",104,"#1F5FD1"),("▶ Play block",108,"#1F5FD1"),("■ Stop",76,"#1B2330"),("● Edit (Esc)",104,"#C62828"),("Octave 4 ±",96,"#1F5FD1"),("Step 1 ±",84,"#1F5FD1"),("Follow ✔",86,"#1E8E3E"),("Lines 64",80,"#1B2330")]:
    pill(x,65,w,t,c,True);x+=w+8
cart(12,110,1000,600,"Tracks")
a('<rect x="28" y="140" width="968" height="555" rx="6" fill="url(#crt)" stroke="#3E4652"/>')
chs=["1 Lead","2 Bass","3 Arp","4 Drums","5 Pad"]
cw=180;x0=86
for i,c in enumerate(chs):
    xx=x0+i*cw
    m=i==4
    a(f'<rect x="{xx}" y="146" width="{cw-8}" height="24" rx="6" fill="{"#4A525E" if m else "#516D96"}"/><text x="{xx+8}" y="163" font-size="12" font-weight="bold" fill="#fff">{c}</text><text x="{xx+cw-16}" y="163" font-size="11" font-weight="bold" fill="{"#FF8A80" if m else "#9FC3FF"}" text-anchor="end">{"M" if m else "S M"}</text>')
import random;random.seed(3)
notes=["C-4","D#4","G-4","A#4","C-5","F-4","---"]
cur=8
for r in range(24):
    y=178+r*21
    if r%4==0: a(f'<rect x="32" y="{y}" width="960" height="21" fill="#24477F" opacity="0.55"/>')
    if r==cur: a(f'<rect x="32" y="{y}" width="960" height="21" rx="4" fill="#1F5FD1" opacity="0.55"/>')
    if 12<=r<=15: a(f'<rect x="{x0+cw-4}" y="{y}" width="{cw}" height="21" fill="#22E3FF" opacity="0.18"/>')
    a(f'<text x="40" y="{y+15}" font-size="13" {M} fill="{"#FFD400" if r%4==0 else "#7FA6D9"}">{r:02X}</text>')
    for i in range(5):
        xx=x0+i*cw
        if random.random()<0.45 or (i==3 and r%2==0):
            n=random.choice(notes[:-1]) if i!=3 else ("C-3" if r%4==0 else "F#3")
            ins=f"{random.randint(1,9):02X}"
            vol=f"{random.choice([40,30,20,'..'])}" if True else ""
            fx=random.choice(["....","....","0C20","0300","0008","0F02","1F03"])
            a(f'<text x="{xx+4}" y="{y+15}" font-size="13" {M}><tspan fill="#FFFFFF">{n}</tspan> <tspan fill="#7CFF9B">{ins}</tspan> <tspan fill="#22E3FF">{vol}</tspan> <tspan fill="#FFD400">{fx}</tspan></text>')
        else:
            a(f'<text x="{xx+4}" y="{y+15}" font-size="13" {M} fill="#4F6FA0">--- .. .. ....</text>')
# cursor box on channel 2 note col row cur

# effect autocomplete dropdown at ch1 effect column row 08
cy=178+cur*21
a(f'<rect x="{x0+100}" y="{cy}" width="38" height="21" rx="3" fill="none" stroke="#FFD400" stroke-width="2"/>')
DX,DY,DW=x0+100,cy+26,520
a(f'<rect x="{DX+3}" y="{DY+4}" width="{DW}" height="336" rx="10" fill="#000" opacity="0.35"/>')
a(f'<rect x="{DX}" y="{DY}" width="{DW}" height="336" rx="10" fill="url(#cart)" stroke="#3E4652"/>')
a(f'<rect x="{DX+10}" y="{DY+10}" width="{DW-20}" height="26" rx="13" fill="url(#pill)" stroke="#1F5FD1"/><text x="{DX+22}" y="{DY+28}" font-size="12.5" fill="#1B2330">🔍 <tspan font-weight="bold" fill="#1F5FD1">cut</tspan>▏  · Ch 1 Lead → instr 03 "Saw Lead" chain</text>')
hdr=[("Effect",DX+14),("Now",DX+262),("Range",DX+322),("Inserts",DX+430)]
for t,xx in hdr: a(f'<text x="{xx}" y="{DY+56}" font-size="11.5" font-weight="bold" fill="#3E4652">{t}</text>')
rows=[("SYNTH","Filter cutoff (LP 24)","1.2k","20–20k Hz","Fcxx",1),("SYNTH","Filter resonance","35 %","0–100 %","Frxx",0),("FX","Delay · time","3/16","1/64–1/1","D1xx",0),("FX","Reverb · mix","18 %","0–100 %","D2xx",0),("CLAP","Surge XT · Filter 1 Cutoff","64","0–127","P1 07xx",0),("LV2","Dragonfly · Decay","2.4 s","0.1–10 s","P2 03xx",0),("CMD","Slide up","—","00–FF","01xx",0),("CMD","Set volume","40","00–40","0Cxx",0)]
TC={"SYNTH":"#516D96","FX":"#946B3E","CLAP":"#4F7D78","LV2":"#6B5C82","CMD":"#4A525E"}
for i,(tg,n,v,rg,cd,sel) in enumerate(rows):
    y=DY+66+i*30
    if sel: a(f'<rect x="{DX+8}" y="{y}" width="{DW-16}" height="28" rx="6" fill="#C9D9F5" stroke="#1F5FD1"/>')
    a(f'<rect x="{DX+14}" y="{y+6}" width="46" height="16" rx="8" fill="{TC[tg]}"/><text x="{DX+37}" y="{y+18}" font-size="9.5" font-weight="bold" fill="#fff" text-anchor="middle">{tg}</text>')
    a(f'<text x="{DX+66}" y="{y+19}" font-size="12.5" font-weight="bold" fill="#1B2330">{n}</text><text x="{DX+262}" y="{y+19}" font-size="12.5" font-weight="bold" fill="#1F5FD1">{v}</text><text x="{DX+322}" y="{y+19}" font-size="12" fill="#2C3644">{rg}</text><text x="{DX+430}" y="{y+19}" font-size="12.5" font-weight="bold" {M} fill="#B85500">{cd}</text>')
a(f'<text x="{DX+14}" y="{DY+324}" font-size="11.5" fill="#2C3644">↑↓ choose · Enter insert · Ctrl+Space open · live preview on hover · hex still typeable</text>')

# side help
cart(1024,110,244,600,"Keys","#946B3E")
ks=[("↑ ↓","row"),("← →","column"),("Tab / ⇧Tab","channel"),("PgUp/PgDn","16 rows"),("Home/End","top/bottom"),("Z S X D C","C C# D D# E"),("Q 2 W 3 E","octave +1"),("Num + / −","octave"),("Space","play / stop"),("Enter","play from row"),("Esc","edit on/off"),("⇧ + arrows","select"),("Ctrl C/X/V","copy/cut/paste"),("Ctrl Z / Y","undo / redo"),("Ctrl + / −","transpose ±1"),("Ctrl I","interpolate"),("Ins / ⌫","insert/delete row"),("F9 / F10","mute / solo")]
for i,(k,v) in enumerate(ks):
    y=148+i*30
    a(f'<rect x="1036" y="{y}" width="104" height="24" rx="6" fill="url(#pill)" stroke="#7D8794"/><text x="1088" y="{y+17}" font-size="11.5" font-weight="bold" fill="#1F5FD1" text-anchor="middle">{k}</text><text x="1148" y="{y+17}" font-size="12" fill="#1B2330">{v}</text>')
a(f'<text x="{W/2}" y="742" font-size="12" text-anchor="middle" fill="#fff">Mockup · Workbench Gradient (A2), rounded · columns: note · instr · vol · effect (OctaMED cmd+arg) · GPL-3 ChipWave</text>')
a('</svg>')
open('pattern_editor.svg','w').write("\n".join(o))
