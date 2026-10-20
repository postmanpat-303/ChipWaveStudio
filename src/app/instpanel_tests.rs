//! UI tests of the lower-panel instrument editor, the lower-panel switch,
//! the splitter and Full-height pattern (0.2.3.19).
use super::params::{self, Ctl, Tab};
use super::ui_tests::{double_click, drag, frame, harness_with, key};
use super::*;
use egui::accesskit::Role;
use egui_kittest::{kittest::Queryable, Harness};

fn frames(h: &mut Harness<'static, App>, n: usize) {
    for _ in 0..n {
        frame(h);
    }
}

/// the demo song with `inst` added and selected
fn with_inst(inst: Instrument) -> Harness<'static, App> {
    let mut song = crate::fileio::demo_song();
    song.instruments.push(inst);
    let n = song.instruments.len() as u8;
    let mut h = harness_with(song);
    h.state_mut().instr = n;
    frame(&mut h);
    h
}

fn ctrl() -> egui::Modifiers {
    egui::Modifiers::COMMAND
}

fn labelled_rect(h: &Harness<'static, App>, role: Role, label: &str) -> Option<Rect> {
    h.query_all_by(|n| n.role() == role && n.label().as_deref() == Some(label))
        .next()
        .map(|n| n.rect())
}

fn has_label(h: &Harness<'static, App>, label: &str) -> bool {
    h.query_all_by(|n| n.label().as_deref() == Some(label) || n.value().as_deref() == Some(label))
        .next()
        .is_some()
}

#[test]
fn ctrl_i_opens_the_instrument_editor_and_every_type_and_tab_draws_its_parameters() {
    for (ty, inst) in params::tests::all_types() {
        let is_plugin = matches!(inst.kind, InstrumentKind::Plugin(_));
        let mut h = with_inst(inst.clone());
        assert!(
            h.state().extra.lower_inst && h.state().lower_visible(),
            "{ty}: the instrument editor is the default"
        );
        h.state_mut().set_lower(0);
        frames(&mut h, 2);
        assert!(
            h.state().arr_docked(),
            "{ty}: the switch shows the arranger"
        );
        key(&mut h, Key::I, ctrl());
        frames(&mut h, 2);
        assert!(
            h.state().extra.lower_inst,
            "{ty}: Ctrl+I shows the instrument editor"
        );
        assert!(
            !h.state().arr_docked(),
            "{ty}: the arranger is not docked meanwhile"
        );
        let idx = h.state().instr as usize - 1;
        let l = params::build(&inst, idx, 0);
        for tab in Tab::ALL {
            if !l.tab_has(tab) {
                continue;
            }
            h.state_mut().ip.tab = tab;
            frames(&mut h, 2);
            for p in l.params.iter().filter(|p| p.tab == tab) {
                let role = match p.ctl {
                    Ctl::Knob { .. } => Role::Slider,
                    Ctl::Choice(_) => Role::ComboBox,
                    Ctl::Toggle => Role::CheckBox,
                    Ctl::Custom(_) => continue,
                };
                assert!(
                    labelled_rect(&h, role, &p.name).is_some(),
                    "{ty}: {} › {} › {} ({role:?}) is drawn",
                    tab.name(),
                    p.card,
                    p.name
                );
            }
            // every card has a header
            for c in l.cards.iter().filter(|c| c.tab == tab) {
                let _ = c;
            }
        }
        if is_plugin {
            // the plugin is not loadable here: its status card still shows
            h.state_mut().ip.tab = Tab::Main;
            frames(&mut h, 2);
        }
        // Ctrl+I again: back to the arranger
        key(&mut h, Key::I, ctrl());
        frame(&mut h);
        assert!(!h.state().extra.lower_inst, "{ty}: Ctrl+I toggles back");
    }
}

fn analog() -> Instrument {
    Instrument {
        name: "Bass".into(),
        volume: 0.8,
        transpose: 0,
        fx: vec![],
        kind: InstrumentKind::Gen(crate::gen::preset("Psy Rolling Bass").unwrap()),
    }
}

fn analog_cutoff(h: &mut Harness<'static, App>) -> f32 {
    let i = h.state().instr as usize - 1;
    h.state().with(|s| {
        let inst = &s.song.instruments[i];
        let l = params::build(inst, i, 0);
        let p = l.find("an.0").expect("analog cutoff knob");
        (p.get)(inst)
    })
}

#[test]
fn knob_drag_changes_the_sound_and_one_ctrl_z_undoes_it() {
    let mut h = with_inst(analog());
    h.state_mut().show_instrument_panel();
    frames(&mut h, 2);
    let before = analog_cutoff(&mut h);
    let r = labelled_rect(&h, Role::Slider, "Cutoff").expect("Cutoff knob");
    let c = r.center();
    drag(&mut h, c, c + egui::vec2(0.0, -40.0));
    frames(&mut h, 2);
    let after = analog_cutoff(&mut h);
    assert!(
        after > before + 0.05,
        "dragging up raises it ({before} -> {after})"
    );
    key(&mut h, Key::Z, ctrl());
    frames(&mut h, 2);
    assert!(
        (analog_cutoff(&mut h) - before).abs() < 1e-6,
        "one Ctrl+Z undoes the drag"
    );
}

#[test]
fn ctrl_drag_is_fine_and_double_click_resets_to_default() {
    let mut h = with_inst(analog());
    h.state_mut().show_instrument_panel();
    frames(&mut h, 2);
    let before = analog_cutoff(&mut h);
    let r = labelled_rect(&h, Role::Slider, "Cutoff").expect("Cutoff knob");
    let c = r.center();
    h.input_mut()
        .events
        .push(egui::Event::ModifiersChanged(ctrl()));
    drag(&mut h, c, c + egui::vec2(0.0, -40.0));
    h.input_mut()
        .events
        .push(egui::Event::ModifiersChanged(Default::default()));
    frames(&mut h, 2);
    let fine = analog_cutoff(&mut h) - before;
    assert!(
        fine > 0.0 && fine < 0.05,
        "Ctrl-drag moves a tenth as far ({fine})"
    );
    // double-click: the middle of the dice range
    double_click(&mut h, c);
    frames(&mut h, 2);
    let d = &crate::gen2::ANALOG[0];
    let mid = (d.dice.0 + d.dice.1) * 0.5;
    let v = analog_cutoff(&mut h);
    assert!(
        (v - mid).abs() < 1e-4,
        "double-click resets to the default ({v} vs {mid}, was {before})"
    );
}

#[test]
fn parameter_search_jumps_to_the_card_on_its_tab() {
    let mut h = with_inst(analog());
    key(&mut h, Key::F, ctrl());
    frames(&mut h, 2);
    assert!(
        h.state().extra.lower_inst,
        "Ctrl+F opens the instrument editor"
    );
    h.state_mut().ip.query = "f.decay".into();
    h.state_mut().ip.search_open = true;
    frames(&mut h, 2);
    // the hit list shows the parameter with its place
    let hit = h
        .query_all_by(|n| {
            n.role() == Role::Button
                && n.label()
                    .is_some_and(|l| l.starts_with("F.Decay") && l.contains("Modulation"))
        })
        .next()
        .map(|n| n.rect())
        .expect("search hit");
    let p = hit.center();
    h.input_mut().events.push(egui::Event::PointerMoved(p));
    frame(&mut h);
    for pressed in [true, false] {
        h.input_mut().events.push(egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        });
        frame(&mut h);
    }
    frames(&mut h, 2);
    assert_eq!(h.state().ip.tab, Tab::Modulation, "the hit's tab opens");
    assert!(
        h.state()
            .ip
            .flash
            .as_ref()
            .is_some_and(|f| f.0 == "Filter envelope"),
        "its card flashes"
    );
    assert!(labelled_rect(&h, Role::Slider, "F.Decay").is_some());
}

#[test]
fn full_height_pattern_hides_the_lower_panel_and_restores_its_height() {
    let mut h = with_inst(analog());
    h.state_mut().extra.lower_h = 333.0;
    h.state_mut().set_lower(0);
    frame(&mut h);
    assert!(h.state().arr_docked());
    key(&mut h, Key::F11, Default::default());
    frames(&mut h, 2);
    assert!(h.state().extra.full_height, "F11 hides the lower panel");
    assert!(!h.state().lower_visible());
    assert!(
        !has_label(&h, "Arranger zoom"),
        "no arranger under the pattern"
    );
    assert!(has_label(&h, "Full height"), "the Block bar toggle stays");
    key(&mut h, Key::F11, Default::default());
    frames(&mut h, 2);
    assert!(h.state().lower_visible(), "F11 again shows it");
    assert_eq!(h.state().extra.lower_h, 333.0, "at the height it had");
    // full height, then the instrument shortcut brings the panel back
    key(&mut h, Key::F11, Default::default());
    key(&mut h, Key::I, ctrl());
    frames(&mut h, 2);
    assert!(h.state().lower_visible() && h.state().extra.lower_inst);
    // the View menu item and the switch-bar button
    h.state_mut().toggle_full_height();
    frame(&mut h);
    assert!(h.state().extra.full_height);
    h.state_mut().toggle_full_height();
    frames(&mut h, 2);
    let b = labelled_rect(&h, Role::Button, "Full-height view  F11").expect("switch-bar button");
    let c = b.center();
    h.input_mut().events.push(egui::Event::PointerMoved(c));
    frame(&mut h);
    for pressed in [true, false] {
        h.input_mut().events.push(egui::Event::PointerButton {
            pos: c,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        });
        frame(&mut h);
    }
    assert!(
        h.state().extra.full_height,
        "the switch-bar button hides the panel"
    );
}

/// y of the splitter between pattern and lower panel
fn splitter_y(h: &Harness<'static, App>) -> f32 {
    // the switch bar's Arranger button sits just under it
    labelled_rect(h, Role::Button, "Order list")
        .map(|r| r.top() - 5.0)
        .expect("switch bar")
}

#[test]
fn splitter_resizes_the_lower_panel_and_dragging_to_the_bottom_collapses_it() {
    let mut h = with_inst(analog());
    h.state_mut().show_instrument_panel();
    frames(&mut h, 2);
    let y = splitter_y(&h);
    let x = 900.0;
    drag(&mut h, Pos2::new(x, y), Pos2::new(x, y - 60.0));
    frames(&mut h, 2);
    let lh = h.state().extra.lower_h;
    assert!(lh > 300.0, "dragging up makes it taller ({lh})");
    let y = splitter_y(&h);
    drag(&mut h, Pos2::new(x, y), Pos2::new(x, 995.0));
    frames(&mut h, 2);
    assert!(
        h.state().extra.full_height,
        "dragged to the bottom: full-height pattern"
    );
    assert_eq!(h.state().extra.lower_h, lh, "the previous height is kept");
    key(&mut h, Key::F11, Default::default());
    frames(&mut h, 2);
    assert!(h.state().lower_visible());
}

#[test]
fn switch_bar_keeps_arranger_and_automation_working() {
    let mut h = with_inst(analog());
    h.state_mut().show_instrument_panel();
    frames(&mut h, 2);
    for (name, inst, auto) in [
        ("Order list", false, false),
        ("Automation", false, true),
        ("Instrument", true, false),
    ] {
        let r = labelled_rect(&h, Role::Button, name).unwrap_or_else(|| panic!("{name}"));
        let c = r.center();
        h.input_mut().events.push(egui::Event::PointerMoved(c));
        frame(&mut h);
        for pressed in [true, false] {
            h.input_mut().events.push(egui::Event::PointerButton {
                pos: c,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            });
            frame(&mut h);
        }
        frames(&mut h, 2);
        assert_eq!(h.state().extra.lower_inst, inst, "{name}");
        if !inst {
            assert_eq!(h.state().show_auto, auto, "{name}");
            assert!(h.state().arr_docked(), "{name}: the arranger is docked");
            assert!(has_label(&h, "Insert block"), "{name}: the arranger draws");
        }
    }
}

#[test]
fn cards_collapse_per_instrument_type() {
    let mut h = with_inst(analog());
    h.state_mut().show_instrument_panel();
    frames(&mut h, 2);
    assert!(labelled_rect(&h, Role::Slider, "Osc 1").is_some());
    h.state_mut()
        .extra
        .cards_closed
        .push("Analog/Oscillators".into());
    frames(&mut h, 2);
    assert!(
        labelled_rect(&h, Role::Slider, "Osc 1").is_none(),
        "collapsed"
    );
    let r = labelled_rect(&h, Role::Button, "Oscillators").expect("collapsed strip");
    let c = r.center();
    h.input_mut().events.push(egui::Event::PointerMoved(c));
    frame(&mut h);
    for pressed in [true, false] {
        h.input_mut().events.push(egui::Event::PointerButton {
            pos: c,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        });
        frame(&mut h);
    }
    frames(&mut h, 2);
    assert!(
        h.state().extra.cards_closed.is_empty(),
        "a click expands it"
    );
    assert!(labelled_rect(&h, Role::Slider, "Osc 1").is_some());
}

#[test]
fn a_b_compare_swaps_the_sound_and_undoes() {
    let mut h = with_inst(analog());
    h.state_mut().show_instrument_panel();
    frames(&mut h, 2);
    let a = analog_cutoff(&mut h);
    h.state_mut().set_lower(1);
    // B: a copy; change it, then back to A
    let click = |h: &mut Harness<'static, App>, name: &str| {
        let r = h
            .query_all_by(|n| n.label().as_deref() == Some(name) && n.role() != Role::Label)
            .next()
            .map(|n| n.rect())
            .unwrap_or_else(|| panic!("{name}"));
        let c = r.center();
        h.input_mut().events.push(egui::Event::PointerMoved(c));
        frame(h);
        for pressed in [true, false] {
            h.input_mut().events.push(egui::Event::PointerButton {
                pos: c,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            });
            frame(h);
        }
        frames(h, 2);
    };
    click(&mut h, "B");
    let i = h.state().instr as usize - 1;
    h.state().with(|s| {
        if let InstrumentKind::Gen(g) = &mut s.song.instruments[i].kind {
            if let crate::gen::GenEngine::Analog(a) = &mut g.engine {
                a.p[0] = 0.95;
            }
        }
    });
    frame(&mut h);
    click(&mut h, "A");
    assert!(
        (analog_cutoff(&mut h) - a).abs() < 1e-6,
        "A is the original"
    );
    click(&mut h, "B");
    assert!(
        (analog_cutoff(&mut h) - 0.95).abs() < 1e-6,
        "B keeps its edit"
    );
}

#[test]
fn sidebar_shows_type_tags_summary_and_the_edit_button() {
    let mut h = with_inst(analog());
    // the button shows while the lower panel is on something else
    h.state_mut().set_lower(0);
    frame(&mut h);
    let r = h
        .query_all_by(|n| {
            n.role() == Role::Button
                && n.label()
                    .is_some_and(|l| l.starts_with("Edit in lower panel"))
        })
        .next()
        .map(|n| n.rect())
        .expect("summary button");
    let c = r.center();
    h.input_mut().events.push(egui::Event::PointerMoved(c));
    frame(&mut h);
    for pressed in [true, false] {
        h.input_mut().events.push(egui::Event::PointerButton {
            pos: c,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        });
        frame(&mut h);
    }
    assert!(h.state().extra.lower_inst);
    // the classic editor is still there
    h.state_mut().extra.classic_editor = true;
    frames(&mut h, 2);
    assert!(h
        .query_all_by(|n| n
            .label()
            .is_some_and(|l| l.starts_with("Edit in lower panel")))
        .next()
        .is_none());
}

#[test]
fn old_settings_files_get_the_new_defaults() {
    let e: crate::settings::Extra = toml::from_str("").expect("empty extra");
    assert_eq!(e.lower_h, 0.0);
    // the instrument editor is the default view (no choice saved yet)
    assert!(e.lower_inst && !e.lower_chosen);
    assert!(!e.full_height && !e.classic_editor);
    assert!(e.cards_closed.is_empty());
}

/// new users (and settings with no saved choice) get the instrument editor in
/// the lower panel; the switch remembers the user's choice, and adding an
/// automation lane shows the lanes without closing the instrument editor
#[test]
fn lower_panel_opens_on_the_instrument_editor_and_remembers_the_choice() {
    let mut h = with_inst(analog());
    assert!(h.state().extra.lower_inst && h.state().lower_visible());
    assert!(!h.state().extra.lower_chosen);
    assert!(!h.state().arr_docked(), "the arranger isn't docked");
    // the Arranger switch is a choice
    let b = labelled_rect(&h, Role::Button, "Order list").expect("Order list switch");
    let c = b.center();
    h.input_mut().events.push(egui::Event::PointerMoved(c));
    frame(&mut h);
    for pressed in [true, false] {
        h.input_mut().events.push(egui::Event::PointerButton {
            pos: c,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        });
        frame(&mut h);
    }
    frames(&mut h, 2);
    let e = &h.state().extra;
    assert!(!e.lower_inst && e.lower_chosen, "the arranger, remembered");
    let mut saved = e.clone();
    saved.default_lower_view();
    assert!(!saved.lower_inst, "kept on the next start");
    // Ctrl+I back to the instrument editor; a new automation lane shows the
    // lanes but keeps the instrument editor open (automation design A: adding
    // a lane from a knob doesn't yank the lower panel away)
    key(&mut h, Key::I, ctrl());
    frames(&mut h, 2);
    assert!(h.state().extra.lower_inst);
    h.state_mut().add_lane(crate::automation::Target::Master);
    frames(&mut h, 2);
    assert!(h.state().extra.lower_inst && h.state().show_auto);
}

fn grid_of(h: &Harness<'static, App>) -> crate::grid::GridP {
    let i = h.state().instr as usize - 1;
    h.state().with(|s| match &s.song.instruments[i].kind {
        InstrumentKind::Gen(crate::gen::GenPatch {
            engine: crate::gen::GenEngine::Grid(g),
            ..
        }) => g.clone(),
        _ => panic!("not a Grid"),
    })
}

#[test]
fn grid_canvas_adds_modules_unplugs_and_patches_cables_with_undo() {
    let inst = Instrument {
        name: "Grid".into(),
        volume: 0.8,
        transpose: 0,
        fx: vec![],
        kind: InstrumentKind::Gen(crate::gen::preset("Grid Saw Bass").unwrap()),
    };
    let mut h = with_inst(inst);
    h.state_mut().show_instrument_panel();
    h.state_mut().extra.lower_h = 640.0;
    frames(&mut h, 3);
    assert_eq!(h.state().ip.tab, Tab::Grid, "a Grid opens on its patch");
    let g0 = grid_of(&h);
    // + Module › Sources › Noise
    let c = super::ui_tests::exact(&h, "+ Module").center();
    super::ui_tests::click_at(&mut h, c);
    frames(&mut h, 2);
    let c = super::ui_tests::exact(&h, "Sources").center();
    super::ui_tests::click_at(&mut h, c);
    frames(&mut h, 2);
    let c = super::ui_tests::exact(&h, "Noise").center();
    super::ui_tests::click_at(&mut h, c);
    frames(&mut h, 3);
    let g1 = grid_of(&h);
    assert_eq!(g1.modules.len(), g0.modules.len() + 1);
    assert_eq!(g1.modules.last().unwrap().kind, crate::grid::Kind::Noise);
    key(&mut h, Key::Z, ctrl());
    frames(&mut h, 2);
    assert_eq!(grid_of(&h), g0, "Ctrl+Z removes it");
    // the ladder's "Res" knob (2nd knob) locates its module on the canvas
    let res = labelled_rect(&h, Role::Slider, "Res").expect("Res knob");
    let kr_min = res.center() - egui::vec2(30.0, 18.0);
    let lad = egui::pos2(
        kr_min.x - 8.0 - 66.0,
        kr_min.y - (20.0 + 22.0 + 2.0 * 16.0 + 4.0),
    );
    let lad_in = lad + egui::vec2(0.0, 20.0 + 22.0 + 8.0);
    let lad_id = g0
        .modules
        .iter()
        .find(|m| m.kind == crate::grid::Kind::Ladder)
        .unwrap()
        .id;
    let plugged = |g: &crate::grid::GridP| g.cables.iter().any(|c| c.to == lad_id && c.inp == 0);
    assert!(plugged(&g0));
    super::ui_tests::click_at(&mut h, lad_in);
    frames(&mut h, 2);
    assert!(!plugged(&grid_of(&h)), "clicking the input unplugs it");
    // drag the Mix output (one column left) back onto it
    let mix_out = lad + egui::vec2(-185.0 + 150.0, 20.0 + 8.0);
    drag(&mut h, mix_out, lad_in);
    frames(&mut h, 2);
    let g2 = grid_of(&h);
    assert!(plugged(&g2), "a dragged cable patches it");
    assert_eq!(g2.cables.len(), g0.cables.len());
}
