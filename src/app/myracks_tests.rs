//! Save Custom Rack and Browser › My Racks (docs/racks/PLAN.md Stage 4-5):
//! the Save dialog writes a .cwr, a name clash asks (Update existing / Save
//! as new) and never overwrites on its own; My Racks lists, searches, loads
//! onto a track as one undo step; Delete asks first and moves the file to
//! Racks/Deleted; a rack needing a missing plugin warns before loading.
use super::chains_tests::{chains_on, frames};
use super::ui_tests::{click_at, exact};
use super::*;
use crate::browser::Tab;
use crate::racks::tests::{info, song_with_rack};
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use std::path::{Path, PathBuf};

fn has(h: &Harness<'static, App>, label: &str) -> bool {
    h.query_all_by(|n| n.label().as_deref() == Some(label) || n.value().as_deref() == Some(label))
        .next()
        .is_some()
}

fn has_part(h: &Harness<'static, App>, part: &str) -> bool {
    h.query_all_by(|n| {
        n.label().is_some_and(|l| l.contains(part)) || n.value().is_some_and(|l| l.contains(part))
    })
    .next()
    .is_some()
}

/// a fresh Racks folder for this test (thread-local, so tests don't mix)
fn racks_dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("cw-myracks-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    crate::racks::TEST_DIR.with(|t| *t.borrow_mut() = Some(d.clone()));
    d
}

fn cwr(d: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(d)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "cwr"))
        .collect();
    v.sort();
    v
}

fn click(h: &mut Harness<'static, App>, label: &str) {
    let r = exact(h, label);
    click_at(h, r.center());
    frames(h, 3);
}

#[test]
fn save_custom_rack_writes_a_file_and_a_name_clash_asks_first() {
    let d = racks_dir("save");
    let mut h = chains_on(7);
    click(&mut h, "Save rack…");
    assert!(h.state().dev.save_rack.is_some(), "the dialog opened");
    assert!(has(&h, "WHAT IS SAVED"));
    h.state_mut().dev.save_rack.as_mut().unwrap().name = "Glass Keys".into();
    h.state_mut().dev.save_rack.as_mut().unwrap().tags = "bright, glass".into();
    frames(&mut h, 2);
    click(&mut h, "Save rack");
    assert!(h.state().dev.save_rack.is_none(), "closed after saving");
    let files = cwr(&d);
    assert_eq!(files, vec![d.join("Glass Keys.cwr")]);
    let r = crate::racks::load(&files[0]).unwrap();
    assert_eq!(r.info.tags, vec!["bright".to_string(), "glass".to_string()]);
    assert!(
        matches!(r.instruments[0].kind, InstrumentKind::Rack(_)),
        "a plain instrument saves as a 1-layer rack"
    );
    let bytes0 = std::fs::read(&files[0]).unwrap();
    // same name again: it asks, and nothing is written until you choose
    click(&mut h, "Save rack…");
    h.state_mut().dev.save_rack.as_mut().unwrap().name = "Glass Keys".into();
    frames(&mut h, 2);
    assert!(!has(&h, "Save rack"), "no plain Save on a clash");
    assert!(has(&h, "Update existing") && has(&h, "Save as new"));
    assert_eq!(std::fs::read(&files[0]).unwrap(), bytes0, "not overwritten");
    click(&mut h, "Save as new");
    assert_eq!(cwr(&d).len(), 2, "saved beside it");
    // Update existing keeps a backup
    click(&mut h, "Save rack…");
    h.state_mut().dev.save_rack.as_mut().unwrap().name = "Glass Keys".into();
    frames(&mut h, 2);
    click(&mut h, "Update existing");
    assert_eq!(cwr(&d).len(), 2);
    assert_eq!(std::fs::read_dir(d.join(".backup")).unwrap().count(), 1);
    // Cancel / Esc write nothing
    click(&mut h, "Save rack…");
    h.state_mut().dev.save_rack.as_mut().unwrap().name = "Not Saved".into();
    frames(&mut h, 2);
    click(&mut h, "Cancel");
    assert!(h.state().dev.save_rack.is_none());
    assert_eq!(cwr(&d).len(), 2);
    std::fs::remove_dir_all(&d).ok();
}

/// a demo song with the browser open on My Racks
fn my_racks() -> Harness<'static, App> {
    let mut h = chains_on(1);
    h.state_mut().extra.browser_open = true;
    h.state_mut().br.filter.tab = Tab::Racks;
    h.state_mut().dev.racks_dirty = true;
    frames(&mut h, 4);
    h
}

fn save_rack(d: &Path, name: &str, song: &crate::model::Song) -> PathBuf {
    let r = crate::racks::from_song(song, 0, info(name)).unwrap();
    let p = d.join(format!("{name}.cwr"));
    crate::racks::save(&r, &p).unwrap();
    p
}

#[test]
fn my_racks_lists_searches_and_loads_onto_the_track_as_one_undo_step() {
    let d = racks_dir("load");
    save_rack(&d, "Glass Keys", &song_with_rack());
    save_rack(&d, "Reese Wall", &song_with_rack());
    let mut h = my_racks();
    assert!(Tab::ALL.contains(&Tab::Racks) && Tab::Racks.name() == "My Racks");
    assert!(has(&h, "Glass Keys") && has(&h, "Reese Wall"));
    assert!(has_part(&h, "MY RACKS · 2"));
    // search
    h.state_mut().br.filter.query = "reese".into();
    frames(&mut h, 2);
    assert!(!has(&h, "Glass Keys") && has(&h, "Reese Wall"));
    h.state_mut().br.filter.query = "zzz".into();
    frames(&mut h, 2);
    assert!(has(&h, "nothing matches the search"));
    h.state_mut().br.filter.query.clear();
    frames(&mut h, 2);
    // select, then Load onto track
    let n0 = h.state().with(|s| s.song.instruments.len());
    click(&mut h, "Glass Keys");
    let ch = h.state().ch;
    click(&mut h, &format!("Load onto track {:02}", ch + 1));
    let n1 = h.state().with(|s| s.song.instruments.len());
    assert_eq!(n1, n0 + 3, "the rack and its 2 layers, as new slots");
    assert_eq!(h.state().instr as usize, n0 + 1, "the rack is selected");
    assert!(h
        .state()
        .with(|s| matches!(s.song.instruments[n0].kind, InstrumentKind::Rack(_))));
    h.state_mut().undo(false);
    frames(&mut h, 2);
    assert_eq!(
        h.state().with(|s| s.song.instruments.len()),
        n0,
        "one undo step"
    );
    // Add as new instrument: added, given to no track
    click(&mut h, "Glass Keys");
    click(&mut h, "Add as new instrument");
    assert_eq!(h.state().with(|s| s.song.instruments.len()), n0 + 3);
    std::fs::remove_dir_all(&d).ok();
}

/// a context-menu item (the last widget with that label: the menu is on top)
fn menu(h: &mut Harness<'static, App>, label: &str) {
    let r = h
        .query_all_by(|n| n.label().as_deref() == Some(label))
        .last()
        .unwrap()
        .rect();
    click_at(h, r.center());
    frames(h, 3);
}

fn right_click(h: &mut Harness<'static, App>, label: &str) {
    let r = exact(h, label);
    h.input_mut()
        .events
        .push(egui::Event::PointerMoved(r.center()));
    frames(h, 1);
    for pressed in [true, false] {
        h.input_mut().events.push(egui::Event::PointerButton {
            pos: r.center(),
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: Default::default(),
        });
        frames(h, 1);
    }
    frames(h, 2);
}

#[test]
fn my_racks_delete_asks_first_and_moves_the_file_to_deleted() {
    let d = racks_dir("delete");
    let p = save_rack(&d, "Glass Keys", &song_with_rack());
    let mut h = my_racks();
    right_click(&mut h, "Glass Keys");
    for item in [
        "Rename",
        "Duplicate",
        "Update from track's sound",
        "Edit description and tags…",
        "Show in folder",
        "Delete…",
    ] {
        assert!(has(&h, item), "context menu has {item}");
    }
    menu(&mut h, "Delete…");
    assert!(p.exists(), "nothing happens before the confirm");
    assert!(h.state().dev.my_racks.confirm_delete.is_some(), "asked");
    assert!(has_part(&h, "Move \"Glass Keys\" to Racks/Deleted?"));
    click(&mut h, "Cancel");
    assert!(p.exists());
    right_click(&mut h, "Glass Keys");
    menu(&mut h, "Delete…");
    click(&mut h, "Delete");
    assert!(!p.exists());
    assert!(d.join("Deleted").join("Glass Keys.cwr").exists());
    frames(&mut h, 2);
    assert!(!has(&h, "Glass Keys"), "out of the list");
    // Duplicate and Rename from the menu
    let p = save_rack(&d, "Reese Wall", &song_with_rack());
    h.state_mut().dev.racks_dirty = true;
    frames(&mut h, 2);
    right_click(&mut h, "Reese Wall");
    menu(&mut h, "Duplicate");
    assert!(d.join("Reese Wall copy.cwr").exists());
    assert!(p.exists());
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn a_rack_needing_a_missing_plugin_warns_and_can_load_anyway() {
    let d = racks_dir("missing");
    let mut s = song_with_rack();
    let mut dev = crate::fx::Device::new(crate::fx::FxKind::Plugin);
    dev.plugin = Some(Box::new(crate::plugin::PluginPatch {
        format: "clap".into(),
        id: "com.example.not-installed-verb".into(),
        name: "Example Verb".into(),
        vendor: "Example".into(),
        path: String::new(),
        state: String::new(),
        params: vec![],
        macros: vec![],
    }));
    s.instruments[2].fx.push(dev);
    save_rack(&d, "With Plugin", &s);
    let mut h = my_racks();
    assert!(has_part(
        &h,
        "Needs plugins not installed here: Example Verb (CLAP)"
    ));
    let n0 = h.state().with(|s| s.song.instruments.len());
    click(&mut h, "With Plugin");
    let ch = h.state().ch;
    click(&mut h, &format!("Load onto track {:02}", ch + 1));
    assert!(has_part(&h, "This rack needs plugins"), "it warns first");
    assert_eq!(
        h.state().with(|s| s.song.instruments.len()),
        n0,
        "nothing loaded yet"
    );
    assert!(has(&h, "Find plugins…"));
    click(&mut h, "Load anyway");
    assert_eq!(h.state().with(|s| s.song.instruments.len()), n0 + 3);
    std::fs::remove_dir_all(&d).ok();
}
