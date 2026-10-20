# ChipWave "Ember" skin: the Slate theme

**Status:** implemented (0.2.4.x, branch `ember-skin`). **Slate** is the default theme; **Dark Circuit** and **Amiga** stay in *Settings › Look*. Approved by Del (postmanpat) from the mockups in `docs/mockups/ember/` (slate edition, pic 2 look).

## Where it lives in the code
- `src/theme.rs`: the `SLATE` theme (palette, `ember: true`), its `Visuals` overrides (`ember_visuals`) and the embedded fonts (`fonts`, `label_font`, `num_font`). `theme::cartridge`, `theme::crt` and `theme::raise` draw the Ember panel, well and bevel when the theme is Slate, so every existing panel picks the look up.
- `src/ember.rs`: the painters and small widgets: `panel`, `card` (with corner `screws`), `well`, `lcd`, `glow_line` / `fill_under` / `glow_rect` / `glow_dot` / `glow_text` (the fake bloom), `led`, `dot_meter`, `knob_big` (pic 1), `knob_small` (pic 2), `fader`, `button` (outlined, LED dot when selected), `led_item`, `tab` and `tab_underline`.
- `src/app/mixstrip.rs`: the mixer strips (insert slots, sends, pan, duck, fader with its dB scale, dot meters with peak hold, LCD readout, M / S) and the Master section (limiter, Gain / Ceiling / Release knobs, LEVEL / REDUCTION dot rows, loudness, target).
- The instrument card (`instpanel/instcard.rs`: pic-1 macro knobs, bevelled sections with screws, glowing graphs), the Devices cards (`devices.rs`, `devcards.rs`), the knob widget (`knob.rs`: pic-2 knob, glowing card graphs), the Clip Arranger's Master panel, the piano roll (light grey notes, selected notes orange with a white rim and a bloom), the Timeline playhead and the lower-panel tabs.
- Paint and interaction only: songs, audio and the mixer data are unchanged; old songs render bit-identically. The mixer fader uses a console taper on screen (0 dB at 72 % of the travel); the stored gain is the same 0..2 value as before.
- **Fonts** (SIL OFL 1.1, embedded with `include_bytes!`): Inter Regular, Barlow Semi Condensed SemiBold and Geist Mono SemiBold in `assets/fonts/`, licences in `LICENSES/`, credited in Help › About › Credits, the README and `THIRD_PARTY_LICENSES.txt`. The two families `label` and `num` are registered in every theme; Inter becomes the body font in Slate only.
- **One knob and meter painter for every theme** (`src/theme.rs`): `knob_face` (every rotary control: instrument and Devices knobs, generator and fill macros, the mixer's send / pan knobs), `knob_hero` (the instrument macros and the Master knobs) and `meter` / `meter_peak` (every level meter). Each draws the Slate look when the theme is Slate and the existing Polished / Flat look in Dark Circuit and Amiga, so the widgets themselves (interaction, accessibility labels, undo) are shared, not duplicated. The Slate mixer strips (`mixstrip.rs`) reuse the same painters and the existing mixer data and learn targets.
- Flat mode (*Settings › Look › Flat*) turns the bevels' shadows and the bloom off. *Reduce motion* is unchanged: Slate adds no animation (the meters' 1.5 s peak hold is a reading, not motion). The app has no high-contrast setting yet; Slate keeps the keyboard focus ring and the accessible names / roles / values of every control (the UI tests find them by label).
- The save prompt (*Save changes to "Song"?*) uses the outlined buttons; **Save**, the default, is lit orange.

## Not (yet) done
- Mono channels have one meter column (the engine keeps one peak per channel); Master shows L and R.
- No **R** (record arm) button on strips: ChipWave has no per-track record arm yet.
- The Timeline header, the transport and the view bar keep their current widgets (reworked on `focus-timeline`); they take the Slate palette and fonts.
- The LED-selector and square-toggle widgets from the style guide are not used yet: the existing radio rows, checkboxes and dropdowns take the Slate visuals instead.
- The classic sidebar instrument editor is wider than its panel in Slate (Inter is a little wider), so long rows run under its scroll bar, as some already did in Dark.

The design guide (from the mockup stage) follows.

---


**Slate grey bevelled panels** with corner screws, outlined buttons and LED selectors (pic 2), plus **pic-1 knobs**: a silver ring, a dark glossy centre, an orange readout and a red-orange segmented LED arc.

From pic 3 we take **only the soft bloom** behind spectrums, curves, meters, the playhead and selected notes.

The layouts are unchanged from the approved mockups (main window, instrument card with its preset dropdown, mixer/EQ); this is a restyle only. Ember is a new theme next to `DARK` and `AMIGA` in `src/theme.rs`, with a darker slate option.

**Mockups** (real renders, HTML/SVG in headless Chrome; sources in `src/`, the earlier warm sources in `src-warm/`):
- `a-main.png`: the main window. It has the Clip Arranger, Timeline, Piano roll, Devices row and view bar.
- `b-cards.png`: the instrument card and the effect cards.
- `c-mixer-eq.png`: the EQ display and the mixer.
- `d-main-slate-dark.png`: the main window in the darker slate, for comparison.
- `e-anatomy.png`: the palette, the knob layers, the knob family and the small controls.
- `*-warm.png`: the earlier warm charcoal-brown renders, kept for reference. This includes the old teal variant.

## 0. Hard rule: no pic-3 controls
- No small knobs with coloured or dotted ring arcs, and no dot-ring knobs at all.
- No pill or segmented toggle buttons, and no paired segmented buttons.
- No big central arc dial.
- Buttons never glow.
- Discrete choices are made with one of these:
  - pic-2 **LED selectors** (labelled LEDs, one lit);
  - pic-2 **outlined buttons** (the selected one has orange text and an LED dot);
  - pic-1 **square toggles** (HQ);
  - a dropdown.
- Glow (bloom) appears only on spectrums, curves, meters, the playhead, selected notes, LED arcs/readouts (pic 1) and the active-tab underline (pic 1).

## 1. Palette

| Token | Slate (primary) | Slate dark (variant) | Use |
|---|---|---|---|
| `bg0` | `#1B1D20` | `#111214` | window behind the panels, status bar |
| `bg1` | `#25282C` | `#191B1E` | panel gradient bottom |
| `bg2` | `#2F3338` | `#212428` | panel gradient top, cards |
| `bg3` | `#3A3E44` | `#2A2E33` | raised: track headers, button tops |
| `bg4` | `#454A51` | `#343940` | toggle tops, highlights |
| `edge` | `#121315` | `#0A0B0C` | 1 px outer panel edge (bevel shadow) |
| `hi` | white 7.5 % | white 6 % | 1 px inner top highlight (bevel light) |
| `line` | `#3B3F45` | `#2E3237` | separators, inputs |
| `line2` | `#6A7078` | `#585E66` | pic-2 button outlines (light grey) |
| `ink` | `#E9EBED` | `#E4E6E8` | text and values |
| `ink2` | `#AAB0B6` | `#9CA2A9` | labels |
| `ink3` | `#767C83` | `#666C73` | ticks, inactive tabs, hints |
| `ink4` / `dot-off` | `#50555C` / `#454A50` | `#43474D` / `#3A3E44` | unlit meter dots and LEDs |
| `disp0` › `disp1` | `#141619` › `#1A1C20` | `#0D0E10` › `#121417` | display wells, timeline lanes, piano roll |
| `grid` / `grid2` | `#262A2F` / `#30353B` | `#1D2024` / `#272B30` | display grid lines |
| `acc-deep` | `#E2481F` | same | start of the LED arc (red-orange) |
| `acc` | `#FF8A2A` | same | accent: tab underline, playhead, selection |
| `acc-hot` | `#FFB45A` | same | readouts, LED tips, curves, selected text |
| `acc-soft` | `#FF8A2A` @ 13 % | same | active-tab wash only |
| `led-off` | `#3A2B28` | same | unlit LED arc segments (dark red, pic 1) |
| knob well | `#1A100D` | same | glossy centre of the pic-1 knob |
| LED bed | `#1E1513` | same | groove under the LED arc |
| meter amber / red | `#FFD36A` / `#FF5A3C` | same | the top meter dots |
| silver | `#F7F5F2` › `#D3CEC9` › `#8E8781` › `#4C4642` (rendered neutral) | same | knob ring and fader cap |
| pic-2 cap | `#5A5F66` › `#2C3035` › `#16181B`, face `#3A3E44` › `#1F2226` | same | knurled dark knob |

**Track colours** stay muted: Kick `#C4623F`, Bass `#C9913A`, Hats `#B3A24A`, Clap `#6E9E5C`, Arp `#4E9A94`, Pad 1 `#5F7FB3`, Pad 2 `#8B6FB0`, Lead `#B85F7E`.
- The clip body is the colour darkened 45 % at the top and 62 % at the bottom.
- The header strip is the full colour.

**Piano roll notes:**
- Unselected notes are light grey `#CBCDD2`.
- Selected notes are `acc-hot` with a 1 px white rim and a bloom, so they are clearly a different colour.

## 2. Surfaces (pic 2)
**Panels:**
- A vertical gradient from `bg2` to `bg1`, radius 7.
- A 1 px `edge` border, a 1 px inner top highlight (`hi`) and a 1 px inner bottom shadow (black 35 %). This is the bevel.
- A soft drop shadow outside.

**Corner screws:** 8 px, a radial grey gradient with a slot at 35°. They go on grouped panels: instrument sections, effect cards, Devices cards, Master and EQ.

**Displays** (waveform, filter, ADSR, EQ, spectrum, timeline lanes, piano roll) are recessed wells: a `disp0` › `disp1` gradient with an inset shadow.

**LCD readouts:** a near-black well, Geist Mono Semibold in `acc-hot` with a glow, and units in uppercase `ink3`. Used for transport, BPM, Speed and mixer dB.

## 3. Knobs
**Pic-1 knob** (big controls: macros, main effect controls, Master, Threshold/Mix, EQ Output). It is drawn in a 100-unit box, over 270° from 135°.
1. **Bed:** a disc r 49.3 at 55 % black, and a groove ring r 43.3, 10 units wide, in LED bed colour.
2. **LED arc:**
   - 31 segments from r 39.5 to r 47, 3.3 units wide.
   - Lit segments are lerped from `acc-deep` through `acc` to `acc-hot`; unlit ones are `led-off`.
   - Bipolar knobs light from 12 o'clock.
3. **Bloom:** the lit segments are drawn again under themselves, blurred about 2 units.
4. **Silver ring:**
   - A drop shadow, a disc r 34 with the silver gradient, a sheen and an inner highlight.
   - A dark pointer notch from r 25.5 to r 33.4.
5. **Well:**
   - A disc r 25 with a radial fill (knob well warmed toward `acc` at the top) and a dark rim.
   - A 0.8 px `acc` rim at 32 %, and a faint gloss.
6. **Readout:**
   - Geist Mono Bold in `acc-hot` with a glow.
   - Size is 17 for 2 characters, 15 for 3, 12.5 for 4 and 10.5 for more.
   - A zero or disabled readout is `ink3` with no glow.

**Pic-2 knob** (section, Devices and mixer controls, sends, pan):
- A dark knurled cap: a radial gradient and a serrated ring at r 32 (dash 1.3/1.5).
- An inner face r 25, and an `acc-hot` pointer line from r 14 to r 29 with a slight glow.
- **Tick ring:** 11 short ticks at r 39–46 in `ink3`, with the nearest tick in `acc-hot`.
- **Scale variant:** 5 to 8 value labels at the tick points (Barlow 16 units). The nearest label is `acc-hot` with a glow and an `acc` tick. Use it where exact values matter: compressor times and ratio, EQ freq/gain/Q, filter cutoff.
- There is no LED arc and no dot ring on pic-2 knobs.

**Labels:**
- Knob names are uppercase Barlow Semi Condensed SemiBold in `ink2`.
- Values sit under small knobs in Geist Mono.

## 4. Buttons and selectors (pic 2 / pic 1)
- **Outlined button:**
  - A `bg3` › `bg2` gradient, a 1 px `line2` outline, radius 4, and uppercase condensed text. Examples: SAVE B / LOAD B, PANIC, ADVANCED ›.
  - **Selected:** `acc-hot` text, a 6 px `acc` LED dot before the label, and a lighter outline. No glow and no orange fill.
  - Used for EQ band types, delay divisions, A/B, Amp/Filter and M/S/R. A muted M shows as orange text, not a filled block.
- **LED selector:** a row of labels, each with a 7 px LED. The chosen one is lit, with an `acc-hot` label. It replaces every former pill/segment control: Ambience Small/Room/Hall/Open, Slope 12/24 dB, Voices, Analyzer Pre/Post/Off.
- **Square toggle (pic-1 HQ):** 14 px and raised grey when off; an orange radial with a highlight when on. Used for HQ, Drive, Sync, Follow and Metronome.
- **Transport buttons:** outlined squares. Play is shown by an orange icon only.
- **Dropdowns, including the preset dropdown:** unchanged in form, with a `#17191C` fill and a 1 px `line` border.
- **Tabs (pic 1):** text in `ink3`, and the active tab in `ink`. The active tab has a 3 px `acc-deep` › `acc-hot` › `acc-deep` underline with a glow, plus a faint `acc-soft` wash. The view bar uses a 2 px version.

## 5. Meters, displays and glow (pic 2 formatting + pic 3 bloom)
- **LED dot meters:**
  - 7 px dots with 3–4 px gaps. Unlit dots are `dot-off` with an inner shadow.
  - Lit dots are a radial gradient from warm white through `acc-hot` to `acc`, with a 5 px glow.
  - Vertical meters go `acc-hot`, then amber for the last 4 dots, then red for the top 2, plus a peak-hold dot.
  - Rows (LEVEL L/R, REDUCTION, OUT) have uppercase labels on the left. Reduction never turns red.
- **Fader scale:** labels sit at the ticks (+6 0 -6 -12 -24 -48 -∞), with 0 in `acc-hot` and an `acc` tick. The slot is dark, the fill below the cap is `acc` with a soft copy, and the cap is silver with an `acc-hot` line.
- **EQ display:**
  - dB labels at the left and frequency labels along the bottom. The value nearest the selected band is shown in `acc-hot`.
  - Bands are numbered rings, and the selected one gets a halo and a value tag.
- **Curves:** a crisp `acc-hot` line 1.6–2.2 px, over the same path in `acc` about 2.4× wider, blurred 4 px at 80 %. Below it is a fill of `acc` going from 28 % to 0.
- **Spectrum bars:**
  - A gradient from `acc` at 55 % to `acc-deep` at 8 %, with a 2 px top cap.
  - A copy blurred about 9 px sits behind, and that copy is the pic-3 bloom.
- **Playhead:** 2 px `acc-hot` with 6 px and 16 px glows.
- **Glow budget:** about 3 strong glows per card, always in the accent family.

## 6. Typography
- UI text is **Inter** 12 px.
- Labels, headings, knob names and buttons are **Barlow Semi Condensed** SemiBold/Bold. They are uppercase where pic 2 is, with 0.05–0.09 em tracking.
- Numbers are **Geist Mono**, tabular.
- All three fonts are SIL OFL, so they can be bundled. They **must be credited** in About › Credits and in the licence list.

## 7. Mapping to egui
- **Theme:** add `EMBER_SLATE` and `EMBER_SLATE_DARK` to `THEMES` in `src/theme.rs`.
  - Map `bg_top/bg_bot` = bg2/bg1, `cart_*` = bg3/bg2, `hdr_*` = bg3/bg2, and `crt_*` = disp0/disp1 (`crt_grid` = grid).
  - Map `border` = edge, `ink`/`ink2`, `accent`/`play` = acc, `value` = acc-hot and `selection_outline` = acc-hot.
  - Add new fields: `acc_deep`, `acc_hot`, `led_off`, `knob_well`, `line2`, `dot_off`, `hi`.
- **`Visuals` overrides in `theme::apply`:**
  - `panel_fill` = bg1, `window_fill` = bg2, `extreme_bg_color` = disp0 and `faint_bg_color` = bg2.
  - `widgets.inactive`: `weak_bg_fill` = bg3, `bg_stroke` = line2 at 1 px.
  - `widgets.hovered.weak_bg_fill` = bg4.
  - `widgets.active`: `bg_stroke` = `#8A8F96`, `fg_stroke` = acc-hot. This gives no fill and no glow.
  - `selection`: `bg_fill` = acc at 25 %, `stroke` = acc-hot.
  - Radius is 7 for windows and 4 for widgets. The window and popup shadow is a 12 px blur at 45 % black.
- **Bevel:** `theme::raise()` already paints a raised rect. Give it the `hi` top line and the dark bottom line, and add a `screws(painter, rect)` helper for grouped panels.
- **Knobs:** there are two painters in `src/app/knob.rs`, chosen by role. Interaction is unchanged.
  - **Pic-1 painter:**
    - The arc is drawn with `line_segment` per segment.
    - The ring and well are `epaint::Mesh` discs with per-vertex gradients (as `round_mesh` does).
    - Bloom is 2–3 extra passes with wider strokes at about 30/15/7 % alpha, because egui has no blur.
    - The readout is `painter.text`, with 4 offset low-alpha copies underneath as the glow.
  - **Pic-2 painter:**
    - A gradient disc, with the knurl drawn as about 60 short radial strokes alternating dark and light.
    - An `acc-hot` pointer line, plus ticks with the nearest one in the accent colour.
    - Optional value labels via `painter.text` at r 67.
  - The static layers can be cached per size in a texture if profiling asks for it.
- **New widgets** in a new `src/app/widgets.rs`. Keyboard focus draws the existing `focus_ring`.
  - `led_selector(&mut usize, &[&str])`
  - `square_toggle(&mut bool)`
  - `outlined_button(selected)`, which shows the LED dot when selected
  - `lcd(text)`
- **Meters:** add `dot_meter(painter, rect, frac, n, vertical, peak)` (`circle_filled` plus a glow pass on lit dots) in place of `theme::meter`.
- **Glow helpers:** extend `theme::glow` into `glow_line` and `glow_dot`. Use them only for curves, spectrum, meters, the playhead, selected notes, LED arcs/readouts and the tab underline.
- **Fonts:** `ctx.set_fonts` with `include_bytes!`.
  - Proportional = Inter.
  - A named `FontFamily::Name("label")` = Barlow Semi Condensed.
  - Monospace = Geist Mono.
- **Settings › Look:** the theme choices become Ember Slate and Ember Slate Dark. Flat mode turns off bevels and bloom.
- **This is a paint-only change.** Audio, songs and layout are untouched, so old songs still render bit-identically.
