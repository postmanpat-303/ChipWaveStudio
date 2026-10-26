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
    // the existing UI tests cover the Tracker too (Settings › Show Tracker)
    h.state_mut().extra.show_tracker = true;
    frame(&mut h);
    frame(&mut h);
    h
}

/// the pre-0.2.3.19 sidebar instrument editor (View > Classic sidebar instrument editor)
fn classic(h: &mut Harness<'static, App>) {
    h.state_mut().extra.classic_editor = true;
    // ...with the arranger in the lower panel, as before 0.2.3.19
    h.state_mut().extra.lower_inst = false;
    frame(h);
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
/// the classic toolbar (Settings → Look → Top display off): the BPM and
/// Speed boxes, the Master under the Clip Arranger
pub(super) fn classic_toolbar(h: &mut Harness<'static, App>) {
    h.state_mut().extra.lcd_toolbar = false;
    frame(h);
    frame(h);
}

pub(super) fn click_at(h: &mut Harness<'static, App>, p: Pos2) {
    click_with(h, p, egui::PointerButton::Primary);
}

pub(super) fn click_with(h: &mut Harness<'static, App>, p: Pos2, button: egui::PointerButton) {
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
    // classic themes: the pan slider under the fader; Slate: the Pan knob
    // above it (same strip)
    let pan = sliders
        .iter()
        .copied()
        .find(|r| {
            r.height() < 40.0
                && r.top() > fader.bottom()
                && (r.center().x - fader.center().x).abs() < 40.0
        })
        .or_else(|| {
            rects(h, Role::Slider, Some("Pan"))
                .into_iter()
                .find(|r| (r.center().x - fader.center().x).abs() < 40.0)
        })
        .expect("the pan control of the fader's strip");
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
    // far to the right: the end of a slider, or a knob turned past half way
    drag(&mut h, sa.center(), sa.center() + egui::vec2(140.0, 0.0));
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
    classic_toolbar(&mut h);
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
const SPEED: &str = "ticks per row (effect 09xx sets it in the Tracker)";

#[test]
fn tempo_typed_then_ctrl_z_while_field_focused() {
    let mut h = harness();
    classic_toolbar(&mut h);
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
    classic_toolbar(&mut h);
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
    classic_toolbar(&mut h);
    let sp0 = song(&mut h, |s| s.speed);
    let t = rects(
        &h,
        Role::SpinButton,
        Some("ticks per row (effect 09xx sets it in the Tracker)"),
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

/// named edits (app/action.rs) show their name in the Edit menu; an edit
/// not routed through them stays a plain "Undo"
#[test]
fn edit_menu_names_the_step() {
    let mut h = harness();
    let n0 = song(&mut h, |s| s.channel_names[0].clone());
    h.state_mut().rename_track(0, "Bass line");
    frames(&mut h, 2);
    let e = rects(&h, Role::Button, Some("Edit"))[0];
    click_at(&mut h, e.center());
    let u = rects(&h, Role::Button, Some("Undo: Rename track Ctrl+Z"));
    assert_eq!(u.len(), 1, "the menu names the step");
    click_at(&mut h, u[0].center());
    assert_eq!(song(&mut h, |s| s.channel_names[0].clone()), n0);
    assert_eq!(h.state().status, "Undo: Rename track");
    let e = rects(&h, Role::Button, Some("Edit"))[0];
    click_at(&mut h, e.center());
    assert_eq!(
        rects(&h, Role::Button, Some("Redo: Rename track Ctrl+Y")).len(),
        1
    );
    key(&mut h, Key::Escape, egui::Modifiers::NONE);
    // add and delete a track: each its own named step
    let ch = song(&mut h, |s| s.channels);
    assert!(h.state_mut().track_add(ch));
    frames(&mut h, 2);
    assert_eq!(h.state().undo_text(false), "Undo: Add track");
    h.state_mut().track_delete(ch);
    frames(&mut h, 2);
    assert_eq!(h.state().undo_text(false), "Undo: Delete track");
    ctrl_z(&mut h);
    assert_eq!(h.state().status, "Undo: Delete track");
    assert_eq!(song(&mut h, |s| s.channels), ch + 1);
    ctrl_z(&mut h);
    assert_eq!(song(&mut h, |s| s.channels), ch);
    // a Tracker cell
    h.state_mut().edit_cell(|c| c.note = 49);
    frames(&mut h, 2);
    assert_eq!(h.state().undo_text(false), "Undo: Edit cell");
    // an edit with no name yet: plain "Undo"
    h.state_mut().with(|s| s.song.bpm += 1.0);
    frames(&mut h, 2);
    assert_eq!(h.state().undo_text(false), "Undo");
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
pub(super) fn exact(h: &Harness<'static, App>, text: &str) -> Rect {
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
pub(super) fn double_click(h: &mut Harness<'static, App>, p: Pos2) {
    double_click_with(h, p, egui::Modifiers::NONE);
}

/// `double_click` with modifier keys held (e.g. Alt)
pub(super) fn double_click_with(h: &mut Harness<'static, App>, p: Pos2, m: egui::Modifiers) {
    h.input_mut().events.push(egui::Event::ModifiersChanged(m));
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
                modifiers: m,
            });
            frame(h);
        }
    }
    // back to the harness clock (RawInput keeps an explicit time)
    h.input_mut().time = None;
    h.input_mut()
        .events
        .push(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
    frame(h);
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
    click_exact(&mut h, "About ChipWave Studio");
    assert!(h.state().show_about);
    exact(&h, "Created by Del (postmanpat)");
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
    assert_eq!(
        click_link("YouTube: demos & promos"),
        ["https://www.youtube.com/@whatgoesaround1551"]
    );
}

/// the About window opens centred in the main window, every time
fn about_centred(h: &Harness<'static, App>) {
    let st = egui::AreaState::load(&h.ctx, egui::Id::unique("about_chipwave_studio"))
        .expect("About window shown");
    let c = st.rect().center();
    let want = h.ctx.content_rect().center();
    assert!(
        (c - want).length() < 2.0,
        "About at {c:?} (size {:?}), centre {want:?}",
        st.size
    );
}

#[test]
fn about_window_is_centred_and_has_credit_tabs() {
    let mut h = harness();
    h.state_mut().show_about = true;
    frames(&mut h, 4);
    about_centred(&h);
    exact(&h, "ChipWave Studio");
    exact(&h, "A DAW and tracker for chip and psytrance sounds");
    exact(&h, "Created by Del (postmanpat)");
    exact(&h, "Full notices: THIRD_PARTY_LICENSES.txt");
    // Credits tab: grouped, headed sections with every credit
    click_exact(&mut h, "Credits");
    frames(&mut h, 2);
    for g in crate::credits::Group::ALL {
        exact(&h, g.title());
    }
    exact(&h, "OctaMED");
    exact(&h, "Peterson & Barney (1952)");
    // the filter narrows the list
    h.state_mut().about.filter = "trombone".into();
    frames(&mut h, 2);
    exact(&h, "Pink Trombone");
    assert!(
        h.query_all_by(|n| n.label().as_deref() == Some("OctaMED"))
            .next()
            .is_none(),
        "filtered out"
    );
    // Licences tab
    click_exact(&mut h, "Licences");
    frames(&mut h, 2);
    exact(&h, "Licences of credited work");
    // dragged away by its title, closed and opened again: centred again
    let title = h
        .query_all_by(|n| n.label().as_deref() == Some("About ChipWave Studio"))
        .map(|n| n.rect())
        .min_by(|a, b| a.height().total_cmp(&b.height()))
        .expect("title");
    // (the window's node covers it all; its title bar is the top strip)
    let grab = egui::pos2(title.center().x - 120.0, title.top() + 14.0);
    drag(&mut h, grab, grab + egui::vec2(300.0, 150.0));
    frames(&mut h, 2);
    let moved = egui::AreaState::load(&h.ctx, egui::Id::unique("about_chipwave_studio"))
        .unwrap()
        .rect()
        .center();
    assert!(
        (moved - h.ctx.content_rect().center()).length() > 50.0,
        "dragged"
    );
    h.state_mut().show_about = false;
    frames(&mut h, 2);
    h.state_mut().show_about = true;
    frames(&mut h, 4);
    about_centred(&h);
}

#[test]
fn help_menu_and_about_open_the_user_manual() {
    let mut h = harness();
    click_exact(&mut h, "Help");
    click_exact(&mut h, "User Manual");
    assert_eq!(h.state().manual.opened, 1, "Help > User Manual opens it");
    assert!(h.state().manual.err.is_none());
    // the About page has a button for it too, beside the support links
    h.state_mut().show_about = true;
    frames(&mut h, 3);
    click_exact(&mut h, "User manual (PDF)");
    assert_eq!(h.state().manual.opened, 2, "About > User manual opens it");
    // a failure shows the fallback window with the GitHub link
    h.state_mut().show_about = false;
    h.state_mut().manual.err = Some("couldn't start the PDF viewer (test)".into());
    frames(&mut h, 3);
    exact(
        &h,
        "Sorry, the user manual couldn't be opened on this computer.",
    );
    let r = exact(&h, "Open the manual on GitHub");
    h.input_mut()
        .events
        .push(egui::Event::PointerMoved(r.center()));
    frame(&mut h);
    let mut urls = vec![];
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
                urls.push(u.url.clone());
            }
        }
    }
    assert_eq!(urls, [super::manual::MANUAL_URL]);
    frames(&mut h, 2);
    assert!(h.state().manual.err.is_none(), "closed after the link");
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
    // the disco demo (0.2.3.18) loads from the menu too
    click_exact(&mut h, "File");
    click_exact(&mut h, "Demo songs");
    click_exact(&mut h, "Velvet Mirrorball");
    let disco = fileio::demo_song_n(2);
    assert!(disco.title.starts_with("Velvet Mirrorball"));
    assert_eq!(song(&mut h, |s| s.title.clone()), disco.title);
    assert_eq!(song(&mut h, |s| s.bpm), 104.0);
    assert!(!h.state().dirty);
    // the psy demo "Chrome Throat" too
    click_exact(&mut h, "File");
    click_exact(&mut h, "Demo songs");
    click_exact(&mut h, "Chrome Throat");
    assert!(song(&mut h, |s| s.title.clone()).starts_with("Chrome Throat"));
    assert_eq!(song(&mut h, |s| s.bpm), 145.0);
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
    classic(&mut h);
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
        [format!(
            "https://github.com/postmanpat-303/ChipWaveStudio/releases/download/v0.2.3.9{}",
            crate::update::SAMPLE_INSTALLER
        )]
    );
    assert_eq!(h.state().upd.win, None, "dialog closed");
    manual_check(&mut h, Outcome::Newer(sample_release()));
    assert_eq!(
        click_urls(&mut h, "Open release page"),
        ["https://github.com/postmanpat-303/ChipWaveStudio/releases/tag/v0.2.3.9"]
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
    classic(&mut h);
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
    classic(&mut h);
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
    // the box itself (the label can run under the panel's scroll bar)
    let lr = exact(&h, "Legato");
    click_at(&mut h, egui::pos2(lr.left() + 8.0, lr.center().y));
    frames(&mut h, 2);
    assert!(patch(&mut h).legato, "legato on");
    // accent threshold: drag the hex value up from 00
    let r = exact(&h, "at vol ≥ 00");
    let s = egui::pos2(r.left() + 10.0, r.center().y);
    drag(&mut h, s, s + egui::vec2(60.0, 0.0));
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

/// double-click instrument 03 and replace the rename field's text
fn start_rename_03(h: &mut Harness<'static, App>, text: &str) {
    let r = labelled(h, "03  ");
    double_click(h, r.center());
    frames(h, 2);
    assert!(h.state().renaming.is_some(), "rename field open");
    assert!(h.ctx.text_edit_focused(), "rename field focused");
    h.state_mut().renaming.as_mut().unwrap().1 = text.into();
    frames(h, 2);
}

#[test]
fn instrument_rename_commits_on_focus_loss_and_esc_cancels() {
    let mut h = harness();
    let before = song(&mut h, |s| s.clone());
    let old = before.instruments[2].name.clone();
    let pats = |h: &mut Harness<'static, App>| song(h, |s| format!("{:?}", s.patterns));
    let pats0 = pats(&mut h);
    // typing goes into the field only: Q and W are note keys in the pattern
    start_rename_03(&mut h, "");
    h.input_mut()
        .events
        .push(egui::Event::Text("Qw Lead".into()));
    frame(&mut h);
    key(&mut h, Key::Q, egui::Modifiers::NONE);
    key(&mut h, Key::W, egui::Modifiers::NONE);
    assert!(h.state().renaming.is_some(), "still editing");
    assert_eq!(pats(&mut h), pats0, "no notes entered while renaming");
    // clicking somewhere else commits
    let away = h.ctx.content_rect().center();
    click_at(&mut h, away);
    frames(&mut h, 2);
    assert!(h.state().renaming.is_none(), "edit mode closed");
    assert_eq!(instr_names(&mut h)[2], "Qw Lead");
    // one undo step
    ctrl_z(&mut h);
    assert_eq!(instr_names(&mut h)[2], old, "undo rename");
    // Esc cancels and keeps the old name
    start_rename_03(&mut h, "Nope");
    key(&mut h, Key::Escape, egui::Modifiers::NONE);
    assert!(h.state().renaming.is_none());
    assert_eq!(instr_names(&mut h)[2], old, "Esc keeps the old name");
    // empty / whitespace-only names revert
    start_rename_03(&mut h, "   ");
    key(&mut h, Key::Enter, egui::Modifiers::NONE);
    assert!(h.state().renaming.is_none());
    assert_eq!(instr_names(&mut h)[2], old, "blank name reverts");
    // Tab commits
    start_rename_03(&mut h, "Tabbed");
    key(&mut h, Key::Tab, egui::Modifiers::NONE);
    assert!(h.state().renaming.is_none(), "Tab closes the field");
    assert_eq!(instr_names(&mut h)[2], "Tabbed");
    // clicking another instrument commits and selects it
    start_rename_03(&mut h, "Clicked away");
    let r5 = labelled(&h, "05  ");
    click_at(&mut h, r5.center());
    frames(&mut h, 2);
    assert!(h.state().renaming.is_none());
    assert_eq!(instr_names(&mut h)[2], "Clicked away");
    assert_eq!(h.state().instr, 5);
    // switching views commits
    start_rename_03(&mut h, "View switch");
    h.state_mut().view = View::Mixer;
    frames(&mut h, 2);
    assert!(h.state().renaming.is_none());
    assert_eq!(instr_names(&mut h)[2], "View switch");
    assert_eq!(pats(&mut h), pats0, "patterns untouched throughout");
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
    classic(&mut h);
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
pub(super) fn loop_instrument() -> Instrument {
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
    classic(&mut h);
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
    classic(&mut h);
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
    classic(&mut h);
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
    classic(&mut h);
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

#[test]
fn stage2_generators_menu_dice_locks_history_and_bake() {
    use crate::gen::GenEngine;
    for (menu, preset, header) in [
        (
            "Generator (analog)",
            "Psy Rolling Bass",
            "GENERATOR · ANALOG",
        ),
        ("Generator (FM-4)", "E-Piano", "GENERATOR · FM-4"),
        ("Generator (noise)", "Impact", "GENERATOR · NOISE"),
        (
            "Generator (wavetable)",
            "Wave Glass Keys",
            "GENERATOR · WAVETABLE",
        ),
    ] {
        let mut h = harness();
        classic(&mut h);
        instr_menu(&mut h, 1);
        click_exact(&mut h, "New instrument");
        click_exact(&mut h, menu);
        click_exact(&mut h, preset);
        frames(&mut h, 2);
        exact(&h, header);
        let n0 = song(&mut h, |s| s.instruments.len());
        let knobs = |g: &crate::gen::GenPatch| -> (Vec<f32>, u64) {
            match &g.engine {
                GenEngine::Analog(a) => (a.p.to_vec(), a.locks),
                GenEngine::Fm(f) => (f.p.to_vec(), f.locks),
                GenEngine::Noise(n) => (n.p.to_vec(), n.locks),
                GenEngine::Wave(w) => (w.p.to_vec(), w.locks),
                _ => panic!("not stage 2"),
            }
        };
        let g0 = knobs(&last_gen(&mut h));
        // the first knob lock (macro 1) is the first "lock" after the choice locks;
        // lock every knob lock on screen except none: pick the lock under macro 1
        let locks: Vec<Rect> = h
            .query_all_by(|n| n.label().as_deref() == Some("lock"))
            .map(|n| n.rect())
            .collect();
        // the macro knob locks are the last 8 visible ones
        assert!(locks.len() >= 8, "{menu}: {} locks", locks.len());
        let first_macro_lock = locks[locks.len() - 8];
        click_at(&mut h, first_macro_lock.center());
        frames(&mut h, 2);
        let g1 = knobs(&last_gen(&mut h));
        assert_eq!(g1.1 & 1, 1, "{menu}: macro 1 locked ({:x})", g1.1);
        click_exact(&mut h, "Dice");
        frames(&mut h, 2);
        let g2 = knobs(&last_gen(&mut h));
        assert_eq!(g2.0[0], g0.0[0], "{menu}: locked knob kept");
        assert_ne!(g2.0, g0.0, "{menu}: Dice changed the rest");
        click_exact(&mut h, "Mutate");
        frames(&mut h, 2);
        let g3 = knobs(&last_gen(&mut h));
        assert_ne!(g3.0, g2.0);
        assert_eq!(g3.0[0], g0.0[0]);
        click_exact(&mut h, "<");
        frames(&mut h, 2);
        assert_eq!(knobs(&last_gen(&mut h)).0, g2.0, "{menu}: history back");
        // bake (scroll the panel down first)
        let at = exact(&h, header).center();
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
        click_exact(&mut h, "Bake to sample");
        frames(&mut h, 2);
        assert_eq!(
            song(&mut h, |s| s.instruments.len()),
            n0 + 1,
            "{menu}: baked"
        );
        let len = song(&mut h, |s| match &s.instruments.last().unwrap().kind {
            InstrumentKind::Sample { data, .. } => data.len(),
            _ => 0,
        });
        assert!(len > 2400, "{menu}: {len}");
        ctrl_z(&mut h);
        assert_eq!(song(&mut h, |s| s.instruments.len()), n0);
    }
}

/// demo song with four 64-row entries in the order, arranger view open
fn ruler_harness() -> Harness<'static, App> {
    let mut song = fileio::demo_song();
    song.patterns[0].rows = 64;
    let row0 = song.patterns[0].cells[0].clone();
    song.patterns[0].cells.resize(64, row0);
    song.order = vec![0, 0, 0, 0];
    let mut h = harness_with(song);
    h.state_mut().view = View::Arranger;
    frames(&mut h, 3);
    h
}

/// screen point on the arranger ruler over timeline row `r`
fn ruler_at(h: &mut Harness<'static, App>, r: f32) -> Pos2 {
    let zoom = song(h, |s| s.arranger_zoom);
    let st = h.state();
    let p = Pos2::new(st.arr_x0 + (r + 0.5) * zoom, st.arr_ruler.center().y);
    assert!(st.arr_ruler.contains(p), "row {r} is on the visible ruler");
    p
}

#[test]
fn arranger_ruler_click_moves_the_play_marker_and_song_play_starts_there() {
    let mut h = ruler_harness();
    let sel0 = (h.state().arr_sel, h.state().arr_multi.clone());
    // row 150 = entry 2, row 22 -> snaps back to the bar line at row 16
    let p = ruler_at(&mut h, 150.0);
    click_at(&mut h, p);
    assert_eq!(h.state().cue, (2, 16));
    assert_eq!(
        (h.state().pattern, h.state().row),
        (0, 16),
        "editor follows"
    );
    assert_eq!(
        (h.state().arr_sel, h.state().arr_multi.clone()),
        sel0,
        "clip selection untouched"
    );
    assert!(
        !h.with_engine(|s| s.engine.playing),
        "a click does not start playback"
    );
    // Play song (Shift+Space) starts at the marker
    key(&mut h, Key::Space, egui::Modifiers::SHIFT);
    let (playing, looped, op, row) = h.with_engine(|s| {
        (
            s.engine.playing,
            s.engine.loop_pattern,
            s.engine.order_pos,
            s.engine.row,
        )
    });
    assert!(playing && !looped);
    assert_eq!((op, row), (2, 16));
    // Alt-click: exact row, no bar snap
    h.with_engine(|s| s.engine.stop());
    let p = ruler_at(&mut h, 69.0);
    h.input_mut()
        .events
        .push(egui::Event::ModifiersChanged(egui::Modifiers::ALT));
    click_at(&mut h, p);
    h.input_mut()
        .events
        .push(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
    frames(&mut h, 2);
    assert_eq!(h.state().cue, (1, 5));
    // a new song puts the marker back at bar 1
    h.state_mut().new_song();
    assert_eq!(h.state().cue, (0, 0));
}

#[test]
fn arranger_ruler_click_while_playing_jumps_and_keeps_playing() {
    let mut h = ruler_harness();
    h.state_mut().follow = false;
    h.with_engine(|s| {
        let Shared { song, engine, .. } = s;
        engine.start(song, 0, 0, false, 0);
    });
    let p = ruler_at(&mut h, 200.0);
    click_at(&mut h, p);
    let (playing, op, row) =
        h.with_engine(|s| (s.engine.playing, s.engine.order_pos, s.engine.row));
    assert!(playing, "still playing after the jump");
    assert_eq!((op, row), (3, 0), "row 200 -> entry 3, bar at row 192");
    assert_eq!(h.state().cue, (3, 0));
    // a block loop whose block is the target keeps looping
    h.with_engine(|s| {
        let Shared { song, engine, .. } = s;
        engine.start(song, 0, 0, true, 0);
    });
    let p = ruler_at(&mut h, 40.0);
    click_at(&mut h, p);
    let (looped, row) = h.with_engine(|s| (s.engine.loop_pattern, s.engine.row));
    assert!(looped);
    assert_eq!(row, 32);
}

#[test]
fn arranger_ruler_drag_scrubs_without_moving_clips() {
    let mut h = ruler_harness();
    let order0 = song(&mut h, |s| s.order.clone());
    let (a, b) = (ruler_at(&mut h, 3.0), ruler_at(&mut h, 180.0));
    drag(&mut h, a, b);
    assert_eq!(h.state().cue, (2, 48), "marker follows the drag to row 176");
    assert_eq!(song(&mut h, |s| s.order.clone()), order0, "no clip moved");
    assert!(h.state().arr_drag.is_none());
}

#[test]
fn engine_seek_keeps_tempo_and_switches_loop_to_song_for_other_blocks() {
    let mut song = fileio::demo_song();
    song.order = vec![0, 1, 0];
    let mut e = Engine::new(48000.0);
    e.seek(&song, 1, 4);
    assert!(!e.playing, "seek on a stopped engine does nothing");
    e.start(&song, 0, 0, true, 0);
    e.seek(&song, 1, 8);
    assert!(
        e.playing && !e.loop_pattern,
        "other block: plays the song order"
    );
    assert_eq!((e.order_pos, e.pattern, e.row), (1, 1, 8));
    e.seek(&song, 9, 9999);
    let rows = song.patterns[0].rows;
    assert_eq!((e.order_pos, e.row), (2, rows - 1), "clamped");
}

// ---- 0.2.3.16: plugin instruments ----

fn has_text(h: &Harness<'static, App>, prefix: &str) -> bool {
    h.query_all_by(|n| {
        n.label().is_some_and(|l| l.starts_with(prefix))
            || n.value().is_some_and(|v| v.starts_with(prefix))
    })
    .next()
    .is_some()
}

#[test]
fn plugin_instrument_loads_shows_its_panel_and_saves_its_state() {
    let mut h = harness_with(crate::plugin::tests::plugin_song());
    classic(&mut h);
    frames(&mut h, 3);
    let loaded = h.state().with(|s| s.engine.plugins.slots.len());
    assert_eq!(loaded, 1, "the song's plugin note loads one instance");
    // macros default to the first parameters (Gain, Tone)
    let m = h.state().with(|s| match &s.song.instruments[0].kind {
        InstrumentKind::Plugin(p) => p.macros.clone(),
        _ => vec![],
    });
    assert_eq!(&m[..2], &[1, 2]);
    assert!(has_text(&h, "Configure…"));
    assert!(has_text(&h, "Parameters (2)"));
    assert!(has_text(&h, "[M1] Gain"), "parameter sliders are listed");
    // a parameter edit is copied into the song on save, without an undo step
    let undo_len = h.state().hist.len();
    h.state().with(|s| s.engine.plugins.set_param(0, 1, 0.9));
    h.state_mut().plug.state_dirty = true;
    h.state_mut().capture_plugins();
    let st = h.state().with(|s| match &s.song.instruments[0].kind {
        InstrumentKind::Plugin(p) => p.state.clone(),
        _ => String::new(),
    });
    assert!(!st.is_empty());
    let mut p = crate::plugin::tests::patch();
    p.state = st;
    let back = crate::plugin::instantiate(&p, 48000.0).unwrap();
    assert_eq!(back.value(1), Some(0.9));
    frames(&mut h, 2);
    assert_eq!(
        h.state().hist.len(),
        undo_len,
        "capturing state is not an undo step"
    );
}

#[test]
fn a_plugin_editor_edit_marks_the_song_dirty_and_touches_its_macro() {
    let mut h = harness_with(crate::plugin::tests::plugin_song());
    classic(&mut h);
    frames(&mut h, 3);
    h.state_mut().dirty = false;
    h.state_mut().plug.state_dirty = false;
    let handle = h
        .state()
        .with(|s| s.engine.plugins.slots[0].inst.test_handle());
    // the editor moves Tone (parameter 2, mapped to macro 2)
    crate::plugin::hosttests::gui_edit(handle, 2, 0.75);
    frames(&mut h, 2);
    assert!(h.state().dirty, "the song is marked dirty");
    assert!(h.state().plug.state_dirty);
    assert_eq!(
        h.state().tlv.last_touch,
        Some(crate::automation::Target::Macro(0, 1)),
        "the mapped macro counts as touched"
    );
    // saved into the song with the next capture
    h.state_mut().capture_plugins();
    let st = h.state().with(|s| match &s.song.instruments[0].kind {
        InstrumentKind::Plugin(p) => p.state.clone(),
        _ => String::new(),
    });
    let mut p = crate::plugin::tests::patch();
    p.state = st;
    let back = crate::plugin::instantiate(&p, 48000.0).unwrap();
    assert_eq!(back.value(2), Some(0.75));
}

#[test]
fn saving_keeps_plugin_edits_the_plugin_never_reported() {
    let mut h = harness_with(crate::plugin::tests::plugin_song());
    classic(&mut h);
    frames(&mut h, 3);
    // many plugins never call mark_dirty: the host must still ask on save
    h.state().with(|s| s.engine.plugins.set_param(0, 1, 0.33));
    h.state_mut().plug.state_dirty = false;
    let undo_len = h.state().hist.len();
    h.state_mut().capture_plugins();
    let st = h.state().with(|s| match &s.song.instruments[0].kind {
        InstrumentKind::Plugin(p) => p.state.clone(),
        _ => String::new(),
    });
    let mut p = crate::plugin::tests::patch();
    p.state = st;
    let back = crate::plugin::instantiate(&p, 48000.0).unwrap();
    assert_eq!(back.value(1), Some(0.33));
    assert_eq!(h.state().hist.len(), undo_len, "still not an undo step");
}

#[test]
fn new_menu_opens_the_plugin_browser() {
    let mut h = harness();
    h.state_mut().plug.scan_msg = "x".into(); // no automatic scan in tests
    h.state_mut().plug.browser = true;
    frames(&mut h, 2);
    assert!(has_text(&h, "Add plugin instrument"));
    assert!(has_text(&h, "Rescan"));
}

#[test]
fn panic_button_sits_at_the_far_right_of_the_toolbar_and_stops_everything() {
    let mut h = harness();
    let r = exact(&h, "Panic");
    let win = h.ctx.content_rect();
    assert!(
        r.right() > win.right() - 40.0,
        "far right: {r:?} in {win:?}"
    );
    assert!(r.top() < 120.0, "in the top toolbar: {r:?}");
    // every other toolbar widget on that row is to its left
    let others = h
        .query_all_by(|n| n.label().is_some_and(|l| l != "Panic" && !l.is_empty()))
        .filter(|n| {
            let q = n.rect();
            q.center().y > r.top()
                && q.center().y < r.bottom()
                && q.width() < 400.0
                && q.left() > r.right() - 1.0
        })
        .count();
    assert_eq!(others, 0, "nothing to the right of Panic");
    // works while playing...
    h.state_mut().play(0);
    assert!(h.state().with(|s| s.engine.playing));
    click_at(&mut h, r.center());
    assert!(h
        .state()
        .with(|s| !s.engine.playing && s.engine.panicking()));
    assert_eq!(h.state().status, "Panic: all sound stopped");
    // ...and while stopped, from the keyboard (Ctrl+.)
    h.state().with(|s| {
        let mut b = vec![0.0f32; 512];
        let song = s.song.clone();
        s.engine.render(&song, &mut b);
        assert!(!s.engine.panicking());
    });
    key(&mut h, Key::Period, egui::Modifiers::COMMAND);
    assert!(h.state().with(|s| s.engine.panicking()));
}

#[test]
fn instrument_list_is_compact_one_toolbar_row_and_the_bar_never_covers_names() {
    let mut s = fileio::demo_song();
    let base = s.instruments[0].clone();
    while s.instruments.len() < 40 {
        let mut i = base.clone();
        i.name = format!(
            "Very Long Instrument Name Number {}",
            s.instruments.len() + 1
        );
        s.instruments.push(i);
    }
    let mut h = harness_with(s);
    frames(&mut h, 2);
    let rows: Vec<Rect> = h
        .query_all_by(|n| {
            n.label()
                .is_some_and(|l| l.len() > 4 && l.as_bytes()[2] == b' ' && l.as_bytes()[3] == b' ')
        })
        .map(|n| n.rect())
        .collect();
    assert!(rows.len() >= 15, "many rows fit: {}", rows.len());
    let h0 = rows[0].height();
    assert!(h0 <= 22.0, "compact rows: {h0}");
    // the list scrolls (40 rows) but the rows stop before the scroll gutter
    let side_right = rows.iter().map(|r| r.right()).fold(0.0f32, f32::max);
    let first = labelled(&h, "Duplicate");
    assert!(
        side_right < 330.0 + 8.0 - 6.0,
        "row ends {side_right} leave a gutter"
    );
    // the seven list actions sit on one row of small buttons
    let ys: Vec<f32> = [
        "New…",
        "Import WAV…",
        "Duplicate",
        "Delete…",
        "Up",
        "Down",
        "Library…",
    ]
    .iter()
    .map(|n| exact(&h, n).center().y)
    .collect();
    assert!(
        ys.iter().all(|y| (y - ys[0]).abs() < 1.0),
        "one toolbar row: {ys:?}"
    );
    assert!(first.height() <= 22.0);
    assert!(
        h.query_all_by(|n| n.label().is_some_and(|l| l.starts_with("or drop")))
            .next()
            .is_none(),
        "the drop hint moved into the tooltip"
    );
    // right-click a row: the list actions are in its menu too
    instr_menu(&mut h, 2);
    let n = h
        .query_all_by(|n| n.label().as_deref() == Some("Import WAV…"))
        .count();
    assert_eq!(n, 2, "toolbar icon + menu item");
    assert!(labelled(&h, "Instrument library…").width() > 0.0);
}

/// 0.2.3.16: alternate column shading is on by default, View > Alternate
/// column shading turns it off, and the pattern view draws with 32
/// channels scrolled sideways (stagger follows the channel index)
#[test]
fn alternate_column_shading_toggles_from_the_view_menu_and_scrolls() {
    let mut h = harness();
    assert!(h.state().extra.alt_columns, "on by default");
    assert!(crate::settings::Extra::default().alt_columns);
    let v = rects(&h, Role::Button, Some("View"))[0];
    click_at(&mut h, v.center());
    let c = rects(&h, Role::CheckBox, Some("Alternate column shading"))[0];
    click_at(&mut h, c.center());
    assert!(!h.state().extra.alt_columns, "View menu turns it off");
    // old settings files without the key load with it on
    let e: crate::settings::Extra = toml::from_str("").unwrap();
    assert!(e.alt_columns);
    let st = h.state_mut();
    st.extra.alt_columns = true;
    st.view = View::Pattern;
    st.set_channels(32);
    st.ch = 31;
    frames(&mut h, 3);
    assert!(h.state().ch_view > 0, "scrolled to the last channel");
    assert_eq!(h.state().ch, 31);
}

// ---- 0.2.3.17: decimal row numbers, block length ----

/// rows are counted in decimal by default; View > Row numbers > Hex goes
/// back to 00..3F; the status bar follows the setting
#[test]
fn row_numbers_are_decimal_by_default_and_hex_from_the_view_menu() {
    let mut h = harness();
    assert!(!h.state().extra.row_hex, "decimal by default");
    let st = h.state_mut();
    st.view = View::Pattern;
    st.pattern = 0;
    st.row = 13;
    frames(&mut h, 2);
    let rows = song(&mut h, |s| s.patterns[0].rows);
    assert!(
        has_text(&h, &format!("Row 13/{rows}")),
        "status bar shows decimal"
    );
    let v = rects(&h, Role::Button, Some("View"))[0];
    click_at(&mut h, v.center());
    let hx = h
        .query_all_by(|n| n.label().is_some_and(|l| l == "Hex"))
        .next()
        .expect("View > Row numbers > Hex")
        .rect();
    click_at(&mut h, hx.center());
    assert!(h.state().extra.row_hex);
    frames(&mut h, 2);
    assert!(
        has_text(&h, &format!("Row 0D/{rows}")),
        "status bar shows hex"
    );
    assert_eq!(h.state().row_text(13, 64), "0D");
    h.state_mut().extra.row_hex = false;
    assert_eq!(h.state().row_text(13, 64), "13");
    assert_eq!(h.state().row_text(7, 128), "007");
}

fn len_of(h: &mut Harness<'static, App>, p: usize) -> usize {
    song(h, |s| s.patterns[p].rows)
}

/// Block bar: x2 / /2 / +16 / -16, the cut warning, undo, Ctrl+Shift+PgDn/PgUp
#[test]
fn block_length_buttons_warn_before_cutting_notes_and_undo() {
    let mut h = harness();
    h.state_mut().view = View::Pattern;
    h.state_mut().pattern = 0;
    h.state_mut().follow = false;
    frames(&mut h, 2);
    let n0 = len_of(&mut h, 0);
    let secs0 = song(&mut h, |s| s.length_seconds());
    click_label(&mut h, "x2");
    assert_eq!(len_of(&mut h, 0), n0 * 2, "x2 doubles");
    assert!(
        song(&mut h, |s| s.length_seconds()) > secs0,
        "song gets longer"
    );
    assert!(song(&mut h, |s| s.patterns[0].cells[n0..]
        .iter()
        .flatten()
        .all(|c| c.is_empty())));
    key(&mut h, Key::Z, egui::Modifiers::COMMAND);
    assert_eq!(len_of(&mut h, 0), n0, "undo");
    click_label(&mut h, "+16");
    assert_eq!(len_of(&mut h, 0), n0 + 16);
    click_label(&mut h, "-16");
    assert_eq!(len_of(&mut h, 0), n0);
    // a note in the second half: halving asks first
    h.state().with(|s| {
        s.song.patterns[0].cells[n0 - 1][0] = Cell {
            note: 49,
            instr: 1,
            ..Default::default()
        }
    });
    let cut = song(&mut h, |s| s.patterns[0].cut_by(n0 / 2));
    assert!(cut >= 1);
    click_label(&mut h, "/2");
    assert_eq!(len_of(&mut h, 0), n0, "nothing cut before confirming");
    assert!(h.state().len_confirm.is_some());
    click_label(&mut h, "Cancel");
    assert_eq!(len_of(&mut h, 0), n0, "cancel keeps it");
    assert!(h.state().len_confirm.is_none());
    click_label(&mut h, "/2");
    click_label(&mut h, &format!("Remove {cut} cell(s)"));
    assert_eq!(len_of(&mut h, 0), n0 / 2, "confirmed");
    key(&mut h, Key::Z, egui::Modifiers::COMMAND);
    assert_eq!(len_of(&mut h, 0), n0);
    assert_eq!(
        song(&mut h, |s| s.patterns[0].cells[n0 - 1][0].note),
        49,
        "undo brings the note back"
    );
    // keyboard: Ctrl+Shift+PgDn doubles (nothing cut, no question)
    let cs = egui::Modifiers::COMMAND | egui::Modifiers::SHIFT;
    key(&mut h, Key::PageDown, cs);
    assert_eq!(len_of(&mut h, 0), n0 * 2);
    key(&mut h, Key::PageUp, cs);
    assert_eq!(len_of(&mut h, 0), n0, "only empty rows go: no question");
    assert!(h.state().len_confirm.is_none());
}

/// odd lengths: the arranger ruler maps onto the new layout and the play
/// marker seeks into the resized block; the length survives save / load
#[test]
fn odd_block_length_maps_in_the_arranger_seeks_and_saves() {
    let mut h = harness();
    h.state_mut().pattern = 0;
    h.state_mut()
        .block_length_now(0, super::blocklen::LenOp::Set(37));
    let s0 = song(&mut h, |s| s.clone());
    assert_eq!(s0.patterns[0].rows, 37);
    let lens: Vec<usize> = s0.order.iter().map(|&b| s0.patterns[b].rows).collect();
    let k = s0.order.iter().position(|&b| b == 0).unwrap();
    let start: usize = lens[..k].iter().sum();
    assert_eq!(
        super::arranger::ruler_target(&lens, start + 36, false),
        (k, 36)
    );
    assert_eq!(
        super::arranger::ruler_target(&lens, start + 37, false).0,
        k + 1,
        "next clip starts right after row 36"
    );
    // seek while playing lands on the right row of the odd block
    h.state_mut().play(0);
    frames(&mut h, 2);
    h.state_mut().seek_to((k, 30));
    let (pat, row) = h.state().with(|s| (s.engine.pattern, s.engine.row));
    assert_eq!((pat, row), (0, 30));
    h.state_mut().play(0);
    // .cwt round trip
    let dir = std::env::temp_dir().join(format!("cw-len-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("odd.cwt");
    crate::fileio::save_cwt(&s0, &p).unwrap();
    let back = crate::fileio::load_cwt(&p).unwrap();
    assert_eq!(back.patterns[0].rows, 37);
    assert_eq!(back.patterns[0].cells.len(), 37);
    let _ = std::fs::remove_dir_all(&dir);
    // text songs: `lines` sets the length
    let t = "[song]\ntitle = \"t\"\n[[instrument]]\nname = \"a\"\npatch = \"builtin:init\"\n[[block]]\nname = \"b\"\nlines = 37\nrows = \"\"\"\n36 | C-4 01 .. ....\n\"\"\"\n";
    let ts = crate::fileio::song_from_text(t).unwrap();
    assert_eq!(ts.patterns[0].rows, 37);
}

// ---- on-screen transpose (Block bar buttons, Edit > Transpose) ----

/// the button with exactly this label (prefix matching would mix up -1 / -16)
fn click_button(h: &mut Harness<'static, App>, label: &str) {
    let r = *rects(h, Role::Button, Some(label))
        .first()
        .unwrap_or_else(|| panic!("no button {label:?}"));
    click_at(h, r.center());
}

/// hover a button (opens a submenu)
fn hover_button(h: &mut Harness<'static, App>, label: &str) {
    let r = *rects(h, Role::Button, Some(label))
        .first()
        .unwrap_or_else(|| panic!("no button {label:?}"));
    h.input_mut()
        .events
        .push(egui::Event::PointerMoved(r.center()));
    frames(h, 3);
}

/// block 0, channel 0: C-4 on rows 0..4, channel 1: E-4 on row 0
fn transpose_song(h: &mut Harness<'static, App>) {
    h.state_mut().view = View::Pattern;
    h.state_mut().pattern = 0;
    h.state_mut().follow = false;
    h.state().with(|s| {
        for r in 0..4 {
            s.song.patterns[0].cells[r][0] = Cell {
                note: 49,
                instr: 1,
                ..Default::default()
            };
        }
        s.song.patterns[0].cells[0][1] = Cell {
            note: 53,
            instr: 1,
            ..Default::default()
        };
    });
    frames(h, 2);
}

fn note(h: &mut Harness<'static, App>, r: usize, c: usize) -> u8 {
    song(h, |s| s.patterns[0].cells[r][c].note)
}

#[test]
fn block_bar_transpose_buttons_selection_channel_and_undo() {
    let mut h = harness();
    transpose_song(&mut h);
    for l in ["-12", "-1", "+1", "+12"] {
        assert_eq!(rects(&h, Role::Button, Some(l)).len(), 1, "one {l} button");
    }
    // selection rows 0-1 of channel 0: only those move
    h.state_mut().sel = Some(((0, 0), (1, 0)));
    h.state_mut().ch = 0;
    frames(&mut h, 2);
    assert!(h
        .state()
        .transpose_target()
        .starts_with("selection (rows 00–01, channel 1"));
    click_button(&mut h, "+1");
    assert_eq!((note(&mut h, 0, 0), note(&mut h, 1, 0)), (50, 50));
    assert_eq!(note(&mut h, 2, 0), 49, "outside the selection");
    assert_eq!(note(&mut h, 0, 1), 53, "other channel");
    assert_eq!(h.state().status, "Transposed selection by +1 semitones");
    click_button(&mut h, "+12");
    assert_eq!(note(&mut h, 0, 0), 62);
    click_button(&mut h, "-12");
    click_button(&mut h, "-1");
    assert_eq!(note(&mut h, 0, 0), 49, "-12 and -1 undo it by hand");
    ctrl_z(&mut h);
    ctrl_z(&mut h);
    assert_eq!(note(&mut h, 0, 0), 62, "each button press is one undo step");
    ctrl_z(&mut h);
    ctrl_z(&mut h);
    assert_eq!(note(&mut h, 0, 0), 49);
    // no selection: the whole cursor channel
    h.state_mut().sel = None;
    h.state_mut().ch = 0;
    frames(&mut h, 2);
    assert!(h.state().transpose_target().starts_with("channel 1"));
    click_button(&mut h, "-12");
    assert!((0..4).all(|r| note(&mut h, r, 0) == 37));
    assert_eq!(note(&mut h, 0, 1), 53);
    assert_eq!(h.state().status, "Transposed channel 1 by -12 semitones");
    ctrl_z(&mut h);
    assert!((0..4).all(|r| note(&mut h, r, 0) == 49));
}

/// the transpose buttons share the Block bar's line when there's room; narrower
/// windows move the right-hand group (then transpose) to the next line, and
/// nothing overlaps at any width
#[test]
fn block_bar_transpose_buttons_fit_one_line_and_wrap_without_overlap() {
    let mut h = harness();
    transpose_song(&mut h);
    // (width, transpose on the first line, Full height on the transpose line)
    for (w, first, with_full) in [
        (1700.0, true, true),
        (1360.0, true, false),
        (1100.0, false, true),
        (1000.0, false, false),
    ] {
        h.set_size(egui::vec2(w, 800.0));
        frames(&mut h, 3);
        let y = |r: Rect| r.center().y;
        let plus = rects(&h, Role::Button, Some("+12"))[0];
        let block = rects(&h, Role::Button, Some("+ New block"))[0];
        let full = h
            .query_all_by(|n| n.label().as_deref() == Some("Full height"))
            .next()
            .expect("Full height toggle")
            .rect();
        assert_eq!(
            (y(plus) - y(block)).abs() < 2.0,
            first,
            "{w}: transpose line"
        );
        assert_eq!(
            (y(plus) - y(full)).abs() < 2.0,
            with_full,
            "{w}: Full height"
        );
        assert!(
            y(full) >= y(plus) - 2.0,
            "{w}: the right group never goes above"
        );
        // nothing overlaps the transpose buttons, and they stay in the window
        let all: Vec<Rect> = h
            .query_all_by(|n| n.label().is_some())
            .map(|n| n.rect())
            .filter(|r| r.height() < 40.0 && r.width() < 600.0)
            .collect();
        for l in ["-12", "-1", "+1", "+12"] {
            let b = rects(&h, Role::Button, Some(l))[0];
            assert!(b.right() <= w, "{w}: {l} inside the window");
            for o in &all {
                if *o != b && !o.contains_rect(b) {
                    assert!(
                        !b.shrink(1.0).intersects(o.shrink(1.0)),
                        "{w}: {l} {b:?} overlaps {o:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn edit_menu_transpose_items_and_transpose_by() {
    let mut h = harness();
    transpose_song(&mut h);
    h.state_mut().sel = Some(((0, 0), (0, 1)));
    frames(&mut h, 2);
    let open = |h: &mut Harness<'static, App>| {
        click_button(h, "Edit");
        hover_button(h, "Transpose");
    };
    open(&mut h);
    click_label(&mut h, "Up octave");
    assert_eq!((note(&mut h, 0, 0), note(&mut h, 0, 1)), (61, 65));
    assert_eq!(note(&mut h, 1, 0), 49, "row 1 not selected");
    open(&mut h);
    click_label(&mut h, "Down semitone");
    assert_eq!(note(&mut h, 0, 0), 60);
    open(&mut h);
    click_label(&mut h, "Up semitone");
    open(&mut h);
    click_label(&mut h, "Down octave");
    assert_eq!(note(&mut h, 0, 0), 49);
    assert_eq!(h.state().status, "Transposed selection by -12 semitones");
    // Transpose by…: any amount from -24 to +24
    h.ctx
        .data_mut(|d| d.insert_temp(egui::Id::unique("transpose_by_amount"), -7i32));
    open(&mut h);
    hover_button(&mut h, "Transpose by…");
    click_button(&mut h, "Transpose -7");
    assert_eq!((note(&mut h, 0, 0), note(&mut h, 0, 1)), (42, 46));
    assert_eq!(h.state().status, "Transposed selection by -7 semitones");
    ctrl_z(&mut h);
    assert_eq!(note(&mut h, 0, 0), 49, "undoable");
    assert_eq!(*super::transpose::BY_RANGE.start(), -24);
    assert_eq!(*super::transpose::BY_RANGE.end(), 24);
}

/// the mixer at 1024, 1360, 1920 and 2560 px: scrolled to the end, the last
/// return strip (Ret B) is fully visible, with a clear gap before the pinned
/// Master strip, and the master section stays in the window (0.2.4.x fix:
/// Ret B used to be cut off right next to Master on wide screens)
#[test]
fn mixer_return_strips_never_hide_behind_master() {
    for song_n in [None, Some(2)] {
        let s = song_n.map_or_else(fileio::demo_song, fileio::demo_song_n);
        let mut h = harness_with(s);
        mixer(&mut h);
        for w in [1024.0f32, 1360.0, 1920.0, 2560.0] {
            h.set_size(egui::vec2(w, 1000.0));
            frames(&mut h, 4);
            let bar = h.state().mix_bar;
            if bar.width() > 10.0 {
                // scrolling: the view ends on a whole strip, so its right edge
                // never cuts one in half. A strip's frame starts 17 px left of
                // its fader (8 px margin + 1 px stroke + 8 px space), 110 px wide
                let cut: Vec<Rect> = rects(&h, Role::Slider, None)
                    .into_iter()
                    .filter(|r| r.height() > 100.0 && r.left() < 2.0 * w)
                    .filter(|r| {
                        let x = r.left() - 17.0;
                        x < bar.right() - 1.0 && x + 110.0 > bar.right() + 1.0
                    })
                    .collect();
                assert!(
                    cut.is_empty(),
                    "{w}: strips cut by the scroll edge: {cut:?}"
                );
                click_at(&mut h, Pos2::new(bar.right() - 1.0, bar.center().y));
                frames(&mut h, 4);
            }
            let on: Vec<Rect> = h
                .query_all_by(|n| n.label().as_deref() == Some("On"))
                .map(|n| n.rect())
                .collect();
            assert_eq!(on.len(), 2, "{w}: two return strips");
            // (the mixer's Master strip, not the toolbar's Master meter)
            let master = h
                .query_all_by(|n| {
                    n.label().as_deref() == Some("Master") || n.value().as_deref() == Some("Master")
                })
                .next()
                .expect("the Master strip")
                .rect();
            // strips are 110 px wide, centred on their labels
            let ret_b_right = on[1].center().x + 55.0;
            let master_left = master.center().x - 55.0;
            assert!(
                ret_b_right <= master_left - (super::mixer::MIXER_GAP - 4.0),
                "{w} {song_n:?}: Ret B ends at {ret_b_right}, Master starts at {master_left}"
            );
            if bar.width() > 10.0 {
                assert!(
                    ret_b_right <= bar.right() + 1.0,
                    "{w} {song_n:?}: Ret B inside the scroll area ({ret_b_right} vs {bar:?})"
                );
            }
            let gr = text_rect(&h, "Limiter & loudness");
            assert!(gr.right() < w, "{w}: master panel in the window");
        }
    }
}

fn missing_fx_patch() -> crate::plugin::PluginPatch {
    crate::plugin::PluginPatch {
        format: "clap".into(),
        id: "org.chipwave.ui-test.missing-fx".into(),
        name: "Missing FX".into(),
        vendor: "Nobody".into(),
        path: "/nonexistent/missing-fx.clap".into(),
        state: "c3RhdGU=".into(),
        params: vec![],
        macros: vec![],
    }
}

#[test]
fn plugin_effect_device_missing_notice_duplicate_and_paste_get_new_uids() {
    let mut h = harness();
    // every add-effect menu offers plugin effects
    instr_menu(&mut h, 2);
    click_exact(&mut h, "Add effect");
    exact(&h, crate::fx::FxKind::Plugin.name());
    h.key_press(egui::Key::Escape);
    frames(&mut h, 2);
    h.with_engine(|s| {
        s.song.instruments[1].fx.clear();
        s.song.instruments[1]
            .fx
            .push(crate::fx::Device::plugin(missing_fx_patch()));
    });
    h.state_mut().instr = 2;
    h.state_mut().view = View::Fx;
    frames(&mut h, 20);
    // the device shows the plugin's name and the missing notice
    exact(&h, "Missing FX");
    let err = h.with_engine(|s| s.engine.fx_plugins.errors.clone());
    assert_eq!(err.len(), 1, "load failed once: {err:?}");
    assert!(
        h.query_all_by(|n| n
            .label()
            .or(n.value())
            .is_some_and(|l| l.contains("bypassed, settings kept")))
            .next()
            .is_some(),
        "missing notice shown"
    );
    let uid0 = fx_of(&mut h, 2)[0].uid;
    assert_ne!(uid0, 0);
    // Duplicate (device menu): a new instance with the same saved state
    h.state_mut()
        .apply_fx_ops(1, vec![super::fxview::FxOp::Dup(0)]);
    frames(&mut h, 2);
    let fx = fx_of(&mut h, 2);
    assert_eq!(fx.len(), 2);
    assert_ne!(fx[1].uid, uid0, "duplicate gets its own uid");
    assert_eq!(fx[1].plugin, fx[0].plugin, "same plugin and state");
    // paste-append into the same instrument: uids stay unique
    instr_menu(&mut h, 2);
    click_exact(&mut h, "Copy effects");
    instr_menu(&mut h, 2);
    click_exact(&mut h, "Paste effects (append)");
    let fx = fx_of(&mut h, 2);
    assert_eq!(fx.len(), 4);
    let mut uids: Vec<u32> = fx.iter().map(|d| d.uid).collect();
    uids.sort();
    uids.dedup();
    assert_eq!(uids.len(), 4, "pasted devices are new instances");
    assert_eq!(fx[3].plugin.as_ref().unwrap().state, "c3RhdGU=");
}

/// UI frames vs the engine thread: how long the engine waits for the shared
/// lock while the UI draws Chrome Throat playing (timeline + Browser > Plugins).
#[test]
#[ignore]
fn rt_lock_wait_chrome_throat() {
    let i = fileio::DEMOS
        .iter()
        .position(|d| d.0 == "Chrome Throat")
        .unwrap();
    let mut h = harness_with(fileio::demo_song_n(i));
    h.state_mut().play(0);
    frame(&mut h);
    let shared = h.state().shared.clone();
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let st = stop.clone();
    let eng = std::thread::spawn(move || {
        let mut buf = vec![0.0f32; 512];
        let (mut waits, mut renders) = (Vec::new(), Vec::new());
        while !st.load(std::sync::atomic::Ordering::Relaxed) {
            let t0 = std::time::Instant::now();
            let mut s = crate::audio::lock(&shared);
            let t1 = std::time::Instant::now();
            let Shared {
                song,
                engine,
                preview,
            } = &mut *s;
            engine.render(preview.as_ref().unwrap_or(song), &mut buf);
            drop(s);
            waits.push((t1 - t0).as_secs_f64() * 1e6);
            renders.push(t1.elapsed().as_secs_f64() * 1e6);
            // real time pacing of 256 frames is 5.3 ms; poll faster to see waits
            std::thread::sleep(std::time::Duration::from_micros(500));
        }
        (waits, renders)
    });
    let mut ft = Vec::new();
    for n in 0..240 {
        if n == 120 {
            h.state_mut().extra.browser_open = true;
            h.state_mut().br.filter.tab = crate::browser::Tab::Plugins;
        }
        let t = std::time::Instant::now();
        frame(&mut h);
        ft.push(t.elapsed().as_secs_f64() * 1e3);
    }
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    let (mut w, mut r) = eng.join().unwrap();
    let pct = |v: &mut Vec<f64>, q: f64| {
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        v[((v.len() - 1) as f64 * q) as usize]
    };
    println!(
        "RTL frames {} ms p50 {:.1} p99 {:.1} max {:.1} | lock wait us p50 {:.0} p99 {:.0} max {:.0} | render us p50 {:.0} p99 {:.0}",
        ft.len(),
        pct(&mut ft.clone(), 0.5),
        pct(&mut ft.clone(), 0.99),
        pct(&mut ft, 1.0),
        pct(&mut w.clone(), 0.5),
        pct(&mut w.clone(), 0.99),
        pct(&mut w, 1.0),
        pct(&mut r.clone(), 0.5),
        pct(&mut r, 0.99),
    );
}

/// Del (0.2.5.0): the status bar's tracker readout (Row · Instr · Oct ·
/// Step) only shows with the Tracker; length and peak stay everywhere
#[test]
fn status_bar_tracker_readout_only_with_the_tracker() {
    let mut h = harness();
    h.state_mut().view = View::Timeline;
    frames(&mut h, 2);
    assert!(!h.state().status_cursor_visible());
    assert!(!has_text(&h, "Row "), "no tracker readout on the main page");
    assert!(has_text(&h, "length "), "length and peak stay");
    h.state_mut().view = View::Pattern;
    frames(&mut h, 2);
    assert!(h.state().status_cursor_visible());
    assert!(has_text(&h, "Row "), "the Tracker shows its cursor");
    let both = h
        .query_all_by(|n| {
            n.label().or(n.value()).is_some_and(|l| {
                l.starts_with("Row ") && l.contains(" · length ") && l.contains(" · peak ")
            })
        })
        .next()
        .is_some();
    assert!(both, "length and peak still there");
}

/// File > Save / Load controller map… are in the menu (on a 1366×768
/// screen), and a loaded map replaces the MIDI learn list
#[test]
fn file_menu_has_controller_maps() {
    let mut h = harness();
    h.set_size(egui::vec2(1366.0, 768.0));
    frames(&mut h, 2);
    click_exact(&mut h, "File");
    for name in ["Save controller map…", "Load controller map…"] {
        let r = labelled(&h, name);
        assert!(r.max.y <= 768.0 && r.max.x <= 1366.0, "{name} at {r:?}");
        assert!(r.height() >= 13.0, "{name} too small: {r:?}");
    }
    h.state_mut().extra.midi.learn = vec![(1, "tempo".into())];
    let text = crate::ctlmap::to_text(
        "Desk",
        &[
            (74, "instrument.1.cutoff".into()),
            (7, "mixer.ch.1.volume".into()),
        ],
    )
    .unwrap();
    let msg = h.state_mut().load_controller_text(&text);
    assert!(
        msg.contains("\"Desk\" loaded: 2 learned control(s)"),
        "{msg}"
    );
    assert_eq!(
        h.state().extra.midi.learn,
        vec![
            (74, "instrument.1.cutoff".to_string()),
            (7, "mixer.ch.1.volume".to_string())
        ]
    );
    let msg = h.state_mut().load_controller_text("name = 3");
    assert!(msg.starts_with("Controller map not loaded"), "{msg}");
    assert_eq!(
        h.state().extra.midi.learn.len(),
        2,
        "a bad file changes nothing"
    );
}
