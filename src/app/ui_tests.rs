//! UI tests: drive the real App (eframe::App::ui + raw_input_hook) with
//! simulated clicks, drags and key presses through egui_kittest.
use super::*;
use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;

pub(super) fn harness_with(song: Song) -> Harness<'static, App> {
    let shared: SharedRef = Arc::new(Mutex::new(Shared {
        song,
        engine: Engine::new(48000.0),
        preview: None,
    }));
    let app = App::new(shared, None, None, None, String::new(), None, vec![]);
    let mut h = Harness::builder()
        .with_size(egui::vec2(1700.0, 1000.0))
        .with_accessibility_check(false)
        .build_eframe(|_cc| app);
    frame(&mut h);
    frame(&mut h);
    h
}

fn harness() -> Harness<'static, App> {
    harness_with(fileio::demo_song())
}

/// one frame, with the app's raw input hook applied like eframe does
pub(super) fn frame(h: &mut Harness<'static, App>) {
    let ctx = h.ctx.clone();
    let mut raw = std::mem::take(h.input_mut());
    eframe::App::raw_input_hook(h.state_mut(), &ctx, &mut raw);
    *h.input_mut() = raw;
    h.step();
}

fn frames(h: &mut Harness<'static, App>, n: usize) {
    for _ in 0..n {
        frame(h);
    }
}

/// press and release a key (with modifiers) over two frames
pub(super) fn key(h: &mut Harness<'static, App>, key: Key, m: egui::Modifiers) {
    for pressed in [true, false] {
        h.input_mut().events.push(egui::Event::ModifiersChanged(m));
        h.input_mut().events.push(egui::Event::Key {
            key,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: m,
        });
        frame(h);
    }
    h.input_mut()
        .events
        .push(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
    frames(h, 3);
}

fn ctrl_z(h: &mut Harness<'static, App>) {
    key(h, Key::Z, egui::Modifiers::COMMAND);
}

/// press, move and release the primary button (a real mouse drag / click)
pub(super) fn drag(h: &mut Harness<'static, App>, from: Pos2, to: Pos2) {
    let ev = |h: &mut Harness<'static, App>, e: egui::Event| {
        h.input_mut().events.push(e);
        frame(h);
    };
    ev(h, egui::Event::PointerMoved(from));
    ev(
        h,
        egui::Event::PointerButton {
            pos: from,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        },
    );
    for k in 1..=6 {
        let p = from + (to - from) * (k as f32 / 6.0);
        ev(h, egui::Event::PointerMoved(p));
    }
    ev(
        h,
        egui::Event::PointerButton {
            pos: to,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        },
    );
    frames(h, 3);
}

/// a quick click (a held press would count as a long-press, not a click)
fn click_at(h: &mut Harness<'static, App>, p: Pos2) {
    click_with(h, p, egui::PointerButton::Primary);
}

fn click_with(h: &mut Harness<'static, App>, p: Pos2, button: egui::PointerButton) {
    h.input_mut().events.push(egui::Event::PointerMoved(p));
    frame(h);
    for pressed in [true, false] {
        h.input_mut().events.push(egui::Event::PointerButton {
            pos: p,
            button,
            pressed,
            modifiers: Default::default(),
        });
        frame(h);
    }
    frames(h, 3);
}

trait EngineAccess {
    fn with_engine<R>(&mut self, f: impl FnOnce(&mut Shared) -> R) -> R;
}

impl EngineAccess for Harness<'static, App> {
    fn with_engine<R>(&mut self, f: impl FnOnce(&mut Shared) -> R) -> R {
        let mut s = audio::lock(&self.state().shared);
        f(&mut s)
    }
}

fn song<R>(h: &mut Harness<'static, App>, f: impl FnOnce(&Song) -> R) -> R {
    let s = audio::lock(&h.state().shared);
    f(&s.song)
}

fn mixer(h: &mut Harness<'static, App>) {
    h.state_mut().view = View::Mixer;
    frames(h, 3);
}

/// rects of widgets with this role (and label, if given), in screen order
fn rects(h: &Harness<'static, App>, role: Role, label: Option<&str>) -> Vec<Rect> {
    h.query_all_by(|n| n.role() == role && (label.is_none() || n.label().as_deref() == label))
        .map(|n| n.rect())
        .collect()
}

/// mixer channel strip widgets: (fader, pan) rects of channel `ch`
fn strip(h: &Harness<'static, App>, ch: usize) -> (Rect, Rect) {
    let sliders: Vec<Rect> = rects(h, Role::Slider, None)
        .into_iter()
        .filter(|r| r.top() > 140.0 && r.left() > 300.0)
        .collect();
    let faders: Vec<Rect> = sliders
        .iter()
        .copied()
        .filter(|r| r.height() > 100.0)
        .collect();
    let fader = faders[ch];
    let pan = *sliders
        .iter()
        .find(|r| {
            r.height() < 40.0
                && r.top() > fader.bottom()
                && (r.center().x - fader.center().x).abs() < 40.0
        })
        .expect("pan slider under the fader");
    (fader, pan)
}

fn engine_bpm(h: &Harness<'static, App>) -> f32 {
    audio::lock(&h.state().shared).engine.bpm()
}

#[test]
fn mixer_fader_pan_mute_solo_send_undo_with_ctrl_z() {
    let mut h = harness();
    mixer(&mut h);
    let orig = song(&mut h, |s| s.mixer.clone());
    let (fader, pan) = strip(&h, 0);
    // fader
    drag(
        &mut h,
        fader.center(),
        fader.center() - egui::vec2(0.0, 80.0),
    );
    assert_ne!(
        song(&mut h, |s| s.mixer.channels[0].volume),
        orig.channels[0].volume
    );
    ctrl_z(&mut h);
    assert_eq!(
        song(&mut h, |s| s.mixer.channels[0].volume),
        orig.channels[0].volume,
        "fader undo"
    );
    // pan
    drag(&mut h, pan.center(), pan.center() + egui::vec2(30.0, 0.0));
    assert!(song(&mut h, |s| s.mixer.channels[0].pan) > 0.2, "pan moved");
    ctrl_z(&mut h);
    assert_eq!(
        song(&mut h, |s| s.mixer.channels[0].pan),
        orig.channels[0].pan,
        "pan undo"
    );
    // mute
    let m = rects(&h, Role::Button, Some(" M "))[0];
    click_at(&mut h, m.center());
    assert!(song(&mut h, |s| s.mixer.channels[0].mute), "mute on");
    ctrl_z(&mut h);
    assert!(!song(&mut h, |s| s.mixer.channels[0].mute), "mute undo");
    // solo
    let so = rects(&h, Role::Button, Some(" S "))[1];
    click_at(&mut h, so.center());
    assert!(song(&mut h, |s| s.mixer.channels[1].solo), "solo on");
    ctrl_z(&mut h);
    assert!(!song(&mut h, |s| s.mixer.channels[1].solo), "solo undo");
    // send A
    let sa = rects(&h, Role::Slider, Some("send A (delay): 0 %"))[0];
    drag(
        &mut h,
        sa.left_center() + egui::vec2(2.0, 0.0),
        sa.right_center(),
    );
    assert!(
        song(&mut h, |s| s.mixer.channels[0].send[0]) > 0.5,
        "send moved"
    );
    ctrl_z(&mut h);
    assert_eq!(
        song(&mut h, |s| s.mixer.channels[0].send[0]),
        orig.channels[0].send[0],
        "send undo"
    );
    // redo brings the last one back
    key(&mut h, Key::Y, egui::Modifiers::COMMAND);
    assert!(
        song(&mut h, |s| s.mixer.channels[0].send[0]) > 0.5,
        "send redo"
    );
}

#[test]
fn tempo_drag_undo_restores_song_and_engine() {
    let mut h = harness();
    let bpm0 = song(&mut h, |s| s.bpm);
    let t = rects(
        &h,
        Role::SpinButton,
        Some("tempo in beats per minute (drag or type)"),
    )[0];
    drag(&mut h, t.center(), t.center() + egui::vec2(40.0, 0.0));
    let bpm1 = song(&mut h, |s| s.bpm);
    assert!(bpm1 > bpm0, "tempo dragged up");
    assert_eq!(engine_bpm(&h), bpm1);
    ctrl_z(&mut h);
    assert_eq!(song(&mut h, |s| s.bpm), bpm0, "song tempo undo");
    assert_eq!(engine_bpm(&h), bpm0, "engine follows tempo undo");
}

fn type_into(h: &mut Harness<'static, App>, at: Rect, text: &str) {
    // click enters text editing (all selected); type the new value
    click_at(h, at.center());
    frame(h);
    h.input_mut().events.push(egui::Event::Text(text.into()));
    frame(h);
}

const TEMPO: &str = "tempo in beats per minute (drag or type)";
const SPEED: &str = "ticks per row (effect 09xx sets it in a pattern)";

#[test]
fn tempo_typed_then_ctrl_z_while_field_focused() {
    let mut h = harness();
    let (bpm0, sp0) = song(&mut h, |s| (s.bpm, s.speed));
    // an earlier, different edit: speed by drag
    let sp = rects(&h, Role::SpinButton, Some(SPEED))[0];
    drag(&mut h, sp.center(), sp.center() + egui::vec2(30.0, 0.0));
    let sp1 = song(&mut h, |s| s.speed);
    assert_ne!(sp1, sp0);
    // type a tempo and press Ctrl+Z while the field is still being edited
    let t = rects(&h, Role::SpinButton, Some(TEMPO))[0];
    type_into(&mut h, t, "170");
    assert_eq!(song(&mut h, |s| s.bpm), 170.0, "typed tempo applied");
    assert!(h.ctx.text_edit_focused(), "field still focused");
    ctrl_z(&mut h);
    assert_eq!(
        song(&mut h, |s| (s.bpm, s.speed)),
        (bpm0, sp1),
        "1st Ctrl+Z: the typed tempo only"
    );
    assert_eq!(engine_bpm(&h), bpm0, "engine follows");
    ctrl_z(&mut h);
    assert_eq!(song(&mut h, |s| s.speed), sp0, "2nd Ctrl+Z: the speed");
    // redo by keyboard: speed, then tempo
    key(&mut h, Key::Y, egui::Modifiers::COMMAND);
    key(
        &mut h,
        Key::Z,
        egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
    );
    assert_eq!(
        song(&mut h, |s| (s.bpm, s.speed)),
        (170.0, sp1),
        "redo both"
    );
    assert_eq!(engine_bpm(&h), 170.0);
}

/// typed, confirmed with Enter, then focus back in the field: Ctrl+Z still
/// undoes the song, not just the text
#[test]
fn tempo_entered_then_ctrl_z_with_field_refocused() {
    let mut h = harness();
    let bpm0 = song(&mut h, |s| s.bpm);
    let t = rects(&h, Role::SpinButton, Some(TEMPO))[0];
    type_into(&mut h, t, "123");
    key(&mut h, Key::Enter, egui::Modifiers::NONE);
    assert_eq!(song(&mut h, |s| s.bpm), 123.0);
    click_at(&mut h, t.center());
    frame(&mut h);
    assert!(h.ctx.text_edit_focused());
    ctrl_z(&mut h);
    assert_eq!(song(&mut h, |s| s.bpm), bpm0);
    assert_eq!(engine_bpm(&h), bpm0);
}

/// F9 / F10 (mute / solo) in the pattern editor, then Ctrl+Z
#[test]
fn pattern_view_mute_solo_keys_undo() {
    let mut h = harness();
    key(&mut h, Key::F9, egui::Modifiers::NONE);
    key(&mut h, Key::F10, egui::Modifiers::NONE);
    let m = song(&mut h, |s| {
        (s.mixer.channels[0].mute, s.mixer.channels[0].solo)
    });
    assert_eq!(m, (true, true));
    ctrl_z(&mut h);
    assert_eq!(
        song(&mut h, |s| (
            s.mixer.channels[0].mute,
            s.mixer.channels[0].solo
        )),
        (true, false)
    );
    ctrl_z(&mut h);
    assert_eq!(
        song(&mut h, |s| (
            s.mixer.channels[0].mute,
            s.mixer.channels[0].solo
        )),
        (false, false)
    );
}

#[test]
fn speed_and_edit_menu_undo() {
    let mut h = harness();
    let sp0 = song(&mut h, |s| s.speed);
    let t = rects(
        &h,
        Role::SpinButton,
        Some("ticks per row (effect 09xx sets it in a pattern)"),
    )[0];
    drag(&mut h, t.center(), t.center() + egui::vec2(30.0, 0.0));
    assert_ne!(song(&mut h, |s| s.speed), sp0, "speed dragged");
    // Edit > Undo with the mouse
    let e = rects(&h, Role::Button, Some("Edit"))[0];
    click_at(&mut h, e.center());
    let u = rects(&h, Role::Button, Some("Undo Ctrl+Z"))[0];
    click_at(&mut h, u.center());
    assert_eq!(song(&mut h, |s| s.speed), sp0, "Edit > Undo");
}

/// several quick mouse edits in a row (well under a second apart), then
/// Ctrl+Z once per edit: each must come back on its own
#[test]
fn quick_mixer_edits_undo_one_at_a_time() {
    let mut h = harness();
    mixer(&mut h);
    let orig = song(&mut h, |s| s.mixer.clone());
    let (fader, pan) = strip(&h, 0);
    drag(
        &mut h,
        fader.center(),
        fader.center() - egui::vec2(0.0, 80.0),
    );
    drag(&mut h, pan.center(), pan.center() + egui::vec2(30.0, 0.0));
    let m = rects(&h, Role::Button, Some(" M "))[0];
    click_at(&mut h, m.center());
    let s = rects(&h, Role::Button, Some(" S "))[2];
    click_at(&mut h, s.center());
    let after = song(&mut h, |s| s.mixer.clone());
    assert!(after.channels[0].mute && after.channels[2].solo);
    ctrl_z(&mut h);
    let m1 = song(&mut h, |s| s.mixer.clone());
    assert!(
        !m1.channels[2].solo && m1.channels[0].mute,
        "1st undo: only solo"
    );
    ctrl_z(&mut h);
    let m2 = song(&mut h, |s| s.mixer.clone());
    assert!(
        !m2.channels[0].mute && m2.channels[0].pan == after.channels[0].pan,
        "2nd undo: only mute"
    );
    ctrl_z(&mut h);
    let m3 = song(&mut h, |s| s.mixer.clone());
    assert_eq!(m3.channels[0].pan, orig.channels[0].pan, "3rd undo: pan");
    assert_eq!(
        m3.channels[0].volume, after.channels[0].volume,
        "3rd undo keeps fader"
    );
    ctrl_z(&mut h);
    assert_eq!(song(&mut h, |s| s.mixer.clone()), orig, "4th undo: fader");
}

/// rect of the first widget whose label starts with `prefix`
fn labelled(h: &Harness<'static, App>, prefix: &str) -> Rect {
    h.query_all_by(|n| n.label().is_some_and(|l| l.starts_with(prefix)))
        .next()
        .unwrap_or_else(|| panic!("no widget labelled {prefix:?}"))
        .rect()
}

fn click_label(h: &mut Harness<'static, App>, prefix: &str) {
    let r = labelled(h, prefix);
    click_at(h, r.center());
}

fn instr_names(h: &mut Harness<'static, App>) -> Vec<String> {
    song(h, |s| {
        s.instruments.iter().map(|i| i.name.clone()).collect()
    })
}

#[test]
fn delete_instrument_button_confirm_and_undo() {
    let mut h = harness();
    let before = song(&mut h, |s| s.clone());
    assert!(before.instruments.len() >= 3);
    let uses = before.instrument_uses(2);
    assert!(uses > 0, "demo song uses instrument 02");
    let name2 = before.instruments[1].name.clone();
    // select 02 in the list, then the small Delete button
    click_label(&mut h, "02  ");
    assert_eq!(h.state().instr, 2);
    click_label(&mut h, "Delete…");
    // confirmation names the number of notes
    let yes = labelled(&h, &format!("Delete and remove {uses} note(s)"));
    assert_eq!(
        instr_names(&mut h).len(),
        before.instruments.len(),
        "nothing deleted yet"
    );
    click_at(&mut h, yes.center());
    let after = song(&mut h, |s| s.clone());
    assert_eq!(after.instruments.len(), before.instruments.len() - 1);
    assert!(
        !after.instruments.iter().any(|i| i.name == name2)
            || before
                .instruments
                .iter()
                .filter(|i| i.name == name2)
                .count()
                > 1
    );
    assert_eq!(
        after.instruments[1].fx, before.instruments[2].fx,
        "FX stay with their instrument"
    );
    assert_eq!(
        after.instrument_uses(2),
        before.instrument_uses(3),
        "later notes moved down a slot"
    );
    // one Ctrl+Z restores everything
    ctrl_z(&mut h);
    let undone = song(&mut h, |s| s.clone());
    assert_eq!(undone.instruments, before.instruments);
    assert_eq!(undone.patterns, before.patterns);
    assert_eq!(undone.automation.len(), before.automation.len());
    // redo deletes again
    key(&mut h, Key::Y, egui::Modifiers::COMMAND);
    assert_eq!(instr_names(&mut h).len(), before.instruments.len() - 1);
}

#[test]
fn delete_cancel_and_unused_instrument_via_context_menu_and_edit_menu() {
    let mut h = harness();
    let n0 = instr_names(&mut h).len();
    // Edit > Duplicate instrument (selected = 01): appended, selected, unused
    click_label(&mut h, "01  ");
    click_label(&mut h, "Edit");
    click_label(&mut h, "Duplicate instrument");
    let names = instr_names(&mut h);
    assert_eq!(names.len(), n0 + 1);
    assert_eq!(names[n0], format!("{} copy", names[0]));
    assert_eq!(h.state().instr as usize, n0 + 1);
    // right-click the copy > Delete instrument: unused, so no question asked
    let slot = format!("{:02X}  ", n0 + 1);
    let r = labelled(&h, &slot);
    click_with(&mut h, r.center(), egui::PointerButton::Secondary);
    click_label(&mut h, "Delete instrument…");
    assert_eq!(instr_names(&mut h).len(), n0, "unused copy deleted at once");
    ctrl_z(&mut h);
    assert_eq!(
        instr_names(&mut h).len(),
        n0 + 1,
        "undo brings the copy back"
    );
    ctrl_z(&mut h);
    assert_eq!(instr_names(&mut h).len(), n0, "undo the duplicate");
    // a used instrument asks; Cancel keeps it
    click_label(&mut h, "01  ");
    click_label(&mut h, "Edit");
    click_label(&mut h, "Delete instrument…");
    click_label(&mut h, "Cancel");
    assert_eq!(instr_names(&mut h).len(), n0, "cancel keeps it");
}

// ---- MIDI keyboard (events injected as raw MIDI bytes) ----

fn midi(h: &mut Harness<'static, App>, bytes: &[u8]) {
    h.state().midi.hub.inject(bytes);
    frames(h, 2);
}

fn cell(h: &mut Harness<'static, App>, p: usize, r: usize, c: usize) -> Cell {
    song(h, |s| s.patterns[p].cells[r][c])
}

/// a harness on an empty block 0, cursor at row 4, channel 1, instrument 02
fn midi_harness() -> Harness<'static, App> {
    let mut s = fileio::demo_song();
    for row in s.patterns[0].cells.iter_mut() {
        row.iter_mut().for_each(|c| *c = Cell::default());
    }
    let mut h = harness_with(s);
    let a = h.state_mut();
    a.row = 4;
    a.ch = 1;
    a.instr = 2;
    a.pattern = 0;
    frames(&mut h, 2);
    h
}

#[test]
fn midi_step_record_velocity_advance_and_undo_per_note() {
    let mut h = midi_harness();
    h.state_mut().step = 2;
    midi(&mut h, &[0x90, 60, 127]);
    let c = cell(&mut h, 0, 4, 1);
    assert_eq!(
        (c.note, c.instr, c.vol),
        (49, 2, 65),
        "C-4, instr 02, volume 40"
    );
    assert_eq!(h.state().row, 6, "cursor moved by the edit step");
    midi(&mut h, &[0x80, 60, 0]);
    // a soft note, with the MIDI octave one up
    h.state_mut().extra.midi.octave = 1;
    h.state_mut().midi_apply(false);
    midi(&mut h, &[0x90, 62, 64]);
    let c = cell(&mut h, 0, 6, 1);
    assert_eq!(c.note, 51 + 12, "D-5");
    assert_eq!(c.vol, crate::midi::vol_column(64.0 / 127.0));
    midi(&mut h, &[0x80, 62, 0]);
    // every recorded note is its own undo step
    ctrl_z(&mut h);
    assert_eq!(cell(&mut h, 0, 6, 1), Cell::default());
    assert_eq!(cell(&mut h, 0, 4, 1).note, 49);
    ctrl_z(&mut h);
    assert_eq!(cell(&mut h, 0, 4, 1), Cell::default());
    // velocity off: no volume column
    h.state_mut().extra.midi.velocity = false;
    h.state_mut().midi_apply(false);
    h.state_mut().row = 0;
    midi(&mut h, &[0x90, 60, 10]);
    assert_eq!(cell(&mut h, 0, 0, 1).vol, 0);
}

#[test]
fn midi_chord_spread_and_edit_off() {
    let mut h = midi_harness();
    h.state_mut().extra.midi.chord_spread = true;
    for k in [60, 64, 67] {
        h.state().midi.hub.inject(&[0x90, k, 100]);
    }
    frames(&mut h, 2);
    let notes: Vec<u8> = (1..4).map(|c| cell(&mut h, 0, 4, c).note).collect();
    assert_eq!(notes, [49, 53, 56], "chord over channels 2, 3, 4");
    assert_eq!(h.state().row, 4, "cursor waits for the chord");
    for k in [60, 64, 67] {
        h.state().midi.hub.inject(&[0x80, k, 0]);
    }
    frames(&mut h, 2);
    assert_eq!(h.state().row, 5, "chord released: one step down");
    // edit mode off: nothing is written
    h.state_mut().edit = false;
    midi(&mut h, &[0x90, 72, 100]);
    midi(&mut h, &[0x80, 72, 0]);
    assert_eq!(cell(&mut h, 0, 5, 1), Cell::default());
    assert_eq!(h.state().row, 5);
}

#[test]
fn midi_live_record_while_playing_at_playing_row() {
    let mut h = midi_harness();
    h.state_mut().row = 30;
    h.state_mut().follow = false;
    h.with_engine(|s| {
        let Shared { song, engine, .. } = s;
        engine.start(song, 0, 12, true, 0);
    });
    midi(&mut h, &[0x90, 60, 100]);
    assert_eq!(
        cell(&mut h, 0, 12, 1).note,
        49,
        "recorded at the playing row"
    );
    assert_eq!(h.state().row, 30, "cursor stays");
    assert_eq!(cell(&mut h, 0, 30, 1), Cell::default());
    // step mode while playing when live record is off
    h.state_mut().extra.midi.live_record = false;
    midi(&mut h, &[0x90, 62, 100]);
    assert_eq!(cell(&mut h, 0, 30, 1).note, 51);
}

#[test]
fn midi_live_notes_play_with_sustain_and_reach_the_engine() {
    let mut h = midi_harness();
    // pick a sustaining synth so voices stay up
    h.with_engine(|s| {
        let mut p = crate::synth::Patch::default();
        p.amp.s = 0.8;
        s.song.instruments[1].kind = InstrumentKind::Synth(p);
    });
    midi(&mut h, &[0xB0, 64, 127]); // pedal down
    midi(&mut h, &[0x90, 60, 100]);
    midi(&mut h, &[0x80, 60, 0]);
    let mut buf = vec![0.0f32; 2048];
    let voices = h.with_engine(|s| {
        let Shared { song, engine, .. } = s;
        engine.render(song, &mut buf);
        engine.live_voices()
    });
    assert_eq!(voices, 1, "note held by the pedal");
    assert!(buf.iter().any(|x| x.abs() > 1e-4), "it sounds");
}

#[test]
fn midi_learn_via_right_click_on_a_fader() {
    let mut h = harness();
    mixer(&mut h);
    let (fader, _) = strip(&h, 0);
    click_with(&mut h, fader.center(), egui::PointerButton::Secondary);
    click_label(&mut h, "MIDI learn");
    assert_eq!(h.state().midi.learn.as_deref(), Some("mixer.ch.1.volume"));
    midi(&mut h, &[0xB0, 1, 10]); // mod wheel: binds
    assert_eq!(
        h.state().extra.midi.learn,
        vec![(1, "mixer.ch.1.volume".to_string())]
    );
    let v0 = song(&mut h, |s| s.mixer.channels[0].volume);
    midi(&mut h, &[0xB0, 1, 127]);
    assert_eq!(song(&mut h, |s| s.mixer.channels[0].volume), 2.0);
    midi(&mut h, &[0xB0, 1, 0]);
    assert_eq!(song(&mut h, |s| s.mixer.channels[0].volume), 0.0);
    // other CCs do nothing; Ctrl+Z undoes the wheel moves
    midi(&mut h, &[0xB0, 2, 99]);
    // a quick sweep is one step; allow a split step on a slow machine
    for _ in 0..3 {
        if song(&mut h, |s| s.mixer.channels[0].volume) != v0 {
            ctrl_z(&mut h);
        }
    }
    assert_eq!(song(&mut h, |s| s.mixer.channels[0].volume), v0);
}

#[test]
fn midi_light_opens_the_midi_settings_card() {
    let mut h = harness();
    click_label(&mut h, "MIDI");
    assert_eq!(h.state().view, View::Settings);
    assert_eq!(h.state().set_open, Some(4));
    frames(&mut h, 2);
    labelled(&h, "Rescan");
    labelled(&h, "Chords: spread");
    // switching velocity off from the card reaches the MIDI thread config
    click_label(&mut h, "use velocity");
    assert!(!h.state().extra.midi.velocity);
    assert!(!h
        .state()
        .midi
        .hub
        .cfg
        .velocity
        .load(std::sync::atomic::Ordering::Relaxed));
}
