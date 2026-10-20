//! UI tests of drag-to-modulate: dropping a modulator chip on a knob or FX
//! slider, depth handles, Alt-drag, the Routings card and undo.
use super::params::{self, Ctl, Tab};
use super::timeline_m2_tests::{click_at, drag_m};
use super::ui_tests::{drag, frame, harness_with, key};
use super::*;
use crate::modulation::ModKind;
use egui::accesskit::Role;
use egui_kittest::{kittest::Queryable, Harness};

fn frames(h: &mut Harness<'static, App>, n: usize) {
    for _ in 0..n {
        frame(h);
    }
}

fn rect_of(h: &Harness<'static, App>, role: Role, label: &str) -> Rect {
    h.query_all_by(|n| n.role() == role && n.label().as_deref() == Some(label))
        .next()
        .unwrap_or_else(|| panic!("{label} ({role:?}) not found"))
        .rect()
}

/// the demo song plus a synth (selected) that owns one LFO
fn synth_with_lfo() -> (Harness<'static, App>, usize) {
    let mut song = crate::fileio::demo_song();
    song.instruments.push(Instrument {
        name: "Lead".into(),
        volume: 0.8,
        transpose: 0,
        fx: vec![crate::fx::Device::new(crate::fx::FxKind::Delay)],
        kind: InstrumentKind::Synth(crate::synth::Patch::default()),
    });
    let idx = song.instruments.len() - 1;
    song.modulation.add(idx, ModKind::Lfo);
    let mut h = harness_with(song);
    h.state_mut().instr = idx as u8 + 1;
    h.state_mut().show_instrument_panel();
    // the knobs of the full editor (Advanced ›)
    h.state_mut().ip.advanced = true;
    frames(&mut h, 3);
    (h, idx)
}

fn routes(h: &Harness<'static, App>) -> Vec<(String, f32)> {
    h.state().with(|s| {
        s.song
            .modulation
            .routes
            .iter()
            .map(|r| (r.target.clone(), r.amount))
            .collect()
    })
}

/// the knob's 0..1 position
fn knob_norm(h: &Harness<'static, App>, idx: usize, key: &str) -> f32 {
    h.state().with(|s| {
        let inst = &s.song.instruments[idx];
        let l = params::build(inst, idx, 0);
        let p = l.find(key).expect("knob");
        let Ctl::Knob { lo, hi, log, .. } = p.ctl else {
            panic!("not a knob")
        };
        params::to_norm((p.get)(inst), lo, hi, log)
    })
}

/// where a knob's first depth handle sits
fn handle_pos(knob: Rect, n: f32, amount: f32) -> Pos2 {
    let a0 = std::f32::consts::PI * 0.75;
    let span = std::f32::consts::PI * 1.5;
    let ang = a0 + span * (n + amount).clamp(0.0, 1.0);
    knob.center() + egui::vec2(ang.cos(), ang.sin()) * (14.0 + 4.5)
}

#[test]
fn dropping_a_modulator_on_a_knob_routes_it_and_the_ring_sets_the_depth() {
    let (mut h, idx) = synth_with_lfo();
    let chip = rect_of(&h, Role::Button, "LFO 1");
    let knob = rect_of(&h, Role::Slider, "Cutoff");
    let before = knob_norm(&h, idx, "cutoff");
    drag(&mut h, chip.center(), knob.center());
    frames(&mut h, 2);
    let tid = format!("instrument.{}.cutoff", idx + 1);
    assert_eq!(routes(&h), vec![(tid.clone(), 0.25)], "a route at 25 %");
    assert_eq!(
        knob_norm(&h, idx, "cutoff"),
        before,
        "dropping does not turn the knob"
    );
    // drag the ring's handle: up/right deepens it
    let hp = handle_pos(knob, before, 0.25);
    drag(&mut h, hp, hp + egui::vec2(15.0, -15.0));
    frames(&mut h, 2);
    let a = routes(&h)[0].1;
    assert!(a > 0.32 && a < 0.5, "deeper: {a}");
    assert_eq!(knob_norm(&h, idx, "cutoff"), before, "the knob stays");
    // Alt-drag on the knob: the selected modulator's depth
    let c = knob.center();
    drag_m(&mut h, c, c + egui::vec2(0.0, 30.0), egui::Modifiers::ALT);
    frames(&mut h, 2);
    let b = routes(&h)[0].1;
    assert!(b < a - 0.1, "Alt-drag down reduces the depth: {a} -> {b}");
    assert_eq!(
        knob_norm(&h, idx, "cutoff"),
        before,
        "Alt-drag keeps the value"
    );
    // undo: depth edits first, then the route itself
    key(&mut h, Key::Z, egui::Modifiers::COMMAND);
    frames(&mut h, 2);
    let mut n = 0;
    while !routes(&h).is_empty() && n < 4 {
        key(&mut h, Key::Z, egui::Modifiers::COMMAND);
        frames(&mut h, 2);
        n += 1;
    }
    assert!(routes(&h).is_empty(), "undo removes the route");
    key(&mut h, Key::Y, egui::Modifiers::COMMAND);
    frames(&mut h, 2);
    assert_eq!(routes(&h).len(), 1, "redo brings it back");
}

#[test]
fn patch_knobs_are_targets_and_routings_card_removes_them() {
    let (mut h, idx) = synth_with_lfo();
    // Sustain (amp.s) has no target of its own: it is a patch target now
    let knob = rect_of(&h, Role::Slider, "Sustain");
    let chip = rect_of(&h, Role::Button, "LFO 1");
    drag(&mut h, chip.center(), knob.center());
    frames(&mut h, 2);
    let tid = format!("instrument.{}.patch.amp-s", idx + 1);
    assert_eq!(routes(&h), vec![(tid, 0.25)]);
    // the Modulation tab lists it; × removes it
    h.state_mut().ip.tab = Tab::Modulation;
    frames(&mut h, 3);
    let x = rect_of(&h, Role::Button, "×");
    click_at(&mut h, x.center());
    frames(&mut h, 2);
    assert!(routes(&h).is_empty(), "removed from the Routings card");
    // + Modulator adds one (and selects it)
    let plus = rect_of(&h, Role::Button, "+ Modulator");
    click_at(&mut h, plus.center());
    frames(&mut h, 2);
    let env = rect_of(&h, Role::Button, "Envelope");
    click_at(&mut h, env.center());
    frames(&mut h, 3);
    let kinds: Vec<ModKind> = h
        .state()
        .with(|s| s.song.modulation.mods.iter().map(|m| m.kind).collect());
    assert_eq!(kinds, vec![ModKind::Lfo, ModKind::Envelope]);
    let sel = h.state().ip.mod_sel;
    assert_eq!(
        sel,
        h.state().with(|s| Some(s.song.modulation.mods[1].id)),
        "the new modulator is selected"
    );
}

#[test]
fn fx_view_sliders_take_a_modulator() {
    let (mut h, idx) = synth_with_lfo();
    h.state_mut().view = View::Fx;
    frames(&mut h, 4);
    let chip = rect_of(&h, Role::Button, "LFO 1");
    // the Delay's first slider (Time)
    let slider = h
        .query_all_by(|n| n.role() == Role::Slider)
        .map(|n| n.rect())
        .filter(|r| r.top() > chip.bottom())
        .min_by(|a, b| a.top().total_cmp(&b.top()))
        .expect("an FX slider");
    drag(&mut h, chip.center(), slider.center());
    frames(&mut h, 2);
    let r = routes(&h);
    assert_eq!(r.len(), 1, "{r:?}");
    assert!(
        r[0].0
            .starts_with(&format!("instrument.{}.fx.1.p", idx + 1)),
        "{r:?}"
    );
}
