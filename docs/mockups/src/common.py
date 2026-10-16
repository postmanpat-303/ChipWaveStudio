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
