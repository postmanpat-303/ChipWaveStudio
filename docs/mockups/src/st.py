exec(open(__import__('os').path.join(__import__('os').path.dirname(__file__),'common.py')).read())
a('<text x="18" y="29" font-size="19" font-weight="bold" fill="#fff">ChipWave · Settings</text>')
a('<rect x="900" y="10" width="360" height="26" rx="13" fill="url(#crt)"/><text x="914" y="28" font-size="12.5" fill="#9FC3FF">🔍 search settings…</text>')
cards=[("Audio","#516D96",[("Device","PipeWire · Scarlett 2i2"),("Rate / buffer","48 kHz · 256"),("Latency","5.3 ms"),("Status","✔ running")]),
("MIDI","#4F7D78",[("Input","Arturia KeyStep"),("Clock","Send · 145 BPM"),("Learned","12 mappings"),("Status","✔ 1 in · 1 out")]),
("Look","#6B5C82",[("Theme","Workbench Gradient"),("Scale","125 % (auto)"),("Font","JetBrains Mono 14"),("Motion","normal")]),
("Keys","#946B3E",[("Preset","ChipWave"),("Piano","FT2 · base oct 4"),("Overrides","3"),("Conflicts","none")]),
("Plugins","#4E7D5B",[("Found","142"),("Verified","131"),("Failed","4 · retry"),("Folders","4")]),
("Files and Safety","#4A525E",[("Autosave","every 2 min"),("Backups","5 rotating"),("Last save","14:16"),("Recovery","nothing pending")])]
for i,(n,c,kv) in enumerate(cards):
    x=12+(i%3)*420;y=56+(i//3)*170
    cart(x,y,408,158,n,c)
    for j,(k,v) in enumerate(kv):
        yy=y+48+j*26
        a(f'<text x="{x+20}" y="{yy}" font-size="12.5" fill="#3E4652" font-weight="bold">{k}</text><text x="{x+392}" y="{yy}" font-size="12.5" font-weight="bold" fill="{"#2F6B3F" if v.startswith("✔") else "#1F5FD1"}" text-anchor="end">{v}</text>')
    a(f'<text x="{x+392}" y="{y+22}" font-size="12" font-weight="bold" fill="#1F5FD1" text-anchor="end">{"▾ open" if i==0 else "▸ details"}</text>')
# expanded audio advanced
cart(12,402,1256,300,"Audio · details","#516D96")
rows=[("Host / API","PipeWire (ALSA/JACK) ▾"),("Output device","Scarlett 2i2 USB ▾"),("Input device","none ▾"),("Sample rate","48000 Hz ▾"),("Buffer size","256 frames ▾"),("Channels out","2 (stereo)")]
for j,(k,v) in enumerate(rows):
    yy=436+j*38
    a(f'<text x="30" y="{yy+17}" font-size="12.5" font-weight="bold" fill="#3E4652">{k}</text><rect x="200" y="{yy}" width="260" height="26" rx="13" fill="url(#pill)" stroke="#7D8794"/><text x="214" y="{yy+18}" font-size="12.5" font-weight="bold" fill="#1F5FD1">{v}</text>')
a('<text x="500" y="442" font-size="13" font-weight="bold" fill="#1B2330">▾ Advanced</text>')
adv=[("☑ Exclusive mode (WASAPI)","Windows only"),("☐ Use ASIO","needs ASIO driver"),("☑ Dither on 16-bit export","TPDF"),("☐ Multi-thread engine","experimental"),("☑ Stop audio when unfocused","off by default on Linux")]
for j,(k,v) in enumerate(adv):
    yy=460+j*32
    a(f'<text x="500" y="{yy+16}" font-size="12.5" fill="#1B2330">{k}</text><text x="780" y="{yy+16}" font-size="12" fill="#2C3644">{v}</text>')
a('<rect x="960" y="436" width="290" height="140" rx="6" fill="url(#crt)" stroke="#3E4652"/>')
for j,(k,v) in enumerate([("Round-trip","10.7 ms"),("CPU (engine)","7 %"),("XRuns (session)","0"),("Test tone","▶ 440 Hz")]):
    a(f'<text x="974" y="{462+j*28}" font-size="12.5" fill="#9FC3FF">{k}</text><text x="1238" y="{462+j*28}" font-size="12.5" font-weight="bold" fill="#FFD400" text-anchor="end">{v}</text>')
x=960
for t,w,c in [("Apply",80,"#2F6B3F"),("Reset section",120,"#1B2330")]:
    pill(x,600,w,t,c,True);x+=w+10
a('<text x="30" y="690" font-size="12" fill="#2C3644">Cards show the important values at a glance; ▸ opens the full section. Safety details: docs/SAFETY.md</text>')
a(f'<text x="{W/2}" y="742" font-size="12" text-anchor="middle" fill="#fff">Mockup · Workbench Gradient (A2), rounded, muted palette · GPL-3 ChipWave</text>')
a('</svg>');open('settings.svg','w').write("\n".join(o))
