# ChipWave Studio: DAW Timeline Plan (plus the other big roadmap items)

**Status:** **M0, M1, M2, M3a, M3b and M4 (audio clips) are built** (19 Oct 2026, unreleased; M3b without MIDI file import, M4 without track FX, warping and reverse): the timeline model and converters, the reader-version check, the **Timeline view** (Ctrl+T), and **timeline mode** (clips are the song: editing, snap, loop region, markers, `timeline.toml`, the engine playing clips directly, Export tracker-compatible copy). See [TIMELINE.md](TIMELINE.md) for how it works and what is still missing. M5 is next. The plan was written 19 Oct 2026, after 0.2.4.0. This page covers the Bitwig-style sideways-scrolling **Timeline** that Del asked for. It also has short roadmaps for the other approved big items: VST3 hosting, plugin effects, a wavetable synth, drag-to-modulate and a modular Grid. The last section suggests an order for all of them.

![Timeline mockup: track headers with M/S/arm and volume on the left, per-track clips on a bar ruler with markers and a loop region, a track automation lane under Bass, an audio track with waveforms and fades, the browser on the right and the device chain plus the lower-panel instrument editor at the bottom](mockups/daw_timeline.png)

*Mockup (Dark Circuit, Velvet Mirrorball demo). The real top bar and lower-panel instrument editor are from the 0.2.3.19 build. Everything else is drawn: the Timeline view tab, track headers, clips, automation lane, audio track, browser and device chain.*

Reference: Del's Bitwig screenshot, and the Bitwig Studio user guide chapters *3. The Arrange View and Tracks*, *5. Arranger Clips*, *6. The Clip Launcher*, *9. Automation*, *16. Modulators* and *17. The Grid* ([bitwig.com/userguide/latest](https://www.bitwig.com/userguide/latest/)).

---

## 0. Where ChipWave is today (what the plan builds on)

| Area | Today (0.2.4.0) | Source |
|---|---|---|
| Song model | `Song { patterns, order: Vec<usize>, channels (≤ 32), instruments, mixer, automation, … }`. A **block** (`Pattern`) spans every channel. The **order list** is a flat list of block numbers | `src/model.rs` |
| Time base | One row is always a **sixteenth note**. The engine's tick length is `sr·60 / (bpm·4·speed)`, so a row lasts `60/(bpm·4)` s whatever the speed. `speed` (09xx) only sets ticks per row, which is how often slides and effects step. `lines_per_beat` is used only for display and in `arrange.rs` time estimates (which disagree with the engine when it isn't 4; M0 fixes that) | `engine.rs::tick_len` |
| Playback | `Engine` walks `order_pos`/`row`. Each row it calls `process_row`, which sends every channel's cell to `trigger` and then runs the effect commands. One **monophonic voice per channel** (`Chan`), plus a declick tail | `src/engine.rs` |
| Arranger | A single lane over `song.order`: coloured block clips with note-density previews, drag to reorder, duplicate, ruler seek and follow. Docked under the pattern editor or as a full view | `src/app/arranger.rs`, [ARRANGER.md](ARRANGER.md) |
| Automation | Song-scope lanes. `Point.pos` is in **rows** (fractional). Targets: instrument cutoff/res/drive/level, channel volume/pan/sends, master, tempo, FX params, return-bus params, plugin macros. Evaluated every 32 frames, so export matches realtime | `src/automation.rs`, `engine.rs::eval_auto` |
| Mixer | One strip per channel (volume, pan, mute, solo, 2 sends, ducking), returns A (delay) and B (reverb), and a master limiter. Insert FX chains belong to the **instrument** and follow the channel's current instrument | `src/model.rs::Mixer`, `fx.rs` |
| Instruments | Synth, Sample, Voice, Generators (Drum Synth, Kit, Analog, FM-4, Noise, Chip FX) and CLAP plugin instruments, all edited in the lower-panel knob-card editor | `instpanel.rs`, `gen*.rs`, `plugin/` |
| Audio thread | `Arc<Mutex<Shared { song, engine }>>`. The engine thread renders into an `rtrb` ring and the cpal callback only pops samples | `src/audio.rs` |
| File | `.cwt` ZIP of TOML and WAV files. `manifest.toml` writes `min_reader_version = 1`, but the reader **doesn't check it** yet | `src/fileio.rs`, [FILE_FORMAT.md](FILE_FORMAT.md) |

Two things decide the design:
1. **A row is already musical time** (a 16th at the current tempo). Tempo commands change how many seconds a row lasts, never how many rows there are. So a timeline measured in beats maps onto the tracker exactly, with no resampling of song structure.
2. **Tracks are already mixer channels**, and each channel keeps its own effect state (slides, filter overrides, Voice state, generator macros). A timeline track can simply *be* a channel. All the tracker semantics carry over.

---

## 1. Concept: a linear timeline that coexists with the tracker

### 1.1 The idea in one paragraph
The timeline is **another view of the same song**. **Tracks are channels.** Track 6 *is* channel 6, with the same mixer strip, the same name and the same effect state. Tracker blocks stop being "the whole song at once" and become **clips placed freely on each track**. A clip says "play column *c* of block *n* here, from row *r*, for this long, looped or not". Double-click a clip and the familiar pattern editor opens on that block with the cursor in that column. Edit it and every clip that uses the block updates, just as reusing a block in the order list works today. Next to tracker clips come **note clips** (polyphonic, edited in a piano roll), **audio clips** (WAV/FLAC/MP3 with waveforms) and **per-track automation lanes**. The tracker stays first-class: a song never *has* to use the timeline, and old songs open and play exactly as before.

### 1.2 Two song modes
| Mode | Source of truth | How you get there |
|---|---|---|
| **Tracker mode** (every song today) | `order` + blocks. The Timeline view shows a *derived*, read-only layout of it (M1) | Default. Old songs open here |
| **Timeline mode** | `song.timeline` (tracks, clips, tempo map, markers, loop). Blocks are still the content of tracker clips. The order list is hidden (it no longer describes the song) | The first timeline edit (moving, resizing or splitting a clip, adding an audio or note clip) asks once: "Switch this song to timeline mode? (Undo puts it back)". Conversion is lossless (§1.6) |

**Decisions (Del, 19 Oct 2026):**
- **Tracks are mixer channels** (§7 question 1).
- **New songs start on the timeline** (§7 question 2). In M1 this means File › New opens on the Timeline view with 8 tracks and 16 bars in view; the song is still in tracker mode. From M2 a new song starts in **timeline mode**. A setting (**Settings › New songs › Open on the Timeline view**) turns it off.
- **Old songs open as they are**, in tracker mode, and keep playing bit-exactly. They switch only when you make a timeline edit and confirm.

The pattern editor works in both modes. In timeline mode its block selector lists blocks as before, and a "used by 3 clips" hint shows where each block is placed. **Play block** loops the block in isolation (as now). **Play song** plays the timeline.

### 1.3 Data model (new, all `serde(default)`, saved as `timeline.toml` in the `.cwt`)
> **As built in M0** (`src/timeline.rs`, described in [TIMELINE.md](TIMELINE.md#the-model-srctimeliners)). It follows the sketch below, with these additions from §8:
> - `Note.vel` and `rel_vel` are 16-bit (MIDI 2.0 M2-115 scaling to and from 7 bits). Notes also have `tune` (semitones), `chance` (%) and `expr`, per-note expression lanes numbered like CLAP note expressions.
> - `Clip.seed` with `ClipRng::new(seed, pass)`, under the rule that a random result depends only on the seed and the loop pass.
> - `TempoPoint.ramp` (exact seconds through ramps), a `SigPoint` time-signature map, and `Marker.track` for per-track markers.
> - Automation values are stored as 0–1 floats and converted at the output. The **innermost lane wins** (clip, then track, then song).
> - `notes` are `Shared<NoteClipData>` (an `Arc`), so linked clips and undo snapshots are cheap.
> - `Track.auto` and `Clip.auto` lanes.
>
> M4 added `audio: Vec<AudioAsset>`, `ClipKind::Audio { asset }` and `Clip.fade_shape`. Not yet included: `Warp` and `Track.fx`. Fields still at their default aren't written.
```rust
/// 960 ticks per quarter note: a row (16th) = 240 ticks; triplets and 1/64 swing are exact.
pub type Tick = i64;
pub const PPQ: Tick = 960;
pub const ROW: Tick = PPQ / 4;

pub struct Timeline {
    pub tracks: Vec<Track>,          // index == mixer channel (≤ MAX_CHANNELS)
    pub clips: Vec<Clip>,            // kept sorted by (track, start); ids are stable
    pub notes: Vec<NoteClipData>,    // pooled piano-roll contents (shared like blocks)
    pub audio: Vec<AudioAsset>,      // pooled audio files (shared by many clips)
    pub tempo: Vec<TempoPoint>,      // tempo map in beats; empty = song.bpm
    pub signature: Vec<SigPoint>,    // 4/4 default; affects ruler/snap only
    pub markers: Vec<Marker>,        // name, tick, colour
    pub loop_range: Option<(Tick, Tick)>,
    pub view: TimelineView,          // zoom, scroll, track heights (UI only)
}

pub struct Track {
    pub name: String,                // mirrors song.channel_names[i]
    pub colour: Option<u32>,
    pub kind: TrackKind,             // Note (tracker + piano roll) | Audio | (later) Group
    pub height: f32, pub folded: bool, pub armed: bool,
    pub fx: Vec<crate::fx::Device>,  // NEW per-track insert chain (after instrument chains)
}

pub struct Clip {
    pub id: u32,
    pub track: u16,
    pub start: Tick, pub len: Tick,  // placement on the timeline
    pub offset: Tick,                // where in the source playback starts (left-trim)
    pub loop_len: Option<Tick>,      // Some = source loops every loop_len (block length by default)
    pub kind: ClipKind,
    pub name: Option<String>, pub colour: Option<u32>,
    pub muted: bool, pub gain: f32,
    pub fade_in: Tick, pub fade_out: Tick,     // audio clips (note clips: velocity ramp, later)
    pub auto: Vec<crate::automation::Lane>,    // clip automation, pos relative to clip start (M5)
}

pub enum ClipKind {
    /// tracker data: column `col` of block `pattern` (rows → ticks at ROW each)
    Block { pattern: usize, col: u8 },
    /// polyphonic notes in a pooled NoteClipData (piano roll)
    Notes { data: u32 },
    /// audio file region
    Audio { asset: u32, warp: Warp },          // Warp::Off (native speed) | Repitch | Stretch (later)
}

pub struct NoteClipData { pub name: String, pub len: Tick, pub notes: Vec<Note> }
pub struct Note { pub start: Tick, pub len: Tick, pub key: u8, pub vel: u8,
                  pub instr: u8, pub fx: Option<(u8, u8)> /* optional tracker command */ }
```

Notes on the model:
- **Block clips reference a single column**, so one block can feed several tracks: drums in block 06 col 1 on the Kick track, col 3 on Hats. The pattern editor still edits the whole block at once. "Make unique" (Ctrl+Shift+D) copies the block so a clip can change on its own, like *Copy as new block* today.
- **Gaps behave like empty rows.** A note keeps ringing across a gap on its track until a note-off or a new note, exactly as an empty block column behaves in the tracker. Conversion is therefore exact, with no clipping of tails.
- **Positions are musical (ticks), not seconds.** Tempo lives in a tempo map (the existing `tempo` automation target becomes its editor). Seconds are derived through the map for the ruler's time display, audio clips and export.
- `song.channels` stays the track count. Adding a track adds a channel, so the mixer, solo/mute and meters keep working unchanged.

### 1.4 Editors for clip content
| Clip | Primary editor | Also |
|---|---|---|
| Block clip | **Pattern editor** (lower panel *Pattern* tab or the full Pattern view), cursor placed on the clip's column and its current row while playing | **Piano roll (mono)** on the same column (M3a): notes as bars on the row grid. Note-offs are bar ends. Effect commands show as small badges and stay editable in an inspector |
| Note clip | **Piano roll** (lower panel *Piano roll* tab): draw, select, move, resize, velocity lane, quantise, legato, transpose (reuses `transpose.rs`), scale highlight from the song key (`chords.rs`) | "Convert to block" writes the notes into a new block, spread over extra columns when they overlap |
| Audio clip | Inline on the timeline (trim, fade, gain, reverse) plus an inspector | "Slice to sampler" sends the region to the existing sample slicer (`sample.rs`) |

### 1.5 Timeline editing (Bitwig-style)
- **Zoom and scroll:** wheel scrolls tracks, Shift+wheel scrolls time, Ctrl+wheel zooms around the mouse, and Alt+wheel changes track height. The ruler zooms from the whole song down to single rows. A mini-map strip lets you jump anywhere. **Follow** keeps the playhead a third of the way in (page-flip as a low-CPU option).
- **Snap:** adaptive grid (bar → beat → 1/16 → row → off), Shift bypasses it, and clips also snap to clip edges, markers and the playhead. Block clips always snap to whole rows, since tracker data is row-based. Note and audio clips can sit anywhere.
- **Clip operations:** drag to move (also across tracks); Alt-drag to duplicate (a linked copy that uses the same block or note data); **drag the right edge** to set the length (past the source end it loops, with notches at the loop points); **drag the left edge** to trim the start (sets `offset`); **S** splits at the mouse or playhead; Ctrl+D duplicates after; **Glue** (Ctrl+J) consolidates the selected clips on a track into one new block or note clip; Del deletes; M mutes the clip; colour, rename and *Make unique* are on the right-click menu.
- **Fades** on audio clips: drag the corner handles, with curve shapes Linear, Equal-power and S. Crossfades appear when two audio clips overlap.
- **Loop region:** Shift-drag on the ruler, or Ctrl+L to set it from the selection. **Markers:** M on the ruler. Markers are named, jumpable with the number keys, and export as cue points / ID3 chapters later.
- **Ranges:** drag in empty space for a time × track selection. Copy, paste, insert time and delete time (ripple) work on all clips and automation in the range.
- **Track headers:** colour strip, number, name, instrument, **M / S / ● arm**, a volume mini-fader, a meter, and fold. Drag to reorder: this reorders channels, which already works in the pattern editor's channel move code.
- **Undo:** every edit goes through the existing song-wide undo stack (`app/undo.rs`), one step per gesture.
- **Keyboard-first** like the rest of ChipWave: arrows step between clips, Enter opens a clip, `[` `]` change the length by one bar, `+` `−` zoom, F11 shows the timeline full height (the same toggle as the pattern view's).

### 1.6 Converting old songs, and backward compatibility
**Order list → timeline (lossless, automatic for the view; once only for timeline mode):**
1. Walk `order`, keeping `start_row` (the sum of block lengths). If a block contains `0F00` (pattern break), its effective length ends at that row.
2. For each order entry and each channel `c` with any non-empty cell in column `c`, create `Clip { track: c, start: start_row·ROW, len: rows·ROW, kind: Block { pattern, col: c } }`. Empty columns produce no clip, which is equivalent because gaps hold the channel state (§1.3).
3. Merge runs of the same block on a track into one looped clip (`loop_len = rows`) so the timeline stays tidy. This is display only and fully reversible.
4. Tempo: `song.bpm` becomes the first tempo point. `0Fxx` tempo commands inside blocks keep working (see §2.4). "Move tempo commands to the tempo lane" is offered, not forced.
5. Song automation (positions in rows) maps 1:1 (`tick = pos·ROW`). Lanes whose target is a channel (`mixer.ch.N.*`, sends) are *shown* under that track. Instrument lanes stay on a "Song" lane group until M5 lets you pin them to a track.
6. Order-list markers and colours (arranger extras) carry over.

**Timeline → tracker** (for MOD/MMD export, the Amiga Lite, and *Back to tracker mode*): **consolidate**.
1. Collect every clip boundary on the row grid across all tracks. Each span between two boundaries is a segment.
2. For each segment, build the tuple (for each track: which block, column and row offset, or empty). Identical tuples are deduplicated into one new block, and the order list is the sequence of segments.
3. Note clips: notes are quantised to rows and written as tracker notes. Overlapping notes go into spare channels (voice allocation), and sub-row timing uses `1F` note delay where the target format has it, otherwise a warning is shown. Audio clips become sample instruments triggered at the clip start (offset via `19` where supported). Anything lossy is listed in one dialog before export. Round-trip test: order list → timeline → consolidate must produce the **same rendered audio** (bit-exact WAV), even if the block numbering differs.

**Files.** Timeline data lives in a new `timeline.toml`, with clip automation in `timeline/auto/*.toml` and audio in `audio/NNN.wav|flac`, stored in the ZIP (or *referenced* by path, with "Collect and save" to embed). Song files in tracker mode are byte-for-byte what they are now.
- **Old readers:** 0.2.4.0 and older ignore `timeline.toml` and would play the stale order list. So **M0 ships the reader check first**: refuse files whose `min_reader_version` is newer than the app understands, with a "made with a newer ChipWave Studio, please update" message (the updater link is already there). Timeline-mode files then write `min_reader_version = 2`. There's also **File → Export tracker-compatible copy…**, which saves a consolidated tracker-mode song.
- **New readers** load tracker-mode files as today and build the derived timeline on the fly (it's never saved until the song switches mode).
- **Interchange (later):** Bitwig's **DAWproject** format is open (MIT, [github.com/bitwig/dawproject](https://github.com/bitwig/dawproject)): a ZIP of XML for tracks, clips, notes, audio, automation and plugin states, read by Bitwig, Studio One, Cubase and others. Import and export map almost directly onto the model above (M7).

---

## 2. Engine changes

### 2.1 Transport and scheduler
Replace "walk `order_pos`/`row`" with a **transport in ticks** whenever the song is in timeline mode (tracker mode keeps today's code path untouched, which also protects the bit-exact demo renders).

```
per render block (frames f0..f1):
  t0 = transport tick at f0 (f64 ticks, advanced by tempo map: ticks/frame = bpm·PPQ/(60·sr))
  t1 = tick at f1
  if loop_range and t1 crosses loop end → split the block at the loop point and wrap
  for each track:
     cursor[track] (index into clips sorted by start) advanced past clips that ended before t0
     for every clip overlapping [t0, t1):
        Block clip   → every row boundary inside [t0,t1) → process_row_for_channel(track, cell)
        Notes clip   → note-on/off events in [t0,t1) → push into the track's event list (sample offset)
        Audio clip   → mark active; the voice reads frames (§2.2)
  render frame by frame as now; at each event's sample offset trigger the voice
```

- **Row boundaries stay global** (every 240 ticks), so block clips start on rows. Every channel still runs `process_row` logic on the same row clock, which is why all effect commands keep their meaning. The row loop becomes **per channel**: `process_row(song)` is split into `process_cell(ch, cell)`, with the source of `cell` abstracted (order list or timeline lookup).
- **Seeking** (ruler click, loop wrap, marker jump) rebuilds the track cursors with a binary search, then **chases state**. It replays the last instrument, note, volume and filter commands per channel since the previous note so that held notes and overrides are correct mid-song (the same chase the tracker needs for "play from cursor").
- **No allocation in the audio thread.** The engine plays an immutable, pre-sorted `Arc<TimelinePlan>` (clips per track sorted, note events flattened per clip), rebuilt on the UI thread after each edit and swapped in under the existing mutex (as `plugin::sync` does). There's a test like `render_is_allocation_free_after_warmup` for timeline playback.
- **Sample-accurate events** for note and audio clips. Block clips fire on tick boundaries as now.

### 2.2 Polyphony (needed for note clips)
Today a channel is one voice. Note clips need chords. Plan: split `Chan` into **`ChanCtl`** (effect state, overrides, mixer gains and the insert chain, one per track) and **`Voice`** (oscillator, envelope, sample position, synth, tail). A track owns a small **voice pool** (8 by default, steal the oldest). Block clips use voice 0 as today (monophonic tracker semantics, glide, retrigger), and note clips allocate from the pool. CLAP instruments are already polyphonic, so they just receive note events with ids. This is the single biggest engine refactor in the plan, so it sits in M3b, after the timeline has proven itself with tracker clips.

### 2.3 Audio clips: playback and streaming
- **Decode** with `symphonia` (MPL-2.0, already a dev-dependency; WAV, FLAC, MP3 and OGG features) and `hound` for WAV as now. Resample to the engine rate once on import, with linear/cubic interpolation (the existing sample code) or `rubato` (MIT) later for higher quality.
- **v1: in memory.** Songs are small, and a 4-minute stereo clip at 48 kHz in f32 is about 92 MB, so cap embedded audio at a few minutes per asset and warn above that. Stored as `Arc<[f32]>` like `SampleData`.
- **v2: streaming** for long files with [`creek`](https://codeberg.org/Meadowlark/creek) (realtime-safe disk streaming, MIT OR Apache-2.0, from the Meadowlark project). A disk thread pre-fills ring buffers ahead of the playhead, and seeks re-prime them.
- **Waveform display:** a min/max peak pyramid (256, 4 096 and 65 536 frames per pixel bucket) built on a background thread at import and cached next to the song (`audio/NNN.peaks`, rebuildable).
- **Warp:** v1 plays at native speed (`Warp::Off`) or **repitched** to follow tempo. Time-stretch (`Stretch`) is a later item, using a phase-vocoder or WSOLA crate after a licence check.
- Clip **gain, fades, reverse** are applied in the voice, and clips route into their track's channel (§2.4).

### 2.4 Routing, and keeping the tracker's effect commands
**Track = mixer channel.** The signal flow per track is: voices → **instrument insert chain** (today's `Instrument.fx`, following the active instrument) → **new track FX chain** (`Track.fx`, needed for audio tracks, which have no instrument) → channel fader, pan and mute → sends A/B → master. Group/bus tracks (sum several tracks, with their own FX) come later as `TrackKind::Group` routed in a tree. The mixer view gains them for free because it is already channel-based.

| Command group | In timeline mode |
|---|---|
| Per-note / per-channel (`01 02 03` slides, `08` decay, `0C 0D` volume, `30–37` Voice, `38–3B` filter, `3C` glide, `3D` accent, `3E` word, `3F–42` Transform, `43` macros) | **Unchanged.** They run on the track's channel state exactly as now, row by row |
| `09` speed (ticks per row) | Becomes **per track** (each `ChanCtl` keeps its own tick counter). It only changes effect resolution, never timing, so tracks with different speeds stay in sync |
| `0Fxx` tempo | Still works and "last writer wins", as in the tracker. On conversion ChipWave offers to move them to the tempo map. When two tracks disagree in the same row, a warning badge appears on the clip |
| `0F00` pattern break (later `0B`, `1D` jumps) | **Clip-local:** ends the clip's source early (the clip shows a "break" notch). They never jump the song transport in timeline mode, because song structure is the timeline |
| Automation lanes | Same evaluator (every 32 frames). Positions become ticks (`pos·ROW`). Clip automation is evaluated relative to the clip and multiplies or overrides the track lane |

### 2.5 Export
`render_song` gains the timeline path (same engine, offline). Stems per track become trivial (render with solo per track). WAV/MP3 export stays sample-identical to realtime, since automation already runs on a fixed grid.

---

## 3. UI design in egui

### 3.1 Wireframe
```
┌ Transport · Tempo · Octave · Edit ··········· VIEW [Pattern|Timeline|Mixer|FX|Sample] ··· MIDI Panic ┐
├──────────────────────────┬───────────────────────────────────────────────────────────┬──────────────┤
│ Tracks 16   [+Note][+Audio]│ TIMELINE ▲Pointer ✎Pencil ✂Split ⌫Erase │Snap 1/16│⟳Loop│Zoom│ Browser      │
│                          │ ▌Intro      ▌Groove     ▌Verse             ▌Chorus  ▼playhead│ [Sounds|Samp │
│                          │ 1     3     5     7     9    11    13    15 ═17═══19═══21══ │  les|Plugins]│
│ 01 Kick    M S ● ━━━━○ ▮ │ [00 Intro ][02 Groove ⟲  ⟲  ⟲      ][06 Chorus ⟲   ⟲     ]│ search…      │
│ 02 Bass    M S ● ━━━━○ ▮ │       [01 ][02 Groove][04 Verse ][05 Verse2][06 Chorus     ]│ ▾ Factory    │
│  ↳ Filter cutoff  R W    │ ●────────────●──────────●─────────────────╱●──●───────────●│   ▸ Drums 32 │
│ 03 Hats    M S ● ━━━○  ▮ │             [02 Groove][04 Verse ⟲      ][06 Chorus      ]│   ▾ Pads 16  │
│ …                        │                  …                                         │ • Disco Str. │
│ 13 Choir   M S ● ━━━━○ ▮ │                                         [▣06 Chorus][07 …]│ • Velvet Sw. │
│ 16 Vox chop(audio) M S ● │                              [∿∿wav∿∿]     [∿∿wav∿]      │ ┌preview───┐ │
│ ▸ 4 folded               │ ═════════════ scrollbar ═══════ │ mini-map of whole song    │ │∿∿∿ ▶ C-3 │ │
├──────────────────────────┼─[Piano roll|Pattern|Instrument|Automation]─────────────────┴──────────────┤
│ Device chain             │  Lower panel (existing): knob cards for the selected track's instrument,  │
│ 0B Cyber Choir (Voice)   │  or the piano roll / pattern editor of the selected clip, or its          │
│  ▼ Insert FX             │  automation. Splitter-resizable; F11 hides it (full-height timeline).     │
│  ▼ Track FX              │                                                                           │
│  ▼ Fader / sends         │                                                                           │
└──────────────────────────┴───────────────────────────────────────────────────────────────────────────┘
```

- **View:** a new top-level **Timeline** view (**Ctrl+T** as built; Ctrl+3 is the Mixer). The existing single-lane **Arranger** remains as the tracker-mode song overview and becomes the timeline's "Blocks" summary lane in tracker mode, so nothing familiar disappears.
- **Left column:** track headers in the top area. In the bottom area the instrument list becomes a compact **device chain** for the selected track (instrument → insert FX → track FX → fader/sends). Each card opens in the lower panel. The classic instrument list stays one click away (a toggle at the top, and in the Pattern view as now).
- **Lower panel:** the existing knob-card editor, plus two new tabs: **Piano roll** and **Pattern** (an embedded pattern editor for the selected block clip). Selecting a clip selects its track's instrument, so the panel always shows "what this clip plays".
- **Right browser** (collapsible, Ctrl+B): tabs for *Sounds* (factory sounds, the user library and song instruments, from the Instrument Library), *Samples* (folders, with preview), *Plugins* (CLAP now, VST3 later) and *Files* (songs, MIDI, audio). Drag onto a track header to set its instrument, onto empty space to create a track, onto the timeline to create an audio clip, or onto the lower panel to load a preset. The preview plays through the cursor track's channel, using the same path as live MIDI.
- **Clip launcher** (M6, optional): a column of slots per track between the headers and the timeline (as in Del's screenshot), toggled with "Scenes ▸".

### 3.2 Rendering and performance
- **One custom widget** using `egui::Painter` inside a clipped rect (not one egui widget per clip). Geometry and hit-testing are our own: binary search per visible track, plus a per-track "max clip length" for the left bound, giving O(visible) work. A single `interact` rect handles all pointer input, the same pattern the current arranger uses.
- **Culling:** only visible tracks × visible time range are painted. Text is skipped on clips narrower than about 24 px, and note previews on clips narrower than 6 px.
- **Cached previews:** each clip's content (note ticks or a waveform) is rendered into a `TextureHandle`. The key is the content id and edit counter (block edit counter, note data version, asset id) plus a zoom bucket (powers of two). Previews are drawn as textured quads, so a 2 000-clip song is a couple of thousand quads per frame. Waveforms come from the peak pyramid level nearest to the zoom.
- **Batching:** grid lines, clip bodies and headers go into one `Mesh` per layer per frame instead of thousands of `rect_filled` calls.
- **Budget:** under 2 ms of UI time for 2 000 clips × 32 tracks at 1080p, checked by a kittest benchmark scene (`egui_kittest` is already a dev-dependency) in CI.
- **Playhead:** read once per frame from the engine (as `song_pos` is today). Repaint is requested only while playing or dragging.

### 3.3 Crates considered (and why we build our own widget)
| Crate | Licence | Fit |
|---|---|---|
| [`egui_timeline`](https://github.com/mitchmindtree/egui_timeline) 0.3.0 | MIT OR Apache-2.0 | General timeline widget, but pinned to egui 0.29 (ChipWave is on 0.37) and not updated since Mar 2025. Useful as a reference only |
| [`armas-audio`](https://github.com/PoHsuanLai/Armas) 0.2.2 | MIT OR Apache-2.0 | egui DAW widgets (Timeline, TimelineTrack, PianoRoll, WaveformDisplay, AutomationEditor). Young (May 2026), its own theme system, and a different egui version. Worth reading for its piano-roll interaction ideas |
| `egui_extras` (part of egui) | MIT OR Apache-2.0 | `TableBuilder` and `StripBuilder` are handy for the browser and track-header layout. Nothing timeline-specific |
| [`egui-snarl`](https://github.com/zakarumych/egui-snarl) 0.12 | MIT OR Apache-2.0 | Node-graph editor for egui. A candidate for the Grid UI (§5.5) |
| [Meadowlark](https://codeberg.org/Meadowlark/Meadowlark) | **AGPL-3.0** (app) | Open-source Rust DAW, incomplete, moved from GitHub to Codeberg in Sep 2025. AGPL code can't be copied into GPL-3 ChipWave without making the combination AGPL, so don't copy it. Its *libraries* are permissive and usable: `creek` (disk streaming), `symphonium` (file loading), `meadow-dsp-essentials`, and the Dropseed engine (check each licence when adopting) |

The current arranger already owns most of the hard parts (painter-based clips, cached previews, ruler seek, follow), so a custom widget is the lower-risk path. Taking a dependency would mean fighting an egui-version lag on every egui update.

---

## 4. Milestones (each one shippable and testable)

Estimates are focused build days for one developer at the pace of recent ChipWave releases (tests, screenshots, docs and a Windows self-test included). LOC is new or changed Rust.

| # | Milestone | What ships | Tests / done when | Size |
|---|---|---|---|---|
| **M0** ✅ | Groundwork | `Tick`/`PPQ`, the `Timeline` model and serde, the order→timeline converter and consolidator (not yet in the UI), the **`min_reader_version` check**, and a fix for the `lines_per_beat` vs 4-rows-per-beat mismatch in `arrange.rs` | Unit tests: convert → consolidate → **bit-exact render** for the three demos and the test songs. Old songs load unchanged | 2–3 d, ~1.2k LOC |
| **M1** ✅ | Read-only Timeline view | The Timeline view of the current song: tracks = channels with headers (M/S, volume and meters wired to the mixer), per-track block clips with previews, ruler (bars/time), markers from the arranger, zoom/scroll/follow, mini-map, playhead and ruler seek. Double-click opens the pattern editor at that block and column | Kittest snapshots. The performance benchmark scene. Clicking the ruler seeks exactly as in the arranger | 3–4 d, ~1.8k LOC |
| **M2** ✅ | Editable timeline and engine scheduler | Timeline mode (switch prompt and undo), the tick transport with per-track cursors and state chase, per-track `09` speed. Move, resize (loop), left-trim, split, duplicate (linked), make unique, glue, delete and mute. Snapping, loop region, markers, range select, insert/delete time. Save/load `timeline.toml`, export of a tracker-compatible copy, MOD/MMD export via consolidate. The browser panel (Sounds tab) | Engine tests: a clip at bar N plays at frame X. Loop wrap is sample-exact. Seek chase gives the same output as playing from the start. Allocation-free render. Round trips | 7–9 d, ~4k LOC |
| **M3a** | Piano roll (mono, on block columns) | A Piano roll tab that edits a block clip's column as bars on the row grid (draw, move, resize = note-off, velocity → volume column), with effect-command badges | Edits round-trip to identical pattern cells | 3 d, ~1.5k LOC |
| **M3b** | Note clips (polyphonic) | The `ChanCtl`/`Voice` split with per-track voice pools, `Notes` clips with free timing, the full piano roll (velocity lane, quantise, scale highlight, chord stamp from `chords.rs`), MIDI file import into note clips (`midly`, Unlicense), and live MIDI recording into note clips | Polyphony tests (chords, voice stealing). Tracker songs render bit-exact after the voice split. CLAP gets note ids | 7–9 d, ~3.5k LOC |
| **M4** | Audio clips and track FX | Audio tracks and import (drag from the browser or the OS), waveforms, gain, fades, crossfades, reverse, repitch-to-tempo, the per-track FX chain, "Slice to sampler", embed vs reference plus "Collect and save" | Decoding tests (WAV/FLAC/MP3). Fade curves. Playback aligned to the frame. Memory cap warning | 5–7 d, ~3k LOC |
| **M4b** | Streaming | `creek` disk streaming for long files, with peak-file caching | Seek and loop stress tests with no underruns | 2–3 d, ~0.8k LOC |
| **M5** | Track and clip automation | Lanes under each track (fold/unfold), pin any lane to a track, clip automation drawn on clips (moves with the clip), pencil/line/curve tools, the tempo map lane, and "record knobs" in timeline mode | Evaluator tests for clip lanes relative to the clip, and for tempo-map seconds ↔ ticks | 4–5 d, ~2k LOC |
| **M6** | Clip launcher (optional) | Slot grid per track, scenes, launch quantise (bar/beat), "record launcher to timeline" | Launch timing tests | 6–8 d, ~3k LOC |
| **M7** | DAWproject import/export (optional) | Exchange with Bitwig, Studio One and Cubase: tracks, clips, notes, audio, automation (plugin states where the format allows) | Round trip with the official example files | 4–5 d, ~2k LOC |

**Done (19 Oct 2026):** M0, M1, M2, M3a, M3b and M4. M4 notes: audio clips (WAV/FLAC/MP3/Ogg via Symphonia) with drop/import, waveforms, move/trim/loop/split, fades (equal power, linear, S-curve; crossfades by overlapping), gain, embed vs reference with relinking of missing files, and export. Instead of resampling once at import, files keep their own rate and the voice converts it while playing (Hermite), so a song exports the same at any rate. Audio voices are a separate preallocated pool per track (`src/engine/audioclips.rs`) that only runs when the plan has audio, so other songs stay bit-exact. Songs with audio need reader version 3. Not done in M4 and moved on: the per-track FX chain, reverse, repitch/warp, Slice to sampler, Collect and save, streaming (M4b), MIDI file import, CLAP note ids/expressions. M3 notes: the piano roll edits block columns (M3a, edits round-trip to identical cells) and note clips (M3b). Instead of splitting `Chan` into `ChanCtl`/`Voice`, note clips got their own preallocated pool of 8 voices per track (`src/engine/notes.rs`) that feeds the track's chain. Block clips keep the old one-voice path, so tracker songs stay bit-exact with no refactor. Live MIDI recording into note clips and Capture are in. Not done in M3: MIDI file import (`midly`), CLAP note ids and expressions, converting note clips to blocks. These move to M4. M2 notes: M2 notes: `09xx` stays song-wide in timeline mode (per-track speed is a track setting instead), so converted songs stay bit-exact; timeline-mode playback renders the same samples as the tracker for every demo and fixture. Not done in M2 and moved to M3: range select with insert/delete time, snapping to clip edges and markers, a MOD/MMD exporter (none exists yet; the tracker-compatible copy is a `.cwt`) and the browser panel. Earlier: M0 and M1. Every song renders bit-exactly through order list → timeline → consolidated order list. That covers the three demos, two fixture songs from 0.2.3.5/0.2.3.6, and a synthetic song with breaks, odd lengths and a 300-row block. The Timeline view has kittest UI tests and a frame-time test with more than 9,000 clips. The shortcut is **Ctrl+T** (Ctrl+3 was already Mixer). Limits and what M2 needs: [TIMELINE.md § Limits](TIMELINE.md#limits-in-this-version).

**Core timeline (M0–M5): about 33–43 days.** Del gets something useful after M1 (about a week) and a real DAW workflow after M2 (about 2.5 weeks).

---

## 5. Roadmaps for the other approved big items

### 5.1 Plugin effects (CLAP FX in the FX chains), *do first, it's small*
**Built (19 Oct 2026, unreleased)** for the instrument FX chains: `FxKind::Plugin` devices, the effect list from the scanner, the device panel (status, editor, parameters), automation lanes per CLAP parameter, state in the song, missing plugins bypassed with a notice, and plugin delay compensation (also in export). Return buses and the master have no device chains yet, so plugins can't go there until those chains (or M4's track chains) exist. Guide and limits: [FX.md › Plugin effects](FX.md#plugin-effects-clap-unreleased).
- Add an `FxKind::Plugin` device (holding a `PluginPatch`, `src/fx.rs`) so a CLAP effect can sit in any instrument chain, the new track chain, the return buses and the master. This reuses `plugin::clap::ClapInst` (stereo in/out audio ports, params, state, native GUI) and the out-of-process scanner (filter on the `audio-effect` feature).
- Its parameters appear as knob cards and automation targets, the same as plugin instruments' macros and the *Advanced* list.
- **Latency:** read the `latency` extension. Report it per chain and add **plugin delay compensation** per track (delay the other tracks), also in export.
- Size: **4–6 d**. Unblocks M4's track FX being genuinely useful, and VST3 FX later.

### 5.2 VST3 hosting
- **Licence (verified):** Steinberg moved the VST 3 SDK to the **MIT licence with version 3.8.0 on 20 Oct 2025**. The GPLv3/proprietary dual licence is retired. Obligations: keep the copyright notice and MIT text (add them to `credits.rs` and `LICENSES/`). Using the "VST" name or logo is optional and, if used, must follow Steinberg's usage guidelines. The **VST2 API is not covered**, so VST2 stays unsupported. Sources: [VST 3.8.0 release notes](https://steinbergmedia.github.io/vst3_dev_portal/pages/Versions/Version+3.8.0.html), [VST 3 Licence page](https://steinbergmedia.github.io/vst3_dev_portal/pages/VST%2B3%2BLicensing/VST3%2BLicense.html), [Steinberg press release, 28 Oct 2025](https://www.steinberg.net/press/2025/vst-3-8/), [Steinberg SDK page](https://www.steinberg.net/developers/vstsdk/). MIT is compatible with ChipWave's GPL-3.
- **Rust bindings:** [`vst3`](https://crates.io/crates/vst3) 0.3.0 (coupler-rs/vst3-rs, **MIT OR Apache-2.0**, Dec 2025). These are bindings generated from the SDK headers plus COM helpers, with no host abstraction. `vst3-sys` (RustAudio) is git-only and older, so prefer `vst3`. There's no mature safe Rust VST3 *host* crate, so we write the host layer (about the size of today's `plugin/clap.rs`).
- **Work:** module scan (`moduleinfo.json` when present, else instantiate in the scanner process), `IPluginFactory` → `IComponent` + `IAudioProcessor` + `IEditController` (connected through `IConnectionPoint`), bus arrangements, `process()` with `IEventList` notes and `IParameterChanges` (sample offsets), state (component + controller streams into `plugins/NN.bin`), `IPlugView` editor embedding (HWND on Windows, NSView on macOS, X11 on Linux), `IComponentHandler` for edits and automation recording, and latency. Instruments and effects share it.
- Size: **10–14 d** (instruments and FX). Unlocks Dexed and the many VST3-only plugins.

### 5.3 Wavetable synth (a new instrument type)
- **Decision: write an original Rust implementation; don't port Vital's code.** Vital ([github.com/mtytel/vital](https://github.com/mtytel/vital)) is **GPL-3.0-or-later**, so its code *could* legally be used in GPL-3 ChipWave with its copyright notices kept. But it's deeply tied to JUCE and C++ SIMD classes, so a port costs about as much as writing it fresh. Its README also forbids using the names "Vital", "Vital Audio", "Tytel" or "Matt Tytel" to market or name builds, and its **factory presets and wavetables are not redistributable**. A clean implementation avoids all of that. We read Vital and Surge XT's `WavetableOscillator` (Surge is also GPL-3.0-or-later) for *ideas* only. If any routine is ever ported line by line, that file carries the original copyright and GPL notice.
- **Credit:** "Wavetable synth design inspired by Vital by Matt Tytel" in the credits and docs. Don't use the Vital name in the instrument's name or marketing. Suggested name: **"Wave"** (Generator family) or "Spectra".
- **Design:** 2 wavetable oscillators (up to 256 frames × 2 048 samples, smooth frame interpolation), band-limited by **per-octave FFT mipmaps** built at load (`realfft`/`rustfft`, MIT OR Apache-2.0). Warp modes: bend, sync, squeeze, PWM-style, mirror, quantise, FM/RM from osc 2, and later *spectral morph* (shift, stretch, low/high-pass in the frequency domain). Unison up to 8 voices with detune and stereo spread. Sub and noise. Routed into the existing filter, envelopes and FX. **Import:** single-cycle and multi-frame WAV (2 048-sample frames, Serum-style `clm` chunk detection), plus "draw/convert" from any ChipWave sample. **Factory tables are generated by us** (chip pulse sets, formant sweeps from `voice.rs`, FM sweeps), so every one is redistributable.
- Built **after drag-to-modulate** so its frame position and warp knobs are modulation targets from day one.
- Size: **8–12 d**.

### 5.4 Drag-to-modulate (Bitwig-style modulators)
**Status: built** (19 Oct 2026, unreleased), see [MODULATION.md](MODULATION.md): the registry for synth-patch knobs (`instrument.N.patch.<key>`, also automatable), per-instrument modulators (LFO, envelope, random, velocity, keytrack, mod wheel, macro, instrument LFO), drag a chip onto any instrument knob or FX slider (built-in and plugin), depth rings with handles, Alt-drag, live dot, Routings card, automatable depth (`mod.K.amount`), `modulation.toml`. Not yet: per-voice modulation, aftertouch, step sequencer and envelope follower sources, mixer drop targets, plugin instrument macros.
- **Foundation, a parameter registry:** one id space for every continuous parameter, covering instrument patches, generator macros and knobs, FX devices, mixer and plugin params. It's built from the knob-card definitions that `params.rs`/`instpanel.rs` already use for search. The same id is used by automation, MIDI learn and modulation. This also closes the **"automation can't move generator knobs"** gap found while making the disco demo.
- **Modulators** per instrument (and per track for FX): LFO (synced/free, shapes, phase, retrigger), ADSR envelope, macro knob, random/S&H, keytrack, velocity, aftertouch/mod wheel, and later step sequencer and envelope follower. They are **per voice** where the target is per voice (filter cutoff, wavetable position) and global otherwise.
- **UI:** every modulator has a routing handle. Drag it onto any knob, or click the handle and then drag a knob, to set a **bipolar depth**. The knob draws a coloured arc showing the modulation range and a moving dot for the live value. Alt-drag on a knob edits the depth of the selected modulator. A "Modulation" card lists all routings (source → target, depth, polarity) for precise edits. The lower panel already has a *Modulation* tab to grow into.
- **Engine:** evaluated at control rate (every 32 frames, like automation) with per-voice smoothing, using the same override path as `AutoState`. No allocation; capacity is reserved per instrument.
- Size: **8–10 d** (registry 3, modulators and engine 3, UI 3).

### 5.5 Grid (modular instrument / FX)
- A **patchable instrument and FX type**: modules (oscillators incl. wavetable, filters, envelopes, LFOs, math, logic, mix, delay, shaper, Voice formant, chip pulse/noise, note in/out, audio in/out) connected by cables. Polyphonic: one graph instance per voice.
- **Engine:** the graph is compiled on the UI thread into a flat, topologically sorted schedule of module `process(block)` calls with preallocated buffers. Feedback cables get a one-block (or one-sample, in "precise" mode) delay. Hot-swapped like `TimelinePlan`, with nothing allocated in the audio thread. Module parameters register in the parameter registry, so drag-to-modulate and automation work inside the Grid.
- **UI:** a node editor in the lower panel ([`egui-snarl`](https://github.com/zakarumych/egui-snarl), MIT OR Apache-2.0, as a candidate, or our own painter if snarl's look can't match Dark Circuit), with ChipWave-style module cards and knobs. Patches save as `.cwi` instruments and presets.
- Size: **20–30 d** for a solid first module set (~30 modules). Last, because it reuses everything above.

---

## 6. Recommended order (timeline plus everything else)

| Step | Item | Why here | Size |
|---|---|---|---|
| 0 | **Quick fixes in flight:** Ret B strip, open the lower panel on the instrument editor, engine gaps (phaser, frequency shifter, filter commands for generators) | Already promised and small | ~3–5 d |
| 1 | **Timeline M0–M2** | Del's headline ask. M1 alone makes a great release | 12–16 d |
| 2 | **Plugin effects (CLAP FX)** + plugin delay compensation | Small, reuses the CLAP host, and gives track chains real power before audio tracks | 4–6 d |
| 3 | **Timeline M3a/M3b** (piano roll, note clips, polyphony) | Completes the "traditional DAW" feel. The voice split also helps the wavetable synth | 10–12 d |
| 4 | **Parameter registry + drag-to-modulate** | Fixes the generator-automation gap. A foundation for the wavetable synth, the Grid and M5 | 8–10 d |
| 5 | **Timeline M4 (+M4b) and M5** (audio clips, track FX, track/clip automation using the registry) | Audio and automation on a proven timeline | 11–15 d |
| 6 | **VST3 hosting** (instruments + FX) | Bigger plugin catalogue. Shares the FX-in-chain work from step 2 | 10–14 d |
| 7 | **Wavetable synth** | Modulation-ready from day one | 8–12 d |
| 8 | **Grid** | Reuses the registry, modulators, wavetable osc and polyphony | 20–30 d |
| 9 | Optional: **clip launcher (M6)**, **DAWproject (M7)** | Nice to have. Do them when asked | 10–13 d |

Each step ends in a normal release (installer, notes, screenshots). The timeline steps keep tracker mode bit-exact, which is checked by the demo-render tests on every build.

## 7. Open questions for Del
*Answered by Del on 19 Oct 2026: tracks are channels; new songs start on the timeline; old songs open as they are and stay bit-exact (§1.2).*

1. **Track = channel (recommended)**, or tracks per instrument? Channels keep all tracker semantics and the mixer as they are. A per-instrument view could later be a filter on the track list.
2. Should **timeline mode be the default for new songs** once M2 ships, or stay opt-in? (Recommended: opt-in until M3b.)
3. Is the audio memory cap OK (a few minutes per clip embedded, streaming in M4b), or should streaming move earlier?
4. Name for the wavetable instrument ("Wave", "Spectra", or something else; not "Vital").

## 8. MIDI ideas (from Microsoft's Windows MIDI Services)
Research on [microsoft/MIDI](https://github.com/microsoft/MIDI), including Microsoft's own MIDI 2.0 timeline app (Windows MIDI Sequencer, 960 PPQ like ours), is in [research/microsoft-midi.md](research/microsoft-midi.md). Proposed for **M0** (cheap now, a format migration later): `Note.vel` as 16-bit plus release velocity, per-note tuning, chance and per-note expression (maps onto CLAP note expressions), tempo ramps, per-track markers, and a `Clip.seed` with the rule "random results depend only on seed + pass" (keeps export bit-exact). **In M0 (19 Oct 2026):** all of these are in the model and saved with defaults. The engine doesn't use them yet. It also proposes recording with **Capture** in M3b, and a new "M2.5 MIDI out" step after M2: look-ahead scheduler, per-device offset, MIDI clock/SPP/MTC.
