OUT='plugin_manager.svg'
W,H=1280,760
o=[]
a=o.append
a(f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" viewBox="0 0 {W} {H}" font-family="DejaVu Sans, sans-serif">')
a('''<defs>
<linearGradient id="bg" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#9AA5B4"/><stop offset="1" stop-color="#6F7A8A"/></linearGradient>
<linearGradient id="cart" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#DFE4EA"/><stop offset="1" stop-color="#AEB6C1"/></linearGradient>
<linearGradient id="pill" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#EEF1F4"/><stop offset="1" stop-color="#C6CCD4"/></linearGradient>
<linearGradient id="hdr" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#3F7FE6"/><stop offset="1" stop-color="#1747A8"/></linearGradient>
<linearGradient id="crt" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#173A72"/><stop offset="1" stop-color="#0A1F45"/></linearGradient>
</defs>''')
a(f'<rect width="{W}" height="{H}" fill="url(#bg)"/>')
a(f'<rect width="{W}" height="44" fill="url(#hdr)"/><rect y="44" width="{W}" height="1" fill="#0D2F75"/>')
a('<text x="18" y="29" font-size="19" font-weight="bold" fill="#fff">ChipWave · Plugin Manager</text>')
a('<text x="1262" y="29" font-size="13" fill="#fff" text-anchor="end">142 found · 131 verified · 4 failed · 2 blocked · 5 new</text>')
CID=[0]
def cart(x,y,w,h,title,spine="#516D96"):
    CID[0]+=1;c=CID[0]
    a(f'<clipPath id="c{c}"><rect x="{x}" y="{y}" width="{w}" height="{h}" rx="8"/></clipPath>')
    a(f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="8" fill="url(#cart)" stroke="#3E4652" stroke-width="1"/>')
    a(f'<rect x="{x+1.5}" y="{y+1.5}" width="{w-3}" height="{h-3}" rx="7" fill="none" stroke="#fff" stroke-opacity="0.7" stroke-width="1.5"/>')
    a(f'<rect x="{x}" y="{y}" width="6" height="{h}" fill="{spine}" clip-path="url(#c{c})"/>')
    a(f'<text x="{x+16}" y="{y+22}" font-size="14" font-weight="bold" fill="#1B2330">{title}</text>')
def pill(x,y,w,t,col="#1B2330",bold=False):
    a(f'<rect x="{x}" y="{y}" width="{w}" height="26" rx="13" fill="url(#pill)" stroke="#7D8794"/>')
    a(f'<text x="{x+w/2}" y="{y+18}" font-size="12.5" text-anchor="middle" fill="{col}" {"font-weight=\"bold\"" if bold else ""}>{t}</text>')
# toolbar
cart(12,56,1256,48,"")
for i,(t,w,c) in enumerate([("⟳ Rescan all",110,"#1F5FD1"),("＋ Add folder…",120,"#1B2330"),("Retry failed (4)",130,"#FF7A00"),("Format: All ▾",112,"#1B2330"),("Show: All ▾",100,"#1B2330")]):
    pass
x=26
for t,w,c in [("⟳ Rescan all",110,"#1F5FD1"),("＋ Add folder…",120,"#1B2330"),("Retry failed (4)",130,"#B85500"),("Format: All ▾",112,"#1B2330"),("Show: All ▾",100,"#1B2330"),("★ Favourites",112,"#1B2330")]:
    pill(x,67,w,t,c,True); x+=w+10
a('<rect x="760" y="67" width="300" height="26" rx="6" fill="url(#crt)" stroke="#3E4652"/><text x="772" y="85" font-size="12.5" fill="#9FC3FF">🔍 search plugins, vendors…</text>')
a('<text x="1252" y="85" font-size="12" text-anchor="end" fill="#1F5FD1" font-weight="bold">Drop .clap / .vst3 / .lv2 here</text>')
# list
cart(12,114,840,600,"Plugins")
cols=[("",30),("Name",200),("Vendor",130),("Format",70),("Type",80),("Status",160),("On",60)]
cx=40;hx=[]
for n,w in cols:
    a(f'<text x="{cx}" y="160" font-size="12" font-weight="bold" fill="#3E4652">{n}</text>');hx.append(cx);cx+=w
a('<rect x="24" y="168" width="816" height="1" fill="#7D8794"/>')
B={"Verified":("#4E7D5B","✔ Verified"),"Failed":("#9C4F4B","✖ Failed"),"Blocked":("#6B5C82","⛔ Blocked"),"New":("#516D96","● New"),"Scanning":("#946B3E","◌ Scanning…")}
rows=[("★","Surge XT","Surge Synth Team","CLAP","Instrument","Verified",1),
("★","Vital","Matt Tytel","VST3","Instrument","Verified",1),
("","Dragonfly Room Reverb","Michael Willis","LV2","Effect","Verified",1),
("","ChowTapeModel","Chowdhury DSP","CLAP","Effect","Verified",0),
("★","Airwindows Consolidated","Airwindows","CLAP","Effect","Verified",1),
("","x42 EQ","x42","LV2","Effect","Verified",1),
("","OldSynth","Unknown","VST3","Instrument","Failed",0),
("","Noisy Dist","Free DSP","LV2","Effect","Failed",0),
("","CrackedThing","?","VST3","Effect","Blocked",0),
("","Odin 2","TheWaveWarden","CLAP","Instrument","New",0),
("","Geonkick","Iurie Nistor","LV2","Instrument","Scanning",0),
("","Cardinal","DISTRHO","VST3","Instrument","New",0)]
y=176
for i,(f,n,v,fm,t,s,on) in enumerate(rows):
    if i==6: a(f'<rect x="24" y="{y}" width="816" height="38" rx="6" fill="#C9D9F5" stroke="#1F5FD1"/>')
    elif i%2: a(f'<rect x="24" y="{y}" width="816" height="38" rx="6" fill="#000" opacity="0.04"/>')
    ty=y+24
    a(f'<text x="{hx[0]}" y="{ty}" font-size="15" fill="{"#B85500" if f else "#7D8794"}">{f or "☆"}</text>')
    a(f'<text x="{hx[1]}" y="{ty}" font-size="13" font-weight="bold" fill="#1B2330">{n}</text>')
    a(f'<text x="{hx[2]}" y="{ty}" font-size="12.5" fill="#2C3644">{v}</text>')
    fc={"CLAP":"#516D96","VST3":"#946B3E","LV2":"#4F7D78"}[fm]
    a(f'<rect x="{hx[3]}" y="{y+10}" width="48" height="19" rx="9.5" fill="{fc}"/><text x="{hx[3]+24}" y="{ty-1}" font-size="11" font-weight="bold" fill="#fff" text-anchor="middle">{fm}</text>')
    a(f'<text x="{hx[4]}" y="{ty}" font-size="12.5" fill="#2C3644">{t}</text>')
    c,l=B[s]
    a(f'<rect x="{hx[5]}" y="{y+8}" width="118" height="23" rx="11" fill="{c}"/><text x="{hx[5]+59}" y="{ty}" font-size="12" font-weight="bold" fill="#fff" text-anchor="middle">{l}</text>')
    tx=hx[6]
    if s in("Verified",):
        a(f'<rect x="{tx}" y="{y+10}" width="40" height="20" rx="10" fill="{"#4E7D5B" if on else "#7D8794"}"/><circle cx="{tx+(30 if on else 10)}" cy="{y+20}" r="8" fill="#fff"/>')
    else:
        a(f'<rect x="{tx}" y="{y+10}" width="40" height="20" rx="10" fill="#C6CCD4" stroke="#7D8794" stroke-dasharray="3 2"/><circle cx="{tx+10}" cy="{y+20}" r="8" fill="#EEF1F4"/>')
    y+=40
a('<text x="40" y="700" font-size="12" fill="#2C3644">Toggle = one click. Only Verified plugins can be switched on; others show why.</text>')
# details
cart(864,114,404,600,"Details · OldSynth","#9C4F4B")
a('<rect x="880" y="148" width="372" height="150" rx="6" fill="url(#crt)" stroke="#3E4652"/>')
lines=[("Verification scan (sandboxed)","#9FC3FF"),("✔ Load bundle               12 ms","#7CFF9B"),("✔ Instantiate @48 kHz/512   40 ms","#7CFF9B"),("✔ Activate                   3 ms","#7CFF9B"),("✖ Process 1 s silence  → NaN out","#FF8A80"),("– Deactivate / unload    skipped","#9FC3FF")]
for i,(t,c) in enumerate(lines):
    a(f'<text x="894" y="{172+i*22}" font-size="12.5" font-family="DejaVu Sans Mono, monospace" fill="{c}">{t}</text>')
kv=[("Reason","Output contained NaN on block 3"),("Path","~/.vst3/OldSynth.vst3"),("Cache key","path + mtime + BLAKE3"),("Scanned","16 Oct 2026, 13:58"),("Scanner","chipwave-scan v1 (child process)")]
for i,(k,v) in enumerate(kv):
    a(f'<text x="880" y="{326+i*24}" font-size="12.5" font-weight="bold" fill="#3E4652">{k}</text><text x="975" y="{326+i*24}" font-size="12.5" fill="{"#C62828" if i==0 else "#1F5FD1"}">{v}</text>')
x=880
for t,w,c in [("↻ Retry",84,"#1F5FD1"),("⛔ Block",84,"#5B2A86"),("Show log",84,"#1B2330"),("Open folder",96,"#1B2330")]:
    pill(x,456,w,t,c,True); x+=w+6
a('<text x="880" y="514" font-size="13" font-weight="bold" fill="#1B2330">Scan folders</text>')
fl=[("✔","~/.clap   /usr/lib/clap"),("✔","~/.vst3   /usr/lib/vst3"),("✔","~/.lv2   /usr/lib/lv2"),("＋","~/Music/MyPlugins  (custom)")]
for i,(m,p) in enumerate(fl):
    a(f'<rect x="880" y="{526+i*32}" width="372" height="26" rx="6" fill="url(#pill)" stroke="#7D8794"/><text x="892" y="{544+i*32}" font-size="12.5" fill="#1E8E3E" font-weight="bold">{m}</text><text x="914" y="{544+i*32}" font-size="12.5" fill="#1B2330">{p}</text>')
a(f'<text x="880" y="{700}" font-size="12" fill="#2C3644">Timeout 10 s per plugin · crash never stops ChipWave</text>')
a(f'<text x="{W/2}" y="748" font-size="12" text-anchor="middle" fill="#fff">Mockup · Workbench Gradient (A2) · GPL-3 ChipWave</text>')
a('</svg>')
open(OUT,'w').write("\n".join(o))
