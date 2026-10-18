# Voice (glitch) instrument (0.2.3.6)

A synthetic voice for dark psytrance chops: **"ahh, eee, ohh, err"** vowel
glitches, babble and stutters. No recorded voice and no samples. Everything
is generated:

```
note pitch -> glottal pulse (Rosenberg, tension, jitter) --+
breath noise (follows the pulse) --------------------------+--> 5 parallel formant
                                                               band-passes (TPT SVF)
vowel sequence / babble / LFO / 30-32 -> formant table morph --^         |
                                                                         v
                         chop gate -> amp env / filter / drive (synth patch) -> stutter (33/34) -> FX chain
```

## Making one
- Use **New… > Voice (glitch) >** a preset (under the instrument list), or
  right-click an instrument and pick **New instrument > Voice (glitch)**.
- The **Voice** panel has these controls:
  - **Add** chips (A E I O U ER AE) add vowels to the sequence (up to 8).
    Click a chip in **Seq** to remove it.
  - **Register:** Bass, Tenor, Alto, Soprano or **Demon** (the bass formants scaled ×0.78).
  - **Source:** Glottal (the most voice-like), Saw, Pulse (robot/chip; uses Pulse width) or Whisper (noise).
  - **Step:** tempo-synced, how often to move to the next vowel. **Morph** is the glide time, and **Babble** is the chance of a random jump.
  - **Formant shift** (0.5–2×, independent of pitch), **Sharpness**, **Breath**, **Tension** and **Jitter**.
  - **LFO > vowel / LFO > shift:** the patch LFO (below the panel) sweeps the vowel or the throat.
  - **Stutter** (on every note: slice length, repeats, pitch step per repeat) and **Chop** (a tempo-synced gate).
- Below it are the usual envelope, filter, drive and LFO settings, plus the
  instrument FX chain (try Reverb + Delay + Chorus).

### Presets

| Preset | Sound |
|---|---|
| Demon Ahh | Demon register, a slow A > O > U morph via the LFO, breathy and driven |
| Choir Eee | soprano E > I > E > AE pad voice with vibrato (add reverb) |
| Glitch Babble | random vowel jumps every 1/16 with a 1/32 chop: alien chatter |
| Robot Err | pulse source, narrow formants, ER > A |
| Stutter Ohh | every note repeats its first 1/32 slice 3×, stepping down |
| Whisper Ooh | noise-only U > O with an LFO formant shift: a ghost voice |

## Effect commands 30–37
The 3x range is unused by OctaMED (see [OCTAMED_SPEC.md §5](OCTAMED_SPEC.md)) and by ChipWave.

| Cmd | Effect | Argument |
|---|---|---|
| `30xy` | Set vowel | x = 0 A, 1 E, 2 I, 3 O, 4 U, 5 ER, 6 AE; y = glide in ×25 ms (0 = hard cut). Holds until the next note or `31` |
| `31xx` | Jump to sequence step xx | 00 = first vowel; stepping continues from there |
| `32xx` | Formant shift | 80 = ×1, 00 = ×0.5, FF ≈ ×2 |
| `33xy` | Stutter / beat-repeat (**any instrument**) | x = slice length: 1 = 1/64, 2 = 1/32, 3 = 1/16 (one row at 4 lines/beat), 4 = 1/8, 5 = 1/4, 6 = 1/2 note; y = repeats (0 = stop) |
| `34xy` | Stutter pitch step per repeat | +x semitones (up/faster), or −y (down/slower) |
| `35xx` | Breath | 00–40 |
| `36xy` | Babble | x = step length (as for 33), y = amount 0–F |
| `37xy` | Chop gate | x = cycle length (as for 33), y = open for y/16 of each cycle (0 = off) |

The effects in a row with a note apply to that new note. A new note resets
`30`, `32`, `35`, `36`, `37` and the stutter.

Example: `C-2 05 3000`, then `--- -- 3053` and later `--- -- 3022`, gives "ahh → err → eee".
`--- -- 3332` on a later row stutters the last 1/16 twice.

## Consonants and words (0.2.3.10)
Give the voice **Words** (the row of small text boxes under the vowel
sequence; **+ word** adds one, right-click or **clear** removes). Each new
note speaks the next word in the list, so a run of notes says
"ka ta ka da …". `3Exx` on a note speaks word `xx` instead; `3Exx` on a row
without a note says word `xx` again on the held note (a new syllable
without a new envelope). With no words the voice works as before (vowels only).

A word is a phoneme string. Letters:

| Kind | Letters | How it sounds |
|---|---|---|
| vowels | `a e i o u`, `er`, `ae` (also `ee` = i, `oo` = u, `y` = i, `w` = u, `r` = er) | the vowel table; a vowel that is not last lasts 150 ms, the last one holds until the note ends |
| plosives | `p b t d k g` (`c q` = k, `x` = k s, `ch` = t sh, `j` = d sh) | a closure (silent; a low *voice bar* for b d g), a 10 ms noise **burst** shaped by the place (lips ≈ 1.2 kHz, tongue tip ≈ 4.5 kHz, soft palate ≈ 2.3 kHz), then aspiration (p t k) and a **formant transition** from the consonant's locus into the vowel |
| fricatives | `s z sh f v h` (`th` = f) | band-limited noise (s ≈ 6.5 kHz, sh ≈ 3.2 kHz, f/v broad and soft), voiced for z and v; `h` is breath through the next vowel's formants |
| nasals | `m n ng` | a voiced, darker murmur (F1 ≈ 270 Hz, upper formants weak), then the transition into the vowel |
| pause | `.` `-` `,` | 80 ms of silence |

A word that ends on a consonant ("ak") goes silent after it until the next
word. **Speech rate** (0.25–3×) scales all phoneme lengths and the
transitions. The **Talk Ka-Ta** preset shows it.

## Throat mode (0.2.3.10)
**Tract: Throat** replaces the five formant filters with a simulated vocal
tract, inspired by **Pink Trombone** by Neil Thapen (MIT): a 44-section
waveguide tube (Kelly-Lochbaum) whose diameters follow a tongue position, the
lips and constrictions, with a 28-section nose behind a velum. The glottal
source, breath, tension and jitter are the same as in formant mode. Vowels
become tongue and lip shapes (A = tongue back and low, I = front and high, U =
back, high and rounded lips, …), consonants become closures and narrow
constrictions at the lips, tongue tip or soft palate with turbulence noise,
and nasals open the velum, so the transitions come out of the moving tract.
Formant shift and register scale the tract length; Morph and Speech rate set
how fast the tongue moves. Presets: **Throat Ahh**, **Throat Speech**, **Demon Words**.
Throat mode costs more CPU (8 throat voices render about 6× faster than realtime
on one core; formant mode is several times cheaper).

## Text songs
In a `.cwt.toml` text song, `patch = "voice:Demon Ahh"` makes a Voice
instrument. An optional `[instrument.voice]` table overrides its fields
(e.g. `vowels = ["Er", "A"]`, `shift = 0.9`, `words = ["ka", "ta"]`,
`rate = 1.2`, `throat = true`), and the usual synth keys
(`cutoff`, `attack`, …) override its envelope and filter.

`scripts/make_voice_examples.py [out.mp3]` writes one example text song per
preset plus a 16-bar dark psy groove with voice glitches (effects 30–37 in
use), renders them with `chipwave --render` and joins them into one MP3.

## Limits (first version)
- The voice is mono, and unison does not apply (use Chorus or the delay for width).
- Consonants are simple (no l, no r colouring beyond the ER vowel, no
  diphthong rules); words are phoneme strings, not English text-to-speech.
- In Throat mode voiced closures (b d g) are almost silent: the tube has no
  wall radiation for the voice bar.
- Stutter slices are at most 1 second long and sit before the instrument FX chain.
  Live MIDI notes are not stuttered.

## Acknowledgements / Credits
- **Emilie Gillet / Mutable Instruments**: the design is inspired by the
  Plaits "naive speech synth" (MIT),
  https://github.com/pichenettes/eurorack/tree/master/plaits/dsp/speech.
  No code or table data is copied. The MIT notice is in THIRD_PARTY_LICENSES.txt.
- **Csound manual**, Appendix "Formant values" (the Csound community; Csound is
  LGPL-2.1): https://csound.com/docs/manual/MiscFormants.html
- **G. E. Peterson & H. L. Barney** (1952), "Control methods used in a study of
  the vowels", JASA 24(2), https://doi.org/10.1121/1.1906875
- **A. E. Rosenberg** (1971), glottal pulse shape, JASA 49(2),
  https://doi.org/10.1121/1.1912389. **G. Fant, J. Liljencrants & Q. Lin**
  (1985), the LF glottal flow model.
- **Dennis H. Klatt** (1980), "Software for a cascade/parallel formant
  synthesizer", JASA 67(3), https://doi.org/10.1121/1.383940
- **Vadim Zavalishin**, *The Art of VA Filter Design*: the TPT state-variable
  filter used for the formants.
- **Neil Thapen**, Pink Trombone (2017, MIT), https://dood.al/pinktrombone/ :
  the Throat mode's vocal-tract design. Independent implementation; the MIT
  notice is in THIRD_PARTY_LICENSES.txt.
- **J. L. Kelly & C. C. Lochbaum** (1962), "Speech synthesis": the tube model.
- **P. Delattre, A. Liberman & F. Cooper** (1955), "Acoustic loci and
  transitional cues for consonants", JASA 27(4),
  https://doi.org/10.1121/1.1908024 : consonant loci.
- **K. N. Stevens**, *Acoustic Phonetics* (MIT Press, 1998): burst and
  frication noise bands.
- Research notes: [research/voice-glitch.md](research/voice-glitch.md).
