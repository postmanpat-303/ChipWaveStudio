# The built-in instrument skin

ChipWave's own instruments share one look: a **violet** tint, slightly
different per theme, that marks them wherever they're painted. Third-party
instruments and effects stay neutral, so violet always means "a ChipWave
built-in". The app accent (Slate orange, Dark Circuit green, Amiga blue)
stays for selection, focus, the playhead, transport, *Add to track* and the
A/B toggle.

Code: [`src/theme/instskin.rs`](../src/theme/instskin.rs).

## What's skinned

| | skinned | neutral |
|---|---|---|
| instruments | every ChipWave generator (WaveForge, Synth, Analog, FM-4, Wavetable, Grid, Noise, Drum Synth, Chip FX, Voice), the Drum Kit, Sample instruments | Racks (a container; their layers are skinned on their own), CLAP / VST3 instruments |
| places | the Instrument tab (simple card, Advanced, the Drum Kit editor, the sample editor), the instrument card in Devices / Nested Device Chains, Browser › Instruments › ChipWave Instruments rows | effect cards, the mixer, the timeline, everything else |

The skin paints:

- **panel**: a translucent wash over the theme's own fill and a 1 px tinted
  frame (the Instrument tab, every section card in it, the instrument card);
- **stripe**: a 3 px tint bar along the top of the Instrument tab and the
  instrument card;
- **badge**: one type badge for every built-in (caps in the text tint, 20 %
  tint fill, tint outline) instead of a hue per type;
- **knobs**: studio dots, pointer and value readout (Slate), the classic value
  arc and the macro knobs' hover ring (Dark Circuit, Amiga);
- **pictures**: curves, dots and marks in the card pictures (`bright` on the
  dark wells).

Every built-in has the same header: `< > NN Name`, the badge, `Preset…`, A/B
and *Advanced ›*. The Drum Kit's toolbar no longer repeats the kit's name.

In the Browser, VST3 and CLAP badges are neutral greys.

## Colours

`wash` and `edge` are unmultiplied RGBA.

| theme | tint | text | wash | edge | bright |
|---|---|---|---|---|---|
| Slate | `#A78BFA` | `#C4B5FD` | `#A78BFA` @ 26 | tint @ 110 | `#C4B5FD` |
| Dark Circuit | `#B39DFF` | `#CDBEFF` | `#B39DFF` @ 24 | tint @ 115 | `#CDBEFF` |
| Amiga | `#7B3FC4` (plum) | `#4E1F8A` | `#8A4FD8` @ 36 | tint @ 150 | tint 45 % lighter |

Amiga gets a deeper plum because the pale violet washes out on the light
Workbench grey, and plum keeps clear of Amiga's blue accent. Its picture
wells are dark navy, so curves there use the lighter `bright`.

## API (for a new instrument panel)

Everything is in `crate::theme` (re-exported from `theme::instskin`):

```rust
use crate::theme::{self, instskin::InstSkin, SkinScope};

// the skin for the current theme, or None (CHIPWAVE_INSTSKIN=off)
let skin: Option<InstSkin> = theme::instskin();

// is this instrument kind a built-in? (not a plugin, not a rack)
let on = theme::is_builtin(&inst.kind);

// 1. scope: the shared painters inside pick the tint up (knob_face,
//    ember::knob_studio, ember::card, instcard frames, knob::draw_viz,
//    instpanel::tag_pill). The guard is per thread and restores the
//    previous skin when dropped, so nothing outside is tinted.
let _scope = SkinScope::enter(skin.filter(|_| on));

// 2. explicit: paint the panel parts yourself
if let Some(s) = theme::active_skin() {
    s.panel(ui.painter(), rect, 6.0);   // wash + frame
    s.stripe(ui.painter(), rect, 6.0);  // the 3 px header stripe
    s.badge(ui, "StringWave");          // the type badge, laid out in ui
}

// your own painter: highlight in the tint, else the app accent
let col = theme::skin_or(theme::th().accent);
```

`InstSkin` fields: `tint` (marks, stripe, badge outline), `text` (readable
tint for text), `wash`, `edge`, `bright` (curves on dark wells). Inside a
skinned scope an effect or plugin part can opt out with
`SkinScope::enter(None)`.

The Instrument tab (`instpanel::instrument_view`) and the Devices instrument
card (`devices::devices_inst_card`) already enter the scope for any built-in
`InstrumentKind`, so a new instrument whose editor paints inside them is
skinned with no extra code.

## Off switch and shots

`CHIPWAVE_INSTSKIN=off` paints the neutral look (comparison shots only; no
setting). Screenshot views: `instskin-waveforge`, `instskin-chains`,
`instskin-browser`, `instskin-plugins`, plus `timeline-drumkit`, e.g.

    chipwave --screenshot out.png --view instskin-chains --theme amiga --size 1366x768
