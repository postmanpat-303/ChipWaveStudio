# Drag-to-modulate (Unreleased)

Drag-to-modulate, inspired by other DAWs: an instrument owns **modulators** (LFOs, envelopes and other
sources), and you **drag one onto any knob** to make it move that parameter. Each connection
is a **routing** with a bipolar **depth**, drawn as a coloured ring on the knob. This is item
§5.4 of [DAW_TIMELINE_PLAN.md](DAW_TIMELINE_PLAN.md).

## 1. Modulators

**+ Modulator** (in the strip above the instrument editor's cards, and in the FX view header)
adds one to the shown instrument:

| Kind | Output | Settings |
|---|---|---|
| **LFO** | −1..1 (or 0..1 unipolar) | shape (sine, triangle, saw up/down, square), sync (1/16 … 8 bars, or free in Hz), phase, retrigger, unipolar |
| **Envelope** | 0..1 | attack, decay, sustain, release; starts on every note of the instrument, releases when no note of it is held |
| **Random** | −1..1 (or 0..1) | rate / sync, glide between steps, retrigger, unipolar; the same steps every render |
| **Velocity** | 0..1 | smoothing; the velocity (volume column) of the instrument's latest note |
| **Keytrack** | −1..1 | smoothing; the latest note's pitch, C-4 = 0, ±1 = ±5 octaves |
| **Mod wheel** | 0..1 | smoothing; MIDI CC 1 from the MIDI input |
| **Macro** | 0..1 | a value knob of its own: one move changes many parameters |
| **Instrument LFO** | −1..1 (or 0..1) | follows the synth's own LFO card (shape, rate, sync) |

Each modulator is a **chip**: a colour dot, its name and a live meter. Click a chip to select
it (the Modulation tab's **Modulators** card edits the selected one); right-click › Remove
modulator removes it with its routings.

## 2. Routing: drag a chip onto a knob

- **Drag a chip** (a cable follows the pointer) and drop it on a knob of the instrument editor
  or a slider in the FX view. The routing starts at **+25 %** depth.
- **The ring:** the knob draws a coloured arc from its value to value + depth (up to three
  rings per knob), and a white dot shows the **live** modulated value while the song plays
  (modulators also run when stopped, so you hear and see them while you tweak).
- **Depth:** drag the **dot at the end of a ring** (up/right = more, Ctrl = fine), or
  **Alt-drag the knob** to set the depth of the selected modulator (it connects it if needed).
  Dragging the knob itself still sets the parameter's own value; the routing moves around it.
  FX sliders show depth bars under the slider; drag the bar's dot sideways.
- **Right-click a knob or slider › Modulate with ›** connects one of the instrument's
  modulators (or a **New** one) without dragging, and lists **Remove …** for its routings.
- **Routings card** (Modulation tab): every routing of the instrument (its modulators, or
  routes onto its parameters) as `LFO 1 > Lead Filter cutoff`, with the depth in % (drag or
  type), **×** to remove, and **Lane** to add an automation lane for the depth.

Depth is a fraction of the parameter's whole range (the same travel as the knob), so +25 % on
a cutoff knob moves it a quarter of the knob. Bipolar sources swing both ways; unipolar ones
only push in the depth's direction. The result is clamped to the parameter's range.

## 3. What can be modulated

Every knob or slider with an automation target:

- **Synth, Voice and Generator patch knobs:** cutoff, resonance, drive, amp level, and (new)
  pulse width, detune, filter envelope amount and decay, ADSR, stereo width, glide, LFO rate
  and depths, velocity > cutoff, accent, start delay, metal, generator release and glide
  (`instrument.N.patch.<key>`). These knobs are also **automatable** now (right-click › Add
  automation lane).
- **Generator knobs** (Drum/Kit macros, Chip FX, Analog, FM-4, Noise).
- **Built-in FX device parameters** and **plugin effect (CLAP) parameters**.
- The engine also takes routings onto mixer targets (track volume, pan, sends, master) and
  return-bus parameters, but the Mixer view has no drop targets yet (see Limits).

Not modulatable: tempo, plugin instrument macros, discrete settings (dropdowns, checkboxes),
and the few knobs without a target (Voice-specific knobs, octave and unison counts…).

## 4. Automation, undo, files

- **On top of automation:** a modulated parameter with a lane moves around the lane's value
  (lane first, then the routings are added).
- **Automatable depth:** **Lane** on the Routings card adds a lane `mod.K.amount` (K = routing
  number, −100..+100 %). While the lane plays, it replaces that routing's depth. Removing a
  routing removes its depth lane and renumbers the others.
- **Undo:** adding/removing modulators and routings are undo steps; depth and setting drags are
  one step per drag.
- **Instruments:** deleting an instrument removes its modulators and the routings onto it;
  moving instruments renumbers both.
- **Saved with the song:** `.cwt` files get `modulation.toml` (only when the song has
  modulation), `.cwt.toml` text songs a `[modulation]` table:

```toml
[[modulator]]
id = 1          # routes point at this
owner = 6       # instrument, 0-based
kind = "Lfo"    # Lfo Envelope Random Velocity Keytrack ModWheel Macro InstrumentLfo
shape = "Triangle"
sync = 2.0      # beats per cycle, 0 = free `rate` in Hz
[[route]]
src = 1
target = "instrument.7.cutoff"   # any automation target id
amount = 0.3                     # -1..1 of the parameter's range
```

  Loading checks every entry (unknown instruments, duplicate ids, unknown targets and
  out-of-range values are dropped or clamped). **Older versions ignore `modulation.toml`** and
  play the song unmodulated; the song's `min_reader_version` is unchanged.

## 5. Engine

- Modulators are evaluated on the automation grid (every 32 frames) in the audio thread with
  fixed-size state (64 modulators, 256 routings per song): **no allocation** (tested).
- Each evaluation collects the lane values into a preallocated list, adds `depth × modulator`
  for every routing (on the lane's value, else the parameter's own value), and applies the
  result through the same override path automation uses (patch copies, FX targets, plugin
  parameter events). **Songs without modulation render bit-identically** to before (checked
  against the previous build with every demo).
- Modulators restart with the song (phases, envelopes and random seeds), so a render is the same
  every time. Note-ons from the tracker, note clips and live MIDI feed envelopes, retriggers,
  velocity and keytrack; MIDI CC 1 feeds the mod wheel.
- Plugin effect parameters return to their own value when automation or modulation stops
  driving them; a parameter edit while modulated moves the value the routing adds to.

## 6. Limits

- Modulators are **per instrument, not per voice**: one value shared by the instrument's notes.
  Envelopes and retriggers restart on every note of the instrument; velocity and keytrack follow
  its latest note. Per-voice modulation (polyphonic envelopes on cutoff) is a later step.
- Plugin instrument macros, tempo and discrete settings can't be modulated.
- `.cwi` instrument files and the instrument library don't carry modulators yet.
- Alt-drag may be taken by the window manager on some Linux desktops; drag the ring's dot
  instead.
- The Mixer view's faders and the return buses aren't drop targets yet (such routings work
  when written in a `.cwt.toml`).
- No modulator-to-modulator routing (e.g. an LFO on another LFO's rate) yet.

## 7. Tests

`src/modulation/tests.rs`: the parameter registry, LFO rates and shapes, retrigger, envelope
stages, random steps, velocity/keytrack/wheel/macro, routings following instrument deletion and
renumbering, an LFO opening and closing a filter in a render, zero-depth routings changing
nothing, modulation on top of a lane, patch and FX parameter modulation, modulators running while
stopped, allocation-free rendering with every kind of modulator, `.cwt` / `.cwt.toml` round
trips and sanitising. `src/plugin/fxtests.rs`: a plugin effect parameter modulated by a macro and
returning to its own value. `src/app/modview_tests.rs` (UI): dropping a chip on a knob, the ring
handle, Alt-drag, undo/redo, patch-knob targets, the Routings card's remove, + Modulator, and
dropping on an FX slider.
