//! The toolbar's LCD display and compact Master (0.2.10): the tempo's TAP,
//! the lamps, the colour setting, the Master popover and the classic
//! toolbar switch.
use super::lcd::{colour_of, colour_setting, tap_bpm, PRESETS};
use super::ui_tests::{click_at, frame, harness_with, key};
use super::*;
use egui::Key;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;

fn frames(h: &mut Harness<'static, App>, n: usize) {
    for _ in 0..n {
        frame(h);
    }
}

fn tl_demo() -> Harness<'static, App> {
    let mut s = fileio::demo_song();
    crate::timeline::enter_timeline_mode(&mut s);
    let mut h = harness_with(s);
    h.state_mut().view = View::Timeline;
    h.state_mut().extra.reduce_motion = true;
    frames(&mut h, 4);
    h
}

fn bpm(h: &Harness<'static, App>) -> f32 {
    h.state().with(|s| s.song.bpm)
}

#[test]
fn tap_tempo_is_the_mean_of_the_gaps() {
    assert_eq!(tap_bpm(&[]), None);
    assert_eq!(tap_bpm(&[1.0]), None);
    assert_eq!(tap_bpm(&[0.0, 0.5, 1.0, 1.5]), Some(120.0));
    assert_eq!(tap_bpm(&[0.0, 0.4, 0.8]), Some(150.0));
    // clamped to the tempo range
    assert_eq!(tap_bpm(&[0.0, 0.01]), Some(300.0));
    assert_eq!(tap_bpm(&[0.0, 1.9, 3.8]), Some(31.578947));
    assert_eq!(tap_bpm(&[0.0, 0.0]), None);
}

#[test]
fn the_colour_setting_is_a_preset_name_or_hex() {
    let mint = colour_of("");
    assert_eq!(mint, colour_of("Mint"));
    assert_eq!(mint, colour_of("nonsense"));
    assert_eq!(mint, colour_of("#12345")); // too short
    for (n, c) in PRESETS {
        let want = Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8);
        assert_eq!(colour_of(n), want, "{n}");
        assert_eq!(colour_of(&n.to_lowercase()), want, "{n}");
    }
    let c = Color32::from_rgb(0x12, 0xAB, 0xFE);
    assert_eq!(colour_setting(c), "#12ABFE");
    assert_eq!(colour_of(&colour_setting(c)), c);
}

#[test]
fn every_backlight_keeps_the_text_readable() {
    use super::lcd::{contrast, pal};
    let mut tints: Vec<Color32> = PRESETS.iter().map(|p| colour_of(p.0)).collect();
    // custom colours, dark and saturated ones too (they are lightened)
    for c in [
        0x000000, 0xFFFFFF, 0x0000FF, 0xFF0000, 0x00FF00, 0x7FE3FF, 0x203040, 0xFFA23A,
    ] {
        tints.push(colour_of(&format!("#{c:06X}")));
    }
    for c in tints {
        let pl = pal(c);
        for (what, fg) in [
            ("digits", pl.lit),
            ("captions", pl.label),
            ("off pills", pl.off_fg),
        ] {
            let k = contrast(fg, pl.bg_bot).min(contrast(fg, pl.bg_top));
            assert!(k >= 4.5, "{what} on {c:?}: {k:.2}");
        }
        // an on pill: the glass colour on the dark fill; REC: white on red
        assert!(contrast(pl.bg_top, pl.lit) >= 4.5, "on pill on {c:?}");
        assert!(contrast(Color32::WHITE, pl.rec) >= 4.5);
    }
    // the default is the mockup's mint, with near-black digits
    let m = pal(colour_of(""));
    assert_eq!(m.bg_top, Color32::from_rgb(0xC9, 0xED, 0xE0));
    assert!(contrast(m.lit, m.bg_bot) >= 10.0);
}

#[test]
fn taps_set_the_tempo_and_the_tap_chip_counts() {
    let mut h = tl_demo();
    let speed = h.state().with(|s| s.song.speed);
    for t in [10.0, 10.5, 11.0, 11.5] {
        h.state_mut().lcd_tap(t);
    }
    assert!((bpm(&h) - 120.0).abs() < 0.01, "{}", bpm(&h));
    assert_eq!(
        h.state().with(|s| s.song.speed),
        speed,
        "TAP keeps the speed"
    );
    // a long pause starts a new count (one tap: no tempo yet)
    let b = bpm(&h);
    h.state_mut().lcd_tap(20.0);
    assert_eq!(h.state().lcd.taps.len(), 1);
    assert_eq!(bpm(&h), b);
    // the chip on the display is the same
    h.state_mut().lcd.taps.clear();
    h.get_by_label("TAP").click();
    frames(&mut h, 2);
    assert_eq!(h.state().lcd.taps.len(), 1);
    h.get_by_label("TAP").click();
    frames(&mut h, 2);
    assert_eq!(h.state().lcd.taps.len(), 2);
}

#[test]
fn the_lamps_switch_loop_click_and_follow() {
    let mut h = tl_demo();
    let looped = |h: &Harness<'static, App>| {
        h.state().with(|s| {
            s.song
                .timeline
                .as_deref()
                .map(|t| t.loop_on)
                .unwrap_or(false)
        })
    };
    // (no loop region yet: LOOP says so, as the header's Loop and L)
    h.get_by_label("LOOP").click();
    frames(&mut h, 2);
    assert!(!looped(&h));
    assert!(
        h.state().status.starts_with("No loop region"),
        "{}",
        h.state().status
    );
    h.state().with(|s| {
        if let Some(t) = s.song.timeline.as_deref_mut() {
            t.loop_range = Some((0, 4 * 4 * 96));
        }
    });
    frames(&mut h, 2);
    let l0 = looped(&h);
    h.get_by_label("LOOP").click();
    frames(&mut h, 2);
    assert_ne!(looped(&h), l0, "LOOP is the Timeline's loop (L)");
    // undoable as the header's Loop
    key(&mut h, Key::Z, egui::Modifiers::COMMAND);
    frames(&mut h, 2);
    assert_eq!(looped(&h), l0);

    let m0 = h.state().extra.metronome;
    h.get_by_label("CLICK").click();
    frames(&mut h, 2);
    assert_ne!(h.state().extra.metronome, m0);

    let f0 = h.state().follow;
    h.get_by_label("FOLLOW").click();
    frames(&mut h, 2);
    assert_ne!(h.state().follow, f0);
}

#[test]
fn the_master_cell_opens_the_master_panel_in_a_popover() {
    let mut h = tl_demo();
    assert!(h.state().lcd_on(), "the LCD toolbar is the default");
    // the Master is not under the Clip Arranger any more
    assert!(!h.state().ca.master.is_positive());
    assert!(!h.state().lcd.master_open);
    h.get_by_label("Master meter").click();
    frames(&mut h, 3);
    assert!(h.state().lcd.master_open);
    assert!(
        h.state().ca.master.is_positive(),
        "the popover draws the Master panel"
    );
    let pop = h.state().lcd.pop_rect;
    assert!(pop.is_positive());
    assert!(
        pop.min.y >= h.state().lcd.master_rect.max.y,
        "below the cell"
    );
    // its controls work there
    let on = h.state().with(|s| s.song.mixer.limiter.enabled);
    let label = if on { "Limiter on" } else { "Limiter off" };
    h.get_by_label(label).click();
    frames(&mut h, 2);
    assert_ne!(h.state().with(|s| s.song.mixer.limiter.enabled), on);
    assert!(h.state().dirty);
    assert!(h.state().lcd.master_open, "a click inside keeps it open");
    // Esc closes it
    key(&mut h, Key::Escape, egui::Modifiers::NONE);
    frames(&mut h, 2);
    assert!(!h.state().lcd.master_open);
    // a click outside closes it too
    h.get_by_label("Master meter").click();
    frames(&mut h, 2);
    assert!(h.state().lcd.master_open);
    let away = Pos2::new(pop.center().x, pop.max.y + 120.0);
    click_at(&mut h, away);
    frames(&mut h, 2);
    assert!(!h.state().lcd.master_open);
}

#[test]
fn the_master_cell_is_in_the_tracker_view_too() {
    let mut h = harness_with(fileio::demo_song());
    h.state_mut().view = View::Pattern;
    frames(&mut h, 3);
    h.get_by_label("Master meter").click();
    frames(&mut h, 3);
    assert!(h.state().lcd.master_open);
    assert!(h.state().ca.master.is_positive());
    // the Tracker's speed is on the display
    assert!(h.query_by_label("Speed").is_some());
}

#[test]
fn the_wheel_over_the_master_cell_nudges_the_volume() {
    let mut h = tl_demo();
    let v0 = h.state().with(|s| s.song.mixer.master);
    let c = h.state().lcd.master_rect.center();
    h.input_mut().events.push(egui::Event::PointerMoved(c));
    frame(&mut h);
    h.input_mut().events.push(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Line,
        delta: egui::vec2(0.0, 1.0),
        modifiers: egui::Modifiers::NONE,
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Wheel,
    });
    frames(&mut h, 2);
    let v1 = h.state().with(|s| s.song.mixer.master);
    assert!(v1 > v0, "{v0} -> {v1}");
}

#[test]
fn a_swatch_sets_the_display_colour() {
    let mut h = tl_demo();
    assert_eq!(h.state().extra.lcd_colour, "");
    // right-click the display: its colour menu
    let r = h.state().lcd.rect;
    h.input_mut().events.push(egui::Event::PointerMoved(
        r.right_top() + egui::vec2(-4.0, 4.0),
    ));
    frame(&mut h);
    let p = r.right_top() + egui::vec2(-4.0, 4.0);
    for pressed in [true, false] {
        h.input_mut().events.push(egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        frame(&mut h);
    }
    frames(&mut h, 2);
    assert!(
        h.query_by_label("Display colour").is_some(),
        "the colour menu"
    );
    h.get_by_label("Amber").click();
    frames(&mut h, 2);
    assert_eq!(h.state().extra.lcd_colour, "Amber");
    assert_eq!(colour_of(&h.state().extra.lcd_colour), colour_of("amber"));
    h.get_by_label("Mint").click();
    frames(&mut h, 2);
    assert_eq!(h.state().extra.lcd_colour, "", "the default saves as empty");
}

#[test]
fn the_classic_toolbar_puts_the_master_back_under_the_clip_arranger() {
    let mut h = tl_demo();
    h.state_mut().extra.lcd_toolbar = false;
    frames(&mut h, 3);
    assert!(h.query_by_label("Master meter").is_none(), "no Master cell");
    assert!(h.query_by_label("TAP").is_none(), "no display");
    assert!(
        h.state().ca.master.is_positive(),
        "the Master panel is back"
    );
    // and the old tempo box
    assert!(h.query_by_label("Metronome").is_some());
}

#[test]
fn the_icon_only_transport_keeps_its_names() {
    let mut h = tl_demo();
    h.state_mut().extra.show_tracker = false;
    frames(&mut h, 2);
    for n in ["Play", "Metronome"] {
        assert!(h.query_by_label(n).is_some(), "{n}");
    }
    h.get_by_label("Play").click();
    frames(&mut h, 2);
    assert!(h.state().with(|s| s.engine.playing));
    assert!(h.query_by_label("Stop").is_some());
    // the LEVEL history steps while playing, with the Master closed
    let n0 = h.state().ca.hist.len();
    frames(&mut h, 90);
    assert!(h.state().ca.hist.len() > n0, "{n0}");
}

#[test]
fn tempo_drag_on_the_display_undoes_in_one_step() {
    let mut h = tl_demo();
    let bpm0 = bpm(&h);
    let t = h.get_by_label("Tempo").rect();
    super::ui_tests::drag(&mut h, t.center(), t.center() - egui::vec2(0.0, 40.0));
    frames(&mut h, 2);
    let bpm1 = bpm(&h);
    assert!(bpm1 > bpm0, "dragged up: {bpm0} -> {bpm1}");
    assert_eq!(
        h.state().with(|s| s.engine.bpm()),
        bpm1,
        "the engine follows"
    );
    key(&mut h, Key::Z, egui::Modifiers::COMMAND);
    frames(&mut h, 2);
    assert_eq!(bpm(&h), bpm0, "one Ctrl+Z");
}

#[test]
fn double_click_types_a_tempo() {
    let mut h = tl_demo();
    let t = h.get_by_label("Tempo").rect();
    let c = t.center();
    for _ in 0..2 {
        for pressed in [true, false] {
            h.input_mut().events.push(egui::Event::PointerButton {
                pos: c,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
        }
        frame(&mut h);
    }
    frames(&mut h, 2);
    assert!(h.state().lcd.bpm_edit.is_some(), "a text field");
    // select all, type, Enter
    key(&mut h, Key::A, egui::Modifiers::COMMAND);
    h.input_mut().events.push(egui::Event::Text("97.5".into()));
    frame(&mut h);
    key(&mut h, Key::Enter, egui::Modifiers::NONE);
    frames(&mut h, 2);
    assert_eq!(bpm(&h), 97.5);
    assert!(h.state().lcd.bpm_edit.is_none());
}
