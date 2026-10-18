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

// ---- 0.2.3.1: toolbar MIDI toggles, instrument FX menu, mixer resets ----

/// rect of the widget whose label (or text, for plain labels) is exactly `text`
fn exact(h: &Harness<'static, App>, text: &str) -> Rect {
    h.query_all_by(|n| n.label().as_deref() == Some(text) || n.value().as_deref() == Some(text))
        .next()
        .unwrap_or_else(|| panic!("no widget labelled exactly {text:?}"))
        .rect()
}

fn click_exact(h: &mut Harness<'static, App>, text: &str) {
    let r = exact(h, text);
    click_at(h, r.center());
}

#[test]
fn toolbar_chord_spread_and_rec_mode_toggles() {
    let mut h = midi_harness();
    assert!(!h.state().extra.midi.chord_spread);
    // the toolbar Chord toggle switches chord spread on...
    click_exact(&mut h, "Chord");
    assert!(h.state().extra.midi.chord_spread);
    // ...and a chord now spreads over channels 2, 3
    for k in [60, 64] {
        h.state().midi.hub.inject(&[0x90, k, 100]);
    }
    frames(&mut h, 2);
    let notes: Vec<u8> = (1..3).map(|c| cell(&mut h, 0, 4, c).note).collect();
    assert_eq!(notes, [49, 53]);
    for k in [60, 64] {
        h.state().midi.hub.inject(&[0x80, k, 0]);
    }
    frames(&mut h, 2);
    // and off again
    click_exact(&mut h, "Chord");
    assert!(!h.state().extra.midi.chord_spread);
    // the Rec indicator shows and switches live / step record mode
    let live0 = h.state().extra.midi.live_record;
    let (a, b) = if live0 {
        ("Rec live", "Rec step")
    } else {
        ("Rec step", "Rec live")
    };
    click_exact(&mut h, a);
    assert_eq!(h.state().extra.midi.live_record, !live0);
    exact(&h, b);
    click_exact(&mut h, b);
    assert_eq!(h.state().extra.midi.live_record, live0);
    exact(&h, a);
}

fn fx_of(h: &mut Harness<'static, App>, slot: usize) -> Vec<crate::fx::Device> {
    song(h, |s| s.instruments[slot - 1].fx.clone())
}

fn instr_menu(h: &mut Harness<'static, App>, slot: usize) {
    let r = labelled(h, &format!("{slot:02X}  "));
    click_with(h, r.center(), egui::PointerButton::Secondary);
}

#[test]
fn instrument_menu_add_effect_is_undoable() {
    let mut h = harness();
    let before = fx_of(&mut h, 2);
    instr_menu(&mut h, 2);
    click_exact(&mut h, "Add effect");
    // every FX device is listed
    for k in crate::fx::FxKind::ALL {
        exact(&h, k.name());
    }
    click_exact(&mut h, "Bitcrusher");
    let after = fx_of(&mut h, 2);
    assert_eq!(after.len(), before.len() + 1);
    assert_eq!(after.last().unwrap().kind, crate::fx::FxKind::Bitcrusher);
    assert_eq!(&after[..before.len()], &before[..], "added at the end");
    ctrl_z(&mut h);
    assert_eq!(fx_of(&mut h, 2), before, "undo removes it");
}

#[test]
fn instrument_menu_copy_paste_effects_and_open_in_fx_view() {
    let mut h = harness();
    // give 02 a known chain of two devices
    for k in ["Delay", "Drive"] {
        instr_menu(&mut h, 2);
        click_exact(&mut h, "Add effect");
        click_exact(&mut h, k);
    }
    let src = fx_of(&mut h, 2);
    assert!(src.len() >= 2);
    let one0 = fx_of(&mut h, 1);
    let three0 = fx_of(&mut h, 3);
    // paste is disabled until something is copied
    instr_menu(&mut h, 3);
    click_exact(&mut h, "Paste effects");
    assert_eq!(fx_of(&mut h, 3), three0, "nothing copied yet");
    h.input_mut().events.push(egui::Event::Key {
        key: Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Default::default(),
    });
    frames(&mut h, 3);
    // copy from 02
    instr_menu(&mut h, 2);
    click_exact(&mut h, "Copy effects");
    // paste (replace) into 03
    instr_menu(&mut h, 3);
    click_exact(&mut h, "Paste effects");
    assert_eq!(fx_of(&mut h, 3), src, "replaced");
    // paste (append) onto 01
    instr_menu(&mut h, 1);
    click_exact(&mut h, "Paste effects (append)");
    let mut want = one0.clone();
    want.extend(src.iter().cloned());
    assert_eq!(fx_of(&mut h, 1), want, "appended");
    // each paste is one undo step
    ctrl_z(&mut h);
    assert_eq!(fx_of(&mut h, 1), one0);
    assert_eq!(fx_of(&mut h, 3), src);
    ctrl_z(&mut h);
    assert_eq!(fx_of(&mut h, 3), three0);
    // Open in FX view selects the instrument and switches view
    instr_menu(&mut h, 3);
    click_exact(&mut h, "Open in FX view");
    assert_eq!(h.state().view, View::Fx);
    assert_eq!(h.state().instr, 3);
}

/// two quick clicks at one spot (frames 50 ms apart: the harness's default
/// 250 ms step is too slow for egui to count a double-click)
fn double_click(h: &mut Harness<'static, App>, p: Pos2) {
    h.input_mut().events.push(egui::Event::PointerMoved(p));
    frame(h);
    let mut t = h.ctx.input(|i| i.time);
    for _ in 0..2 {
        for pressed in [true, false] {
            t += 0.05;
            h.input_mut().time = Some(t);
            h.input_mut().events.push(egui::Event::PointerButton {
                pos: p,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            });
            frame(h);
        }
    }
    // back to the harness clock (RawInput keeps an explicit time)
    h.input_mut().time = None;
    frames(h, 3);
}

#[test]
fn mixer_double_click_resets_fader_and_pan() {
    let mut h = harness();
    mixer(&mut h);
    let (fader, pan) = strip(&h, 0);
    // move both away from their defaults
    drag(
        &mut h,
        fader.center(),
        fader.center() + egui::vec2(0.0, 50.0),
    );
    drag(&mut h, pan.center(), pan.center() + egui::vec2(30.0, 0.0));
    let m = song(&mut h, |s| s.mixer.channels[0]);
    assert!(
        m.volume != 1.0 && m.pan > 0.2,
        "moved: {} {}",
        m.volume,
        m.pan
    );
    // double-click near the ends, so a plain click would not land on the default
    double_click(&mut h, fader.center_bottom() - egui::vec2(0.0, 8.0));
    assert_eq!(song(&mut h, |s| s.mixer.channels[0].volume), 1.0, "0 dB");
    double_click(&mut h, pan.right_center() - egui::vec2(6.0, 0.0));
    assert_eq!(song(&mut h, |s| s.mixer.channels[0].pan), 0.0, "centre");
    // the reset is undoable
    ctrl_z(&mut h);
    assert!(song(&mut h, |s| s.mixer.channels[0].pan) != 0.0);
}

#[test]
fn help_about_shows_credit_and_coffee_link() {
    let mut h = harness();
    click_exact(&mut h, "Help");
    click_exact(&mut h, "About ChipWave");
    assert!(h.state().show_about);
    exact(&h, "Created by Derek Heasley");
    exact(&h, "To keep the software going I need Coffee :)");
    // each link asks the platform to open its page
    let mut click_link = |label: &str| {
        let r = exact(&h, label);
        h.input_mut()
            .events
            .push(egui::Event::PointerMoved(r.center()));
        frame(&mut h);
        let mut opened = vec![];
        for pressed in [true, false] {
            h.input_mut().events.push(egui::Event::PointerButton {
                pos: r.center(),
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            });
            frame(&mut h);
            for c in &h.output().platform_output.commands {
                if let egui::OutputCommand::OpenUrl(u) = c {
                    opened.push(u.url.clone());
                }
            }
        }
        opened
    };
    assert_eq!(
        click_link("Buy me a coffee"),
        ["https://buymeacoffee.com/postmanpat"]
    );
    assert_eq!(click_link("PayPal"), ["https://paypal.me/derekh100"]);
}

#[test]
fn file_demo_songs_menu_and_help_load_demo_song() {
    let mut h = harness();
    // start from an empty song, then File > Demo songs > Lumen Drift
    h.state_mut().with(|s| s.song = Song::empty());
    frames(&mut h, 2); // the change is noticed (dirty); pretend it was saved
    h.state_mut().dirty = false;
    frames(&mut h, 2);
    click_exact(&mut h, "File");
    click_exact(&mut h, "Demo songs");
    // every built-in demo is listed; Hollow Ritual loads
    for (name, _, _) in fileio::DEMOS {
        exact(&h, name);
    }
    click_exact(&mut h, "Hollow Ritual");
    assert_eq!(
        song(&mut h, |s| s.title.clone()),
        fileio::demo_song_n(1).title
    );
    assert!(!h.state().dirty);
    click_exact(&mut h, "File");
    click_exact(&mut h, "Demo songs");
    click_exact(&mut h, "Lumen Drift");
    let demo = fileio::demo_song();
    assert_eq!(song(&mut h, |s| s.title.clone()), demo.title);
    assert_eq!(song(&mut h, |s| s.order.clone()), demo.order);
    assert!(!h.state().dirty);
    // an edited song asks first; Help > Load demo song, then Don't save
    h.state_mut().with(|s| s.song.title = "mine".into());
    h.state_mut().dirty = true;
    click_exact(&mut h, "Help");
    click_exact(&mut h, "Load demo song");
    assert!(h.state().prompt.is_some(), "unsaved prompt");
    assert_eq!(song(&mut h, |s| s.title.clone()), "mine");
    click_label(&mut h, "Don");
    assert_eq!(song(&mut h, |s| s.title.clone()), demo.title);
}

#[test]
fn mixer_duck_amount_drags_and_undoes() {
    let mut h = harness();
    mixer(&mut h);
    // the demo's bass ducks 55 % under the kick (the sidechain key, channel 1)
    assert_eq!(song(&mut h, |s| s.mixer.duck_key), 0);
    exact(&h, "1 Kick");
    let r = exact(&h, "55.0 %");
    drag(&mut h, r.center(), r.center() + egui::vec2(30.0, 0.0));
    let d = song(&mut h, |s| s.mixer.channels[1].duck);
    assert!(d > 0.6, "dragged up: {d}");
    ctrl_z(&mut h);
    assert_eq!(song(&mut h, |s| s.mixer.channels[1].duck), 0.55);
    // the release time is shown in the header
    assert!(
        h.query_all_by(|n| n.value().is_some_and(|v| v.starts_with("release 300")))
            .next()
            .is_some(),
        "release 300 ms"
    );
}

#[test]
fn instrument_lfo_sync_and_shape_are_editable_and_undoable() {
    let mut h = harness();
    h.state_mut().instr = 2;
    frames(&mut h, 2);
    let lfo = |h: &mut Harness<'static, App>| {
        song(h, |s| match &s.instruments[1].kind {
            crate::model::InstrumentKind::Synth(p) => p.lfo,
            _ => panic!("02 is a synth"),
        })
    };
    assert_eq!(lfo(&mut h).sync, 0.0);
    // the patch editor is taller than the test window: scroll it to the LFO row
    let at = exact(&h, "free Hz").center() - egui::vec2(0.0, 300.0);
    h.input_mut().events.push(egui::Event::PointerMoved(at));
    frame(&mut h);
    for _ in 0..4 {
        h.input_mut().events.push(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -200.0),
            phase: egui::TouchPhase::Move,
            source: egui::MouseWheelSource::Wheel,
            modifiers: Default::default(),
        });
        frame(&mut h);
    }
    frames(&mut h, 10);
    click_exact(&mut h, "free Hz");
    click_exact(&mut h, "1/16");
    assert_eq!(lfo(&mut h).sync, 0.25, "tempo-synced to 1/16");
    click_exact(&mut h, "Sine");
    click_exact(&mut h, "Square");
    assert_eq!(lfo(&mut h).shape, crate::synth::Wave::Square);
    ctrl_z(&mut h);
    assert_eq!(lfo(&mut h).shape, crate::synth::Wave::Sine);
}

// ---- 0.2.3.5: update check ----

/// click the widget labelled `label`; the URLs the app asked the platform to open
fn click_urls(h: &mut Harness<'static, App>, label: &str) -> Vec<String> {
    let r = exact(h, label);
    h.input_mut()
        .events
        .push(egui::Event::PointerMoved(r.center()));
    frame(h);
    let mut opened = vec![];
    for pressed in [true, false] {
        h.input_mut().events.push(egui::Event::PointerButton {
            pos: r.center(),
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        });
        frame(h);
        for c in &h.output().platform_output.commands {
            if let egui::OutputCommand::OpenUrl(u) = c {
                opened.push(u.url.clone());
            }
        }
    }
    frames(h, 2);
    opened
}

fn sample_release() -> crate::update::Release {
    crate::update::parse_release(crate::update::SAMPLE_JSON).unwrap()
}

/// Help > Check for updates…, then the mocked network answers `o`
fn manual_check(h: &mut Harness<'static, App>, o: crate::update::Outcome) {
    click_exact(h, "Help");
    click_exact(h, "Check for updates…");
    assert_eq!(h.state().upd.win, Some(updates::UpdWin::Checking));
    exact(h, "Checking for updates…");
    let tx = h.state().upd.test_tx.clone().expect("check started");
    tx.send(o).unwrap();
    frames(h, 3);
}

#[test]
fn update_check_newer_version_dialog_opens_links_only_on_click() {
    use crate::update::Outcome;
    let mut h = harness();
    manual_check(&mut h, Outcome::Newer(sample_release()));
    exact(
        &h,
        &format!("You have {}, latest is 0.2.3.9", crate::VERSION),
    );
    exact(&h, "Update check in Help"); // release notes, markdown stripped
    exact(&h, "What's new");
    // nothing was opened just by showing the dialog; the installer opens on click
    assert_eq!(
        click_urls(&mut h, "Download installer"),
        ["https://github.com/postmanpat-303/ChipWaveTracker/releases/download/v0.2.3.9/ChipWave-0.2.3.9-setup-x64.exe"]
    );
    assert_eq!(h.state().upd.win, None, "dialog closed");
    manual_check(&mut h, Outcome::Newer(sample_release()));
    assert_eq!(
        click_urls(&mut h, "Open release page"),
        ["https://github.com/postmanpat-303/ChipWaveTracker/releases/tag/v0.2.3.9"]
    );
    manual_check(&mut h, Outcome::Newer(sample_release()));
    assert!(click_urls(&mut h, "Not now").is_empty());
    assert_eq!(h.state().upd.win, None);
    assert_eq!(h.state().extra.update_skip, "");
    manual_check(&mut h, Outcome::Newer(sample_release()));
    assert!(click_urls(&mut h, "Skip this version").is_empty());
    assert_eq!(h.state().extra.update_skip, "0.2.3.9");
    assert_eq!(h.state().upd.win, None);
}

#[test]
fn update_check_up_to_date_and_failure_messages() {
    use crate::update::Outcome;
    let mut h = harness();
    manual_check(&mut h, Outcome::Latest(crate::VERSION.into()));
    exact(
        &h,
        &format!("You're on the latest version ({})", crate::VERSION),
    );
    click_exact(&mut h, "OK");
    frames(&mut h, 2);
    assert_eq!(h.state().upd.win, None);
    manual_check(&mut h, Outcome::Failed("no connection (dns failed)".into()));
    exact(&h, "Couldn't check for updates right now.");
    // Try again starts a new check; Close dismisses without opening anything
    click_exact(&mut h, "Try again");
    frames(&mut h, 2);
    assert_eq!(h.state().upd.win, Some(updates::UpdWin::Checking));
    let tx = h.state().upd.test_tx.clone().unwrap();
    tx.send(Outcome::Failed("timeout".into())).unwrap();
    frames(&mut h, 3);
    assert!(click_urls(&mut h, "Close").is_empty());
    assert_eq!(h.state().upd.win, None);
    // a vanished checker thread is a friendly failure, not a hang
    click_exact(&mut h, "Help");
    click_exact(&mut h, "Check for updates…");
    h.state_mut().upd.test_tx = None; // drop the sender
    frames(&mut h, 3);
    assert!(matches!(
        h.state().upd.win,
        Some(updates::UpdWin::Failed(_))
    ));
}

#[test]
fn quiet_startup_check_shows_a_badge_and_respects_skip() {
    use crate::update::Outcome;
    assert!(
        crate::settings::Extra::default().update_check,
        "on by default"
    );
    let mut h = harness();
    h.state_mut().check_updates(false);
    let tx = h.state().upd.test_tx.clone().unwrap();
    tx.send(Outcome::Newer(sample_release())).unwrap();
    frames(&mut h, 3);
    assert_eq!(h.state().upd.win, None, "quiet: no dialog");
    assert!(h.state().upd.badge.is_some());
    // clicking the menu-bar badge shows the details (still opens nothing)
    assert!(click_urls(&mut h, "Update 0.2.3.9 available").is_empty());
    assert!(matches!(h.state().upd.win, Some(updates::UpdWin::Newer(_))));
    // up to date or failures stay silent
    let mut h = harness();
    for o in [
        Outcome::Latest("0.2.3.4".into()),
        Outcome::Failed("offline".into()),
    ] {
        h.state_mut().check_updates(false);
        h.state().upd.test_tx.clone().unwrap().send(o).unwrap();
        frames(&mut h, 3);
        assert_eq!(
            (h.state().upd.win.clone(), h.state().upd.badge.clone()),
            (None, None)
        );
    }
    // a skipped version gives no badge
    h.state_mut().extra.update_skip = "0.2.3.9".into();
    h.state_mut().check_updates(false);
    h.state()
        .upd
        .test_tx
        .clone()
        .unwrap()
        .send(Outcome::Newer(sample_release()))
        .unwrap();
    frames(&mut h, 3);
    assert!(h.state().upd.badge.is_none());
}

#[test]
fn file_open_text_song_loads_unsaved() {
    let mut h = harness();
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("demos/dark_psy.cwt.toml");
    h.state_mut().open_path(p);
    frames(&mut h, 2);
    assert!(song(&mut h, |s| s.title.clone()).starts_with("Hollow Ritual"));
    // never saved back over the text file: Save goes to Save As
    assert!(h.state().path.is_none());
    assert_eq!(song(&mut h, |s| s.mixer.channels[0].pan), 0.0);
}

#[test]
fn new_voice_instrument_from_the_instrument_menu_edits_and_undoes() {
    let mut h = harness();
    let n0 = song(&mut h, |s| s.instruments.len());
    instr_menu(&mut h, 1);
    click_exact(&mut h, "New instrument");
    click_exact(&mut h, "Voice (glitch)");
    for (name, _, _) in crate::voice::presets() {
        exact(&h, name);
    }
    click_exact(&mut h, "Demon Ahh");
    frames(&mut h, 2);
    assert_eq!(song(&mut h, |s| s.instruments.len()), n0 + 1);
    assert_eq!(
        h.state().instr as usize,
        n0 + 1,
        "the new voice is selected"
    );
    let seq = |h: &mut Harness<'static, App>| {
        song(h, |s| match &s.instruments.last().unwrap().kind {
            InstrumentKind::Voice(v) => v.vowels.clone(),
            _ => panic!("not a voice"),
        })
    };
    assert_eq!(
        seq(&mut h),
        crate::voice::preset("Demon Ahh").unwrap().vowels
    );
    // the Voice panel is shown; clicking the ER chip adds it to the sequence
    exact(&h, "VOICE (glitch)");
    let before = seq(&mut h);
    click_exact(&mut h, "ER");
    let after = seq(&mut h);
    assert_eq!(after.len(), before.len() + 1);
    assert_eq!(*after.last().unwrap(), crate::voice::Vowel::Er);
    // undo: first the chip, then the new instrument
    ctrl_z(&mut h);
    assert_eq!(seq(&mut h), before);
    ctrl_z(&mut h);
    assert_eq!(song(&mut h, |s| s.instruments.len()), n0);
}

#[test]
fn mixer_limiter_panel_toggles_edits_and_undoes() {
    let mut h = harness();
    mixer(&mut h);
    exact(&h, "Limiter & loudness");
    assert!(song(&mut h, |s| s.mixer.limiter.enabled));
    click_exact(&mut h, "True-peak limiter");
    frames(&mut h, 2);
    assert!(!song(&mut h, |s| s.mixer.limiter.enabled), "bypassed");
    ctrl_z(&mut h);
    assert!(song(&mut h, |s| s.mixer.limiter.enabled), "undo");
    // drag the ceiling value up
    let r = exact(&h, "-1.0 dBTP");
    drag(&mut h, r.center(), r.center() + egui::vec2(40.0, 0.0));
    let c = song(&mut h, |s| s.mixer.limiter.ceiling_db);
    assert!(c > -1.0 && c <= 0.0, "ceiling {c}");
    ctrl_z(&mut h);
    assert_eq!(song(&mut h, |s| s.mixer.limiter.ceiling_db), -1.0);
    // the loudness readout rows exist (no audio in tests: dashes)
    exact(&h, "Integrated");
    exact(&h, "True peak");
}

#[test]
fn instrument_glide_legato_and_accent_are_editable_and_undoable() {
    let mut h = harness();
    h.state_mut().instr = 2;
    frames(&mut h, 2);
    let patch = |h: &mut Harness<'static, App>| {
        song(h, |s| {
            s.instruments[1].kind.patch().cloned().expect("synth")
        })
    };
    assert!(!patch(&mut h).legato);
    let at = exact(&h, "free Hz").center() - egui::vec2(0.0, 300.0);
    h.input_mut().events.push(egui::Event::PointerMoved(at));
    frame(&mut h);
    for _ in 0..4 {
        h.input_mut().events.push(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -200.0),
            phase: egui::TouchPhase::Move,
            source: egui::MouseWheelSource::Wheel,
            modifiers: Default::default(),
        });
        frame(&mut h);
    }
    frames(&mut h, 10);
    click_exact(&mut h, "Legato");
    frames(&mut h, 2);
    assert!(patch(&mut h).legato, "legato on");
    // accent threshold: drag the hex value up from 00
    let r = exact(&h, "at vol ≥ 00");
    drag(&mut h, r.center(), r.center() + egui::vec2(60.0, 0.0));
    assert!(patch(&mut h).accent_vel > 0.0);
    ctrl_z(&mut h);
    assert_eq!(patch(&mut h).accent_vel, 0.0);
    ctrl_z(&mut h);
    assert!(!patch(&mut h).legato, "undo legato");
}

#[test]
fn thirty_two_channels_scroll_in_the_pattern_editor_and_mixer() {
    let mut h = harness();
    let n0 = song(&mut h, |s| s.channels);
    let spin = rects(&h, Role::SpinButton, Some(super::CHANNELS_TIP))[0];
    type_into(&mut h, spin, "32");
    key(&mut h, Key::Enter, egui::Modifiers::NONE);
    assert_eq!(song(&mut h, |s| s.channels), 32, "typed channel count");
    assert!(song(&mut h, |s| s
        .patterns
        .iter()
        .all(|p| p.cells.iter().all(|r| r.len() == 32))));
    // all 32 don't fit: a scroll bar appears; Tab walks the cursor to ch 32
    frames(&mut h, 2);
    let bar = h.state().ch_bar;
    assert!(bar.width() > 50.0, "channel scroll bar shown");
    for _ in 0..31 {
        key(&mut h, Key::Tab, egui::Modifiers::NONE);
    }
    assert_eq!(h.state().ch, 31);
    let v = h.state().ch_view;
    assert!(v > 8, "view followed the cursor: first channel {v}");
    // typing a note on channel 32 lands there
    h.state_mut().edit = true;
    h.state_mut().row = 0;
    h.state_mut().col = 0;
    frames(&mut h, 2);
    key(&mut h, Key::Q, egui::Modifiers::NONE);
    let c = song(&mut h, |s| s.patterns[h_pattern(s)].cells[0][31]);
    assert!(c.note > 0, "note on ch 32: {c:?}");
    // clicking the left end of the bar scrolls back; the cursor follows
    click_at(&mut h, Pos2::new(bar.left() + 2.0, bar.center().y));
    assert_eq!(h.state().ch_view, 0, "scrolled to channel 1");
    assert!(h.state().ch < v, "cursor moved into view");
    // mixer: 32 strips in a horizontal scroll area, master stays pinned
    mixer(&mut h);
    let faders = |h: &Harness<'static, App>| -> Vec<Rect> {
        rects(h, Role::Slider, None)
            .into_iter()
            .filter(|r| r.height() > 100.0)
            .collect()
    };
    let f = faders(&h);
    assert!(f.len() >= 33, "32 channel faders + master: {}", f.len());
    // the master strip comes after the scroll area
    let master_x = f.last().unwrap().left();
    assert!(master_x < 1700.0, "master on screen");
    let first = f[0];
    for _ in 0..20 {
        h.input_mut()
            .events
            .push(egui::Event::PointerMoved(first.center()));
        h.input_mut().events.push(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(-400.0, 0.0),
            modifiers: Default::default(),
            phase: egui::TouchPhase::Move,
            source: egui::MouseWheelSource::Wheel,
        });
        frame(&mut h);
    }
    frames(&mut h, 5);
    let f2 = faders(&h);
    assert!(f2[0].left() < first.left() - 500.0, "strips scrolled");
    let m2 = f2.last().unwrap().left();
    assert!((m2 - master_x).abs() < 1.0, "master pinned");
    // the bar under the strips: click its left end -> back to channel 1,
    // its right end -> the last strips
    let bar = h.state().mix_bar;
    assert!(bar.width() > 100.0, "mixer channel bar shown");
    click_at(&mut h, Pos2::new(bar.left() + 2.0, bar.center().y));
    frames(&mut h, 3);
    assert!(
        (faders(&h)[0].left() - first.left()).abs() < 2.0,
        "back at channel 1"
    );
    click_at(&mut h, Pos2::new(bar.right() - 2.0, bar.center().y));
    frames(&mut h, 3);
    let f3 = faders(&h);
    let last_ch = f3[31];
    assert!(
        last_ch.right() <= bar.right() + 2.0 && last_ch.left() > bar.left(),
        "channel 32 in view: {last_ch:?} bar {bar:?}"
    );
    // undo the channel count change: back to the old layout
    h.state_mut().view = View::Pattern;
    frames(&mut h, 2);
    ctrl_z(&mut h);
    ctrl_z(&mut h);
    assert_eq!(song(&mut h, |s| s.channels), n0);
}

fn h_pattern(s: &Song) -> usize {
    s.order.first().copied().unwrap_or(0)
}

// ---- 0.2.3.9: instrument library, instrument management, chord helper ----

/// rect of the first widget (or plain label) whose text starts with `prefix`
fn text_rect(h: &Harness<'static, App>, prefix: &str) -> Rect {
    h.query_all_by(|n| {
        n.label().is_some_and(|l| l.starts_with(prefix))
            || n.value().is_some_and(|l| l.starts_with(prefix))
    })
    .next()
    .unwrap_or_else(|| panic!("no text starting {prefix:?}"))
    .rect()
}

fn temp_library(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("cw-lib-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn library_add_save_reload_replace_and_undo() {
    let mut h = harness();
    let lib = temp_library("ui");
    h.state_mut().extra.library_dir = lib.display().to_string();
    let n0 = song(&mut h, |s| s.instruments.len());
    // File > Instrument library…
    click_exact(&mut h, "File");
    click_label(&mut h, "Instrument library");
    assert!(h.state().library.open, "library window open");
    assert!(!h.state().library.factory.is_empty(), "factory list");
    // "+" on the first factory sound adds it as a new slot
    let first = h.state().library.factory[0].name.clone();
    let row = text_rect(&h, &format!("{first} ("));
    let plus = *rects(&h, Role::Button, Some("+"))
        .iter()
        .find(|r| (r.center().y - row.center().y).abs() < 4.0 && r.right() <= row.left() + 1.0)
        .expect("+ next to the first factory sound");
    click_at(&mut h, plus.center());
    assert_eq!(song(&mut h, |s| s.instruments.len()), n0 + 1);
    assert_eq!(instr_names(&mut h)[n0], first);
    assert_eq!(h.state().instr as usize, n0 + 1, "new slot selected");
    ctrl_z(&mut h);
    assert_eq!(song(&mut h, |s| s.instruments.len()), n0, "undo add");
    // save slot 02 to the library from the footer button; it shows up in "My library"
    h.state_mut().instr = 2;
    frames(&mut h, 2);
    let name2 = instr_names(&mut h)[1].clone();
    click_label(&mut h, "Save 02 ");
    let files: Vec<_> = std::fs::read_dir(&lib)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .collect();
    assert_eq!(files.len(), 1, "one .cwi written: {files:?}");
    frames(&mut h, 2);
    assert!(
        h.state().library.user.iter().any(|e| e.name == name2),
        "rescanned library lists {name2}"
    );
    // the saved file round-trips
    let back = crate::instfile::load_cwi(&files[0]).unwrap();
    assert_eq!(back.name, name2);
    // right-click a factory sound > Replace sound of 05 keeps slot 05's notes
    h.state_mut().instr = 5;
    frames(&mut h, 2);
    let uses5 = song(&mut h, |s| s.instrument_uses(5));
    assert!(uses5 > 0);
    let label = format!("{} (", h.state().library.factory[1].name);
    let r = text_rect(&h, &label);
    click_with(&mut h, r.center(), egui::PointerButton::Secondary);
    click_label(&mut h, "Replace sound of 05");
    assert_eq!(instr_names(&mut h)[4], h.state().library.factory[1].name);
    assert_eq!(song(&mut h, |s| s.instrument_uses(5)), uses5, "notes kept");
    ctrl_z(&mut h);
    assert_ne!(
        instr_names(&mut h)[4],
        h.state().library.factory[1].name,
        "undo replace"
    );
    let _ = std::fs::remove_dir_all(&lib);
}

#[test]
fn instrument_rename_move_and_swap_from_the_instrument_list() {
    let mut h = harness();
    let before = song(&mut h, |s| s.clone());
    // double-click renames inline; Enter commits; Ctrl+Z undoes
    let r = labelled(&h, "03  ");
    double_click(&mut h, r.center());
    assert!(h.state().renaming.is_some(), "rename field open");
    h.state_mut().renaming.as_mut().unwrap().1 = "Hat X".into();
    key(&mut h, Key::Enter, egui::Modifiers::NONE);
    assert_eq!(instr_names(&mut h)[2], "Hat X");
    ctrl_z(&mut h);
    assert_eq!(
        instr_names(&mut h)[2],
        before.instruments[2].name,
        "undo rename"
    );
    // context menu > Move down: slot 03 becomes 04 and its notes follow
    let u3 = before.instrument_uses(3);
    let u4 = before.instrument_uses(4);
    instr_menu(&mut h, 3);
    click_label(&mut h, "Move down");
    let after = song(&mut h, |s| s.clone());
    assert_eq!(after.instruments[3].name, before.instruments[2].name);
    assert_eq!(after.instruments[2].name, before.instruments[3].name);
    assert_eq!(after.instrument_uses(4), u3);
    assert_eq!(after.instrument_uses(3), u4);
    // the moved song renders exactly like the original
    let render = |s: &Song| {
        let mut e = Engine::new(48000.0);
        let mut s = s.clone();
        s.order.truncate(2);
        e.start(&s, 0, 0, false, 0);
        let mut buf = vec![0.0f32; 48000];
        e.render(&s, &mut buf);
        buf
    };
    assert_eq!(render(&before), render(&after), "move keeps the sound");
    ctrl_z(&mut h);
    assert_eq!(
        instr_names(&mut h),
        before
            .instruments
            .iter()
            .map(|i| i.name.clone())
            .collect::<Vec<_>>()
    );
    // Up / Down buttons
    h.state_mut().instr = 2;
    frames(&mut h, 2);
    click_exact(&mut h, "Up");
    assert_eq!(instr_names(&mut h)[0], before.instruments[1].name);
    assert_eq!(h.state().instr, 1, "selection follows");
    ctrl_z(&mut h);
    // Use this instead of > 02 (whole song): 02's notes now play 06
    let u2 = before.instrument_uses(2);
    let u6 = before.instrument_uses(6);
    instr_menu(&mut h, 6);
    click_label(&mut h, "Use this instead of");
    click_label(&mut h, &format!("02 {}", before.instruments[1].name));
    assert_eq!(song(&mut h, |s| s.instrument_uses(2)), 0);
    assert_eq!(song(&mut h, |s| s.instrument_uses(6)), u2 + u6);
    ctrl_z(&mut h);
    assert_eq!(song(&mut h, |s| s.instrument_uses(2)), u2, "undo swap");
    // search filters the list
    h.state_mut().instr_query = "bass".into();
    frames(&mut h, 2);
    let shown = h
        .query_all_by(|n| {
            n.label()
                .is_some_and(|l| l.len() > 4 && l.as_bytes()[2] == b' ' && l.as_bytes()[3] == b' ')
        })
        .count();
    let want = before
        .instruments
        .iter()
        .filter(|i| i.name.to_lowercase().contains("bass"))
        .count();
    assert_eq!(shown, want, "search shows only matching instruments");
}

#[test]
fn chord_helper_pads_progression_write_undo_and_scale_lock() {
    let mut h = harness();
    // Song > Chord helper…
    click_exact(&mut h, "Song");
    click_label(&mut h, "Chord helper");
    assert!(h.state().chords.open);
    click_label(&mut h, "Detect from song");
    let k = song(&mut h, |s| s.info.key).expect("key set by detection");
    assert_eq!(k.name(), "A minor", "Lumen Drift is in A minor");
    // click pads i, VI, III, VII (A minor: Am F C G)
    let pads: Vec<Rect> = (1..=7)
        .map(|d| {
            h.query_all_by(|n| {
                n.role() == Role::Button
                    && n.label().is_some_and(|l| {
                        l.ends_with(["Am", "Bdim", "C", "Dm", "Em", "F", "G"][d - 1])
                            && l.contains('\n')
                    })
            })
            .next()
            .unwrap_or_else(|| panic!("pad {d}"))
            .rect()
        })
        .collect();
    for d in [0, 5, 2, 6] {
        click_at(&mut h, pads[d].center());
    }
    let degs: Vec<usize> = h.state().chords.prog.iter().map(|s| s.deg).collect();
    assert_eq!(degs, [0, 5, 2, 6]);
    // a pad click previews: live voices sound
    assert!(!h.state().chords.offs.is_empty(), "preview notes held");
    // write into an empty block on channel 2 as chords
    let p = h.state().pattern;
    h.state_mut().ch = 1;
    let before = song(&mut h, |s| s.patterns[p].clone());
    click_label(&mut h, "Write into block");
    let after = song(&mut h, |s| s.patterns[p].clone());
    let ch_notes = |c: usize| -> Vec<u8> {
        after
            .cells
            .iter()
            .map(|r| r[c].note)
            .filter(|&n| n > 0 && n < 97)
            .collect()
    };
    assert!(
        !ch_notes(1).is_empty() && !ch_notes(2).is_empty() && !ch_notes(3).is_empty(),
        "triad over 3 channels"
    );
    for c in 1..4 {
        for n in ch_notes(c) {
            assert!(k.contains(crate::chords::pc_of(n)), "note {n} in A minor");
        }
    }
    assert_eq!(
        after.cells.iter().map(|r| r[0]).collect::<Vec<_>>(),
        before.cells.iter().map(|r| r[0]).collect::<Vec<_>>(),
        "channel 1 untouched"
    );
    ctrl_z(&mut h);
    assert_eq!(
        song(&mut h, |s| s.patterns[p].clone()).cells,
        before.cells,
        "undo write"
    );
    // bass style writes one channel
    click_exact(&mut h, "Bass");
    click_label(&mut h, "Write into block");
    let after = song(&mut h, |s| s.patterns[p].clone());
    assert!(after.cells.iter().any(|r| r[1].note > 0 && r[1].note < 97));
    assert_eq!(
        after.cells.iter().map(|r| r[2]).collect::<Vec<_>>(),
        before.cells.iter().map(|r| r[2]).collect::<Vec<_>>(),
        "bass: one channel"
    );
    ctrl_z(&mut h);
    // scale lock: typing C# (key 2, in octave 4) snaps into A minor
    click_label(&mut h, "Scale lock");
    assert!(h.state().extra.scale_lock);
    exact(&h, "lock A minor");
    h.state_mut().chords.open = false;
    h.state_mut().edit = true;
    h.state_mut().row = 1;
    h.state_mut().col = 0;
    h.state_mut().ch = 5;
    frames(&mut h, 2);
    key(&mut h, Key::Num2, egui::Modifiers::NONE); // C#
    let c = song(&mut h, |s| s.patterns[p].cells[1][5]);
    assert!(
        c.note > 0 && k.contains(crate::chords::pc_of(c.note)),
        "snapped note {}",
        c.note
    );
    // without the lock the same key gives C#
    h.state_mut().extra.scale_lock = false;
    h.state_mut().row = 2;
    frames(&mut h, 2);
    key(&mut h, Key::Num2, egui::Modifiers::NONE);
    let c2 = song(&mut h, |s| s.patterns[p].cells[2][5]);
    assert_eq!(crate::chords::pc_of(c2.note), 1, "unlocked C#: {}", c2.note);
}

// ---- 0.2.3.10: Voice words, Throat mode, sample slicing ----

fn last_voice(h: &mut Harness<'static, App>) -> crate::voice::VoicePatch {
    song(h, |s| match &s.instruments.last().unwrap().kind {
        InstrumentKind::Voice(v) => v.clone(),
        _ => panic!("not a voice"),
    })
}

#[test]
fn voice_words_and_throat_mode_from_the_voice_panel() {
    let mut h = harness();
    instr_menu(&mut h, 1);
    click_exact(&mut h, "New instrument");
    click_exact(&mut h, "Voice (glitch)");
    click_exact(&mut h, "Init voice");
    frames(&mut h, 2);
    assert!(last_voice(&mut h).words.is_empty() && !last_voice(&mut h).throat);
    // Tract: Throat
    click_exact(&mut h, "Throat");
    assert!(last_voice(&mut h).throat, "throat on");
    // + word twice: ka, ta
    click_exact(&mut h, "+ word");
    click_exact(&mut h, "+ word");
    assert_eq!(last_voice(&mut h).words, ["ka", "ta"]);
    // the new word plays on the next note: preview and check the phoneme
    let ins = h.state().instr;
    h.with_engine(|s| {
        let Shared { song, engine, .. } = s;
        engine.preview(song, 0, 37, ins);
    });
    let ph = h.with_engine(|s| s.engine.chan_speech(0));
    assert_eq!(ph, Some(crate::speech::Ph::K));
    // undo: the words, then Throat
    ctrl_z(&mut h);
    assert_eq!(last_voice(&mut h).words, ["ka"]);
    ctrl_z(&mut h);
    ctrl_z(&mut h);
    assert!(!last_voice(&mut h).throat, "undo throat");
    let names: Vec<&str> = crate::voice::presets().iter().map(|p| p.0).collect();
    for p in ["Talk Ka-Ta", "Throat Ahh", "Throat Speech", "Demon Words"] {
        assert!(names.contains(&p), "{p} preset");
    }
}

/// a 2 s "drum loop": 8 decaying noise hits, one every 250 ms (120 BPM 8ths)
fn loop_instrument() -> Instrument {
    let mut n = 0x2545_F491u32;
    let mut d = vec![0.0f32; 96000];
    for h in 0..8 {
        for k in 0..6000 {
            n ^= n << 13;
            n ^= n >> 17;
            n ^= n << 5;
            let r = n as f32 / u32::MAX as f32 * 2.0 - 1.0;
            d[h * 12000 + k] += r * 0.7 * (-(k as f32) / 900.0).exp();
        }
    }
    Instrument {
        name: "Loop".into(),
        volume: 1.0,
        transpose: 0,
        fx: vec![],
        kind: InstrumentKind::Sample {
            file: String::new(),
            rate: 48000,
            data: d.into(),
            base_note: 49,
            lp: Default::default(),
        },
    }
}

#[test]
fn sample_editor_slices_selects_edits_and_makes_notes() {
    let mut sg = fileio::demo_song();
    sg.bpm = 120.0;
    sg.lines_per_beat = 4;
    sg.instruments.push(loop_instrument());
    let slot = sg.instruments.len() as u8;
    let mut h = harness_with(sg);
    h.state_mut().instr = slot;
    h.state_mut().view = View::Sample;
    frames(&mut h, 3);
    click_exact(&mut h, "Transients");
    assert_eq!(h.state().smp.slices.len(), 8, "{:?}", h.state().smp.slices);
    exact(&h, "8 slices");
    // Next selects slice 1, then 2; Reverse only touches that slice
    click_exact(&mut h, "Next");
    click_exact(&mut h, "Next");
    let sel = h.state().smp.sel.expect("slice selected");
    assert!(
        sel.0.abs_diff(12000) < 200 && sel.1.abs_diff(24000) < 200,
        "{sel:?}"
    );
    exact(&h, "slice 2/8");
    let data = |h: &mut Harness<'static, App>| {
        song(h, |s| match &s.instruments[slot as usize - 1].kind {
            InstrumentKind::Sample { data, .. } => data.to_vec(),
            _ => unreachable!(),
        })
    };
    let before = data(&mut h);
    click_exact(&mut h, "Reverse");
    let after = data(&mut h);
    assert_eq!(
        after[..sel.0],
        before[..sel.0],
        "before the slice untouched"
    );
    assert_eq!(after[sel.1..], before[sel.1..], "after the slice untouched");
    assert_eq!(after[sel.0], before[sel.1 - 1], "slice reversed");
    // the sample editor's own undo puts it back (and drops the stale slices)
    ctrl_z(&mut h);
    assert_eq!(data(&mut h), before);
    assert!(h.state().smp.slices.is_empty());
    // Grid 4 replaces the slices
    h.state_mut().smp.grid_n = 4;
    click_exact(&mut h, "Grid");
    assert_eq!(h.state().smp.slices.len(), 4);
    click_exact(&mut h, "Transients");
    // To notes: 8 instruments + 8 notes on the cursor channel, at rows 0, 2, 4 ...
    let n0 = song(&mut h, |s| s.instruments.len());
    h.state_mut().ch = 6;
    let p = h.state().pattern;
    click_exact(&mut h, "To notes");
    assert_eq!(song(&mut h, |s| s.instruments.len()), n0 + 8);
    assert_eq!(h.state().view, View::Pattern, "shows the notes");
    let rows: Vec<usize> = song(&mut h, |s| {
        s.patterns[p]
            .cells
            .iter()
            .enumerate()
            .filter(|(_, r)| r[6].note > 0)
            .map(|(i, _)| i)
            .collect()
    });
    assert_eq!(
        rows,
        [0, 2, 4, 6, 8, 10, 12, 14],
        "one note per 8th at 120 BPM"
    );
    let first = song(&mut h, |s| s.patterns[p].cells[0][6]);
    assert_eq!(first.instr as usize, n0 + 1);
    // the slice instruments hold the audio of their slice
    let l1 = song(&mut h, |s| match &s.instruments[n0 + 1].kind {
        InstrumentKind::Sample { data, .. } => data.len(),
        _ => 0,
    });
    assert!(l1.abs_diff(12000) < 400, "slice 2 length {l1}");
    // one undo step removes it all
    ctrl_z(&mut h);
    assert_eq!(song(&mut h, |s| s.instruments.len()), n0);
    assert!(song(&mut h, |s| s.patterns[p].cells[0][6].instr as usize) <= n0);
}

#[test]
fn voice_transform_section_edits_and_undoes() {
    let mut h = harness();
    instr_menu(&mut h, 1);
    click_exact(&mut h, "New instrument");
    click_exact(&mut h, "Voice (glitch)");
    click_exact(&mut h, "Init voice");
    frames(&mut h, 2);
    let d = crate::transform::Transform::default();
    assert_eq!(
        last_voice(&mut h).transform,
        d,
        "a new voice has the transform off"
    );
    // scroll the instrument panel down to the section
    super::voiceview::TF_SCROLL.with(|f| f.set(true));
    frames(&mut h, 3);
    exact(&h, "TRANSFORM");
    exact(&h, "Metallic resonance");
    exact(&h, "Digital distortion");
    exact(&h, "Sub bass");
    click_exact(&mut h, "Res > Dist");
    click_exact(&mut h, "Fold");
    click_exact(&mut h, "Fixed");
    click_exact(&mut h, "-2");
    let t = last_voice(&mut h).transform;
    assert_eq!(t.order, crate::transform::Order::ResDist);
    assert!(t.dig.fold && !t.res.keyed && t.sub.octave == 2, "{t:?}");
    // the fixed-pitch slider replaces Tune
    exact(&h, "   Fixed pitch");
    for _ in 0..4 {
        ctrl_z(&mut h);
    }
    assert_eq!(last_voice(&mut h).transform, d, "four undos restore it");
    // the presets are there and load a transform
    let names: Vec<&str> = crate::voice::presets().iter().map(|p| p.0).collect();
    for p in [
        "Metal Demon",
        "Digital Ghost",
        "Sub Beast",
        "Cyber Choir",
        "Ritual Growl",
    ] {
        assert!(names.contains(&p), "{p} preset");
        assert!(
            crate::voice::preset(p).unwrap().transform.blend > 0.5,
            "{p} uses it"
        );
    }
}

fn last_gen(h: &mut Harness<'static, App>) -> crate::gen::GenPatch {
    song(h, |s| match &s.instruments.last().unwrap().kind {
        InstrumentKind::Gen(g) => g.clone(),
        _ => panic!("not a generator"),
    })
}

#[test]
fn generator_dice_locks_history_and_bake() {
    let mut h = harness();
    instr_menu(&mut h, 1);
    click_exact(&mut h, "New instrument");
    click_exact(&mut h, "Generator (drums)");
    click_exact(&mut h, "Kicks");
    click_exact(&mut h, "Psy Zap Kick");
    frames(&mut h, 2);
    let n0 = song(&mut h, |s| s.instruments.len());
    let g0 = last_gen(&mut h);
    exact(&h, "GENERATOR · DRUM SYNTH");
    exact(&h, "Punch");
    exact(&h, "no rolls yet");
    let mac = |g: &crate::gen::GenPatch| match &g.engine {
        crate::gen::GenEngine::Drum(d) => (d.macros, d.locks),
        _ => panic!(),
    };
    click_exact(&mut h, "Dice");
    frames(&mut h, 2);
    let g1 = last_gen(&mut h);
    assert_ne!(mac(&g1).0, mac(&g0).0, "Dice changes the sound");
    assert_ne!(g1.seed, g0.seed, "and stores its seed");
    exact(&h, "roll 1/1");
    // lock Tune (the first lock), roll again: Tune stays
    click_exact(&mut h, "lock");
    frames(&mut h, 2);
    assert_eq!(mac(&last_gen(&mut h)).1, 1);
    let tune = mac(&last_gen(&mut h)).0[0];
    click_exact(&mut h, "Dice");
    frames(&mut h, 2);
    let g2 = last_gen(&mut h);
    assert_eq!(mac(&g2).0[0], tune, "the locked knob keeps its value");
    assert_ne!(mac(&g2).0, mac(&g1).0);
    click_exact(&mut h, "Mutate");
    frames(&mut h, 2);
    exact(&h, "roll 3/3");
    // back twice: the first roll's sound again
    click_exact(&mut h, "<");
    frames(&mut h, 2);
    click_exact(&mut h, "<");
    frames(&mut h, 2);
    exact(&h, "roll 1/3");
    assert_eq!(mac(&last_gen(&mut h)).0, mac(&g1).0);
    // Bake: a new sample instrument with the hit in it; one undo removes it
    click_exact(&mut h, "Bake to sample");
    frames(&mut h, 2);
    assert_eq!(song(&mut h, |s| s.instruments.len()), n0 + 1);
    let (name, len) = song(&mut h, |s| {
        let i = s.instruments.last().unwrap();
        (
            i.name.clone(),
            match &i.kind {
                InstrumentKind::Sample { data, .. } => data.len(),
                _ => 0,
            },
        )
    });
    assert_eq!(name, "Psy Zap Kick (baked)");
    assert!(len > 4800, "baked {len} samples");
    ctrl_z(&mut h);
    assert_eq!(song(&mut h, |s| s.instruments.len()), n0);
}

#[test]
fn synth_panel_offers_the_ladder_filter() {
    let mut h = harness();
    instr_menu(&mut h, 1);
    click_exact(&mut h, "New instrument");
    click_exact(&mut h, "Synth (init)");
    frames(&mut h, 2);
    click_exact(&mut h, "SVF 12 dB");
    frames(&mut h, 1);
    click_exact(&mut h, "Ladder (vintage)");
    frames(&mut h, 2);
    let p = song(&mut h, |s| match &s.instruments.last().unwrap().kind {
        InstrumentKind::Synth(p) => p.clone(),
        _ => panic!(),
    });
    assert_eq!(p.filter_model, crate::ladder::FilterModel::Vintage);
    click_exact(&mut h, "12 dB");
    frames(&mut h, 2);
    let p = song(&mut h, |s| match &s.instruments.last().unwrap().kind {
        InstrumentKind::Synth(p) => p.clone(),
        _ => panic!(),
    });
    assert!(p.ladder_12db);
}

#[test]
fn chip_fx_categories_randomize_mutate_locks_and_bake() {
    use crate::chipfx::{BASE_FREQ, WAVE_LOCK};
    let mut h = harness();
    instr_menu(&mut h, 1);
    click_exact(&mut h, "New instrument");
    click_exact(&mut h, "Generator (chip FX)");
    click_exact(&mut h, "Coin");
    frames(&mut h, 2);
    let n0 = song(&mut h, |s| s.instruments.len());
    exact(&h, "GENERATOR · CHIP FX");
    let sfx = |g: &crate::gen::GenPatch| match &g.engine {
        crate::gen::GenEngine::ChipFx(s) => *s,
        _ => panic!("not chip fx"),
    };
    let s0 = sfx(&last_gen(&mut h));
    for c in [
        "Pickup",
        "Laser",
        "Explosion",
        "Power-up",
        "Hit",
        "Jump",
        "Blip",
    ] {
        exact(&h, c);
    }
    click_exact(&mut h, "Explosion");
    frames(&mut h, 2);
    let s1 = sfx(&last_gen(&mut h));
    assert_eq!(s1.wave, crate::chipfx::Wave::Noise, "explosions are noise");
    assert_ne!(s1.p, s0.p);
    // lock the Freq knob (first knob lock) and the wave, then Randomize
    let locks: Vec<Rect> = h
        .query_all_by(|n| n.label().as_deref() == Some("lock"))
        .map(|n| n.rect())
        .collect();
    assert!(locks.len() >= 9, "wave lock + 8 knob locks");
    click_at(&mut h, locks[0].center());
    frames(&mut h, 1);
    click_at(&mut h, locks[1].center());
    frames(&mut h, 2);
    let s2 = sfx(&last_gen(&mut h));
    assert_eq!(s2.locks, WAVE_LOCK | (1 << BASE_FREQ), "{:x}", s2.locks);
    click_exact(&mut h, "Randomize");
    frames(&mut h, 2);
    let s3 = sfx(&last_gen(&mut h));
    assert_eq!(s3.wave, s1.wave);
    assert_eq!(s3.p[BASE_FREQ], s1.p[BASE_FREQ]);
    assert_ne!(s3.p, s1.p);
    click_exact(&mut h, "Mutate");
    frames(&mut h, 2);
    let s4 = sfx(&last_gen(&mut h));
    assert_ne!(s4.p, s3.p);
    assert_eq!(s4.p[BASE_FREQ], s1.p[BASE_FREQ]);
    // history: back to the explosion
    exact(&h, "roll 3/3");
    for _ in 0..2 {
        click_exact(&mut h, "<");
        frames(&mut h, 2);
    }
    assert_eq!(sfx(&last_gen(&mut h)).p, s1.p);
    // scroll the side panel down (pointer on the header, not on a knob)
    let at = exact(&h, "GENERATOR · CHIP FX").center();
    h.input_mut().events.push(egui::Event::PointerMoved(at));
    frame(&mut h);
    for _ in 0..3 {
        h.input_mut().events.push(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -200.0),
            phase: egui::TouchPhase::Move,
            source: egui::MouseWheelSource::Wheel,
            modifiers: Default::default(),
        });
        frame(&mut h);
    }
    frames(&mut h, 10);
    click_exact(&mut h, "Bake to sample");
    frames(&mut h, 2);
    assert_eq!(song(&mut h, |s| s.instruments.len()), n0 + 1);
    ctrl_z(&mut h);
    assert_eq!(song(&mut h, |s| s.instruments.len()), n0);
}
