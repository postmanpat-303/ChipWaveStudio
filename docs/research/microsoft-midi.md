# Research: Microsoft's Windows MIDI Services (microsoft/MIDI) and ideas for ChipWave's timeline

**Status:** research only, no app code changed. Written 19 Oct 2026 for the Timeline plan ([DAW_TIMELINE_PLAN.md](../DAW_TIMELINE_PLAN.md)). It builds on today's MIDI support in [MIDI.md](../MIDI.md) and the MIDI/plugin roadmap in [MIDI_VST.md](../MIDI_VST.md).

**Sources** (all read 19 Oct 2026): the repo [github.com/microsoft/MIDI](https://github.com/microsoft/MIDI) at `main`, its [releases](https://github.com/microsoft/MIDI/releases) (notes for In-box Preview 11, Preview 10 and Developer Preview 9), the docs site [microsoft.github.io/MIDI](https://microsoft.github.io/MIDI/) (KB and SDK reference, read from `docs/` in the repo), the samples in `samples/`, and above all the **Windows MIDI Sequencer design record** in [`src/prototypes/midi-sequencer/design/MIDI-Sequencer-design.md`](https://github.com/microsoft/MIDI/blob/main/src/prototypes/midi-sequencer/design/MIDI-Sequencer-design.md). That's Microsoft's own MIDI 2.0 timeline + clip-launcher app, and it was built while this research was being done, which makes it the most useful part of the repo for us. Crate facts come from crates.io on the same day. A blobless sparse clone (7 MB) was used and then deleted.

---

## 1. What it is

| | |
|---|---|
| **What** | **Windows MIDI Services**: Microsoft's new MIDI stack for Windows. It's a MIDI service (`midisrv`) that every MIDI app goes through, multi-client by default (no more "port in use by another app"). It comes with new USB MIDI 2.0 class drivers and transports (USB, Bluetooth LE MIDI 1.0, Network MIDI 2.0 over UDP, RTP-MIDI 1.0, MIDI 1.0 and 2.0 loopbacks, app-hosted virtual devices, a built-in General MIDI 2.0 synth). There's a new WinRT API, **`Windows.Devices.Midi2`**, that speaks **UMP** (Universal MIDI Packet, MIDI 2.0's 32/64/96/128-bit message format), plus a family of tools. Old WinMM and WinRT MIDI 1.0 apps keep working on top of the same service, with translation |
| **Licence** | Repo: **MIT** (`LICENSE`, "Copyright (c) Microsoft Corporation"). That covers the source, samples and docs. **The preview binaries have their own terms:** shipping the preview `Windows.Devices.Midi2.dll/.winmd/.pri` next to an app needs the "go-live" agreement in the Preview 10/11 notes (keep up with previews, remove the copy once Windows has it, tell users, **never ship it to anything below Windows 11 25H2**). The installers themselves may not be redistributed |
| **Status** | **Preview, API locked.** The latest release is **In-box Preview 11** (`inbox-preview-11`, published 12 Oct 2026), which **locks the API metadata**: no more breaking changes. Nothing is marked "Latest stable" on GitHub yet; every release is a pre-release. Timing of the in-box release isn't consistent across sources. Dev Preview 9's notes say the API and transports reach Windows 11 25H2+ "starting during the last week of November 2026". The current *Porting a MIDI library* KB says "in box in Windows 11 25H2 and later starting in 2027". Plan for **early 2027** and treat November as a bonus |
| **Platforms** | **Windows 11 only, 64-bit (x64 and Arm64).** The `Windows.Devices.Midi2` API needs **Windows 11 25H2 (build 26200) or later**. 24H2 has an older service inside (so WinMM traffic already goes through it) but **never gets the API**, and 24H2 leaves support in Oct 2026. There's no Windows 10, no 32-bit, no Xbox/HoloLens. Users can switch to **Legacy API mode** (the old stack); apps must treat that as a normal choice and fall back to WinMM |
| **Install** | Once it's in box, nothing: the service, drivers and API come with Windows. Until then, testers install two signed packages from Releases: **Tools** (`Windows.MIDI.Services.Tools.0.99.92-preview.11-x64.exe`) and **Transports** (`…Transports.1.0.41-preview.9-x64.exe`), with arm64 builds of both. App developers get the API from the NuGet package `Windows.Devices.Midi2` 0.99.92-preview.11 |
| **Tools** (Tools package) | MIDI Settings (rename endpoints, set per-device latency offsets, create loopbacks), MIDI Console (`midi.exe`: enumerate, monitor, send, capture to CSV) and a PowerShell module, MIDI Monitor, Scratch Pad, SysEx Tool, Keyboard, File Player, **Clock** (MIDI clock and MTC generator), **Patchbay** (routing graph with filter/transform/logic/clock/MTC/LFO steps), **MIDI Glass** (touch control surfaces), Troubleshooting and Repair plus `mididiag`, Network and Bluetooth setup, and new in Preview 11, **Windows MIDI Sequencer** |

### Repo layout (what's where)
| Path | What it is |
|---|---|
| `src/in-box/` | The service, transports, drivers, client API (C++/WinRT), IDL and the in-box user tools, including `user-tools/midi-sequencer/` (the sequencer engine and model) |
| `src/service-plugins/` | Transport and transform plugins (Network MIDI 2.0, BLE, loopbacks, virtual device…) |
| `src/prototypes/` | Design records with HTML comps: `midi-sequencer`, `midi-patchbay`, `midi-glass`, `midi-synth`, `rtp-midi`, `midi-ai` |
| `samples/cpp-winrt`, `samples/csharp-net` | `scheduled-send-messages`, `simple-app-to-app-midi`, `loopback-endpoints`, `capability-inquiry-*` (browse, program list, virtual device responder), `watch-endpoints`, `sequence-builder`, `midi-file-player/-writer`, `detect-midi-services` (the three start-up checks), `app-manifest`. **There's no Rust sample**, but the Preview 11 notes say apps built with the Rust `windows` crate find an app-local DLL without needing a manifest |
| `docs/kb`, `docs/sdk-reference` | KB articles (porting from WinMM, the UMP primer, data translation, USB timing measurements, Network MIDI 2.0) and the API reference |

### The API in one screen (what matters to a DAW)
- **Sessions and connections.** `MidiSession.Create("ChipWave Studio")` → `CreateEndpointConnection(endpointDeviceId)` gives a **bidirectional** connection to an *endpoint*. One endpoint has up to **16 groups** × 16 channels; the group is a field inside each UMP, not a separate port. Messages are sent as 1–4 32-bit words.
- **Timestamps and scheduling.** Every send takes a `timestamp`. `0` means send now; any other value is an **absolute time** on `MidiClock` (QPC-based, 100 ns ticks since boot, never wraps) when **the service** should send it, up to 5 minutes ahead. Received messages carry the same timestamps. Microsoft measured scheduled sends arriving a **median of 10–14 µs late (worst 216 µs)** through the diagnostic loopback, with nothing lost. `MidiClock.OffsetTimestampByMilliseconds(t, -latency)` is how you compensate a device's delay.
- **Enumeration.** `MidiEndpointDeviceWatcher` (event-driven: added/updated/removed with "what changed" flags). Each endpoint has **function blocks** (MIDI 2.0, discovered in-protocol, can be renamed or moved at runtime and are authoritative) or **group terminal blocks** (from USB descriptors). They say which groups are in/out, MIDI 1.0 or 2.0, and a UI hint. Endpoint *ids* are stable; names are user-editable display text, so **don't key saved settings on names**.
- **Protocol.** Each endpoint negotiates MIDI 1.0 or 2.0. Windows **down-converts only for some endpoints, and never up-converts**. Microsoft's own sequencer does both conversions itself, per destination, using the MIDI 2.0 bit-scaling rules (M2-115).
- **Jitter-reduction (JR) timestamps.** These are the UMP utility messages a sender can add so a receiver can remove transport jitter. The API exposes whether an endpoint negotiated them (`MidiDeclaredStreamConfiguration.Send/ReceiveJitterReductionTimestamps`), but **virtual devices don't support them**, and in practice the service's own 100 ns timestamps and scheduler do this job. Microsoft's USB measurements found jitter (IQR) of tens of µs on good interfaces, unchanged under 100% CPU load. **Takeaway: we don't need to implement JR. Use service timestamps.**
- **Virtual devices** (`MidiVirtualDeviceManager`). An app publishes its own MIDI 2.0 endpoint (name, function blocks, identity) that lives as long as the app's connection. Other apps, *including WinMM ones*, see it as an ordinary device. It needs the WinRT connection path (not the COM fast path), and the MIDI-CI responder is "manual for now".
- **Loopbacks** (`MidiLoopbackManager`). User- or app-created A↔B endpoint pairs that persist, plus the always-present **Diagnostics Loopback A/B** for testing. The default **App Loopback** is how Windows MIDI Keyboard plays into other apps. There's feedback protection.
- **MIDI-CI** (`MidiCapabilityInquirySession`). Discovery, profiles, **Property Exchange** (`GetProgramListAsync`, channel lists, `ChCtrlList` controller names), and a responder for virtual devices.
- **Utilities.** `MidiMessageBuilder`/`Converter` (MIDI 1.0 bytes ↔ UMP), SysEx7 send/receive, the SMF reader/writer, and a **Sequencing** API (`MidiSequenceBuilder`, `MidiSequencePlayer`, with tempo map, bar positions, lyrics).
- **Two receive paths:** the **COM Extensions** (a raw buffer of UMP words, no allocations, several messages per callback; best for a realtime host) or the **WinRT `MessageReceived`** path (one event per message, allocates; the only path that can host a virtual device). The choice is per connection.

---

## 2. Ideas for ChipWave, ranked

Ranking = value to Del and to the timeline's future ÷ effort. "M0–M7" are the plan's milestones (§4 of the plan). Effort is in focused build days, like the plan's.

### Summary table
| # | Idea | Value | Effort | Milestone |
|---|---|---|---|---|
| 1 | **High-resolution note model** (16-bit velocity, release velocity, per-note expression) and 32-bit-normalised automation values | ★★★★★ (future-proofing, free now, costly later) | 0.5 d now (model + serde) | **M0** (model), used from M3b |
| 2 | **Tempo map with ramps + meter map + per-track tags** (Microsoft's Tempo-and-meter track) | ★★★★ | 0.5 d in M0, UI in M5 | **M0**/M2/M5 |
| 3 | **Deterministic "seed + pass number" rule** for anything random (chance, generators, humanise) | ★★★★ (bit-exact export, seek = replay) | 0.5 d (rule + seed field) | **M0** (field), M6+ (use) |
| 4 | **MIDI recording into timeline note clips**, with timestamps, per-source offset and **Capture** (retroactive record) | ★★★★★ (Del already records MIDI) | 3–4 d on top of M3b | **M3b** |
| 5 | **MIDI output scheduler** (look-ahead, per-device offset, chase on seek, silence rules), on WMS timestamps on Windows | ★★★★ (hardware synths, other apps) | 5–7 d | new **"M2.5 MIDI out"**, after M2 |
| 6 | **MIDI clock + SPP + MTC out, and follow incoming clock** | ★★★ | 2–3 d out, +2 d follow | with idea 5 |
| 7 | **Pattern clips and generator clips** (step lanes with own lengths, chance, ratchet, conditions; Euclidean, chance melody) + "Make notes" | ★★★★ (very ChipWave) | 6–9 d | after M3b (and with M6's launcher) |
| 8 | **Endpoint hygiene**: watcher instead of 2 s polling, persist endpoint ids not names, show function-block/group names | ★★★ | 1–2 d (WinMM/midir now), WMS backend later | any time; WMS backend after Windows ships the API |
| 9 | **ChipWave as a MIDI device** (virtual endpoint): play ChipWave instruments from other apps, and a remote-control map (launch, transport, M/S/arm) | ★★★ | 3–5 d (Mac/Linux easy, Windows needs WMS) | after M2.5 / with M6 |
| 10 | **Lyrics / Flex Data track → Voice instrument** (tempo, key, chord names, lyrics as timeline metadata; lyrics drive `3E` words) | ★★★ (unique to ChipWave's singing voice) | 3–4 d | M5 or later |
| 11 | **File interchange**: SMF import (planned M3b), **MIDI 2.0 Clip File (.midi2)** export, DAWproject (M7) with the `dawproject` crate | ★★★ | SMF 1–2 d, .midi2 2 d, DAWproject per plan | M3b / M7 |
| 12 | **MIDI-CI Property Exchange**: program names for MIDI-out instruments, controller names for automation lanes | ★★ | 4–6 d (crates immature) | late, after M2.5 |
| 13 | **Loopback-based tests and docs** (Diagnostics Loopback A/B on Windows, ALSA/CoreMIDI virtual ports in CI) | ★★ (quality) | 1 d | with M2.5 |
| — | Network MIDI 2.0, BLE MIDI, RTP-MIDI | (free) | 0 | Nothing to build: the OS turns them into ordinary endpoints |

### 2.1 High-resolution note model (adopt in M0)
**What Microsoft does.** A notes clip stores, per note: `start, length, channel, key, velocity (16-bit)`, plus optional `release velocity, attribute type/value (e.g. exact pitch), chance`. Per-note pitch bend and per-note controllers are kept as *ordinary messages inside the clip* (that's what they are on the wire), and everything else is the raw UMP. Automation points are `[tick, value]` with **values 0…2³²−1 whatever the destination**, scaled down at playback ("so nothing is lost on a MIDI 2.0 device"). Compact JSON: `[tick, len, ch, key, vel]`, with optional fields appended only when used.

**For ChipWave.** Plan §1.3 has `Note { start, len, key: u8, vel: u8, instr: u8, fx }`. Changing `vel` later means a file-format migration. Proposal for M0 (all `serde(default)`, so the cost is near zero):
```rust
pub struct Note {
    pub start: Tick, pub len: Tick, pub key: u8,
    pub vel: u16,                     // MIDI 2.0 resolution; tracker volume 00–40 and MIDI 1.0 map via M2-115 bit scaling
    #[serde(default)] pub rel_vel: u16, // release velocity (0 = unset)
    pub instr: u8, pub fx: Option<(u8, u8)>,
    #[serde(default)] pub tune: f32,  // per-note detune in semitones (MIDI 2.0 "pitch 7.9" attribute, CLAP tuning expression)
    #[serde(default)] pub chance: u8, // 0 = always (see 2.3)
    #[serde(default)] pub expr: Vec<NoteExpr>, // empty for almost every note
}
pub struct NoteExpr { pub kind: ExprKind, pub points: Vec<(Tick /*rel. to note*/, f32 /*0..1 or ±1*/)> }
pub enum ExprKind { PitchBend, Pressure, Timbre /*CC74, CLAP brightness*/, Pan, Volume, Vibrato }
```
- *Correction to the brief:* MIDI 2.0 velocity is **16-bit**, and **controllers** are 32-bit.
- **Why this is cheap for ChipWave:** CLAP (already hosted) has note expressions for volume, pan, tuning, vibrato, expression, brightness and pressure, so `NoteExpr` maps 1:1 to CLAP events with note ids (M3b already plans note ids). Internal Synth/Analog/FM voices can read `Timbre` as a cutoff offset and `PitchBend` as per-voice detune.
- **Automation:** store values as `f32` 0…1 (already the plan's habit) and quantise only at the destination (7-bit CC, 14-bit bend/NRPN, 32-bit MIDI 2.0). The rule from Microsoft that matters: **thin output to the destination's resolution and rate** (a ramp to a MIDI 1.0 CC can't usefully send more than 128 values; default 10 ms minimum interval).
- **Innermost lane wins** (clip lane beats track lane for the same target, only inside the clip). The plan currently says clip automation "multiplies or overrides". Pick *override, innermost wins* by default: Microsoft chose it for predictability.
- **Undo:** today's Arc-shared song snapshots are fine if each `NoteClipData` is its own `Arc` (an edit copies one clip, not the pool). Microsoft had to move to undo-by-change because they snapshot the whole sequence. We avoid that by keeping per-clip Arcs.

### 2.2 Tempo map with ramps, meter map, tags (M0 model, M5 UI)
Microsoft's file: `ticksPerQuarterNote: 960` (**the same PPQ as our plan**, which is a nice validation), `tempo: [{tick, bpm, rampToNext?}]`, `meter: [{tick, num, den}]`, `tags: [{tick, text, color}]` on the song **and per track** ("Filter opens here"). Tags export as SMF markers and DAWproject markers. Proposal: add `ramp: bool` to `TempoPoint` now (the engine can treat a ramp as stepped per row until M5), and let `Marker` carry an optional `track`. The pinned **Tempo and meter** row at the top of the timeline is a good UI model for M5's tempo lane. Note: tempo changes in exported SMF/clip files have to sit on MIDI-clock positions (every 1/24 quarter), and Microsoft's exporter snaps them.

### 2.3 Determinism rule (M0 field, used later)
"**A result depends only on the clip's seed and the pass number.**" Chance, conditions, generators and humanise draw from an RNG seeded by `(clip.seed, pass)`. So playing from the start always gives the same hits, seeking replays what you heard, and export equals realtime. ChipWave's demo-render tests are bit-exact, so this rule is required, not optional. Add `seed: u32` to `Clip` in M0. (ChipWave's `fill_to_length` / Auto-arrange work could use the same rule.)

### 2.4 Recording MIDI into timeline clips (M3b)
Microsoft's design, which maps directly onto ours:
- Arm a track (the plan's ● button) → incoming notes are recorded with **their arrival timestamps**, converted to ticks using the transport's start time and the tempo map. A per-source **"recorded late by" offset** handles device delay. Several tracks can record from one source; open one connection per device and share it.
- **Echo while armed:** play through the track's instrument (ChipWave's live path already does this for the cursor channel; in timeline mode, route to the armed track).
- **Three places to record:** at the playhead with **loop recording** (layer or replace takes); into an empty launcher slot (M6); and **Capture**, where every armed track keeps the last few minutes of what you played and *Capture* turns it into a clip even if you never pressed record. Capture is cheap (a ring buffer of timestamped events) and great for jamming. Strongly recommended.
- MIDI 1.0 velocity → 16-bit via M2-115 scaling (exactly reversible).
- **ChipWave today:** `midir` already gives µs timestamps on input. Live record rounds to rows after latency compensation; note clips keep the exact tick. Count-in and punch come later.

### 2.5 MIDI output scheduler (new milestone "M2.5 MIDI out", after M2)
MIDI_VST.md §2 already plans MIDI instruments (port, channel, program) and per-port latency. Microsoft's measured design gives us the engine rules:
- **Look-ahead scheduling.** An engine thread hands each message to the OS with its due time, **~40 ms ahead** (stretched automatically if the thread wakes late; min 20, max 250). A handed-over message **can't be recalled**, so: hold back each **note-off until it's nearly due** (an early note-off would cut a re-triggered note after a loop or seek). Mute, edit or launch inside the window plays once more, which is acceptable.
- **Per-device offset** (from settings, or reported by the transport, e.g. BLE/Network): send that device's messages that much earlier, and start playback late enough for the earliest-needing device. For ChipWave, add **the audio output latency** so external synths line up with internal audio.
- **Chase on start-mid-song:** send bank, program, CCs and pitch bend per channel first (same as the plan's state chase for internal voices).
- **Silence rules** (measured by Microsoft): note-offs for sounding notes → sustain off → all notes off → all sound off + bend centre, **only on channels this track uses** (the service is multi-client, so never blast other apps), and again after the look-ahead.
- **Per-destination protocol:** MIDI 2.0 messages for MIDI 2.0 function blocks, MIDI 1.0 otherwise, converted by ChipWave. Warn when per-note expression would be dropped ("this destination is MIDI 1.0: per-note bend will be lost"; MPE as a later opt-in).
- **Backends:**
  - **Windows 11 25H2+ with the API:** WMS connection + `SendSingleMessageWords(timestamp, …)`. The service scheduler is µs-accurate.
  - **Everything else (older Windows, Legacy mode, today):** `midir` (WinMM) has **no timed send**, so a ChipWave sender thread with a high-resolution waitable timer does the timing (Microsoft measured ~0.5 ms median wake-up lateness), with WinMM via `midir`.
  - **macOS:** CoreMIDI supports timestamped sends (`midir` feature `coremidi_send_timestamped`, or the `coremidi` crate's `EventBuffer` with `Protocol::Midi20` for UMP).
  - **Linux:** ALSA sequencer queues support scheduled events (`alsa` crate); `midir` sends immediately, so use the same sender thread as on Windows, or an ALSA queue.

### 2.6 Clock and sync
- **Send:** MIDI clock pulses **scheduled from the tempo map like notes** (not from a fixed-rate generator, so ramps stay locked), Start/Continue/Stop with the transport, **Song Position Pointer on seek and at each loop wrap** (just before the next pulse), Stop *after* the last handed-over pulse. Per-device offset. Optionally a **MIDI 2.0 Set Tempo** Flex Data message for MIDI 2.0 devices. **MTC** quarter frames at 24/25/29.97df/30 fps, counting wall time from song start (01:00:00:00 default), with full-frame messages on start, loop and stop.
- **Follow incoming clock:** plan only up to the next expected pulse, take tempo from the last few pulses, and stop if pulses are missing for max(8 pulses, 0.3 s). Microsoft explicitly **didn't** build MTC chase (it needs a phase-locked receiver), and we shouldn't either.
- Hooks into the plan's tick transport (M2). This also covers MIDI_VST.md's "Clock / sync" item.

### 2.7 Pattern clips and generator clips (after M3b; pairs with M6)
Microsoft concluded (citing Logic, FL Studio and Live 12) that step sequencers and generators belong **as clip kinds next to note clips**, not as separate tools:
- **Pattern clip:** drum lanes (one note per lane) or melodic mode (accent and slide rows). **Each lane has its own step count**, so a 12-step hat drifts against a 16-step kick. Per step: velocity, chance, ratchet 2–8 (softer/louder), conditions (`1:2`, first pass, not first pass), nudge, length, and later locks (a per-step CC or per-note controller). Swing 50–75.
- **Generator clip:** Euclidean rings (pulses, steps, rotate, note, velocity, chance) and a **chance melody** (scale, range, busy, *keep from last pass* for slowly evolving loops). An arpeggiator comes later.
- **"Make notes"** converts either into a plain note clip (enough bars for the lanes to line up, max 64).
- **ChipWave fit:** tracker blocks already *are* our step sequencer, but polymetric lanes, chance and Euclid don't exist in the tracker. As a `ClipKind::Pattern`/`ClipKind::Generator` they'd play through the M3b voice pools and obey the seed rule (2.3). The plan's existing `chords.rs` scale data feeds the chance melody.

### 2.8 Endpoint hygiene (small, any time)
From Microsoft's porting KB:
- **Don't poll** the device list. ChipWave rescans every 2 s for hot-plug (MIDI.md). Under the MIDI service each poll is a cross-process call. With a WMS backend, use `MidiEndpointDeviceWatcher`. With WinMM, poll only while the MIDI settings card is open or a saved device is missing.
- **Persist the endpoint id, not the port name or index.** Users can now rename endpoints in MIDI Settings.
- **Display names:** endpoint name + group + function-block name ("Keystep 37 · Group 1 · Main").
- Good news for free: on Windows 11 24H2/25H2, WinMM already goes through the service, so ChipWave via `midir` gets **multi-client ports** and the full device names without code changes. `midir` 0.11.1 (10 Oct 2026) even includes WinMM/WinRT fixes from Microsoft's own review.

### 2.9 ChipWave as a MIDI device (virtual endpoint)
- **Play ChipWave from other apps** (a DAW or Windows MIDI Sequencer plays ChipWave's instruments live) and **remote control**. Microsoft's sequencer publishes an endpoint "MIDI Sequencer" with a simple, MIDI 1.0-compatible map: channels 1–15 = tracks; notes 0–63 launch scenes' clips, 64 stop, 65 back to timeline; CC 20/21/22 = mute/solo/arm; channel 16 = global (scenes, play 96, stop 97, record 98, loop 99, metronome 100, to start 101). State is echoed back so pads light up. ChipWave could adopt the same map (and get Windows MIDI Glass layouts for free).
- **Backends:**
  - **macOS and Linux:** `midir::os::unix::VirtualInput/VirtualOutput` (CoreMIDI virtual source/destination, ALSA seq port). Easy.
  - **Windows:** only via WMS `MidiVirtualDeviceManager` (25H2+, WinRT receive path). Older Windows needs a user-made loopback (Windows MIDI Services loopbacks, or loopMIDI).

### 2.10 Lyrics / Flex Data track → Voice (ChipWave-specific)
Microsoft added a **lyrics track** (hyphenated syllables on the timeline, karaoke `.kar` import, sent to MIDI 2.0 devices as lyric Flex Data). ChipWave has a singing **Voice** instrument with a `3E` word command. A timeline lyrics row whose syllables drive the Voice track's words would be a standout feature. Same for chord-name and key-signature Flex Data from `chords.rs`. SMF lyric/marker meta events are the MIDI 1.0 fallback.

### 2.11 File interchange
- **SMF import into note clips** is already planned for M3b with `midly`. Map format-1 tracks to tracks, markers to tags, and copyright text to clip provenance.
- **MIDI 2.0 Clip File** (`.midi2`, M2-116-U, header `SMF2CLIP`, big-endian UMP with delta clockstamps, 960 PPQ). It keeps 16-bit velocity and per-note expression. Microsoft writes one file per track because the multi-track "MIDI Container File" spec isn't published yet. Low priority; there's no Rust crate, but it's simple to write with the `midi2` crate.
- **DAWproject (M7):** there's now a Rust crate, [`dawproject`](https://crates.io/crates/dawproject) 0.11.0 (MIT OR Apache-2.0, Aug 2026). Microsoft's notes on what to cap when reading untrusted ZIP/XML (size, entries, depth) and its mapping table (per-note expressions as `Points` inside `Note`) are directly reusable. Their stated gap is the same as ours: per-note expressions aren't round-tripped yet.

### 2.12 MIDI-CI Property Exchange (late)
Use it to fill a MIDI-out instrument's **program list with real names** (`ProgramList`), and to label automation lanes ("Cutoff" instead of "NRPN 7:54", from `ChCtrlList`). On Windows the API does the session work. Cross-platform, the [`midi2`](https://crates.io/crates/midi2) crate has a `ci` feature (message types only) and [`midici-core`](https://crates.io/crates/midici-core) 0.1.0 (MIT, Sep 2026) is a sans-io state machine, but it's brand new (36 downloads). Few devices support PE yet, so leave this until there's demand.

### 2.13 Testing with loopbacks
Windows: the always-present **Diagnostics Loopback A/B** gives a real round trip through the service, which makes a MIDI-out timing test possible ("note at bar 3 arrives at t ± 1 ms"). Linux CI: ALSA seq virtual ports (`snd-seq-dummy`) for the same test. Microsoft's sequencer tests ("renders a busy pattern in small windows and matches the whole") are a good model for checking that block-size independence matches the plan's bit-exact tests.

### Rust crates (crates.io, 19 Oct 2026)
| Crate | Version | Licence | Use for ChipWave |
|---|---|---|---|
| [`midir`](https://crates.io/crates/midir) | 0.11.1 (10 Oct 2026) | MIT | Already used. Input + output on WinMM (Windows, the default), WinRT MIDI 1.0 (feature), CoreMIDI, ALSA, JACK. Virtual ports on Unix only. No timed send except CoreMIDI (`coremidi_send_timestamped`). **No Windows MIDI Services backend** |
| [`wmidi`](https://crates.io/crates/wmidi) | 4.0.11 | MIT | Typed MIDI 1.0 messages (already planned) |
| [`midi2`](https://crates.io/crates/midi2) | 0.11.1 (Apr 2026) | MIT OR Apache-2.0 | Typed **UMP** messages: MIDI 1.0/2.0 channel voice, Flex Data, UMP stream, SysEx7/8, utility (JR timestamps), `ci`. `no_std`-friendly. **The best fit for an internal UMP event type and for WMS words** |
| [`midly`](https://crates.io/crates/midly) | 0.5.3 | Unlicense | SMF read/write (M3b import, export) |
| [`midi-msg`](https://crates.io/crates/midi-msg) | 0.9.0 | MIT | Fuller MIDI 1.0 spec incl. MTC, SPP and sysex. Handy for clock/MTC out |
| [`windows`](https://crates.io/crates/windows) | 0.62.2 | MIT OR Apache-2.0 | Has only `Devices_Midi` (the old WinRT MIDI 1.0 API). **`Windows.Devices.Midi2` isn't in the Windows SDK metadata yet** |
| [`windows-bindgen`](https://crates.io/crates/windows-bindgen) | 0.100.0 (Sep 2026) | MIT OR Apache-2.0 | Generate Rust bindings for `Windows.Devices.Midi2` from the NuGet package's `.winmd` (check the NuGet package licence before committing the winmd; generate at build time otherwise). At runtime, use the Windows copy of the DLL (don't ship the preview DLL) and run Microsoft's three-step check: build ≥ 26200 → `MidiApi` resolves → `EnsureServiceAvailable()`; else fall back to `midir` WinMM |
| [`coremidi`](https://crates.io/crates/coremidi) | 0.9.2 | MIT | macOS: `EventBuffer` with `Protocol::Midi10/Midi20` (UMP event lists, macOS 11+), timestamps, virtual endpoints |
| [`alsa`](https://crates.io/crates/alsa) | 0.12.1 | Apache-2.0/MIT | Linux: sequencer queues (scheduled events), virtual ports. The kernel and alsa-lib have UMP/MIDI 2.0 since Linux 6.5 / alsa-lib 1.2.10, but this crate doesn't wrap UMP yet, so stay on MIDI 1.0 sequencer events there |
| [`midici-core`](https://crates.io/crates/midici-core) | 0.1.0 | MIT | MIDI-CI state machines (very new) |
| [`dawproject`](https://crates.io/crates/dawproject) | 0.11.0 | MIT OR Apache-2.0 | DAWproject read/write for M7 |

**Cross-platform shape:** one `MidiBackend` trait (`list/watch`, `open_in`, `open_out`, `send_at(timestamp_ns, &[u32] ump)`, `create_virtual`), with implementations **Wms** (Windows 25H2+, UMP native, service scheduling), **Midir** (WinMM/ALSA/CoreMIDI MIDI 1.0, own sender thread), and later **CoreMidiUmp** / **AlsaUmp**. Inside ChipWave, everything is UMP words (`midi2` types) and gets down-converted at the edge. That's the same internal model Microsoft recommends to library authors, and it costs nothing on MIDI 1.0 paths.

---

## 3. Recommendation

**Adopt now (M0, about 1 day, all `serde(default)`, so tracker-mode files stay byte-identical):**
1. `Note.vel: u16` + `rel_vel`, `tune`, `chance`, and an (empty by default) `expr: Vec<NoteExpr>` (§2.1). Automation values stay normalised floats and are quantised only at the destination.
2. `TempoPoint.ramp`, a meter map, and markers that can belong to a track (§2.2).
3. `Clip.seed: u32` and the **seed + pass** rule written into the plan as an invariant (§2.3).
4. Write down two policies: **innermost automation lane wins**, and **linked clips by default** (the plan already has this; Microsoft reached the same conclusion).
5. Keep `NoteClipData` behind its own `Arc` so the existing undo stays cheap with big recordings.

**Next (M2–M3b):** a `MidiBackend` trait with UMP words inside (`midi2` crate) but still `midir` underneath. Recording into note clips with timestamps, per-source offset and **Capture** (§2.4). SMF import. Stop the 2 s polling when nothing is missing (§2.8).

**Then ("M2.5 MIDI out", 1–1.5 weeks):** the look-ahead scheduler with per-device offset, chase and silence rules, plus MIDI clock/SPP/MTC out and clock follow (§2.5–2.6). This delivers MIDI_VST.md's MIDI instruments and clock items on top of the timeline transport.

**Later:**
- Pattern and generator clips (§2.7, with or after M6).
- The WMS backend (`windows-bindgen` + `Windows.Devices.Midi2`) **once the API ships in Windows** (expected late 2026 to early 2027; don't ship the preview DLL).
- Virtual endpoint and remote-control map (§2.9).
- Lyrics → Voice (§2.10).
- `.midi2` export and DAWproject via the `dawproject` crate (§2.11).
- MIDI-CI names (§2.12).

**Skip:** implementing JR timestamps (the service timestamps do the job), MTC chase, and anything for Network/BLE MIDI (the OS handles them).
