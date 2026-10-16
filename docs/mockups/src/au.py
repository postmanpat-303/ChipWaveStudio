exec(open(__import__('os').path.join(__import__('os').path.dirname(__file__),'common.py')).read())
import math
M='font-family="DejaVu Sans Mono, monospace"'
a('<text x="18" y="29" font-size="19" font-weight="bold" fill="#fff">ChipWave · Arranger — Automation lanes</text>')
a('<text x="1262" y="29" font-size="13" fill="#fff" text-anchor="end">● REC armed · 145 BPM · bar 17</text>')
cart(12,56,1256,44,"")
x=26
for t,w,c in [("✎ Pencil",90,"#1F5FD1"),("⌒ Bezier",90,"#1B2330"),("▁▔ Step",80,"#1B2330"),("╱ Ramp",80,"#1B2330"),("∿ LFO stamp ▾",120,"#1B2330"),("● Record knobs",128,"#9C4F4B"),("＋ Add lane…",110,"#1B2330"),("Snap 1/16",90,"#1B2330")]:
    pill(x,65,w,t,c,True);x+=w+8
cart(12,110,1256,600,"")
X0,X1=200,1250;bars=32;bw=(X1-X0)/bars
a(f'<rect x="{X0}" y="122" width="{X1-X0}" height="20" rx="6" fill="url(#crt)"/>')
for b in range(0,bars,4): a(f'<text x="{X0+b*bw+4}" y="137" font-size="11" {M} fill="#FFD400">{b+1}</text>')
blocks=[("Intro",0,8,"#4F7D78"),("Verse",8,16,"#516D96"),("Build",16,20,"#9C4F4B"),("Drop",20,32,"#946B3E")]
a('<text x="28" y="172" font-size="13" font-weight="bold" fill="#1B2330">Song</text>')
for n,s,e,c in blocks:
    a(f'<rect x="{X0+s*bw+1}" y="150" width="{(e-s)*bw-3}" height="40" rx="6" fill="{c}"/><text x="{X0+s*bw+10}" y="175" font-size="12.5" font-weight="bold" fill="#fff">{n}</text>')
lanes=[("Bass · Filter cutoff","Fc","1.2 kHz","bez"),("Acid · Delay feedback","D1","42 %","step"),("Pad · Gate depth (M1)","M1","80 %","lfo"),("Surge XT · Cutoff","P1 07","64","ramp")]
for i,(n,code,val,kind) in enumerate(lanes):
    y=204+i*120;h=104
    a(f'<rect x="24" y="{y}" width="168" height="{h}" rx="6" fill="url(#pill)" stroke="#7D8794"/>')
    a(f'<text x="34" y="{y+20}" font-size="12" font-weight="bold" fill="#1B2330">{n}</text><text x="34" y="{y+42}" font-size="13" font-weight="bold" fill="#1F5FD1">{val}</text><text x="34" y="{y+62}" font-size="11.5" {M} fill="#946B3E" font-weight="bold">cmd {code}xx</text><text x="34" y="{y+84}" font-size="11" fill="#2C3644">{["Bezier","Step","LFO stamp 1/16","Ramp"][i]} · R W</text>')
    a(f'<rect x="{X0}" y="{y}" width="{X1-X0}" height="{h}" rx="6" fill="url(#crt)" stroke="#3E4652"/>')
    for b in range(0,bars,4): a(f'<line x1="{X0+b*bw}" y1="{y}" x2="{X0+b*bw}" y2="{y+h}" stroke="#24477F"/>')
    pts=[]
    for k in range(0,401):
        t=k/400;xx=X0+t*(X1-X0)
        if kind=="bez": v=0.15+0.75*(1/(1+math.exp(-12*(t-0.55))))-0.25*max(0,t-0.62)*(1 if t<0.64 else 0)
        elif kind=="step": v=[0.3,0.3,0.5,0.5,0.4,0.7,0.7,0.9][int(t*8)%8]
        elif kind=="lfo": v=0.5+0.4*(1 if math.sin(t*2*math.pi*64)>0 else -1)*(0.3 if t<0.5 else 1) if 0.5<=t or True else 0
        else: v=0.2+0.7*min(1,max(0,(t-0.5)/0.12)) if t<0.625 else 0.4
        pts.append((xx,y+h-8-v*(h-16)))
    col={"bez":"#22E3FF","step":"#7CFF9B","lfo":"#FF7AD0","ramp":"#FFD400"}[kind]
    if kind=="lfo":
        a(f'<rect x="{X0+(X1-X0)*0.5}" y="{y+4}" width="{(X1-X0)*0.125}" height="{h-8}" rx="4" fill="#FF7AD0" fill-opacity="0.12" stroke="#FF7AD0" stroke-dasharray="4 3"/>')
    d="M"+" L".join(f"{p[0]:.1f},{p[1]:.1f}" for p in pts)
    a(f'<path d="{d} L{X1},{y+h} L{X0},{y+h} Z" fill="{col}" fill-opacity="0.13"/><path d="{d}" fill="none" stroke="{col}" stroke-width="2.5"/>')
    if kind=="lfo": a(f'<rect x="{X0+(X1-X0)*0.5+4}" y="{y+6}" width="150" height="17" rx="4" fill="#0A1F45"/><text x="{X0+(X1-X0)*0.5+10}" y="{y+19}" font-size="11" fill="#FF7AD0">LFO stamp · square 1/16</text>')
    if kind=="bez":
        for t in (0.2,0.55,0.62,0.9):
            p=pts[int(t*400)];a(f'<rect x="{p[0]-5}" y="{p[1]-5}" width="10" height="10" rx="2" fill="#FFFFFF" stroke="{col}" stroke-width="2"/>')
        p=pts[int(0.55*400)];a(f'<line x1="{p[0]-50}" y1="{p[1]+20}" x2="{p[0]+50}" y2="{p[1]-20}" stroke="#9FC3FF" stroke-dasharray="3 3"/><circle cx="{p[0]-50}" cy="{p[1]+20}" r="4" fill="#9FC3FF"/><circle cx="{p[0]+50}" cy="{p[1]-20}" r="4" fill="#9FC3FF"/>')
        a(f'<rect x="{p[0]+12}" y="{p[1]+6}" width="108" height="22" rx="6" fill="#0A1F45" stroke="#22E3FF"/><text x="{p[0]+20}" y="{p[1]+21}" font-size="11.5" {M} fill="#FFD400">bar 18 · 1.2 kHz</text>')
a(f'<line x1="{X0+16*bw}" y1="146" x2="{X0+16*bw}" y2="684" stroke="#FF7A00" stroke-width="2"/>')
a(f'<text x="{W/2}" y="742" font-size="12" text-anchor="middle" fill="#fff">Mockup · Workbench Gradient (A2), rounded, muted palette · curves = functional colour · GPL-3 ChipWave</text>')
a('</svg>');open('automation.svg','w').write("\n".join(o))
