//! Browser top (direction A): category tiles + More, search + Filter
//! popover, active-filter pills, one place per action (docs/BROWSER.md)
use super::browserpanel::BrPop;
use super::ui_tests::{click_at, frame, harness_with, key};
use super::*;
use crate::browser::{LocFilter, Tab};
use egui_kittest::{kittest::Queryable, Harness};

fn frames(h: &mut Harness<'static, App>, n: usize) {
    for _ in 0..n {
        frame(h);
    }
}

fn harness() -> Harness<'static, App> {
    let mut h = harness_with(fileio::demo_song());
    h.state_mut().view = View::Timeline;
    h.state_mut().extra.browser_open = true;
    h.state_mut().psetup = None;
    frames(&mut h, 4);
    h
}

/// click the widget labelled `label` inside the Browser panel
fn click(h: &mut Harness<'static, App>, label: &str) {
    let br = h.state().br.rect;
    let c = h
        .query_all_by_label(label)
        .map(|n| n.rect())
        .find(|r| br.contains(r.center()))
        .unwrap_or_else(|| panic!("no {label} in the Browser"))
        .center();
    click_at(h, c);
    frames(h, 2);
}

/// how many widgets labelled `label` the Browser panel shows
fn has(h: &Harness<'static, App>, label: &str) -> usize {
    let br = h.state().br.rect;
    h.query_all_by_label(label)
        .filter(|n| br.contains(n.rect().center()))
        .count()
}

#[test]
fn the_five_main_tiles_and_more_switch_category() {
    let mut h = harness();
    for tb in Tab::MAIN {
        assert_eq!(has(&h, tb.name()), 1, "{}", tb.name());
    }
    click(&mut h, "Effects");
    assert_eq!(h.state().br.filter.tab, Tab::Effects);
    // the More tile opens the other seven, each with its line
    click(&mut h, "More");
    assert_eq!(h.state().br.pop, BrPop::More);
    for tb in Tab::MORE {
        assert_eq!(has(&h, tb.name()), 1, "{}", tb.name());
    }
    click(&mut h, "Wavetables");
    assert_eq!(h.state().br.filter.tab, Tab::Wavetables);
    assert_eq!(h.state().br.pop, BrPop::None);
    // the More tile now carries the chosen category's name
    assert_eq!(has(&h, "More"), 0);
    assert_eq!(has(&h, "Wavetables"), 1);
    // and a main tile takes it back
    click(&mut h, "Instruments");
    assert_eq!(h.state().br.filter.tab, Tab::Instruments);
    assert_eq!(has(&h, "More"), 1);
}

#[test]
fn filter_popover_sets_filters_shows_the_count_and_pills_remove_them() {
    let mut h = harness();
    h.state_mut().br.filter.tab = Tab::Instruments;
    frames(&mut h, 2);
    assert_eq!(has(&h, "Filter"), 1);
    click(&mut h, "Filter");
    assert_eq!(h.state().br.pop, BrPop::Filter);
    for l in ["Clear all", "+ Add folder…"] {
        assert_eq!(has(&h, l), 1, "{l}");
    }
    click(&mut h, "Bass");
    assert_eq!(h.state().br.filter.tags, vec!["Bass"]);
    assert_eq!(has(&h, "Filter · 1"), 1);
    // Esc closes the popover first (the Browser keeps the keyboard)
    h.state_mut().br.focus = true;
    key(&mut h, egui::Key::Escape, Default::default());
    frames(&mut h, 2);
    assert_eq!(h.state().br.pop, BrPop::None);
    // the pill removes it
    click(&mut h, "Bass  ×");
    assert!(h.state().br.filter.tags.is_empty());
    assert_eq!(has(&h, "Filter"), 1);
}

#[test]
fn favourites_is_a_filter_chip_and_clear_all_resets_every_filter() {
    let mut h = harness();
    h.state_mut().br.filter.tab = Tab::Instruments;
    h.state_mut().br.filter.loc = LocFilter::Favourites;
    h.state_mut().br.filter.tags = vec!["Bass"];
    h.state_mut().br_refresh();
    frames(&mut h, 2);
    assert_eq!(has(&h, "♥ Favourites  ×"), 1);
    assert_eq!(has(&h, "Filter · 2"), 1);
    click(&mut h, "Filter · 2");
    click(&mut h, "Clear all");
    let f = &h.state().br.filter;
    assert!(f.tags.is_empty() && f.loc == LocFilter::All && f.category.is_none());
}

#[test]
fn each_action_lives_in_one_place() {
    let mut h = harness();
    for tb in [Tab::Instruments, Tab::Effects, Tab::Plugins, Tab::Samples] {
        h.state_mut().br.filter.tab = tb;
        frames(&mut h, 2);
        for l in [
            "Find plugins…",
            "Get showcase instruments…",
            "Rescan plugins",
            "Rescan locations",
            "+ Add folder…",
            "Show device chains",
        ] {
            assert_eq!(has(&h, l), 0, "{l} on {}", tb.name());
        }
    }
    // housekeeping is in •••
    click(&mut h, "•••");
    for l in [
        "Rescan locations",
        "Find plugins…",
        "Get showcase instruments…",
        "Rescan plugins",
        "Plugin thumbnails",
    ] {
        assert_eq!(has(&h, l), 1, "{l}");
    }
    assert_eq!(has(&h, "Add folder…"), 0);
}

#[test]
fn switching_category_clears_its_own_filters_but_keeps_the_search() {
    let mut h = harness();
    h.state_mut().br.filter.tab = Tab::Instruments;
    h.state_mut().br.filter.tags = vec!["Bass"];
    h.state_mut().br.filter.query = "wave".into();
    frames(&mut h, 2);
    click(&mut h, "Effects");
    let f = &h.state().br.filter;
    assert_eq!(f.tab, Tab::Effects);
    assert!(f.tags.is_empty());
    assert_eq!(f.query, "wave");
}
