#!/usr/bin/env python3
"""Write THIRD_PARTY_LICENSES.txt for a release build: every crate linked into
the Windows exe, its licence expression, and the licence files it ships.
Usage: python3 scripts/third_party_licenses.py [target] > THIRD_PARTY_LICENSES.txt"""
import glob, json, os, subprocess, sys

target = sys.argv[1] if len(sys.argv) > 1 else "x86_64-pc-windows-gnu"
meta = json.loads(subprocess.check_output(
    ["cargo", "metadata", "--format-version", "1", "--filter-platform", target]))
nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
root = meta["resolve"]["root"]
# normal (non-dev, non-build) dependency closure of the app
keep, todo = set(), [root]
while todo:
    i = todo.pop()
    if i in keep:
        continue
    keep.add(i)
    for d in nodes[i]["deps"]:
        if any(k["kind"] is None for k in d["dep_kinds"]):
            todo.append(d["pkg"])
pkgs = sorted((p for p in meta["packages"] if p["id"] in keep and p["id"] != root),
              key=lambda p: (p["name"], p["version"]))
CREDITS = """Acknowledgements / Credits
-------------------------
The Voice (glitch) instrument (src/voice.rs) is an independent implementation
whose design is inspired by the "naive speech synth" of Mutable Instruments
Plaits by Emilie Gillet: a pulse source into zero-delay state-variable
formant filters with a vowel x register morph table. No Plaits source code
or table data is copied; Plaits' MIT notice is reproduced below in
acknowledgement. https://github.com/pichenettes/eurorack/tree/master/plaits/dsp/speech

  Copyright 2016 Emilie Gillet.

  Author: Emilie Gillet (emilie.o.gillet@gmail.com)

  Permission is hereby granted, free of charge, to any person obtaining a copy
  of this software and associated documentation files (the "Software"), to deal
  in the Software without restriction, including without limitation the rights
  to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
  copies of the Software, and to permit persons to whom the Software is
  furnished to do so, subject to the following conditions:

  The above copyright notice and this permission notice shall be included in
  all copies or substantial portions of the Software.

  THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
  IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
  FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
  AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
  LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
  OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
  THE SOFTWARE.

  See http://creativecommons.org/licenses/MIT/ for more information.

Data and literature the Voice instrument uses (facts and published
measurements, credited with thanks):
- Csound Manual, Appendix "Formant values" (bass/tenor/alto/soprano a e i o u),
  the Csound community: https://csound.com/docs/manual/MiscFormants.html
  (Csound is LGPL-2.1: https://github.com/csound/csound)
- G. E. Peterson & H. L. Barney (1952), "Control methods used in a study of
  the vowels", JASA 24(2) (ER and AE formants)
- A. E. Rosenberg (1971), "Effect of glottal pulse shape on the quality of
  natural vowels", JASA 49(2) (glottal pulse)
- G. Fant, J. Liljencrants & Q. Lin (1985), "A four-parameter model of glottal
  flow", STL-QPSR (the LF model; the "tension" control)
- D. H. Klatt (1980), "Software for a cascade/parallel formant synthesizer",
  JASA 67(3) (parallel formant structure)
Research notes: docs/research/voice-glitch.md

Master limiter and loudness meter (src/master.rs), independent
implementations of published methods, credited with thanks:
- ITU-R BS.1770-4 (2015), "Algorithms to measure audio programme loudness and
  true-peak audio level": K-weighting, gating, 4x oversampled true peak.
  https://www.itu.int/rec/R-REC-BS.1770
- EBU R 128 and EBU Tech 3341 (momentary / short-term / integrated loudness).
  https://tech.ebu.ch/publications/r128
- K-weighting biquad formulas for any sample rate, as published in
  libebur128 by Jan Kokemueller (MIT) https://github.com/jiixyj/libebur128
  and pyloudnorm by Christian Steinmetz (MIT)
  https://github.com/csteinmetz1/pyloudnorm (no code copied)
- Lookahead limiter with peak hold + moving-average gain smoothing, after
  Geraint Luff, "Designing a straightforward limiter" (Signalsmith Audio).
  https://signalsmith-audio.co.uk/writing/2022/limiter/
- Glide and accent (effects 3C / 3D) follow the behaviour of the Roland
  TB-303 Bass Line (1981); no Roland material is used.

Voice consonants and Throat mode (src/speech.rs, 0.2.3.10):
- Throat mode is inspired by Pink Trombone by Neil Thapen (2017),
  https://dood.al/pinktrombone/ , released under the MIT licence. ChipWave's
  tract is an independent Rust implementation of the structure Pink Trombone
  describes (a 44-section Kelly-Lochbaum waveguide with a tongue-shaped
  diameter profile, lips, a 28-section nasal branch behind a velum, and
  turbulence noise at narrow constrictions); no Pink Trombone code is
  copied. With thanks, its licence:

  MIT License

  Copyright (c) 2017 Neil Thapen

  Permission is hereby granted, free of charge, to any person obtaining a
  copy of this software and associated documentation files (the
  "Software"), to deal in the Software without restriction, including
  without limitation the rights to use, copy, modify, merge, publish,
  distribute, sublicense, and/or sell copies of the Software, and to permit
  persons to whom the Software is furnished to do so, subject to the
  following conditions:

  The above copyright notice and this permission notice shall be included
  in all copies or substantial portions of the Software.

  THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
  OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
  MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN
  NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
  DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
  OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE
  USE OR OTHER DEALINGS IN THE SOFTWARE.

- The tube model: J. L. Kelly & C. C. Lochbaum (1962), "Speech synthesis",
  Proc. 4th International Congress on Acoustics.
- Consonant formant loci: P. C. Delattre, A. M. Liberman & F. S. Cooper
  (1955), "Acoustic loci and transitional cues for consonants", JASA 27(4).
  https://doi.org/10.1121/1.1908024
- Frication / burst noise bands: K. N. Stevens (1998), "Acoustic
  Phonetics", MIT Press.

Voice Transform (src/transform.rs, 0.2.3.11), independent implementations:
- Metallic resonance: two-pole modal resonators in the constant-peak-gain
  form of J. O. Smith & J. B. Angell (1982), "A constant-gain digital
  resonator tuned by a single coefficient", Computer Music Journal 6(4);
  see also J. O. Smith, "Introduction to Digital Filters",
  https://ccrma.stanford.edu/~jos/filters/
- Mode ratios: church-bell partials and free-free bar modes after
  N. H. Fletcher & T. D. Rossing, "The Physics of Musical Instruments"
  (Springer, 1998); the "metal" ratios are the six square-wave oscillator
  frequencies of the Roland TR-808 cymbal / hi-hat (no Roland material used).
- Sub crossover: Linkwitz-Riley filters, S. H. Linkwitz (1976), "Active
  crossover networks for noncoincident drivers", JAES 24(1); the side-only
  high-pass ("mono maker") follows the elliptical EQ of vinyl mastering.
- Octave divider: a flip-flop on the zero crossings, the principle of analog
  octave pedals such as the Boss OC-2 (1982); no circuit or code copied.

Generators (src/gen.rs, src/ladder.rs, 0.2.3.12):
- Moog ladder filter: the zero-delay-feedback (TPT) ladder of Vadim
  Zavalishin, "The Art of VA Filter Design" (Native Instruments, 2012-2018);
  stage saturation after A. Huovilainen, "Non-linear digital implementation
  of the Moog ladder filter", DAFx-04 (2004), and V. Valimaki &
  A. Huovilainen, "Oscillator and filter algorithms for virtual analog
  synthesis", Computer Music Journal 30(2) (2006); 12/24 dB LP/BP/HP pole
  mixing as in those papers. Cross-checked in the tests against the DaisySP
  ladder (Richard van Hoesel / Infrasonic Audio, MIT, notice below), the
  fundsp Moog filter (Sami Perttu, MIT / Apache-2.0,
  https://github.com/SamiPerttu/fundsp) and the MoogLadders collection by
  Dimitri Diakopoulos (https://github.com/ddiakopoulos/MoogLadders; the
  Improved model by Stefano D'Angelo, ISC). The DaisySP and fundsp ladders
  are ported only inside the test that compares against them; the 0.5 k
  passband compensation follows DaisySP.
- Drum Synth recipes: the Sonic Pi sc808 SynthDefs (Sonic Pi, Sam Aaron and
  contributors, MIT, https://github.com/sonic-pi-net/sonic-pi), based on
  Yoshinosuke Horiuchi's free SuperCollider TR-808; and the analog / synthetic
  bass drum, snare and hi-hat models of Mutable Instruments Plaits by
  Emilie Gillet (MIT, https://github.com/pichenettes/eurorack), as ported to
  DaisySP by Electrosmith (MIT, https://github.com/electro-smith/DaisySP).
  Independent Rust implementations of those recipes; no samples used. The
  macro-knob + dice workflow is inspired by Bitwig Studio's Grid and drum
  devices (ideas only, no code or assets).
- Analog, FM-4 and Noise generators (src/gen2.rs, 0.2.3.14) are written from
  scratch for ChipWave; no code from the projects below is copied. They were
  informed by, and ChipWave gratefully credits:
  * the Minimoog Model D (Robert Moog / Moog Music): the 3-oscillator +
    noise + ladder + two-envelope voice and its glide;
  * V. Valimaki & A. Huovilainen, "Antialiasing oscillators in subtractive
    synthesis", IEEE Signal Processing Magazine 24(2), 2007 (polyBLEP);
    M. Finke's polyBLEP notes;
  * Vital by Matt Tytel, GPL-3.0, https://github.com/mtytel/vital (oscillator
    drift and macro/random workflow ideas);
  * Surge XT by the Surge Synth Team, GPL-3.0,
    https://github.com/surge-synthesizer/surge (analog-style drift, noise
    colours);
  * Dexed by Pascal Gauthier, GPL-3.0, https://github.com/asb2m10/dexed , and
    the Music Synthesizer for Android (MSFA) by Raph Levien / Google,
    Apache-2.0, https://github.com/google/music-synthesizer-for-android
    (operator feedback averaging, envelope behaviour); the 8 four-operator
    algorithms follow the Yamaha DX21 / DX100 / TX81Z;
  * Mutable Instruments Plaits by Emilie Gillet (MIT) and DaisySP by
    Electrosmith (MIT) (FM and noise models; DaisySP's Dust);
  * SuperCollider's Dust UGen (GPL-3.0, https://github.com/supercollider/supercollider);
  * Paul Kellet's "pink noise filter" (musicdsp.org, public domain).
  GPL-3.0 and Apache-2.0 sources were read as references only; ChipWave is
  itself distributed under the GPL-3.0, which is compatible with both.
- Chip FX (src/chipfx.rs, 0.2.3.13) is a port of sfxr by Tomas Pettersson
  (DrPetter), https://www.drpetter.se/project_sfxr.html , by way of its Rust
  reimplementation sfxr-rs by Teemu Erkkola, https://github.com/bzar/sfxr-rs
  (synthesis, category recipes, Randomize and Mutate). Both MIT:

  sfxr: Copyright (c) 2007 Tomas Pettersson
  sfxr-rs: Copyright (c) 2018 Teemu Erkkola

  Permission is hereby granted, free of charge, to any person obtaining a
  copy of this software and associated documentation files (the
  "Software"), to deal in the Software without restriction, including
  without limitation the rights to use, copy, modify, merge, publish,
  distribute, sublicense, and/or sell copies of the Software, and to permit
  persons to whom the Software is furnished to do so, subject to the
  following conditions:

  The above copyright notice and this permission notice shall be included
  in all copies or substantial portions of the Software.

  THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
  OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
  MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN
  NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
  DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
  OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE
  USE OR OTHER DEALINGS IN THE SOFTWARE.

  DaisySP ladder (src/Filters/ladder.cpp), ported in a test:

  Copyright (c) 2021, Richard van Hoesel
  Copyright (c) 2024, Infrasonic Audio LLC

  Permission is hereby granted, free of charge, to any person obtaining a
  copy of this software and associated documentation files (the
  "Software"), to deal in the Software without restriction, including
  without limitation the rights to use, copy, modify, merge, publish,
  distribute, sublicense, and/or sell copies of the Software, and to permit
  persons to whom the Software is furnished to do so, subject to the
  following conditions:

  The above copyright notice, development funding notice, and this
  permission notice shall be included in all copies or substantial portions
  of the Software.

  THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
  OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
  MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN
  NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
  DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
  OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE
  USE OR OTHER DEALINGS IN THE SOFTWARE.

  fundsp (moog.rs), ported in a test: Copyright (c) Sami Perttu, MIT
  licence (same text as below).

  Sonic Pi sc808 (etc/synthdefs/designs/supercollider/sc808.scd):
  Copyright (c) Sam Aaron and the Sonic Pi contributors, MIT:

  Permission is hereby granted, free of charge, to any person obtaining a
  copy of this software and associated documentation files (the
  "Software"), to deal in the Software without restriction, including
  without limitation the rights to use, copy, modify, merge, publish,
  distribute, sublicense, and/or sell copies of the Software, and to permit
  persons to whom the Software is furnished to do so, subject to the
  following conditions:

  The above copyright notice and this permission notice shall be included
  in all copies or substantial portions of the Software.

  THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
  OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
  MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN
  NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
  DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
  OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE
  USE OR OTHER DEALINGS IN THE SOFTWARE.

  DaisySP (Electrosmith) and Plaits (Copyright Emilie Gillet) are MIT
  licensed with the same text.

Chord helper and key detection (src/chords.rs), independent implementations:
- Key finding: the Krumhansl-Schmuckler algorithm, as described in
  C. L. Krumhansl (1990), "Cognitive Foundations of Musical Pitch", Oxford
  University Press, using the major / minor key profiles of C. L. Krumhansl &
  E. J. Kessler (1982), "Tracing the dynamic changes in perceived tonal
  organization in a spatial representation of musical keys", Psychological
  Review 89(4), 334-368. https://doi.org/10.1037/0033-295X.89.4.334
- The chord pad / progression workflow is inspired by Scaler by Plugin
  Boutique (https://www.pluginboutique.com); no code, data or artwork from
  Scaler is used. Scales and chord names are standard music theory.

Plugin instrument hosting (src/plugin/, 0.2.3.16):
- CLAP, the CLever Audio Plug-in API by the free-audio project
  (Alexandre Bique and contributors), MIT licence.
  https://github.com/free-audio/clap . ChipWave talks to CLAP plugins
  through the clap-sys crate (Rust bindings of the CLAP headers, by
  micahrj and contributors, MIT OR Apache-2.0,
  https://github.com/micahrj/clap-sys ), listed with the crates below.
  With thanks, the CLAP licence:

  MIT License

  Copyright (c) 2021 Alexandre BIQUE

  Permission is hereby granted, free of charge, to any person obtaining a
  copy of this software and associated documentation files (the
  "Software"), to deal in the Software without restriction, including
  without limitation the rights to use, copy, modify, merge, publish,
  distribute, sublicense, and/or sell copies of the Software, and to permit
  persons to whom the Software is furnished to do so, subject to the
  following conditions:

  The above copyright notice and this permission notice shall be included
  in all copies or substantial portions of the Software.

  THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
  OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
  MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN
  NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
  DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
  OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE
  USE OR OTHER DEALINGS IN THE SOFTWARE.

- Host design references (no code copied): the clap-host example and
  clap-helpers of the free-audio project (MIT), clack by Adrien Prokopowicz
  (prokopyl) and contributors (MIT OR Apache-2.0,
  https://github.com/prokopyl/clack ), and the plugin hosts of Carla by
  Filipe Coelho (falkTX, GPL-2.0+, https://github.com/falkTX/Carla ) and
  Ardour (GPL-2.0+, https://ardour.org ): out-of-process plugin scanning,
  main-thread / audio-thread rules, sample-accurate event blocks.
- Dynamic loading: libloading by Simonas Kazlauskas (ISC); Win32 editor
  windows: windows-sys by Microsoft (MIT OR Apache-2.0); plugin state in
  songs: base64 by Marshall Pierce and Alice Maz (MIT OR Apache-2.0).
- Plugins tested with ChipWave: Surge XT by the Surge Synth Team (GPL-3,
  https://surge-synthesizer.github.io ). Plugins are separate programs
  installed by the user; none are bundled with ChipWave.

"""

out = sys.stdout
out.write("ChipWave Studio - third-party software\n")
out.write("=" * 60 + "\n\n")
out.write("ChipWave Studio is GPL-3.0-or-later (see LICENSE.txt). It includes the\n"
          "crates below. MP3 export uses LAME 3.100 (LGPL), statically linked through\n"
          "mp3lame-sys; LAME's source is at https://lame.sourceforge.io and the\n"
          "complete ChipWave source (needed to relink) is at\n"
          "https://github.com/postmanpat-303/ChipWaveStudio\n\n")
out.write(CREDITS)
for p in pkgs:
    out.write(f"- {p['name']} {p['version']}: {p.get('license') or 'see files'}\n")
out.write("\n\n")
seen = set()
for p in pkgs:
    d = os.path.dirname(p["manifest_path"])
    files = sorted(f for pat in ("LICEN[CS]E*", "COPYING*", "NOTICE*", "lame*/COPYING", "*/COPYING")
                   for f in glob.glob(os.path.join(d, pat)) if os.path.isfile(f))
    for f in files:
        try:
            txt = open(f, encoding="utf-8", errors="replace").read().strip()
        except OSError:
            continue
        key = (p["name"], txt)
        if key in seen:
            continue
        seen.add(key)
        out.write("=" * 60 + "\n")
        out.write(f"{p['name']} {p['version']} - {os.path.relpath(f, d)}\n")
        out.write("=" * 60 + "\n")
        out.write(txt + "\n\n")
