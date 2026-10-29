#!/usr/bin/env bash
# Rebuild ChipWave-Studio-User-Manual.pdf from parts/*.html + style.css + img/.
# Needs: Google Chrome or Chromium (headless print-to-PDF) and Python 3 with
# PyMuPDF (`pip install pymupdf`, used to fill in the contents page numbers).
# Fonts come from ../../assets/fonts (the app's own). The app embeds the PDF
# (src/app/manual.rs), so rebuild the app after replacing it.
set -euo pipefail
cd "$(dirname "$0")"
CHROME=${CHROME:-$(command -v google-chrome || command -v chromium || command -v chromium-browser)}
OUT=${1:-ChipWave-Studio-User-Manual.pdf}
# The version printed on the cover, the footer and in the text (@VERSION@ in
# parts/*.html and style.css) comes from Cargo.toml, written the way the app
# shows it (crate::VERSION): "0.2.18+fix.1" -> "0.2.18.1".
VERSION=$(sed -nE 's/^version = "([0-9]+\.[0-9]+\.[0-9]+)\+fix\.([0-9]+)"/\1.\2/p' ../../Cargo.toml | head -n1)
[ -n "$VERSION" ] || { echo "no version in ../../Cargo.toml" >&2; exit 1; }
cat parts/00_head.html parts/01.html parts/03.html parts/04.html parts/06.html parts/07.html \
  parts/08.html parts/09.html parts/10.html parts/11.html parts/12.html parts/13.html \
  parts/14.html parts/15.html parts/16.html \
  | sed -e "s/@VERSION@/$VERSION/g" -e 's/href="style.css"/href="style.gen.css"/' > index.html
sed "s/@VERSION@/$VERSION/g" style.css > style.gen.css
render() { "$CHROME" --headless --no-sandbox --no-pdf-header-footer --print-to-pdf=out.pdf "file://$PWD/index.html" 2>/dev/null; }
render
# second pass: put each chapter's real page number into the contents page
python3 - <<'PY'
import re
try:
    import pymupdf
except ImportError:
    import fitz as pymupdf
d = pymupdf.open('out.pdf'); pg = {}
for i, p in enumerate(d):
    t = p.get_text().replace(' ', '')
    for m in re.finditer(r'CHAPTER(\d\d)', t):
        n = int(m.group(1))
        if n not in pg and i > 1:
            pg[n] = i + 1
s = open('index.html').read()
for n, v in pg.items():
    s = re.sub(r'(<span class="pg" data-ch="%d">)[^<]*(</span>)' % n, r'\g<1>%d\g<2>' % v, s)
open('index.html', 'w').write(s)
PY
render
mv out.pdf "$OUT"
rm -f index.html style.gen.css
echo "built $OUT ($VERSION)"
