//! Side panels must stay as narrow as the user drags them: no content may
//! force the Browser (every tab), the Inspector or the lower panel wider
//! than the drag (Del's "it bounces back out to a wider panel", 0.2.4.1)
use super::ui_tests::{frame, harness_with};
use super::*;
use crate::browser::Tab;
use egui_kittest::Harness;

fn frames(h: &mut Harness<'static, App>, n: usize) {
    for _ in 0..n {
        frame(h);
    }
}

fn ev(h: &mut Harness<'static, App>, e: egui::Event) {
    h.input_mut().events.push(e);
    frame(h);
}

/// press at `from`, move to `to` over several frames, release
fn slow_drag(h: &mut Harness<'static, App>, from: Pos2, to: Pos2) {
    ev(h, egui::Event::PointerMoved(from));
    frames(h, 2);
    ev(
        h,
        egui::Event::PointerButton {
            pos: from,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        },
    );
    for k in 1..=12 {
        ev(
            h,
            egui::Event::PointerMoved(from + (to - from) * (k as f32 / 12.0)),
        );
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
    // move away so nothing hovers the edge any more
    ev(h, egui::Event::PointerMoved(Pos2::new(400.0, 500.0)));
    frames(h, 3);
}

/// a deep folder with long names, for the Files path bar and rows
fn long_dir() -> std::path::PathBuf {
    let mut d = std::env::temp_dir().join(format!("cw-narrow-{}", std::process::id()));
    for part in [
        "A rather long folder name for the breadcrumb bar",
        "Another very long sub folder with samples and songs",
        "Psytrance kicks and basslines, 145 BPM, part two",
    ] {
        d = d.join(part);
    }
    std::fs::create_dir_all(&d).unwrap();
    for f in [
        "An extremely long sample file name that goes on and on and on.wav",
        "Another_long_song_name_without_any_spaces_at_all_to_break_it_up.cwt",
    ] {
        std::fs::write(d.join(f), b"").unwrap();
    }
    d
}

fn browser_harness() -> Harness<'static, App> {
    let mut s = fileio::demo_song();
    let (tl, _) = crate::timeline::from_order(&s);
    s.timeline = Some(Box::new(tl));
    // a long track name, shown as "Adds to: <track>" on the Effects tab
    if let Some(tl) = s.timeline.as_mut() {
        if let Some(t) = tl.tracks.get_mut(1) {
            t.name = "A really long track name that never seems to end at all".into();
        }
    }
    let mut h = harness_with(s);
    h.state_mut().view = View::Timeline;
    h.state_mut().extra.browser_open = true;
    frames(&mut h, 4);
    h
}

fn browser_w(h: &Harness<'static, App>) -> f32 {
    // br.rect is the panel's inner rect grown by its 6 px side margins
    h.state().br.rect.width()
}

#[test]
fn browser_stays_as_narrow_as_dragged_in_every_tab() {
    let dir = long_dir();
    let mut h = browser_harness();
    let mut bad = vec![];
    for tab in Tab::ALL {
        h.state_mut().br.filter.tab = tab;
        if tab == Tab::Files {
            h.state_mut().files_go(dir.clone(), false);
        }
        frames(&mut h, 6);
        // start wide, then drag the left edge in to about 230 px
        let r = h.state().br.rect;
        let target = 230.0;
        let from = Pos2::new(r.left() + 1.0, r.center().y);
        let to = Pos2::new(r.right() - target, r.center().y);
        slow_drag(&mut h, from, to);
        frames(&mut h, 10);
        let w = browser_w(&h);
        let saved = h.state().extra.browser_w;
        let want = h.state().br.want_w;
        eprintln!("{}: panel {w:.0}, content {want:.0}", tab.name());
        if want > w - 6.0 {
            bad.push(format!(
                "{}: content {want:.0} px is wider than the panel",
                tab.name()
            ));
        }
        if (w - target).abs() > 8.0 || (saved - w).abs() > 8.0 {
            bad.push(format!("{}: {w:.0} px (saved {saved:.0})", tab.name()));
        }
        // and back out wide for the next tab
        let r = h.state().br.rect;
        slow_drag(
            &mut h,
            Pos2::new(r.left() + 1.0, r.center().y),
            Pos2::new(r.right() - 420.0, r.center().y),
        );
    }
    let _ = std::fs::remove_dir_all(
        std::env::temp_dir().join(format!("cw-narrow-{}", std::process::id())),
    );
    assert!(bad.is_empty(), "the Browser bounced back wider: {bad:?}");
}

#[test]
fn browser_clamps_at_its_minimum_and_keeps_the_width_after_reopening() {
    let mut h = browser_harness();
    // Drum Patterns inside a style and a long search: long crumbs and rows
    h.state_mut().br.filter.tab = Tab::Patterns;
    h.state_mut().br.pat.folder = Some(0);
    frames(&mut h, 4);
    let r = h.state().br.rect;
    // far past the minimum: stops at BROWSER_MIN_W (the whole panel)
    slow_drag(
        &mut h,
        Pos2::new(r.left() + 1.0, r.center().y),
        Pos2::new(r.right() - 120.0, r.center().y),
    );
    frames(&mut h, 10);
    let w = browser_w(&h);
    assert!(
        (w - super::browserpanel::BROWSER_MIN_W).abs() < 3.0,
        "the minimum is about 220 px: {w}"
    );
    assert!(h.state().br.want_w <= w, "content fits at the minimum");
    h.state_mut().br.filter.query = "a very long search that matches nothing at all".into();
    frames(&mut h, 6);
    assert!((browser_w(&h) - w).abs() < 1.0);
    assert!(h.state().br.want_w <= w);
    // the width is saved, and a hidden-then-shown Browser comes back at it
    let saved = h.state().extra.browser_w;
    assert!((saved - w).abs() < 2.0, "saved {saved}, shown {w}");
    h.state_mut().extra.browser_open = false;
    frames(&mut h, 3);
    h.state_mut().extra.browser_open = true;
    frames(&mut h, 6);
    assert!(
        (browser_w(&h) - w).abs() < 2.0,
        "reopened at {}",
        browser_w(&h)
    );
}

#[test]
fn a_saved_narrow_width_is_used_on_start() {
    let mut s = fileio::demo_song();
    let (tl, _) = crate::timeline::from_order(&s);
    s.timeline = Some(Box::new(tl));
    let mut h = harness_with(s);
    h.state_mut().view = View::Timeline;
    h.state_mut().extra.browser_w = 236.0;
    h.state_mut().extra.browser_open = true;
    for tab in Tab::ALL {
        h.state_mut().br.filter.tab = tab;
        frames(&mut h, 5);
        let w = browser_w(&h);
        assert!((w - 236.0).abs() < 3.0, "{}: {w}", tab.name());
        assert!((h.state().extra.browser_w - 236.0).abs() < 3.0);
    }
}

#[test]
fn inspector_keeps_its_width_with_long_names() {
    let mut h = browser_harness();
    h.state_mut().extra.inspector_open = true;
    frames(&mut h, 4);
    let w0 = h.state().insp.rect.expect("Inspector drawn").width();
    for ch in [0, 1, 2] {
        h.state_mut().ch = ch;
        frames(&mut h, 6);
        let w = h.state().insp.rect.unwrap().width();
        assert!((w - w0).abs() < 1.0, "track {ch}: {w} vs {w0}");
    }
    // and with a clip selected
    let clip = h.state().with(|s| {
        s.song
            .timeline
            .as_ref()
            .and_then(|t| t.clips.first().map(|c| c.id))
    });
    h.state_mut().tlv.sel = clip;
    frames(&mut h, 6);
    let w = h.state().insp.rect.unwrap().width();
    assert!((w - w0).abs() < 1.0, "clip: {w} vs {w0}");
}

#[test]
fn lower_panel_splitter_stays_where_it_is_dropped() {
    let mut h = harness_with(fileio::demo_song());
    h.state_mut().extra.browser_open = true;
    frames(&mut h, 4);
    let sp = h.state().ip.split.expect("splitter drawn");
    let from = sp.center();
    let to = from - egui::vec2(0.0, 120.0);
    slow_drag(&mut h, from, to);
    frames(&mut h, 10);
    let lh = h.state().extra.lower_h;
    let after = h.state().ip.split.unwrap().center().y;
    assert!(
        (after - to.y).abs() < 6.0,
        "splitter at {after}, dropped at {}",
        to.y
    );
    frames(&mut h, 10);
    assert_eq!(h.state().extra.lower_h, lh, "no creep");
    assert!((h.state().ip.split.unwrap().center().y - after).abs() < 0.5);
}
