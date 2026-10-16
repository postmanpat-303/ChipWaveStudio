exec(open(__import__('os').path.join(__import__('os').path.dirname(__file__),'common.py')).read())
M='font-family="DejaVu Sans Mono, monospace"'
a('<text x="18" y="29" font-size="19" font-weight="bold" fill="#fff">ChipWave · Mixer</text>')
a('<text x="1262" y="29" font-size="13" fill="#fff" text-anchor="end">Master −0.3 dBTP · −9.2 LUFS-I · −7.8 LUFS-S</text>')
chs=[("Kick","#4F7D78"),("Bass","#516D96"),("Hats","#4A525E"),("Acid","#946B3E"),("Lead","#516D96"),("Pad","#6B5C82"),("FX","#4A525E"),("Alien","#6B5C82")]
import math
for i,(n,c) in enumerate(chs):
    x=12+i*132;w=124
    cart(x,56,w,640,f"{i+1} {n}",c)
    # inserts
    for j,t in enumerate(["Dist","Comp","—"] if i in(0,3) else ["EQ","—","—"]):
        a(f'<rect x="{x+14}" y="{90+j*26}" width="{w-24}" height="22" rx="6" fill="url(#pill)" stroke="#7D8794"/><text x="{x+22}" y="{105+j*26}" font-size="11.5" fill="{"#7D8794" if t=="—" else "#1B2330"}" font-weight="bold">{t}</text>')
    # sends knobs
    for j,(t,vv) in enumerate([("Dly",0.2+0.08*i),("Rev",0.5-0.05*i)]):
        cx=x+38+j*48;cy=200
        a(f'<circle cx="{cx}" cy="{cy}" r="16" fill="url(#pill)" stroke="#3E4652"/>')
        ang=math.radians(-225+270*vv);a(f'<line x1="{cx}" y1="{cy}" x2="{cx+12*math.cos(ang)}" y2="{cy+12*math.sin(ang)}" stroke="#1F5FD1" stroke-width="3" stroke-linecap="round"/>')
        a(f'<text x="{cx}" y="{cy+30}" font-size="11" text-anchor="middle" fill="#3E4652" font-weight="bold">{t}</text><text x="{cx}" y="{cy+44}" font-size="11" text-anchor="middle" fill="#1F5FD1" font-weight="bold">{int(vv*100)}%</text>')
    # pan
    a(f'<rect x="{x+14}" y="262" width="{w-24}" height="10" rx="5" fill="url(#crt)"/>')
    pv=[0,0,-0.3,0.2,0.1,-0.1,0.5,-0.6][i];px=x+14+(w-24)*(0.5+pv/2)
    a(f'<rect x="{px-5}" y="258" width="10" height="18" rx="3" fill="#EEF1F4" stroke="#3E4652"/><text x="{x+w/2+3}" y="292" font-size="11" text-anchor="middle" fill="#1F5FD1" font-weight="bold">{"C" if pv==0 else ("L" if pv<0 else "R")+str(int(abs(pv)*100))}</text>')
    # meter display + fader
    a(f'<rect x="{x+14}" y="302" width="{w-24}" height="300" rx="6" fill="url(#crt)" stroke="#3E4652"/>')
    lv=[0.85,0.78,0.55,0.62,0.7,0.5,0.3,0.45][i]
    for k,(off,col) in enumerate([(0,"#22E3FF"),(14,"#7CFF9B")]):
        h=280*lv*(1 if k==0 else 0.82)
        a(f'<rect x="{x+24+off}" y="{592-h}" width="10" height="{h}" rx="2" fill="{col}"/>')
    a(f'<line x1="{x+22}" y1="{592-280*min(lv+0.06,0.97)}" x2="{x+50}" y2="{592-280*min(lv+0.06,0.97)}" stroke="#FFD400" stroke-width="2"/>')
    a(f'<rect x="{x+80}" y="312" width="6" height="280" rx="3" fill="#0A1F45" stroke="#24477F"/>')
    fy=592-280*[0.75,0.72,0.6,0.65,0.7,0.55,0.5,0.6][i]
    a(f'<rect x="{x+68}" y="{fy-9}" width="30" height="18" rx="4" fill="url(#pill)" stroke="#3E4652"/>')
    db=[ -1.5,-2.1,-6.0,-4.4,-3.0,-7.5,-9.0,-6.0][i]
    a(f'<text x="{x+w/2+3}" y="626" font-size="12.5" text-anchor="middle" {M} font-weight="bold" fill="#1F5FD1">{db:+.1f} dB</text>')
    ms,so=(i==6),(False)
    a(f'<rect x="{x+14}" y="642" width="46" height="26" rx="13" fill="{"#9C4F4B" if ms else "url(#pill)"}" stroke="#3E4652"/><text x="{x+37}" y="660" font-size="12" font-weight="bold" text-anchor="middle" fill="{"#fff" if ms else "#1B2330"}">M</text>')
    a(f'<rect x="{x+66}" y="642" width="46" height="26" rx="13" fill="url(#pill)" stroke="#3E4652"/><text x="{x+89}" y="660" font-size="12" font-weight="bold" text-anchor="middle" fill="#1B2330">S</text>')
# returns + master
x=1068;cart(x,56,200,640,"Master","#4A525E")
for j,(t,v) in enumerate([("Return A · Delay","1/8 · fb 40%"),("Return B · Reverb","4.0 s · 30%")]):
    a(f'<rect x="{x+14}" y="{90+j*52}" width="174" height="44" rx="6" fill="url(#pill)" stroke="#7D8794"/><text x="{x+24}" y="{108+j*52}" font-size="12" font-weight="bold" fill="#1B2330">{t}</text><text x="{x+24}" y="{126+j*52}" font-size="12" font-weight="bold" fill="#1F5FD1">{v}</text>')
a(f'<rect x="{x+14}" y="200" width="174" height="120" rx="6" fill="url(#crt)" stroke="#3E4652"/>')
for j,(k,v) in enumerate([("Limiter","ON"),("Ceiling","−0.3 dBTP"),("Gain red.","−2.1 dB"),("LUFS-I","−9.2"),("LUFS-S","−7.8")]):
    a(f'<text x="{x+24}" y="{222+j*21}" font-size="12" fill="#9FC3FF">{k}</text><text x="{x+178}" y="{222+j*21}" font-size="12.5" {M} font-weight="bold" fill="#FFD400" text-anchor="end">{v}</text>')
a(f'<rect x="{x+14}" y="332" width="174" height="270" rx="6" fill="url(#crt)" stroke="#3E4652"/>')
for k,lv in enumerate([0.9,0.88]):
    a(f'<rect x="{x+40+k*30}" y="{592-250*lv}" width="16" height="{250*lv}" rx="2" fill="{"#22E3FF" if k==0 else "#7CFF9B"}"/>')
for k,d in enumerate([0,-6,-12,-24,-48]):
    yy=342+k*58;a(f'<text x="{x+130}" y="{yy+4}" font-size="11" {M} fill="#9FC3FF">{d}</text><line x1="{x+100}" y1="{yy}" x2="{x+124}" y2="{yy}" stroke="#24477F"/>')
a(f'<text x="{x+101}" y="626" font-size="12.5" text-anchor="middle" {M} font-weight="bold" fill="#1F5FD1">0.0 dB</text>')
a(f'<text x="{x+101}" y="660" font-size="11.5" text-anchor="middle" fill="#2C3644">Peak · RMS · hold</text>')
a(f'<text x="{W/2}" y="742" font-size="12" text-anchor="middle" fill="#fff">Mockup · Workbench Gradient (A2), rounded, muted palette · GPL-3 ChipWave</text>')
a('</svg>');open('mixer.svg','w').write("\n".join(o))
