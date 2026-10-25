//! UI tests of StringWave's panel (sub-tabs, full view, text size) and its
//! knob pie, at a 1366x768 laptop size in every theme.
use super::objpie_tests::right_click;
use super::ui_tests::{click_at, frame, harness_with, key};
use super::*;
use egui::accesskit::Role;
use egui_kittest::{kittest::Queryable, Harness};

fn frames(h: &mut Harness<'static, App>, n: usize) {
    for _ in 0..n {
        frame(h);
    }
}

fn strings_inst() -> Instrument {
    Instrument {
        name: "Strings".into(),
        volume: 0.8,
        transpose: 0,
        fx: vec![],
        kind: InstrumentKind::Gen(crate::gen::GenPatch {
            engine: crate::gen::GenEngine::Strings(Default::default()),
            ..Default::default()
        }),
    }
}

/// the demo song plus a StringWave, selected, its panel open at 1366x768
fn laptop(theme: crate::theme::Theme) -> Harness<'static, App> {
    let mut song = crate::fileio::demo_song();
    song.instruments.push(strings_inst());
    let n = song.instruments.len() as u8;
    let mut h = harness_with(song);
    h.set_size(egui::vec2(1366.0, 768.0));
    h.state_mut().theme = theme;
    let ctx = h.ctx.clone();
    h.state_mut().apply_theme(&ctx);
    h.state_mut().instr = n;
    h.state_mut().show_instrument_panel();
    frames(&mut h, 4);
    h
}

fn sp(h: &Harness<'static, App>) -> crate::strings::StringsP {
    let i = h.state().instr as usize - 1;
    h.state().with(|s| match &s.song.instruments[i].kind {
        InstrumentKind::Gen(g) => match &g.engine {
            crate::gen::GenEngine::Strings(x) => x.clone(),
            _ => panic!("not StringWave"),
        },
        _ => panic!("not StringWave"),
    })
}

fn slider(h: &Harness<'static, App>, label: &str) -> Option<Rect> {
    h.query_all_by(|n| n.role() == Role::Slider && n.label().as_deref() == Some(label))
        .next()
        .map(|n| n.rect())
}

/// the knob keys a sub-tab shows
fn tab_keys(tab: usize) -> Vec<String> {
    super::instpanel::stringsview::tab_knob_keys(tab)
}

/// every knob of every sub-tab is drawn inside the panel's body (no
/// scrolling at 100 %) in Slate, Dark and Amiga; at 150 % text nothing
/// overlaps and the knobs stay in the window's width when scrolled to
#[test]
fn stringwave_panel_fits_a_laptop_in_every_theme() {
    for theme in crate::theme::THEMES {
        let mut h = laptop(theme);
        assert!(!h.state().ip.advanced);
        for text in [0u16, 150] {
            h.state_mut().extra.sw_text = text;
            for tab in 0..3 {
                h.state_mut().ip.sw.tab = tab;
                frames(&mut h, 4);
                let sw = &h.state().ip.sw;
                let body = sw.body.expect("the panel's body");
                let want = tab_keys(tab);
                for k in &want {
                    let r = sw
                        .knobs
                        .iter()
                        .find(|(kk, _)| kk == k)
                        .map(|(_, r)| *r)
                        .unwrap_or_else(|| panic!("{}: tab {tab}: no {k}", theme.name));
                    assert!(r.width() >= 20.0, "{}: {k} too small {r:?}", theme.name);
                    if text == 0 {
                        assert!(
                            body.expand(0.5).contains_rect(r),
                            "{} tab {tab}: {k} {r:?} outside the body {body:?}",
                            theme.name
                        );
                    }
                }
                let rs: Vec<Rect> = sw.knobs.iter().map(|(_, r)| *r).collect();
                for (i, a) in rs.iter().enumerate() {
                    for b in &rs[i + 1..] {
                        assert!(
                            a.intersect(*b).area() < 1.0,
                            "{} tab {tab} text {text}: knobs overlap {a:?} {b:?}",
                            theme.name
                        );
                    }
                }
                assert!(body.bottom() <= 768.5 && body.right() <= 1366.5);
            }
        }
        // the full view: all fourteen sections at once at 100 %
        h.state_mut().extra.sw_text = 0;
        h.state_mut().toggle_sw_full();
        frames(&mut h, 4);
        let sw = &h.state().ip.sw;
        assert!(sw.full);
        let body = sw.body.unwrap();
        for tab in 0..3 {
            for k in tab_keys(tab) {
                let r = sw.knobs.iter().find(|(kk, _)| *kk == k).unwrap().1;
                assert!(
                    body.expand(0.5).contains_rect(r),
                    "{} full view: {k} {r:?} outside {body:?}",
                    theme.name
                );
            }
        }
    }
}

/// the three layer levels have their own names (no three "Level"s); the
/// macro knobs aren't called "… macro" on the panel
#[test]
fn stringwave_layer_levels_have_their_own_names() {
    let h = laptop(crate::theme::SLATE);
    for k in ["Layer 1 Level", "Layer 2 Level", "Layer 3 Level"] {
        assert!(slider(&h, k).is_some(), "no {k}");
    }
    assert!(slider(&h, "Level").is_none());
    assert!(h
        .query_all_by(
            |n| n.role() == Role::Slider && n.label().is_some_and(|l| l.ends_with(" macro"))
        )
        .next()
        .is_none());
}

fn pie_open(h: &Harness<'static, App>) -> bool {
    h.state().ip.sw.pie.is_some()
}

/// choose `label` (then the outer ring's `child`) in StringWave's pie
fn pick(h: &mut Harness<'static, App>, label: &str, child: Option<&str>) {
    let pie = h.state().ip.sw.pie.as_ref().expect("a pie").radial.clone();
    let i = pie
        .items
        .iter()
        .position(|x| x.label == label)
        .unwrap_or_else(|| panic!("no wedge {label}"));
    let w = pie.wedge_centre(i, (super::radial::r_in() + super::radial::r_mid()) / 2.0);
    h.input_mut().events.push(egui::Event::PointerMoved(w));
    frames(h, 2);
    let p = match child {
        Some(c) => {
            let k = pie.items[i]
                .children
                .iter()
                .position(|x| x == c)
                .unwrap_or_else(|| panic!("no {c} in {label}"));
            let p = pie.child_centre(i, k);
            h.input_mut().events.push(egui::Event::PointerMoved(p));
            frames(h, 2);
            p
        }
        None => w,
    };
    click_at(h, p);
    frames(h, 3);
}

/// right-click a knob: its pie; Reset puts it back, MIDI learn waits for
/// a controller, Esc closes the pie
#[test]
fn stringwave_knob_pie_resets_and_learns() {
    let mut h = laptop(crate::theme::DARK);
    let k = crate::strings::LAYER[1] + crate::strings::L_LVL;
    let def = sp(&h).p[k];
    let i = h.state().instr as usize - 1;
    h.state_mut().with(|s| {
        if let InstrumentKind::Gen(g) = &mut s.song.instruments[i].kind {
            if let crate::gen::GenEngine::Strings(x) = &mut g.engine {
                x.p[k] = 0.1;
            }
        }
    });
    frames(&mut h, 2);
    let r = slider(&h, "Layer 2 Level").expect("the knob");
    right_click(&mut h, r.center());
    assert!(pie_open(&h), "the knob's pie");
    {
        let pie = &h.state().ip.sw.pie.as_ref().unwrap().radial;
        let labels: Vec<&str> = pie.items.iter().map(|x| x.label.as_str()).collect();
        for want in ["Reset", "Automate", "MIDI learn", "Modulate", "Layer 2"] {
            assert!(labels.contains(&want), "no {want} in {labels:?}");
        }
        assert!(labels.iter().any(|l| l.ends_with("for dice")), "{labels:?}");
    }
    pick(&mut h, "Reset", None);
    assert!(!pie_open(&h));
    assert!((sp(&h).p[k] - def).abs() < 1e-6, "reset to its default");
    right_click(&mut h, r.center());
    pick(&mut h, "MIDI learn", None);
    assert!(h.state().midi.learn.is_some(), "learning");
    right_click(&mut h, r.center());
    assert!(pie_open(&h));
    key(&mut h, egui::Key::Escape, egui::Modifiers::NONE);
    assert!(!pie_open(&h), "Esc closes it");
}

/// right-click the empty panel: layer quick items, text size, full view;
/// Shift+F11 toggles the full view too
/// an empty spot in the panel's body (away from every knob and the
/// scroll bars)
fn empty_spot(h: &Harness<'static, App>) -> Pos2 {
    let body = h.state().ip.sw.body.unwrap();
    let knobs: Vec<Rect> = h.state().ip.sw.knobs.iter().map(|(_, r)| *r).collect();
    (0..40)
        .flat_map(|i| (0..10).map(move |j| (i, j)))
        .map(|(i, j)| {
            Pos2::new(
                body.right() - 30.0 - i as f32 * 20.0,
                body.bottom() - 24.0 - j as f32 * 10.0,
            )
        })
        .find(|p| !knobs.iter().any(|r| r.expand(40.0).contains(*p)))
        .expect("an empty spot")
}

#[test]
fn stringwave_panel_pie_and_full_view() {
    let mut h = laptop(crate::theme::AMIGA);
    let p = empty_spot(&h);
    right_click(&mut h, p);
    assert!(pie_open(&h), "the panel's pie at {p:?}");
    assert!(h.state().ip.sw.pie.as_ref().unwrap().knob.is_none());
    pick(&mut h, "Text size", Some("150 %"));
    assert_eq!(h.state().extra.sw_text, 150);
    let on = sp(&h).layer_on[0];
    // (the text is bigger now: find a spot again)
    let p = empty_spot(&h);
    right_click(&mut h, p);
    pick(
        &mut h,
        "Layer 1",
        Some(if on { "Turn off" } else { "Turn on" }),
    );
    assert_eq!(sp(&h).layer_on[0], !on);
    assert!(!h.state().ip.sw.full);
    key(&mut h, egui::Key::F11, egui::Modifiers::SHIFT);
    assert!(h.state().ip.sw.full, "Shift+F11: the full view");
    key(&mut h, egui::Key::F11, egui::Modifiers::SHIFT);
    assert!(!h.state().ip.sw.full);
}
