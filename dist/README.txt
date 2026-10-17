ChipWave Tracker 0.2.2.2 - Windows x64
==============================================================
GPL-3.0-or-later - https://github.com/postmanpat-303/ChipWaveTracker
Licence: LICENSE.txt (GPL-3). Third-party notices incl. LAME (LGPL): THIRD_PARTY_LICENSES.txt

Run: double-click ChipWave.exe (no install needed; single self-contained exe).
Windows SmartScreen may warn because the exe is not code-signed:
"More info" -> "Run anyway".

It opens the first-run demo (psy kick, hats, rolling bass at 145 BPM).
Press SPACE to play / stop the current block (Shift+Space = whole song).

Keys (press "? Keys" in the toolbar for the full list)
  Arrows, PgUp/PgDn, Home/End    move (Ctrl+Home/End = first/last block)
  Tab / Shift+Tab                next / previous channel
  Z S X D C V G B H N J M        notes C..B,  Q 2 W 3 E R 5 T 6 Y 7 U I 9 O 0 P = octave up
  1                              note off
  F1..F8, numpad - / +           base octave (also the - / + buttons by "Oct")
  0-9 A-F                        hex in instrument / volume / effect columns
  Delete / Backspace / Insert    clear (selection) / pull up / push down
  Shift+arrows/PgUp/PgDn, drag   select (highlighted green)
  Ctrl+A                         select channel; press again = whole block
  Ctrl+C / Ctrl+X / Ctrl+V       copy / cut / paste at cursor
  Space / Shift+Space            play/stop current block (loop) / play/stop song
  Enter / Ctrl+Enter             restart block / play from cursor row
  Esc                            edit mode on/off
  Ctrl+Z / Ctrl+Y                undo / redo (edits, paste, arranger changes)
  Alt +/-  (Shift = octave)      transpose selection (or whole channel)
  Ctrl+= / Ctrl+- / Ctrl+0       UI scale up / down / 100 %
  F9 / F10                       mute / solo channel
  Ctrl+Tab, Ctrl+1 .. 5          views: Pattern / Arranger / Mixer / FX / Sample
  FX / Sample view: Up / Down    previous / next instrument
  Sample view: Space             preview; drag = select, drag S/E = loop points
  F12                            key list (Help > Keys)
  Ctrl+Q                         quit
  FX device strip                power = bypass, folder = presets, click name =
                                 collapse, drag name = reorder, right-click = menu
  Ctrl+D / Ctrl+Shift+D          arranger: duplicate clip / as new block (also when docked)
  Mouse wheel                    pattern: 1 row per notch (Shift = 16); arranger: scroll, Ctrl = zoom
  Ctrl+Shift+K                   debug: show the last 10 input events in the status bar
  Arranger (click to focus, light outline + ARRANGER tag): click / Shift+click / Ctrl+click select,
    Ctrl+X / C / V cut / copy / paste after selection (Ctrl+Shift+V = as new blocks),
    Del, Ctrl+A, Shift+Left/Right, right-click menu; click the pattern to focus it again
  Arranger: Left/Right, Del, Ins, Ctrl+D (Shift = unique copy), +/-  zoom
  Ctrl+N / Ctrl+O / Ctrl+S       new / open / save .cwt   (Ctrl+Shift+S = save as)
  Ctrl+E                         quick export WAV
  Ctrl+Shift+E                   export dialog (WAV / MP3, tags, range)
  Ctrl+,                         settings (arrows + Enter move/open cards)
  Ctrl+Shift+F                   Song > Fill to length (auto arrange)
  1-7 / 0 (arranger focus)       block role Intro/Verse/Chorus/Bridge/Breakdown/Outro/Fill / clear
  Fill dialog                    G generate, R reroll, L lock, P preview, H hide, S style,
                                 Up/Down variation, Left/Right section, Enter accept, Esc discard
  Unsaved prompt                 Enter = save, N = don't save, Esc = cancel
  (starts maximised; the last window size/position is remembered)

Effects: 01/02 slide up/down, 09 speed, 0C set volume, 0D volume slide,
0F tempo (0F00 = pattern break).

Instruments: the left panel lists the factory psytrance/alien synth sounds;
select one to edit oscillator, filter (LP/HP/BP, cutoff, resonance), envelope.
Import WAV samples with "Import WAV..." or drag .wav files onto the window.
Audio: "Buffer" in the toolbar (Auto/256/512/1024/2048). The status bar
shows latency and an underrun counter; latency grows automatically on dropouts.
BPM / Speed changes apply instantly, also while playing.
Arranger: song order as coloured clips; drag to reorder, click to open a block.
  "Dock arranger" shows it under the pattern editor.
Automation: lanes under the arranger (toggle "Automation", "+ Lane"): click = add point,
  drag = move, right-click = delete, Pencil = draw, Shape = Linear/Step/Smooth.
  Targets: synth cutoff/resonance/drive, amp level, mixer volume/pan, master, tempo.
Mixer: per-channel fader, pan, mute/solo, meters; master fader. Saved in .cwt.
Instrument panel: Transpose -12 / - / + / +12 per instrument.
Theme: "Dark Circuit (default)" or "Amiga (Workbench Gradient)" in the toolbar.

Command line:
  ChipWave.exe --self-test [out.wav]   render the demo, check it is not silent
  ChipWave.exe --render demo out.wav   render the demo (or a .cwt) to WAV
  ChipWave.exe --version
