# MIDI keyboard input (0.2.3, toolbar toggles 0.2.3.1)

ChipWave 0.2.3 plays and records from a MIDI keyboard. Windows is the main target (WinMM through [midir](https://crates.io/crates/midir), MIT licence). Linux (ALSA) and macOS (CoreMIDI) use the same code.

The wider plan (MIDI output, clock and plugins) is in [MIDI_VST.md](MIDI_VST.md).

## Setting it up

**Settings > MIDI** (or click the **MIDI** light on the right of the toolbar):

| Setting | What it does |
|---|---|
| Input device | **None**, **All devices**, or one port. Your choice is saved. |
| Rescan | Looks for newly plugged-in devices. |
| Connect automatically | At start, connects to the last device. It also rescans every 2 s, so unplugging and replugging reconnects by itself. |
| MIDI channel | **All**, or 1–16 (the keyboard must send on that channel). |
| Velocity | On or off, with a curve: Linear, Soft (a light touch is already loud), Hard (needs a firm touch), Fixed (always full). |
| MIDI octave | Shifts the keyboard by whole octaves. With 0, middle C (key 60) plays **C-4**. This is separate from the computer-keyboard octave (F1–F8). |
| Pitch bend range | 1–24 semitones (default 2). |
| While playing: record at the playing row | Live record (on by default). |
| Chords: spread | Chord notes go into the cursor channel and the channels to its right. |
| MIDI learn | Lists the learned controls, with a Forget button for each. |

**Toolbar light:**
- grey: not connected;
- dark green: connected;
- bright green: flickers on every message.

Hover over it to see the device names.

**Command line (Windows: run from `cmd` in the ChipWave folder):**
- `ChipWave.exe --midi-list` lists the MIDI inputs Windows can see.
- `ChipWave.exe --midi-monitor 15` connects to every input for 15 s and prints each note, pedal, bend, CC and key/channel pressure that arrives. Use it to check a keyboard without the GUI.

## Playing

- **What sounds:** the keyboard plays the **current instrument** (the one selected in the list), with velocity.
- **Polyphony:** 12 live voices. Pressing the same key again retriggers its voice. When all voices are busy, the oldest released voice is stolen first, then the oldest held one.
- **Sustain pedal (CC 64):** holds released notes until the pedal goes up.
- **Pitch bend:** works for synth and sample instruments.
- **Key pressure (aftertouch)** (0.2.14.0): pressing harder on a key you are already holding. Keyboards send it either per key (**poly pressure**, status 0xA0) or for the whole keyboard (**channel pressure**, 0xD0); channel pressure counts as pressure on every held key. Add a **Pressure** modulator (the quick-mod popover's *Pressure* source, or the Movement list) to make it move the cutoff, volume, vibrato or any other target. It follows the hardest-pressed held key of that instrument and falls back to 0 on each new strike. Hover the toolbar's MIDI light to see a live pressure meter ("Key pressure (key 60) 42 %" or "Channel pressure 42 %").
- **All notes off (CC 120 / 123):** stops everything.
- **Routing:** live notes go through the instrument's FX chain, then the **cursor channel's fader, pan and sends**. They ignore that channel's mute and solo, so you can still hear yourself while jamming on a muted channel.
- **Latency:** notes reach the audio engine through a lock-free queue. The audio side takes no locks and makes no allocations for MIDI. The time to sound is the audio latency shown in the toolbar (plus at most one 256-frame engine block, about 5 ms). A smaller buffer in Settings > Audio lowers it.

## Recording

Recording needs **edit mode** (Esc) and the **Pattern** view. Each note writes:
- the note (with the MIDI octave applied);
- the current instrument;
- if velocity is on, the velocity in the volume column (00–40).

**Step record** (song stopped, or live record switched off):
- The note goes at the cursor, and the cursor moves down by **Step**. With Step 0 it stays put.
- **Toolbar shortcuts (0.2.3.1):** next to the MIDI light, **Chord** switches chord spread on/off and **Rec live / Rec step** switches the record mode (it turns red when edit mode is on, i.e. MIDI notes will be recorded). Hover them for a tooltip.
- **Chord spread** on: the notes of a chord go into adjacent channels on the same row, and the cursor moves once all the keys are released.

**Live record** (song playing and live record on):
- The note goes into the **playing** block at the row you heard when you pressed the key. The row is rounded to the nearest row, after allowing for the audio latency and the screen delay.
- The cursor isn't moved for you. With Follow on it tracks playback as usual.
- Note-offs are not recorded.

**On the Timeline view in timeline mode** (0.2.4.1): live record writes into a **note clip** on the cursor track. Notes keep their exact time, length and velocity, and nothing is rounded to rows. Right-click empty space › **Capture played MIDI here** turns the phrase you just played into a note clip, even if you weren't recording. See [TIMELINE.md › Piano roll and note clips](TIMELINE.md#piano-roll-and-note-clips).

**Record on the Timeline** (0.2.5.0): the toolbar's **Record** button (●) or **R**. The armed (or selected) track records what you play, in time, with lengths and velocities. Chords and overlapping notes are kept. The notes go into the note clip under the playhead, or a new Take clip; loop recording overdubs into one clip. Recording from stopped counts in first: one bar by default, or 2 bars or Off from the arrow next to the toolbar's **Metronome** (or right-click it, or Settings › Recording › Count-in). The count-in clicks even with the metronome off; the song waits, Record blinks, the position reads "Count-in 4, 3, 2, 1" and the Metronome symbol shows the beats left. Notes played in the count-in sound but aren't recorded; click Record (or **R**) to cancel it. The **Metronome** toggle clicks on every beat while the timeline plays; its symbol swings with the beat and flashes on the downbeat. Quantise on record is a setting, off by default (Settings › Recording). The take is one undo step. **Live block** (0.2.13.0): as soon as recording starts a red **REC** block opens on the track and grows with the playhead, with no length limit; each note appears as you play it with a short fade-in, and on Stop the block settles into a normal clip rounded to the bar (a take with no notes is dropped). **Step input** (one note after another, chords on one step) is the piano roll header's toggle, off by default. **Keys → Notes** (Ctrl+K, the toolbar's keyboard icon button, or the piano roll header's) plays and records from the computer keyboard the same way: **A S D F G H J K L ;** white keys, **W E T Y U O P** black keys, **Z / X** octave, **C / V** velocity. The toolbar icon is lit while it's on; hover it for the octave and velocity ("Computer keys play notes · C4 · vel 102"). Off, those keys are normal shortcuts again. It starts on when no MIDI keyboard is found at start (system loopbacks such as ALSA's "Midi Through" don't count) and off when one is. Once you switch it yourself (icon or Ctrl+K) that choice is saved and used from then on. Until then it also follows the selected MIDI input's hot-plug: connecting turns it off, unplugging turns it on. Typing in text fields never plays notes.

**Key pressure while recording** (timeline note clips, 0.2.14.0): each recorded note keeps a **pressure curve**: what your keyboard sent while you held it, latency-compensated like the notes. Steady pressure is stored as one point, and notes with no pressure store nothing, so files stay small and old songs unchanged. On playback the curve drives the instrument's **Pressure** modulators exactly as when you played it, and returns to 0 at the note's end. In the piano roll, open the lower lane's menu (click its label) and choose **Pressure**: each note shows its curve, notes with a curve get a shaded foot (brighter = harder), drag along a note to draw or redraw it (one undo step per stroke), and right-click a note in the lane to clear it.

**Importing .mid files** (0.2.14.0): MIDI files in the browser's **MIDI** tab (or found in your folders) import as one **note clip**: double-click / Enter puts it on the cursor track at the play marker, or drag it onto a track. Every track and channel of the file goes into the clip with its velocities, and poly or channel pressure becomes each note's pressure curve. Times are in beats, so the clip follows the song's tempo; tempo changes, controllers and program changes in the file are not read. SMPTE-timed files are refused with a message. One undo step.

**Undo:** in the Tracker every recorded note is its own undo step (Ctrl+Z removes the last note). On the Timeline a take is one step.

Edit mode off means play only: nothing is written.

## MIDI learn

1. Right-click one of these and choose **MIDI learn**:
   - a mixer **fader**, **pan** or **send A/B**, or the **master** fader;
   - the instrument **Volume**, **Cutoff**, **Resonance** or **Drive** slider;
   - the toolbar **BPM**.

   A coloured ring shows it is waiting.
2. Move a knob, slider or the **mod wheel (CC 1)**. That controller now drives the control. The binding is saved in the settings, not in the song.
3. Right-click again and choose **Forget MIDI CC n** to remove it, or use the list in Settings > MIDI.

Learned moves are song edits. A continuous sweep becomes one undo step.

## How it works (for developers)

- **`src/midi.rs`:**
  - parser: note on/off (velocity 0 = off), CC, pitch bend, poly key pressure (0xA0) and channel pressure (0xD0, sent as `ALL_KEYS`); anything else is ignored;
  - velocity curves and key-to-note mapping;
  - `Config`: atomics written by the UI and read by the MIDI thread;
  - `Hub`: connections, hot-plug rescan, and two rtrb queues (engine and UI). A full queue drops events instead of blocking.
- **`src/engine.rs`:** `Live`, the live voice pool and its own FX chain state. The queue is drained at the start of every `render`, and `live_event` doesn't allocate. The queue survives an audio device change.
- **`src/app/midiview.rs`:** recording (step, live, chord spread), MIDI learn (`apply_learned`), the settings card, the toolbar light and its pressure meter.
- **Pressure:** `LiveEv::Pressure` → `Engine::live_pressure` writes `NoteFeed::press` (per instrument) for `ModKind::Pressure`. Recorded curves live in each note's `ExprKind::Pressure` expression (`Note::press_push`, `press_set`, `pressure_at`); `plan::note_press_spans` turns them into `Plan::press` step points (a closing 0 at each note's end; notes skipped by a condition or chance drop theirs) and `engine/notes.rs` applies them, seek-stable. The plan key hashes them only when present, so songs without pressure render bit-identically. `src/app/noterec.rs::rec_pressure` records, `src/app/prlanes.rs` (Lane::Pressure) edits.
- **`src/smf.rs`:** the Standard MIDI File reader (format 0/1, running status, meta/sysex skipped) used by `src/app/midiimport.rs`.
- **Tests:**
  - parser and mapping (`midi::tests`), pressure (`pressure_messages_parse_and_translate`, `timeline::press_tests`, `app::prpress_tests`), .mid import (`smf::tests`, `app::midiimport::tests`);
  - live voices, sustain, bend, stealing, velocity and fader (`engine::tests::live_*`);
  - no allocation while MIDI events stream in during playback (`live_midi_through_queue_does_not_allocate`);
  - UI tests that inject raw MIDI bytes into the real app (`app::ui_tests::midi_*`).
