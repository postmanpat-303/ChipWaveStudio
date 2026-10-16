# ChipWave Design Language: "Wave Cartridge"

**Status:** design, chosen direction. Applies to every ChipWave screen.

ChipWave keeps the *functions* Del liked in Bitwig (horizontal device strips, a live draggable filter curve, a waveform display with markers, knobs that show modulation, drag-to-modulate) but has its **own look**. The Bitwig Sampler screenshot in [`reference/`](reference/del-visual-reference-bitwig-sampler.png) was **functional inspiration only**. None of its visual style is reused.

## 1. Two directions explored

| | **A: Workbench Modern** | **B: Wave/Neon** (recommended) |
|---|---|---|
| Idea | Amiga Workbench 3.1 grey bevels redone flat: light grey cartridges, deep-blue CRT displays, blue/orange/yellow accents | BigWave Radio's "wave" identity: deep ink-navy, a cyan→magenta gradient, neon lime/yellow accents, CRT displays with a pixel grid |
| Mockup | [`mockups/variants/sound_designer_A_workbench_modern.png`](mockups/variants/sound_designer_A_workbench_modern.png) | [`mockups/variants/sound_designer_B_wave_neon.png`](mockups/variants/sound_designer_B_wave_neon.png) |
| Strengths | Instantly reads as "Amiga"; very distinct from dark DAWs; high contrast | Ties ChipWave and BigWave Radio together as a family; comfortable for long night sessions; colours stay vivid on any monitor |
| Weaknesses | Light UI is tiring in dark studios; its blue + orange accents edge back towards common DAW colours | Neon has to be used sparingly so it doesn't get noisy |

**Decision: B "Wave/Neon"**, with Workbench Modern kept as the basis of the optional **Amiga mode** skin ([UI_SCALING.md §9](UI_SCALING.md)). Both use the same shapes and layout described below, so a skin only changes the palette and the bevels.

## 2. Distinguishing choices (what makes it ChipWave, not Bitwig)
| Element | ChipWave | Avoided Bitwig hallmark |
|---|---|---|
| **Modules** | **Cartridges**: panels with a cut (45°) top-left corner, like a game cartridge, and a **coloured vertical spine on the left edge** carrying the module name in rotated monospace caps (OSCILLATOR, FILTER, MOD, OUT) | Flat, evenly rounded charcoal panels with small top headers |
| **Signal flow** | Visible **wave-shaped patch arrows** between cartridges, in cyan, showing audio flowing left to right | A plain row of panels |
| **Knobs** | **Hex-cap LED-ring knobs**: a hexagonal cap with a dot pointer, ringed by **15 discrete LED segments** | Round knobs with a thin continuous arc |
| **Faders** | **"Chip" faders**: vertical segmented LED meters in a recessed slot (Res, Drive, Env, ADSR) | Thin vertical sliders with a cap |
| **Displays** | **CRT style**: near-black blue, an 8-px pixel grid, faint scanlines, gradient or neon traces | Smooth flat dark displays |
| **Filter curve** | Gradient (cyan→magenta) line; modulation sweep drawn as **stepped pixel columns**; a **square** handle | Smooth filled curve |
| **Markers** | Yellow lines with **triangle feet and letters below** the display (S = start, L = loop) | Flag markers at the top |
| **Modulation** | Mod sources are **chips** (coloured pills: ENV2, LFO1, M1) you **drag onto a control**. LED segments inside the modulated range light in the **source's colour**, and a small coloured square next to the knob shows each assigned source | Routing-button mode plus coloured arcs |
| **Source selection** | **Pill tabs in a row** at the top of the cartridge | A left-hand column of mode tabs |
| **Top bar** | A **wave band**: a gradient header with a wavy bottom edge, holding the patch name and macros | — |
| **Actions** | Rounded **outline pills** inside the OUT cartridge (RENDER ▸ SMP, FX CHAIN · 4) | A right-hand column of buttons with coloured top strips |
| **Playhead** | A **wavy yellow line** with a triangle cap (arranger and tracker) | A straight line |
| **Type** | Monospace caps for labels and values (JetBrains Mono, OFL), giving a tracker/terminal feel | Proportional sans |
| **Warnings** | **Guru red** (`#FF2A3D`) on black, in a blinking-border box, like an Amiga Guru Meditation | — |

## 3. Palette (Wave/Neon)
| Token | Hex | Use |
|---|---|---|
| `bg` | `#070B1C` | window background |
| `bg2` | `#0D1430` | toolbars, secondary areas |
| `panel` | `#121B3D` | cartridge body |
| `panel2` | `#18234D` | pills, knob wells |
| `edge` | `#2A3A78` | outlines (1.5 px) |
| `display` | `#050817` | CRT displays, fader slots |
| `grid` | `#13204A` | 8-px display grid |
| `led_off` | `#1F2B5C` | unlit LED segments |
| `text` | `#E6F0FF` | primary text |
| `dim` | `#7F8FBF` | labels, hints |
| `cyan` | `#22E3FF` | primary accent: oscillators, audio arrows, LFOs |
| `magenta` | `#FF3FB4` | filter, PWM/mod overlays, render |
| `lime` | `#B6FF3C` | envelopes, "on" lamps |
| `yellow` | `#FFD23F` | markers, handles, playhead, output |
| `violet` | `#8B5CFF` | extra clip colour |
| `guru_red` | `#FF2A3D` | warnings, clipping |
| gradient | `#22E3FF → #FF3FB4` | wave band, filter curve, branding |

**Colour roles by module:** Oscillator cyan, Filter magenta, Mod lime, Out yellow. FX cartridges take their category colour (drive/space magenta, crush yellow, chorus cyan, echo lime). Modulation chips use their source's colour: envelopes lime, LFOs cyan, macros magenta, tracker-command lanes yellow.

**Colour-blind-safe set:** swap to the Okabe–Ito accents ([UI_SCALING.md §8](UI_SCALING.md)). Mod chips also carry a letter prefix (E/L/M) so colour is never the only cue.

**Workbench Modern palette** (Amiga-mode skin): bg `#8E98A6`, panel `#C3C9D1`, display `#0F2B5B`, grid `#1D3F78`, text `#101418`, accents blue `#1F5FD1`, orange `#FF7A00`, yellow `#FFD400`, white; 2-px white/grey bevels.

## 4. Component spec (for egui)
**Cartridge**
- Body `panel`, 1.5 px `edge` outline, square corners except a 14 px 45° cut at top-left.
- Spine: a 14 px wide strip on the left in the module colour, with the name rotated −90° in monospace caps, 10 px, weight 800, in `bg`.
- 26 px gap between cartridges holds the patch arrow: a 2.5 px cyan S-curve with an arrowhead.

**LED-ring knob**
- Hexagon cap (point-up rotated 30°) of radius r: 11 small, 13 standard, 20–22 large. Fill `#0A1028`, outline `edge`, 2.6 px dot pointer in the accent at 55 % of r.
- Well: a circle of radius r+9 in `panel2`.
- Ring: 15 radial segments, 3.2 px thick, from r+3 to r+7, sweeping 270° from bottom-left.
  - Unipolar values light from the start; bipolar ones light from the centre segment.
- **Modulation:** segments between the base value and base+amount light in the source colour. When the patch plays, the live position blinks one segment brighter. Small 6×6 source squares sit at the knob's top-right.
- Labels: monospace caps 9 px `dim` below; value 9 px in the accent below that.
- Interaction: vertical drag (Shift = fine), double-click resets, wheel nudges, right-click opens the menu. **Drop a mod chip** on the knob to create a routing, then drag the knob while hovering the chip's square to set the depth.

**Chip fader**
- Recessed `display` slot (30×62–78 px) with 10 LED blocks lit from the bottom in the accent, `led_off` otherwise.
- Drag vertically; modulated blocks show the source colour like the knob.

**CRT display**
- `display` fill, 8-px `grid` lines at 0.6 px, scanlines every 3 px at 18 % black.
- Traces 1.6–3 px. Handles are square with a 2.5 px yellow outline.
- Integer-snapped to physical pixels ([UI_SCALING.md](UI_SCALING.md)).

**Pills**
- Height 18–26 px, fully rounded. Selected: accent fill with `bg` text. Unselected: `panel2` with `dim` text. Action pills use an accent outline and accent text.

**Mod chip**
- 64×22 px, radius 4, in the source colour, with a grip square on the left and a monospace label (E·ENV2, L·LFO1, M·M1).
- Dragging it shows a cyan "cable" from the chip to the cursor, and valid targets get a dashed outline.

**Wave band header**
- 22 px gradient band with a sine-wave bottom edge (amplitude 3 px, period ~190 px) that drifts slowly when audio plays. Turned off by "reduce motion".

All of it is drawn with `egui::Painter` as vectors, so no assets are needed. The only bundled font is JetBrains Mono (OFL).

## 5. Mockups
- Sound designer: [`mockups/sound_designer.png`](mockups/sound_designer.png) / `.svg`
- FX chain: [`mockups/fx_chain.png`](mockups/fx_chain.png) / `.svg`
- Arranger: [`mockups/arranger.png`](mockups/arranger.png) / `.svg`
- Variants: `mockups/variants/sound_designer_A_workbench_modern.*`, `mockups/variants/sound_designer_B_wave_neon.*`

![Sound designer](mockups/sound_designer.png)
![FX chain](mockups/fx_chain.png)
![Arranger](mockups/arranger.png)
