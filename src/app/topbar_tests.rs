//! The top bar's three movable panels (app/tbpanels.rs): the middle one
//! exactly on the window's middle at every width, drag to reorder, the
//! saved order, Reset top bar, the compact display when the full one does
//! not fit, and nothing overlapping at 1366 × 768. "Exactly" centred is
//! to within half a pixel (egui snaps the panel's edges).
use super::tbpanels::{self as tp, Part};
use super::ui_tests::{click_at, exact, frame, harness_with};
use super::*;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;

fn frames(h: &mut Harness<'static, App>, n: usize) {
    for _ in 0..n {
        frame(h);
    }
}

fn at(w: f32, view: View) -> Harness<'static, App> {
    let mut h = harness_with(fileio::demo_song());
    h.state_mut().extra.reduce_motion = true;
    h.state_mut().view = view;
    h.set_size(egui::vec2(w, 768.0));
    frames(&mut h, 6);
    h
}

fn ev(h: &mut Harness<'static, App>, e: egui::Event) {
    h.input_mut().events.push(e);
    frame(h);
}

fn button(h: &mut Harness<'static, App>, pos: Pos2, pressed: bool) {
    ev(
        h,
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        },
    );
}

/// press on slot `from`'s grip, move to x, release
fn drag_grip(h: &mut Harness<'static, App>, from: usize, to_x: f32) {
    let r = h.state().tb.rects[from];
    let p = Pos2::new(r.left() - tp::GRIP / 2.0, r.center().y);
    ev(h, egui::Event::PointerMoved(p));
    button(h, p, true);
    let to = Pos2::new(to_x, p.y);
    for k in 1..=8 {
        ev(
            h,
            egui::Event::PointerMoved(p + (to - p) * (k as f32 / 8.0)),
        );
    }
    button(h, to, false);
    ev(h, egui::Event::PointerMoved(Pos2::new(600.0, 500.0)));
    frames(h, 3);
}

fn order(h: &Harness<'static, App>) -> [Part; 3] {
    tp::order_of(&h.state().extra.topbar)
}

#[test]
fn the_slots_put_the_middle_panel_on_the_window_middle() {
    for w in [1366.0f32, 1440.0, 1600.0, 1920.0, 2560.0] {
        let mid = w / 2.0;
        for ws in [
            [523.0, 230.0, 150.0],
            [150.0, 523.0, 230.0],
            [230.0, 0.0, 646.0],
        ] {
            let x = tp::slots(0.0, w, mid, ws);
            assert_eq!(x[1] + ws[1] / 2.0, mid, "{w} {ws:?}");
            assert_eq!(x[0], tp::EDGE + tp::GRIP);
            assert_eq!(x[2] + ws[2], w - tp::EDGE);
        }
    }
}

#[test]
fn the_middle_panel_is_exactly_centred_at_every_width() {
    for (w, view) in [
        (1366.0, View::Timeline),
        (1366.0, View::Pattern),
        (1600.0, View::Timeline),
        (1920.0, View::Timeline),
        (1920.0, View::Pattern),
        (2560.0, View::Mixer),
    ] {
        let h = at(w, view);
        let mid = h.ctx.content_rect().center().x;
        assert_eq!(mid, w / 2.0);
        let rs = h.state().tb.rects;
        assert!(
            (rs[1].center().x - mid).abs() <= 0.5,
            "{w} {view:?}: transport {:?} centre {} vs {mid}",
            rs[1],
            rs[1].center().x
        );
        assert!(
            (rs[0].left() - (tp::EDGE + tp::GRIP)).abs() < 0.01,
            "{rs:?}"
        );
        assert!((rs[2].right() - (w - tp::EDGE)).abs() < 0.01, "{rs:?}");
    }
}

#[test]
fn reorder_moves_one_panel_into_a_gap() {
    use Part::*;
    let d = tp::DEFAULT;
    assert_eq!(d, [Display, Transport, Master]);
    // a drop where it already is changes nothing
    for i in 0..3 {
        assert_eq!(tp::reorder(d, i, i), d);
        assert_eq!(tp::reorder(d, i, i + 1), d);
    }
    assert_eq!(tp::reorder(d, 0, 2), [Transport, Display, Master]);
    assert_eq!(tp::reorder(d, 0, 3), [Transport, Master, Display]);
    assert_eq!(tp::reorder(d, 2, 0), [Master, Display, Transport]);
    assert_eq!(tp::reorder(d, 2, 1), [Display, Master, Transport]);
    assert_eq!(tp::reorder(d, 1, 0), [Transport, Display, Master]);
    assert_eq!(tp::reorder(d, 1, 3), [Display, Master, Transport]);
    let c = [100.0, 683.0, 1200.0];
    assert_eq!(tp::drop_gap(c, 50.0), 0);
    assert_eq!(tp::drop_gap(c, 400.0), 1);
    assert_eq!(tp::drop_gap(c, 900.0), 2);
    assert_eq!(tp::drop_gap(c, 1300.0), 3);
}

#[test]
fn dragging_a_grip_reorders_and_the_new_middle_panel_is_centred() {
    let mut h = at(1366.0, View::Timeline);
    assert_eq!(order(&h), tp::DEFAULT);
    // the Display past the Transport
    let x = h.state().tb.rects[1].right() + 20.0;
    drag_grip(&mut h, 0, x);
    assert_eq!(order(&h), [Part::Transport, Part::Display, Part::Master]);
    assert_eq!(h.state().extra.topbar, ["transport", "display", "master"]);
    assert!(
        h.state()
            .status
            .starts_with("Top bar: Transport · Display · Master"),
        "{}",
        h.state().status
    );
    assert!(h.state().tb.drag.is_none());
    frames(&mut h, 3);
    let r = h.state().tb.rects[1];
    assert!((r.center().x - 683.0).abs() <= 0.5, "{r:?}");
    assert_eq!(r, h.state().lcd.rect, "the display is in the middle slot");
    // the Master to the far left
    let x = h.state().tb.rects[0].left() - 4.0;
    drag_grip(&mut h, 2, x);
    assert_eq!(order(&h), [Part::Master, Part::Transport, Part::Display]);
    // a drop back where it started changes nothing
    let before = h.state().extra.topbar.clone();
    let x = h.state().tb.rects[1].center().x + 5.0;
    drag_grip(&mut h, 1, x);
    assert_eq!(h.state().extra.topbar, before);
}

#[test]
fn the_order_saves_and_reads_back() {
    use Part::*;
    let perms = [
        [Display, Transport, Master],
        [Display, Master, Transport],
        [Transport, Display, Master],
        [Transport, Master, Display],
        [Master, Display, Transport],
        [Master, Transport, Display],
    ];
    for o in perms {
        assert_eq!(tp::order_of(&tp::keys_of(o)), o);
        let mut s = crate::settings::Settings::default();
        s.extra.topbar = tp::keys_of(o);
        let t = toml::to_string(&s).unwrap();
        let back: crate::settings::Settings = toml::from_str(&t).unwrap();
        assert_eq!(tp::order_of(&back.extra.topbar), o);
        if o == tp::DEFAULT {
            // an older file without the key reads as the default
            let old: String = t
                .lines()
                .filter(|l| !l.starts_with("topbar"))
                .collect::<Vec<_>>()
                .join("\n");
            assert!(!old.contains("topbar"));
            let back: crate::settings::Settings = toml::from_str(&old).unwrap();
            assert_eq!(tp::order_of(&back.extra.topbar), tp::DEFAULT);
        }
    }
    // the default saves as nothing; a file without the key is the default
    assert!(tp::keys_of(tp::DEFAULT).is_empty());
    for bad in [
        vec!["master"],
        vec!["master", "master", "display"],
        vec!["display", "transport", "nonsense"],
        vec!["display", "transport", "master", "master"],
    ] {
        let v: Vec<String> = bad.iter().map(|s| s.to_string()).collect();
        assert_eq!(tp::order_of(&v), tp::DEFAULT, "{bad:?}");
    }
}

#[test]
fn reset_top_bar_in_settings_look_puts_the_default_back() {
    let mut h = at(1700.0, View::Settings);
    h.set_size(egui::vec2(1700.0, 1000.0));
    h.state_mut().set_card = 1;
    h.state_mut().set_open = Some(1);
    h.state_mut().extra.topbar = tp::keys_of([Part::Master, Part::Display, Part::Transport]);
    frames(&mut h, 6);
    assert!(exact(&h, "Master · Display · Transport").width() > 0.0);
    let r = exact(&h, "Reset top bar");
    click_at(&mut h, r.center());
    frames(&mut h, 2);
    assert!(h.state().extra.topbar.is_empty());
    assert_eq!(order(&h), tp::DEFAULT);
    assert_eq!(
        h.state().status,
        "Top bar reset: Display · Transport · Master"
    );
    assert!(exact(&h, "Display · Transport · Master").width() > 0.0);
}

#[test]
fn the_display_goes_compact_only_when_the_full_one_does_not_fit() {
    let h = at(1920.0, View::Timeline);
    assert!(!h.state().lcd.narrow);
    let full = h.state().lcd.rect.width();
    let master_full = h.state().lcd.master_rect.width();
    let h = at(1366.0, View::Timeline);
    assert!(h.state().lcd.narrow);
    assert!(h.state().lcd.rect.width() < full);
    assert!(h.state().lcd.master_rect.width() < master_full);
    assert!(tp::fits(0.0, 1920.0, 960.0, [646.0, 230.0, 176.0]));
    assert!(!tp::fits(0.0, 1366.0, 683.0, [646.0, 230.0, 176.0]));
    assert!(tp::fits(0.0, 1366.0, 683.0, [523.0, 230.0, 150.0]));
}

fn no_overlap(h: &Harness<'static, App>, what: &str) {
    let win = h.ctx.content_rect();
    let mut rs: Vec<egui::Rect> = h
        .state()
        .tb
        .rects
        .iter()
        .copied()
        .filter(|r| r.is_positive())
        .collect();
    let ex = h.state().tb.extras;
    if ex.is_positive() {
        rs.push(ex);
    }
    for (i, a) in rs.iter().enumerate() {
        assert!(
            a.left() >= win.left() && a.right() <= win.right(),
            "{what}: {a:?} outside {win:?}"
        );
        for b in &rs[i + 1..] {
            assert!(!a.intersects(*b), "{what}: {a:?} overlaps {b:?} in {rs:?}");
        }
    }
    // the menu bar: the last menu and the status cluster
    let row: Vec<(&str, egui::Rect)> = ["Help", "Chord", "Keys → Notes", "Panic"]
        .iter()
        .map(|l| (*l, exact(h, l)))
        .collect();
    for (i, (na, a)) in row.iter().enumerate() {
        assert!(
            a.right() <= win.right() && a.top() < 30.0,
            "{what}: {na} {a:?}"
        );
        for (nb, b) in &row[i + 1..] {
            assert!(!a.intersects(*b), "{what}: {na} {a:?} overlaps {nb} {b:?}");
        }
    }
}

#[test]
fn nothing_overlaps_at_1366_in_any_view_or_toolbar() {
    for lcd in [true, false] {
        for view_bar in [true, false] {
            for view in [View::Timeline, View::Pattern, View::Mixer, View::Fx] {
                for o in [tp::DEFAULT, [Part::Master, Part::Display, Part::Transport]] {
                    let mut h = at(1366.0, view);
                    h.state_mut().extra.lcd_toolbar = lcd;
                    h.state_mut().extra.view_bar = view_bar;
                    h.state_mut().extra.topbar = tp::keys_of(o);
                    frames(&mut h, 6);
                    let what = format!("lcd {lcd} view bar {view_bar} {view:?} {o:?}");
                    no_overlap(&h, &what);
                    let r = h.state().tb.rects[1];
                    assert!(
                        (r.center().x - 683.0).abs() <= 0.5,
                        "{what}: {:.3}..{:.3} w {:?}",
                        r.left(),
                        r.right(),
                        h.state().tb.w
                    );
                }
            }
        }
    }
}

#[test]
fn every_view_keeps_its_controls() {
    // the Tracker's Octave and Edit / Step
    let mut h = at(1366.0, View::Pattern);
    assert!(
        h.state().tb.extras.is_positive(),
        "the Tracker's extras are drawn"
    );
    for l in [" − ", " + "] {
        assert!(exact(&h, l).width() > 0.0, "octave {l:?}");
    }
    assert!(h
        .query_all_by(|n| n.label().is_some_and(|l| l.starts_with("Edit mode")))
        .next()
        .is_some());
    assert!(exact(&h, "Step").width() > 0.0);
    // classic: BPM and Speed in the Display panel; the view switch while
    // the view tab bar is hidden
    h.state_mut().extra.lcd_toolbar = false;
    h.state_mut().extra.view_bar = false;
    h.state_mut().view = View::Timeline;
    frames(&mut h, 6);
    assert!(exact(&h, "BPM").width() > 0.0);
    assert!(exact(&h, "Speed").width() > 0.0);
    assert!(exact(&h, "Browser").width() > 0.0);
    assert!(h.state().tb.extras.is_positive());
    no_overlap(&h, "classic, view switch");
}

fn starts(h: &Harness<'static, App>, prefix: &str) -> egui::Rect {
    h.query_all_by(|n| n.label().is_some_and(|l| l.starts_with(prefix)))
        .next()
        .unwrap_or_else(|| panic!("no widget starting {prefix:?}"))
        .rect()
}

/// timeline mode (where Record, Loop and the Metronome live, as on main)
fn tl_at(w: f32, view: View) -> Harness<'static, App> {
    let mut h = super::unified_tests::tl_song(fileio::demo_song());
    h.state_mut().extra.reduce_motion = true;
    h.state_mut().view = view;
    h.set_size(egui::vec2(w, 768.0));
    frames(&mut h, 6);
    h
}

/// the Transport panel has every control main's toolbar had, in main's
/// order (to the start, Play, Record, Loop, Metronome, its ▾), all inside
/// the panel, in every order, toolbar style and width
#[test]
fn the_transport_panel_has_all_its_controls() {
    let ctl = [
        "play marker to the start",
        "Play",
        "Record (R)",
        "Loop (L)",
        "Metronome",
        "Metronome options",
    ];
    for w in [1366.0, 1920.0] {
        for lcd in [true, false] {
            for o in [tp::DEFAULT, [Part::Transport, Part::Master, Part::Display]] {
                let mut h = tl_at(w, View::Timeline);
                h.state_mut().extra.lcd_toolbar = lcd;
                h.state_mut().extra.topbar = tp::keys_of(o);
                frames(&mut h, 6);
                let what = format!("{w} lcd {lcd} {o:?}");
                let slot = o.iter().position(|p| *p == Part::Transport).unwrap();
                let pr = h.state().tb.rects[slot].expand(1.0);
                let mut last = f32::MIN;
                for c in ctl {
                    let r = if c == "Metronome" {
                        exact(&h, c)
                    } else {
                        starts(&h, c)
                    };
                    assert!(
                        pr.contains_rect(r),
                        "{what}: {c} {r:?} outside the panel {pr:?}"
                    );
                    assert!(r.left() > last, "{what}: {c} out of order");
                    last = r.left();
                }
                no_overlap(&h, &what);
            }
        }
    }
    // a song in tracker mode: Record, Loop and the Metronome are timeline
    // features (hidden there on main too); the Tracker adds Play block
    let h = at(1366.0, View::Pattern);
    let pr = h.state().tb.rects[1].expand(1.0);
    for c in ["Play block", "play marker to the start", "Play song"] {
        assert!(pr.contains_rect(starts(&h, c)), "tracker mode: {c}");
    }
}

/// the Display and the Master lost nothing: every readout and control of
/// the display (main's lcd_tests cover what each one does) and the Master
/// meter, inside their panels
#[test]
fn the_display_and_master_panels_have_all_theirs() {
    for (w, view) in [
        (1366.0, View::Timeline),
        (1920.0, View::Timeline),
        (1366.0, View::Pattern),
    ] {
        for o in [tp::DEFAULT, [Part::Master, Part::Display, Part::Transport]] {
            let mut h = tl_at(w, view);
            h.state_mut().extra.topbar = tp::keys_of(o);
            frames(&mut h, 6);
            let what = format!("{w} {view:?} {o:?}");
            let ds = o.iter().position(|p| *p == Part::Display).unwrap();
            let ms = o.iter().position(|p| *p == Part::Master).unwrap();
            let (dr, mr) = (
                h.state().tb.rects[ds].expand(1.0),
                h.state().tb.rects[ms].expand(1.0),
            );
            assert!(dr.contains_rect(h.state().lcd.rect), "{what}: display");
            assert!(
                mr.contains_rect(h.state().lcd.master_rect),
                "{what}: master"
            );
            assert!(mr.contains_rect(exact(&h, "Master meter")), "{what}");
            let d = h.state().lcd.rect.expand(1.0);
            for c in ["Tempo", "TAP", "LOOP", "CLICK", "FOLLOW"] {
                let n: Vec<egui::Rect> = h
                    .query_all_by(|n| n.label().is_some_and(|l| l.starts_with(c)))
                    .map(|n| n.rect())
                    .filter(|r| d.contains_rect(*r))
                    .collect();
                assert!(
                    !n.is_empty(),
                    "{what}: the display's {c} in {d:?}: {:?}",
                    h.query_all_by(|n| n.label().is_some_and(|l| l.starts_with(c)))
                        .map(|n| n.rect())
                        .collect::<Vec<_>>()
                );
            }
        }
    }
}
