# Auto Arrange: Fill to length, plus Fun & Fast

Del's ask: *"fun to use, easy to get results out quick into a working track… if the user has created an intro, a chorus etc, an option to fill to a certain time length with some music computational variations."*

![Auto arrange mockup](mockups/auto_arrange.png) (SVG: [`mockups/auto_arrange.svg`](mockups/auto_arrange.svg))

**Status (0.2.2):** first working version. Implemented: roles (keys 1–7, right-click > Role, badges, saved in `.cwt`); the Song > Fill to length… dialog (Ctrl+Shift+F); five style templates; the DP + annealing generator below; derived blocks with 9 transforms; ghost preview, Lock, Accept (one undo step) and Discard. See [§9](#9-implementation-status-022) for what's deferred.

Builds on the Order list data model in [ORDER_LIST.md](ORDER_LIST.md): blocks → play sequences → section list. Auto Arrange only *proposes* a new section list plus derived blocks; nothing is changed until you press Accept.

## 1. Workflow
1. **Tag blocks with roles:** Intro, Verse, Chorus, Bridge, Breakdown, Outro or Fill. Right-click a block, or press `1`–`7` in the Order list. Untagged blocks are ignored. One role can hold several blocks (Chorus A/B).
2. **Pick a target length** (e.g. 3:30) and a **snap** setting (1 / 4 / 8 / 16 bars). Bar length is computed from BPM, speed and lines-per-beat (`bar_secs = beats_per_bar × 60 / bpm`). The generator works in whole bars and reports the real result, e.g. *3:29.1 (122 bars)*. Tempo changes inside blocks are handled by the same pre-scan the ruler uses.
3. **Pick a style template:** Pop, Chiptune demo-scene, Club build/drop or Ambient.
4. Press **Generate**. Ghost blocks appear in the Order list. **Preview** plays them, **Reroll** tries a new seed, **Lock** keeps a section on reroll, and **Accept** or **Discard** commits or drops the proposal. Accept is one undo step.

## 2. Style templates
A template is a small TOML grammar plus a default energy curve. The factory files are in `presets/styles/`. On first use they're copied to the user config folder (`%APPDATA%\ChipWave\styles` on Windows, `~/.config/chipwave/styles` on Linux), where you can edit them or add new ones. **Edit styles…** in the dialog opens that folder, and reopening the dialog reloads them.

| Style | Skeleton | Typical lengths | Variations favoured |
|---|---|---|---|
| Pop | I V C V C B C C O | 4–8 bar intro, 16-bar verse/chorus | Key change before final chorus, drum fill, thin verse 2 |
| Chiptune demo-scene | I V C V′ C′ B F C+ O | 8-bar units | Arpeggio variants, echo via tracker effects, octave shift, fast fills |
| Club build/drop | I Bd(build) C(drop) Bk V Bd C O | 16/32 bars | Filter sweep, density increase into the drop, mute kick in the breakdown |
| Psytrance (145) | I(32) V Bd F C(drop) V Bd F C O(32) | 16-bar units, 7:00 default | Filter sweeps, mute kick in the breakdown, risers into drops, echo. Full template in [STARTER_SOUNDS.md](STARTER_SOUNDS.md) |
| Ambient | I V B V′ O, long repeats | 16–32 bars | Density reduce, slow filter, ritardando outro |

```toml
# pop.toml
name = "Pop"
default_length = "3:30"
max_repeat = 2
start = ["Intro"]
end = ["Outro"]          # only end roles may finish the song
energy = [[0.0,0.2],[0.3,0.6],[0.5,0.5],[0.75,1.0],[1.0,0.1]]
favour = ["key_change", "drum_fill", "thin", "fade"]

[next]
Intro  = [["Verse", 1.0]]
Verse  = [["Chorus", 0.8], ["Verse", 0.2]]
Chorus = [["Verse", 0.5], ["Bridge", 0.3], ["Outro", 0.2]]
Bridge = [["Chorus", 1.0]]
```

If a role is missing (no Bridge tagged, say), the generator skips it: the untagged role is replaced by its own successors, with the weights multiplied. The result panel says so. If the strict grammar still can't reach the length, a relaxed pass lets any tagged role follow, and that is reported too.

## 3. Generator (deterministic and seeded)
Same blocks + settings + seed always give the same result. Reroll just picks a new seed. The approach follows audio **music retargeting**, which treats a song as a graph of segments and finds a constrained shortest path of exactly the target length. It was inspired by radiotool (Rubin & Agrawala, UIST 2014, ISC licence), Wenner et al., "Scalable Music" (Eurographics 2013) and Adobe Audition's Remix. No code was copied; see [research/fill_to_length_algorithms.md](research/fill_to_length_algorithms.md).

1. **Grammar.** The style is a weighted role-transition graph, a Markov FSM over roles. `start` roles begin the song, and only `end` roles (Outro) may finish it.
2. **Exact length by dynamic programming.** Time is counted in rows, so the fit is exact.
   - State: (role, last block, repeat count, pads used) × rows elapsed.
   - Edge cost: `−ln p(transition) + energy error + repetition penalty + seeded noise`. The energy error is (block energy − curve at its midpoint)² weighted by length. Block energy is note density blended with a per-role prior.
   - The repeat count is capped by `max_repeat`. Noise is a hash of (seed, time, item), scaled by Variation %, so different seeds give different good paths.
   - **Pad fills:** 1- and 2-bar cuts of a Fill block (or another middle block) are extra DP items. They cost more, and at most three are allowed. They make odd bar counts reachable.
   - **Locks:** at a locked section's start row only that block may be placed, and no item may straddle it.
   - The path is recovered by backtracking from the cheapest end-role state at exactly the target.
3. **Simulated annealing** (seeded, 2000 iterations) runs on the plan and never changes its length. Moves: add a variation, remove one, or swap a block for another of the same role and length. Locked sections and pads are frozen.
   - Score: energy fit, novelty (an identical repeat is penalised more at high Variation %), op count (penalised more at low Variation %) and style `favour` bonuses.
   - Placement rules: drum fill only before a Chorus, mute drums in a Breakdown or Intro, fade only on the last section, and a key change from a later Chorus to the end.
4. **Fallback.** If no exact path exists, take the nearest end state within ±1 bar. Within 2 % of the target, nudge the tempo to land exactly. Otherwise report the difference (e.g. "−0.9 s").

Speed: about 1–5 ms per generation in release builds, so changing a setting regenerates at once.

```text
dp[0][START] = 0
for t in 0..=T+bar:
  for (state, cost) in dp[t]:
    for item in blocks ∪ pads:            # pads: 1/2-bar cuts, ≤3 per song
      t2 = t + rows(item); skip if t2 > T+bar or it breaks a lock
      p  = grammar(state.role → item.role)  (pads keep the role)
      c  = cost − ln p + w_e·energy_err(item, t) + w_r·(rep−1) + noise(seed, t, item)
      relax dp[t2][state'] with c
best = argmin dp[T][s] with s.role ∈ end  (else ±1 bar → tempo nudge ≤ 2 % → report)
plan = anneal(backtrack(best), seed, 2000 iters, locks frozen)
```

## 4. Variation transforms (non-destructive)
A **derived block** stores `source: BlockId` plus an ordered list of transforms. It's rendered to real pattern data on demand and cached. Edit the source and every variant updates. Accept keeps them as derived blocks, and right-click > **Bake derived block** turns one into an ordinary block. Editing a derived block by hand also bakes it, so your edit is kept. MMD export always bakes them.

**Implemented in 0.2.2:**
- Transpose and Key change: melodic channels only. Drum channels are detected from instrument names.
- Octave shift and Drum fill (last bar, seeded roll with a crescendo).
- Mute drums and Thin (off-beats first).
- Density − (removes weak-beat notes) and Density + (ghost repeats on empty even rows).
- Fade out (volume ramp) and TakeRows (pad cuts).

The rest are deferred (§9).

```rust
struct DerivedBlock { source: BlockId, ops: Vec<VarOp>, seed: u64 }
enum VarOp {
    Transpose(i8), OctaveShift { channels: ChanMask, oct: i8 },
    DrumFillLastBar { channel: u8, density: f32 }, MuteChannels(ChanMask), ThinChannels(ChanMask, f32),
    Arpeggiate { channel: u8, pattern: ArpShape }, ChordInvert { channels: ChanMask, steps: i8 },
    Swing(f32), Density(f32),  // <1 removes weak-beat notes, >1 adds ghost notes on the scale
    FilterSweep { from_hz: f32, to_hz: f32, curve: Curve }, // writes synth automation lane
    EchoFx { channel: u8, delay_lines: u8, feedback: u8 },  // uses tracker volume/delay commands in free channels
    CallResponse { call: ChanMask, answer: ChanMask },      // copies bar 1–2 phrase to answer channel in bars 3–4, transposed by a scale step
    KeyChange(i8), FadeOut { bars: u8 }, Ritardando { bars: u8, end_bpm_pct: u8 },
}
```

| Transform | How (implementable) | Default use |
|---|---|---|
| Transpose / octave shift | Shift note numbers; clamp to the sample's range; skip drum channels (tagged) | Verse 2, final chorus |
| Drum fill on last bar | Replace the last bar of the drum channel with a template fill (rolls, toms) seeded by rng | Before every Chorus |
| Mute / thin channels | Clear a channel, or drop notes with probability p (off-beats first) | Intro, breakdown, verse 1 |
| Arpeggio / chord inversion | Detect chord notes per row across channels; rotate the inversion or emit `0xy` arpeggio commands | Chiptune repeats |
| Swing | Delay odd 16ths via delay command `Fxx`/note-delay | Style-dependent |
| Density ± | Remove weak-beat notes or add passing notes from the detected scale (simple Krumhansl key profile) | Builds and ambient |
| Filter sweep | Automation lane on the synth cutoff, start→end over the block | Club builds |
| Tracker echo | Echo note copies at lower volume in a free channel, N rows later | Demo-scene |
| Call and response | Copy the phrase from channel A to channel B a bar later, shifted by a scale degree | Bridge |
| Key change | +1 or +2 semitones from the block before the final chorus to the end | Pop |
| Outro fade / ritardando | Volume ramp over the last N bars; tempo commands lowering BPM by up to 20 % | Outro |

Variation intensity scales with the **Variation %** knob. Transforms are chosen where the energy curve rises (add density/fills/sweeps) or falls (thin/mute/fade).

## 5. Energy curve UI
- A CRT display with draggable square handles (A2 style), from 0 to 1 over song time. Double-click adds a point; right-click removes it.
- Each style has a default curve. Dragging re-scores live (debounced 100 ms) and updates the ghost blocks.
- Section energy is estimated from note density, active channels and average volume, so the generator can match sections to the curve.

## 6. Order list integration
- **Ghost blocks:** translucent fill with a dashed outline; derived ones are labelled *derived* with a ′ mark. Hovering shows the transform list.
- **Time display:** live total versus target in the header, plus a time ruler.
- **Live preview:** loops the ghost song from the cursor. Click a ghost block to audition just that one.
- Keys: `G` generate, `R` reroll, `L` lock, `Enter` accept, `Esc` / `Ctrl+Z` undo.

## 7. Fun & fast
- **Quick-start templates:** File → New from template (Chiptune, Pop, Club, **Psytrance 145**, Ambient, Amiga MOD 4-channel). Each comes with a tempo, instrument slots filled from the bundled sample pack, a drum block and an empty tagged Verse/Chorus.
- **First track in 5 minutes:** a skippable onboarding overlay. (1) Pick a template. (2) Type a melody in the Chorus block (keyboard as piano, step highlighted). (3) Press 🎲 for a bass line from the chord guess. (4) Fill to length 2:30. (5) Export. Progress dots, no account, never pops up again once dismissed.
- **Randomise patch:** 🎲 on the sound designer randomises within musical limits (cutoff 200 Hz–8 kHz, envelope sane, levels capped) and keeps a history so you can step back.
- **Happy-accident dice:** in any block, randomly applies one transform from §4 to the selection (also seeded and undoable), e.g. "inverted chords on ch 2".
- **Instant export:** `Ctrl+E` renders WAV/MP3 with the last settings ([EXPORT.md](EXPORT.md)) and shows the file in a toast with "Open folder".

## 8. Roadmap
- **Phase 2:** roles, generator, Pop/Chiptune templates, transforms Transpose/Fill/Mute/Thin/KeyChange/Fade, ghost preview and accept/undo, quick-start templates, instant export.
- **Phase 2.5:** energy curve editing, the remaining transforms, Club/Ambient templates, onboarding, dice.

## 9. Implementation status (0.2.2)
**Done:**
- Roles: keys 1–7 / 0, right-click > Role, muted role badges, saved in `.cwt` `blocks/NNN.toml`.
- Dialog (fixed size, Ctrl+Shift+F):
  - mm:ss target with 1/4/8/16-bar snap and a live result line;
  - five editable TOML styles (Pop, Chiptune demo-scene, Club build/drop, Ambient, Psytrance 145);
  - Variation knob, seed with Reroll;
  - energy curve editor (drag, double-click adds, right-click removes);
  - section strip with Lock.
- Generator: DP + annealing, exact length, deterministic.
- Derived blocks with 9 transforms; they follow their source and bake when edited.
- Order list: dashed ghost lane. Preview plays the proposal without touching the song. Accept is one undo step; Discard.
- WAV/MP3 export of the accepted result.
- Keys: G generate, R reroll, L lock, P preview, H hide/show the dialog, S next style, Up/Down variation, Left/Right section, Enter accept, Esc or Ctrl+Z discard.

**Deferred:**
- Transforms: Arpeggiate, Chord inversion, Swing, Filter sweep (automation), Tracker echo, Call and response, Ritardando.
- Order list: auditioning a single ghost clip by clicking it; hover tooltips on ghost clips.
- Live re-scoring while dragging the curve (it regenerates on release instead).
- Tempo changes inside blocks are not pre-scanned (song BPM is used).
- Accept with a tempo nudge doesn't undo the BPM change.
- §7 extras: quick-start templates, onboarding, randomise patch, dice, toast.
