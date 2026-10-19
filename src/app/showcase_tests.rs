//! Get showcase instruments dialog (docs/BROWSER.md)
use super::ui_tests::{click_at, frame, harness_with};
use super::*;
use crate::showcase::{self, Progress, Roots, State, ITEMS};
use egui_kittest::{kittest::Queryable, Harness};
use std::sync::atomic::AtomicBool;

fn frames(h: &mut Harness<'static, App>, n: usize) {
    for _ in 0..n {
        frame(h);
    }
}

fn harness() -> Harness<'static, App> {
    let mut h = harness_with(fileio::demo_song());
    h.state_mut().view = View::Timeline;
    h.state_mut().extra.browser_open = true;
    h.state_mut().br.filter.tab = crate::browser::Tab::Plugins;
    // no first-run Find plugins window on top
    h.state_mut().psetup = None;
    frames(&mut h, 4);
    h
}

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("cw-showcase-ui-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

/// a downloader that always fails (no network in tests)
struct Offline;
impl showcase::Fetch for Offline {
    fn fetch(
        &self,
        _: &str,
        _: u64,
        _: &mut dyn std::io::Write,
        _: &mut dyn FnMut(u64),
        _: &AtomicBool,
    ) -> Result<(), String> {
        Err("no connection (test)".into())
    }
}

#[test]
fn plugins_tab_button_opens_the_dialog_with_all_eleven_and_credits() {
    let mut h = harness();
    let c = h.get_by_label("Get showcase instruments…").rect().center();
    click_at(&mut h, c);
    assert!(h.state().showcase.is_some());
    for it in ITEMS {
        h.get_by_label(it.name);
        assert!(h.query_all_by_label(it.creator).next().is_some(), "{}", it.creator);
    }
    // the big one starts unticked, the small auto ones ticked
    let d = h.state().showcase.as_ref().unwrap();
    for (it, on) in ITEMS.iter().zip(&d.ticked) {
        assert_eq!(*on, it.auto() && !it.big, "{}", it.name);
    }
    // installer-only releases: a download page button, never an auto install
    let pages = ITEMS.iter().filter(|i| !i.auto()).count();
    assert_eq!(h.query_all_by_label("Open download page").count(), pages);
    let n = h.query_all_by_label("Close").count();
    assert_eq!(n, 1);
    // let the auto-sized window settle before aiming at its button
    frames(&mut h, 10);
    let c = h.get_by_label("Close").rect().center();
    click_at(&mut h, c);
    assert!(h.state().showcase.is_none());
}

#[test]
fn a_failed_download_shows_a_clear_error_and_installs_nothing() {
    let mut h = harness();
    let base = tmp("fail");
    h.state_mut().showcase = Some(super::showcase::ShowcaseDlg::new(Some(Roots::under(&base))));
    h.state_mut().showcase.as_mut().unwrap().fetch = Arc::new(Offline);
    frames(&mut h, 2);
    let dirs0 = h.state().extra.plugin_dirs.clone();
    h.state_mut().showcase_start();
    for _ in 0..200 {
        frame(&mut h);
        if h.state().showcase.as_ref().unwrap().applied {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    frames(&mut h, 2);
    let d = h.state().showcase.as_ref().unwrap();
    assert!(d.msg.contains("Nothing was installed"), "{}", d.msg);
    let prog = d.job.as_ref().unwrap().progress.lock().unwrap().clone();
    assert!(prog.iter().all(|p| matches!(&p.state, State::Failed(e) if e.contains("no connection"))));
    assert!(h.query_all_by_label_contains("no connection").next().is_some());
    assert_eq!(h.state().extra.plugin_dirs, dirs0);
    assert!(!base.join("VST3").join("Dexed.vst3").exists());
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn a_finished_install_adds_the_folders_and_shows_showcase_in_the_browser() {
    let mut h = harness();
    let base = tmp("done");
    let roots = Roots::under(&base);
    std::fs::create_dir_all(&roots.vst3).unwrap();
    std::fs::create_dir_all(&roots.clap).unwrap();
    let mut dlg = super::showcase::ShowcaseDlg::new(Some(roots.clone()));
    dlg.job = Some(showcase::Job {
        progress: Arc::new(Mutex::new(vec![
            Progress { id: "dexed", state: State::Done, done: 1, total: 1 },
            Progress { id: "adlplug", state: State::Skipped, done: 1, total: 1 },
        ])),
        cancel: Arc::new(AtomicBool::new(false)),
        done: Arc::new(AtomicBool::new(true)),
    });
    h.state_mut().showcase = Some(dlg);
    h.state_mut().br.filter.tab = crate::browser::Tab::Instruments;
    frames(&mut h, 3);
    let st = h.state();
    let d = st.showcase.as_ref().unwrap();
    assert!(d.applied && d.msg.starts_with("1 installed, 1 already there"), "{}", d.msg);
    assert!(st.extra.plugin_dirs.contains(&roots.vst3.display().to_string()));
    assert!(st.extra.plugin_dirs.contains(&roots.clap.display().to_string()));
    assert_eq!(st.br.filter.tab, crate::browser::Tab::Plugins);
    assert_eq!(st.br.filter.tags, vec!["Showcase"]);
    let k = ITEMS.iter().position(|i| i.id == "dexed").unwrap();
    assert!(!d.ticked[k], "installed items are unticked");
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn the_dialog_leaves_an_old_song_untouched() {
    let s0 = fileio::demo_song();
    let mut h = harness_with(s0.clone());
    h.state_mut().open_showcase();
    frames(&mut h, 4);
    h.state_mut().showcase = None;
    frames(&mut h, 2);
    let d = std::env::temp_dir().join(format!("cw-showcase-old-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&d);
    crate::fileio::save_cwt(&s0, &d.join("a.cwt")).unwrap();
    let now = {
        let s = h.state().shared.clone();
        let g = audio::lock(&s);
        g.song.clone()
    };
    crate::fileio::save_cwt(&now, &d.join("b.cwt")).unwrap();
    assert_eq!(std::fs::read(d.join("a.cwt")).unwrap(), std::fs::read(d.join("b.cwt")).unwrap());
    let _ = std::fs::remove_dir_all(&d);
}
