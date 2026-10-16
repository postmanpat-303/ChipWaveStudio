"""Recolour a Workbench Gradient (A2) mockup SVG into the 'Dark Circuit' theme.
usage: python3 theme_dark_circuit.py in.svg out.svg"""
import re,sys
M={ # gradients
'#9AA5B4':'#24272B','#6F7A8A':'#17191C',          # window bg
'#DFE4EA':'#33373C','#AEB6C1':'#26292D',          # cartridge (slightly lighter)
'#EEF1F4':'#3A3F45','#C6CCD4':'#2C3035',          # pills
'#3F7FE6':'#2A2D31','#1747A8':'#1F2124','#0D2F75':'#4CAF6A', # header band + green rule
'#173A72':'#1A1C1F','#0A1F45':'#121416',          # displays
'#FFFFFF':'#E3E6EA','#C9D0D8':'#2F3338','#7D8794':'#4A5058',
'#3E4652':'#4CAF6A',                              # outer borders -> muted green
'#1B2330':'#D4D8DD','#2C3644':'#AEB4BC','#3F4755':'#C2C7CE','#5B6573':'#9AA1AA',
'#1F5FD1':'#5CC07A','#24477F':'#2B3A30','#C9D9F5':'#2E4A37',
'#516D96':'#3F7A55','#4A525E':'#4A5058','#9FC3FF':'#A9E2BA','#7FA6D9':'#7E8790',
'#4F6FA0':'#4E555D','#FFD400':'#E8C35A','#FF7A00':'#E59A4F','#22E3FF':'#6FC8D6',
'#7CFF9B':'#8FD9A4','#1E8E3E':'#5CC07A','#C62828':'#D9605A','#B85500':'#E59A4F',
'#8F99A6':'#7E8790','#F3F5F7':'#3A3F45','#101418':'#0E1012',
}
s=open(sys.argv[1]).read()
s=re.sub(r'#[0-9A-Fa-f]{6}\b',lambda m:M.get(m.group(0).upper(),m.group(0)),s)
s=s.replace('fill="#fff"','fill="#E3E6EA"').replace('stroke="#fff"','stroke="#4CAF6A"')
s=s.replace('Workbench Gradient (A2)','Dark Circuit (proposed default)')
# soft glow on the outer window edge
w,h=re.search(r'width="(\d+)" height="(\d+)"',s).groups()
s=s.replace('</svg>',f'<rect x="1.5" y="1.5" width="{int(w)-3}" height="{int(h)-3}" rx="10" fill="none" stroke="#4CAF6A" stroke-opacity="0.35" stroke-width="3"/><rect x="1" y="1" width="{int(w)-2}" height="{int(h)-2}" rx="10" fill="none" stroke="#4CAF6A" stroke-width="1"/></svg>')
open(sys.argv[2],'w').write(s)
