exec(open(__import__('os').path.join(__import__('os').path.dirname(__file__),'common.py')).read().replace("W,H=1280,760","W,H=1280,760"))
a('<text x="18" y="29" font-size="19" font-weight="bold" fill="#fff">ChipWave · Auto Arrange — Fill to length</text>')
a('<text x="1262" y="29" font-size="13" fill="#fff" text-anchor="end">Target 3:30 · 140 BPM · 4/4 · result 3:29.1 (122 bars)</text>')
# settings cart
cart(12,56,330,330,"Fill settings")
kv=[("Target length","3:30"),("Snap","to 8 bars"),("BPM","140"),("Style","Chiptune demo-scene ▾"),("Seed","#4F2A · 🎲 Reroll"),("Variation","62 %")]
for i,(k,v) in enumerate(kv):
    yy=92+i*36
    a(f'<text x="28" y="{yy+17}" font-size="12.5" font-weight="bold" fill="#3E4652">{k}</text>')
    a(f'<rect x="140" y="{yy}" width="188" height="26" rx="13" fill="url(#pill)" stroke="#7D8794"/><text x="152" y="{yy+18}" font-size="12.5" font-weight="bold" fill="#1F5FD1">{v}</text>')
a('<text x="28" y="330" font-size="12.5" font-weight="bold" fill="#3E4652">Your tagged blocks</text>')
R={"Intro":"#4F7D78","Verse":"#516D96","Chorus":"#946B3E","Bridge":"#6B5C82","Outro":"#4A525E","Fill":"#9C4F4B"}
x=28
for r in ["Intro","Verse","Chorus","Bridge","Outro"]:
    w=len(r)*7+14
    a(f'<rect x="{x}" y="342" width="{w}" height="22" rx="11" fill="{R[r]}"/><text x="{x+w/2}" y="357" font-size="11.5" font-weight="bold" fill="#fff" text-anchor="middle">{r}</text>'); x+=w+4
# variations cart
cart(354,56,914,330,"Energy curve (drag points) · variations")
a('<rect x="372" y="88" width="878" height="170" rx="6" fill="url(#crt)" stroke="#3E4652"/>')
for gx in range(372,1250,44): a(f'<line x1="{gx}" y1="88" x2="{gx}" y2="258" stroke="#24477F"/>')
pts=[(372,230),(470,200),(600,150),(720,120),(800,190),(900,110),(1040,95),(1150,140),(1250,235)]
a('<polyline fill="none" stroke="#22E3FF" stroke-width="3" points="'+" ".join(f"{p[0]},{p[1]}" for p in pts)+'"/>')
for p in pts[1:-1]: a(f'<rect x="{p[0]-5}" y="{p[1]-5}" width="10" height="10" rx="2" fill="#FFD400"/>')
a('<text x="384" y="106" font-size="12" fill="#FFD400" font-weight="bold">Energy</text><text x="905" y="102" font-size="12" fill="#FFD400">key +2 →</text>')
tr=["✔ Transpose","✔ Drum fill last bar","✔ Thin channels","✔ Arp / inversion","☐ Swing","✔ Density ±","✔ Filter sweep","✔ Echo (8xx)","☐ Call &amp; response","✔ Key change","✔ Outro fade"]
x,y=372,270
for t in tr:
    w=len(t)*7.2+20
    if x+w>1250: x=372;y+=34
    on=t[0]=="✔"
    a(f'<rect x="{x}" y="{y}" width="{w}" height="26" rx="13" fill="{"#516D96" if on else "url(#pill)"}" stroke="#3E4652"/><text x="{x+w/2}" y="{y+18}" font-size="12" font-weight="bold" fill="{"#fff" if on else "#1B2330"}" text-anchor="middle">{t}</text>')
    x+=w+8
# arranger
cart(12,398,1256,300,"Arranger — preview (ghost blocks = proposed)")
a('<rect x="28" y="430" width="1224" height="22" rx="6" fill="url(#crt)"/>')
for i,t in enumerate(["0:00","0:30","1:00","1:30","2:00","2:30","3:00","3:30"]):
    a(f'<text x="{34+i*168}" y="446" font-size="11.5" fill="#FFD400" font-family="DejaVu Sans Mono, monospace">{t}</text>')
seq=[("Intro",8,0,0),("Verse",16,0,0),("Chorus",16,0,0),("Verse′",16,1,0),("Chorus′",16,1,0),("Bridge",8,0,1),("Fill",4,1,0),("Chorus +2",16,1,0),("Chorus +2′",14,1,0),("Outro fade",8,1,0)]
sc=1200/122;x=28
for n,b,ghost,lock in seq:
    w=b*sc-3;r=n.split()[0].rstrip("′")
    c=R.get(r,"#B85500")
    if ghost:
        a(f'<rect x="{x}" y="466" width="{w}" height="70" rx="6" fill="{c}" fill-opacity="0.35" stroke="{c}" stroke-width="2" stroke-dasharray="6 3"/>')
        a(f'<text x="{x+8}" y="490" font-size="12.5" font-weight="bold" fill="#1B2330">{n}</text>{"" if w<60 else f"<text x=\"{x+8}\" y=\"508\" font-size=\"11\" fill=\"#1B2330\">{b} bars · derived</text>"}')
    else:
        a(f'<rect x="{x}" y="466" width="{w}" height="70" rx="6" fill="{c}" stroke="#1B2330"/>')
        a(f'<text x="{x+8}" y="490" font-size="12.5" font-weight="bold" fill="#fff">{n}{" 🔒" if lock else ""}</text><text x="{x+8}" y="508" font-size="11" fill="#fff">{b} bars</text>')
    x+=b*sc
a('<line x1="300" y1="460" x2="300" y2="545" stroke="#C62828" stroke-width="2"/>')
a('<text x="28" y="566" font-size="12.5" fill="#1B2330">Selected: <tspan font-weight="bold" fill="#1F5FD1">Chorus′</tspan> = Chorus + drum fill bar 16 + filter sweep 400→2.4 kHz + echo on ch 3</text>')
x=28
for t,w,c in [("▶ Preview",100,"#1F5FD1"),("🎲 Reroll",96,"#1F5FD1"),("🔒 Lock section",130,"#1B2330"),("✔ Accept",96,"#1E8E3E"),("↶ Undo",84,"#1B2330"),("⬇ Export WAV/MP3",150,"#B85500")]:
    pill(x,590,w,t,c,True); x+=w+10
a('<text x="28" y="650" font-size="12" fill="#2C3644">Ghost blocks are non-destructive: originals untouched until Accept. Same seed + settings = same result.</text>')
a(f'<text x="{W/2}" y="740" font-size="12" text-anchor="middle" fill="#fff">Mockup · Workbench Gradient (A2), rounded · GPL-3 ChipWave</text>')
a('</svg>')
open('auto_arrange.svg','w').write("\n".join(o))
