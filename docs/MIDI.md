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
- `ChipWave.exe --midi-monitor 15` connects to every input for 15 s and prints each note, pedal, bend and CC that arrives. Use it to check a keyboard without the GUI.

## Playing

- **What sounds:** the keyboard plays the **current instrument** (the one selected in the list), with velocity.
- **Polyphony:** 12 live voices. Pressing the same key again retriggers its voice. When all voices are busy, the oldest released voice is stolen first, then the oldest held one.
- **Sustain pedal (CC 64):** holds released notes until the pedal goes up.
- **Pitch bend:** works for synth and sample instruments.
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

**On the Timeline view in timeline mode** (unreleased): live record writes into a **note clip** on the cursor track. Notes keep their exact time, length and velocity, and nothing is rounded to rows. Right-click empty space › **Capture played MIDI here** turns the phrase you just played into a note clip, even if you weren't recording. See [TIMELINE.md › Piano roll and note clips](TIMELINE.md#piano-roll-and-note-clips).

**Undo:** every recorded note is its own undo step (Ctrl+Z removes the last note).

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
  - parser: note on/off (velocity 0 = off), CC, pitch bend; anything else is ignored;
  - velocity curves and key-to-note mapping;
  - `Config`: atomics written by the UI and read by the MIDI thread;
  - `Hub`: connections, hot-plug rescan, and two rtrb queues (engine and UI). A full queue drops events instead of blocking.
- **`src/engine.rs`:** `Live`, the live voice pool and its own FX chain state. The queue is drained at the start of every `render`, and `live_event` doesn't allocate. The queue survives an audio device change.
- **`src/app/midiview.rs`:** recording (step, live, chord spread), MIDI learn (`apply_learned`), the settings card and the toolbar light.
- **Tests:**
  - parser and mapping (`midi::tests`);
  - live voices, sustain, bend, stealing, velocity and fader (`engine::tests::live_*`);
  - no allocation while MIDI events stream in during playback (`live_midi_through_queue_does_not_allocate`);
  - UI tests that inject raw MIDI bytes into the real app (`app::ui_tests::midi_*`).
