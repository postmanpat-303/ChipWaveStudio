//! Browser panel, timeline drops / reorder / add track and the Find
//! plugins dialog (docs/BROWSER.md, docs/TIMELINE.md)
use super::ui_tests::{drag, frame, harness_with, key};
use super::*;
use egui::Key;
use egui_kittest::{kittest::Queryable, Harness};

fn frames(h: &mut Harness<'static, App>, n: usize) {
    for _ in 0..n {
        frame(h);
    }
}

fn song<R>(h: &mut Harness<'static, App>, f: impl FnOnce(&Song) -> R) -> R {
    let s = h.state().shared.clone();
    let g = audio::lock(&s);
    f(&g.song)
}

fn ctrl(h: &mut Harness<'static, App>, k: Key) {
    key(h, k, egui::Modifiers::COMMAND);
}

/// the demo song in timeline mode, Timeline view, Browser open
fn tl_harness() -> Harness<'static, App> {
    let mut s = fileio::demo_song();
    let (tl, _) = crate::timeline::from_order(&s);
    s.timeline = Some(Box::new(tl));
    let mut h = harness_with(s);
    h.state_mut().view = View::Timeline;
    h.state_mut().extra.browser_open = true;
    frames(&mut h, 4);
    h
}

/// show `query` on `tab` and return the first result's key and row centre
fn find(h: &mut Harness<'static, App>, tab: crate::browser::Tab, query: &str) -> (String, Pos2) {
    h.state_mut().br.filter.tab = tab;
    h.state_mut().br.filter.query = query.into();
    frames(h, 3);
    let (k, r) = h
        .state()
        .br
        .row_rects
        .first()
        .cloned()
        .unwrap_or_else(|| panic!("no result for {query}"));
    (k, r.center())
}

fn dbl(h: &mut Harness<'static, App>, p: Pos2) {
    super::ui_tests::double_click(h, p);
    frames(h, 3);
}

/// wait for a background load (files decode on a worker thread)
fn settle(h: &mut Harness<'static, App>) {
    for _ in 0..200 {
        if h.state().br.load.is_none() && h.state().br.scan.is_none() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
        frame(h);
    }
    frames(h, 2);
}

#[test]
fn ctrl_b_and_the_toolbar_button_toggle_the_browser() {
    let mut h = harness_with(fileio::demo_song());
    assert!(
        !h.state().extra.browser_open,
        "tests start without the Browser"
    );
    ctrl(&mut h, Key::B);
    frames(&mut h, 2);
    assert!(h.state().extra.browser_open);
    let r = h.state().br.rect;
    assert!(
        r.width() >= 250.0 && r.right() > 1600.0,
        "docked right: {r:?}"
    );
    // the central view got narrower, not covered
    assert!(h.state().br.central.right() <= r.left() + 1.0);
    ctrl(&mut h, Key::B);
    frames(&mut h, 2);
    assert!(!h.state().extra.browser_open);
    // the Browser menu / toolbar toggle share this path
    h.state_mut().toggle_browser();
    frames(&mut h, 2);
    assert!(h.state().extra.browser_open);
}

#[test]
fn search_tags_and_filters_narrow_the_results() {
    let mut h = tl_harness();
    use crate::browser::Tab;
    h.state_mut().br.filter.tab = Tab::Instruments;
    frames(&mut h, 2);
    let all = h.state().br_result_items().len();
    assert!(all >= 10, "instruments: {all}");
    // the rack layout: Instruments lists instruments, not their presets
    assert!(h.state().br_result_items().iter().all(|i| matches!(
        i.src,
        crate::browser::Src::Tool(_) | crate::browser::Src::Plugin(_)
    )));
    h.state_mut().br.filter.query = "808".into();
    frames(&mut h, 2);
    assert!(
        h.state().br_result_items().is_empty(),
        "808 Boom is a preset: it is on the Rack card, not in Instruments"
    );
    h.state_mut().br.filter.query = "fm".into();
    frames(&mut h, 2);
    let names: Vec<String> = h
        .state()
        .br_result_items()
        .iter()
        .map(|i| i.name.clone())
        .collect();
    assert!(names.contains(&"FM-4".to_string()), "{names:?}");
    assert!(names.len() < all);
    h.state_mut().br.filter.query.clear();
    // the song's own instruments: Location "This song"
    h.state_mut().br.filter.tags.clear();
    h.state_mut().br.filter.loc = crate::browser::LocFilter::Song;
    frames(&mut h, 2);
    let n_song = song(&mut h, |s| s.instruments.len());
    assert_eq!(h.state().br_result_items().len(), n_song);
    // every tab shows something built in except samples / MIDI (no folders)
    h.state_mut().br.filter.loc = crate::browser::LocFilter::All;
    for t in [Tab::Presets, Tab::Grid, Tab::Wavetables, Tab::Songs] {
        h.state_mut().br.filter.tab = t;
        frames(&mut h, 2);
        assert!(!h.state().br_result_items().is_empty(), "{t:?}");
    }
}

#[test]
fn double_click_loads_and_enter_loads_the_selection() {
    let mut h = tl_harness();
    let n0 = song(&mut h, |s| s.instruments.len());
    let ch0 = song(&mut h, |s| s.channels);
    let (_, p) = find(&mut h, crate::browser::Tab::Instruments, "FM-4");
    dbl(&mut h, p);
    assert_eq!(song(&mut h, |s| s.instruments.len()), n0 + 1);
    assert_eq!(h.state().instr as usize, n0 + 1);
    assert_eq!(song(&mut h, |s| s.instruments[n0].name.clone()), "E-Piano");
    // double-click makes a new track playing it and opens the instrument panel
    assert_eq!(song(&mut h, |s| s.channels), ch0 + 1);
    assert!(h.state().extra.lower_inst && h.state().lower_visible());
    // keyboard: the Browser has it after a click; Down + Enter loads the next
    super::ui_tests::click_at(&mut h, p);
    frames(&mut h, 2);
    assert!(h.state().br_active());
    h.state_mut().br.filter.query = "bass".into();
    frames(&mut h, 2);
    key(&mut h, Key::ArrowDown, egui::Modifiers::NONE);
    let sel = h.state().br_selected().expect("a selection").name;
    let ch1 = song(&mut h, |s| s.channels);
    let ni = song(&mut h, |s| s.instruments.len());
    key(&mut h, Key::Enter, egui::Modifiers::NONE);
    frames(&mut h, 2);
    // a ChipWave instrument row loads its starting sound on a new track
    assert!(!sel.is_empty());
    assert_eq!(song(&mut h, |s| s.instruments.len()), ni + 1);
    assert_eq!(song(&mut h, |s| s.channels), ch1 + 1);
    // undo takes the load back
    ctrl(&mut h, Key::Z);
    frames(&mut h, 2);
    assert_eq!(song(&mut h, |s| s.instruments.len()), n0 + 1);
}

#[test]
fn favourites_and_ratings_are_saved_in_settings() {
    let mut h = tl_harness();
    let (k, p) = find(&mut h, crate::browser::Tab::Instruments, "FM-4");
    let r = h.state().br.row_rects[0].1;
    // the heart at the right end of the row
    super::ui_tests::click_at(&mut h, Pos2::new(r.right() - 12.0, p.y));
    frames(&mut h, 2);
    assert!(h.state().extra.browser_favs.contains(&k));
    // third star
    super::ui_tests::click_at(
        &mut h,
        Pos2::new(r.right() - 22.0 - 56.0 + 5.0 + 2.0 * 11.0, p.y),
    );
    frames(&mut h, 2);
    assert_eq!(h.state().extra.browser_ratings.get(&k), Some(&3));
    // the Favourites location shows it
    h.state_mut().br.filter.query.clear();
    h.state_mut().br.filter.loc = crate::browser::LocFilter::Favourites;
    frames(&mut h, 2);
    let v: Vec<String> = h
        .state()
        .br_result_items()
        .iter()
        .map(|i| i.key.clone())
        .collect();
    assert_eq!(v, vec![k]);
}

#[test]
fn preview_plays_through_the_current_track_and_stops() {
    let mut h = tl_harness();
    h.state_mut().ch = 2;
    let (_, p) = find(&mut h, crate::browser::Tab::Instruments, "FM-4");
    super::ui_tests::click_at(&mut h, p);
    frames(&mut h, 2);
    // auto-play is on by default
    assert!(h.state().br.preview_active());
    let ok = {
        let s = h.state().shared.clone();
        let g = audio::lock(&s);
        g.preview.as_ref().map(|p| p.channels)
    };
    assert_eq!(ok, Some(1), "a one-channel preview song");
    key(&mut h, Key::Escape, egui::Modifiers::NONE);
    frames(&mut h, 2);
    assert!(!h.state().br.preview_active());
    let s = h.state().shared.clone();
    assert!(audio::lock(&s).preview.is_none());
}

#[test]
fn drop_on_a_track_header_assigns_the_instrument_with_undo() {
    let mut h = tl_harness();
    let before = song(&mut h, |s| s.clone());
    let (_, p) = find(&mut h, crate::browser::Tab::Instruments, "FM-4");
    let (head, _) = h.state().tlv.track_rects(1).expect("track 2 header");
    drag(&mut h, p, head.center());
    frames(&mut h, 2);
    let slot = song(&mut h, |s| s.instruments.len()) as u8;
    assert_eq!(
        song(&mut h, |s| s.instruments.last().unwrap().name.clone()),
        "E-Piano"
    );
    // every note of track 2 now plays it
    let ok = song(&mut h, |s| {
        let t = s.timeline.as_ref().unwrap();
        let mut n = 0;
        for c in t.clips.iter().filter(|c| c.track == 1) {
            if let crate::timeline::ClipKind::Block { pattern, col } = c.kind {
                for r in &s.patterns[pattern].cells {
                    let x = r[col as usize];
                    if (1..=120).contains(&x.note) {
                        assert_eq!(x.instr, slot);
                        n += 1;
                    }
                }
            }
        }
        n
    });
    assert!(ok > 0, "track 2 has notes");
    assert_eq!(
        song(&mut h, |s| s.channels),
        before.channels,
        "no new track"
    );
    // one undo step takes it all back
    ctrl(&mut h, Key::Z);
    frames(&mut h, 2);
    assert_eq!(
        song(&mut h, |s| s.instruments.len()),
        before.instruments.len()
    );
    assert_eq!(song(&mut h, |s| s.patterns.clone()), before.patterns);
}

#[test]
fn drop_below_the_last_track_makes_a_named_track() {
    let mut h = tl_harness();
    h.state_mut().set_channels(4);
    frames(&mut h, 3);
    let n0 = song(&mut h, |s| s.channels);
    let (_, p) = find(&mut h, crate::browser::Tab::Instruments, "FM-4");
    let y = h.state().tlv.tracks_bottom() + 30.0;
    let x = h.state().tlv.heads.center().x;
    drag(&mut h, p, Pos2::new(x, y));
    frames(&mut h, 2);
    assert_eq!(song(&mut h, |s| s.channels), n0 + 1);
    assert_eq!(song(&mut h, |s| s.channel_names[n0].clone()), "FM-4");
    let slot = song(&mut h, |s| s.instruments.len()) as u8;
    assert_eq!(
        song(&mut h, |s| s.timeline.as_ref().unwrap().tracks[n0].instr),
        slot
    );
    assert_eq!(h.state().ch, n0);
    assert_eq!(h.state().instr, slot);
    ctrl(&mut h, Key::Z);
    frames(&mut h, 2);
    assert_eq!(song(&mut h, |s| s.channels), n0);
}

#[test]
fn drop_on_an_instrument_slot_gives_the_track_the_new_instrument() {
    let mut h = tl_harness();
    // the Instruments list (the Clip Arranger takes its place on the Timeline)
    h.state_mut().extra.clip_arranger = false;
    frames(&mut h, 2);
    let n0 = song(&mut h, |s| s.instruments.len());
    let (_, p) = find(&mut h, crate::browser::Tab::Instruments, "FM-4");
    let (slot, r) = h.state().br.slot_rects[2];
    let old = song(&mut h, |s| s.instruments[slot as usize - 1].clone());
    drag(&mut h, p, r.center());
    frames(&mut h, 2);
    // a ChipWave instrument is a fresh one for the selected track; the
    // slot's own sound is left alone (Browser › Presets replaces in place)
    assert_eq!(song(&mut h, |s| s.instruments.len()), n0 + 1);
    assert_eq!(song(&mut h, |s| s.instruments[n0].name.clone()), "E-Piano");
    assert_eq!(
        song(&mut h, |s| s.instruments[slot as usize - 1].name.clone()),
        old.name
    );
}

#[test]
fn header_drag_reorders_tracks_and_playback_stays_the_same() {
    let mut h = tl_harness();
    let names0 = song(&mut h, |s| s.channel_names.clone());
    let a = song(&mut h, |s| crate::engine::render_song(s, 8000));
    let (h0, _) = h.state().tlv.track_rects(0).unwrap();
    let (h2, _) = h.state().tlv.track_rects(2).unwrap();
    // grab the name area (left part) of header 1, drop past header 3's middle
    let from = Pos2::new(h0.left() + 40.0, h0.center().y);
    drag(
        &mut h,
        from,
        Pos2::new(from.x, h2.center().y + h2.height() * 0.3),
    );
    frames(&mut h, 2);
    let names = song(&mut h, |s| s.channel_names.clone());
    assert_eq!(names[2], names0[0], "{names:?}");
    assert_eq!(names[0], names0[1]);
    let b = song(&mut h, |s| crate::engine::render_song(s, 8000));
    assert_eq!(a.len(), b.len());
    let d = a
        .iter()
        .zip(&b)
        .map(|(x, y)| (x - y).abs())
        .fold(0.0f32, f32::max);
    assert!(d < 1e-4, "reorder changed playback by {d}");
    ctrl(&mut h, Key::Z);
    frames(&mut h, 2);
    assert_eq!(song(&mut h, |s| s.channel_names.clone()), names0);
}

#[test]
fn add_track_button_and_the_32_track_limit() {
    let mut h = tl_harness();
    h.state_mut().set_channels(4);
    frames(&mut h, 3);
    let n0 = song(&mut h, |s| s.channels);
    let b = h
        .query_all_by(|n| n.label().as_deref() == Some("+ Add track"))
        .next()
        .expect("+ Add track")
        .rect();
    super::ui_tests::click_at(&mut h, b.center());
    frames(&mut h, 2);
    assert_eq!(song(&mut h, |s| s.channels), n0 + 1);
    ctrl(&mut h, Key::Z);
    frames(&mut h, 2);
    assert_eq!(song(&mut h, |s| s.channels), n0);
    // full: a clear message, no change
    h.state_mut().set_channels(MAX_CHANNELS);
    frames(&mut h, 2);
    assert!(!h.state_mut().track_add(3));
    assert!(h.state().status.contains("32"), "{}", h.state().status);
    assert_eq!(song(&mut h, |s| s.channels), MAX_CHANNELS);
}

#[test]
fn add_track_with_instrument_opens_the_browser_and_makes_the_track() {
    let mut h = tl_harness();
    h.state_mut().extra.browser_open = false;
    frames(&mut h, 2);
    let n0 = song(&mut h, |s| s.channels);
    h.state_mut().track_add_with(1);
    frames(&mut h, 2);
    assert!(h.state().extra.browser_open);
    assert_eq!(h.state().br.filter.tab, crate::browser::Tab::Instruments);
    let (_, p) = find(&mut h, crate::browser::Tab::Instruments, "FM-4");
    dbl(&mut h, p);
    assert_eq!(song(&mut h, |s| s.channels), n0 + 1);
    assert_eq!(song(&mut h, |s| s.channel_names[1].clone()), "FM-4");
}

/// Del's report: a new track made with an instrument sometimes wasn't the
/// active track / instrument. The song's own instruments, presets, samples
/// and plugins go through one path that (1) used the asked-for index even
/// past the last track (Song / Master's "Add track with instrument…" asks
/// for m + 1: the instrument went to a track that doesn't exist and `ch`
/// pointed there) and (2) never lit the new track, so a lit Song / Master
/// (a stale `song_sel`) stayed the current track.
#[test]
fn a_new_track_with_an_instrument_is_the_active_track_and_instrument() {
    use super::browserview::Act;
    let mut h = tl_harness();
    frames(&mut h, 2);
    let n0 = song(&mut h, |s| s.channels);
    let m = h.state().tlv.layout.master.expect("Song / Master");
    assert_eq!(m, n0);
    let items = song(&mut h, crate::browser::song_items);
    assert!(items.len() >= 2, "the test song has instruments");
    // 1: Master lit, its menu's "Add track with instrument…" (at m + 1)
    h.state_mut().tl_select_track(m, false);
    h.state_mut().br_instrument(&items[1], Act::NewTrack(m + 1));
    frames(&mut h, 2);
    assert_eq!(song(&mut h, |s| s.channels), n0 + 1);
    assert_eq!(h.state().tl_cur_track(), n0, "the new track is lit");
    assert_eq!(h.state().ch, n0);
    assert_eq!(h.state().instr, 2, "its instrument is the active one");
    assert_eq!(h.state().track_slot(n0), 2, "and the new track plays it");
    // 2: track 1 then Master lit; a new track with an instrument at 1
    h.state_mut().tl_select_track(1, false);
    let m = h.state().tlv.layout.master.unwrap();
    h.state_mut().tl_select_track(m, false);
    h.state_mut().br_instrument(&items[0], Act::NewTrack(1));
    frames(&mut h, 2);
    assert_eq!(h.state().tl_cur_track(), 1, "not Song / Master");
    assert_eq!(h.state().instr, 1);
    assert_eq!(h.state().track_slot(1), 1);
}

#[test]
fn folders_scan_in_the_background_and_samples_load() {
    let d = std::env::temp_dir().join(format!("cw-br-ui-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(d.join("Kicks")).unwrap();
    let data: Vec<f32> = (0..4800)
        .flat_map(|i| {
            let v = (i as f32 * 0.05).sin() * 0.5;
            [v, v]
        })
        .collect();
    crate::engine::write_wav(&d.join("Kicks").join("Boom Kick.wav"), &data, 48000).unwrap();
    let mut h = tl_harness();
    h.state_mut().view = View::Pattern;
    h.state_mut().br_add_folder(d.clone());
    settle(&mut h);
    let n0 = song(&mut h, |s| s.instruments.len());
    let (_, p) = find(&mut h, crate::browser::Tab::Samples, "boom kick");
    dbl(&mut h, p);
    settle(&mut h);
    assert_eq!(song(&mut h, |s| s.instruments.len()), n0 + 1);
    assert!(song(&mut h, |s| matches!(
        s.instruments[n0].kind,
        InstrumentKind::Sample { .. }
    )));
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn find_plugins_searches_in_the_background_and_adds_the_folders() {
    let d = std::env::temp_dir().join(format!("cw-findplug-ui-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    let v = d.join("Plug-Ins").join("VST3");
    std::fs::create_dir_all(v.join("Thing.vst3")).unwrap();
    let mut h = tl_harness();
    h.state_mut().open_plugin_setup(false);
    frames(&mut h, 2);
    h.state_mut().psetup.as_mut().unwrap().roots = Some(vec![d.clone()]);
    h.state_mut().psetup_search();
    for _ in 0..200 {
        frame(&mut h);
        if h.state()
            .psetup
            .as_ref()
            .is_some_and(|p| p.step == plugsetup::Step::Found)
        {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let found: Vec<PathBuf> = h
        .state()
        .psetup
        .as_ref()
        .unwrap()
        .found
        .iter()
        .map(|f| f.0.clone())
        .collect();
    assert_eq!(found, vec![v.clone()]);
    h.state_mut().psetup_apply(found);
    frames(&mut h, 2);
    let s = v.display().to_string();
    assert!(h.state().extra.plugin_dirs.contains(&s));
    assert!(h.state().extra.browser_dirs.contains(&s));
    assert!(h.state().extra.plugin_setup_done);
    assert!(h.state().psetup.is_none());
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn opening_the_browser_leaves_an_old_song_untouched() {
    let s0 = fileio::demo_song();
    let mut h = harness_with(s0.clone());
    ctrl(&mut h, Key::B);
    frames(&mut h, 5);
    for t in crate::browser::Tab::ALL {
        h.state_mut().br.filter.tab = t;
        frames(&mut h, 2);
    }
    let d = std::env::temp_dir().join(format!("cw-br-old-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&d);
    crate::fileio::save_cwt(&s0, &d.join("a.cwt")).unwrap();
    let now = song(&mut h, |s| s.clone());
    crate::fileio::save_cwt(&now, &d.join("b.cwt")).unwrap();
    assert_eq!(
        std::fs::read(d.join("a.cwt")).unwrap(),
        std::fs::read(d.join("b.cwt")).unwrap()
    );
    let _ = std::fs::remove_dir_all(&d);
    assert!(!h.state().dirty);
}

#[test]
fn interface_style_switch_still_works_with_the_browser_open() {
    let mut h = tl_harness();
    for flat in [true, false] {
        h.state_mut().extra.flat_ui = flat;
        let ctx = h.ctx.clone();
        h.state().apply_theme(&ctx);
        frames(&mut h, 3);
        assert!(h.state().br.rect.width() > 250.0);
        assert!(!h.state().br.row_rects.is_empty());
    }
}

#[test]
fn plugin_thumbnails_setting_defaults_on() {
    let h = tl_harness();
    assert!(h.state().extra.browser_thumbs);
    let (w, hgt, rgba) = crate::plugin::thumb::generate_card(
        "Dexed",
        "Digital Suburban",
        "vst3",
        [0x7A, 0xA2, 0xE8],
    );
    assert!(w >= 64 && hgt >= 64 && rgba.len() == (w * hgt * 4) as usize);
    let k = crate::plugin::thumb::key("vst3", "ABCDEF", "1.0");
    assert!(k.contains("vst3"));
}

// ---- Browser › Instruments › ChipWave Instruments (Del, 0.2.5.x)

/// the ChipWave Instruments rows as listed (names in order)
fn tool_rows(h: &mut Harness<'static, App>) -> Vec<(String, Pos2)> {
    h.state_mut().br.filter.tab = crate::browser::Tab::Instruments;
    h.state_mut().br.filter.query.clear();
    h.state_mut().br.filter.category = Some(crate::browser::TOOLS_CAT.into());
    frames(h, 3);
    let rows = h.state().br.row_rects.clone();
    rows.into_iter()
        .map(|(k, r)| {
            (
                k.strip_prefix("tool:").unwrap_or(&k).to_string(),
                r.center(),
            )
        })
        .collect()
}

#[test]
fn chipwave_instruments_lead_the_instruments_tab_with_the_pies_add_list() {
    use super::trackpie::{Builtin, TrackInfo};
    use super::trackring::{ring_slots, Level};
    let mut h = tl_harness();
    // the pie's Add › Instrument › ring (without Back and Browse…)
    let info = TrackInfo {
        tracks: 4,
        timeline: true,
        ..Default::default()
    };
    let ring = ring_slots(Level::Instrument, &info);
    let pie: Vec<String> = ring[1..ring.len() - 1]
        .iter()
        .map(|s| s.label.clone())
        .collect();
    let all: Vec<&str> = Builtin::ALL.iter().map(|b| b.label()).collect();
    assert_eq!(pie, all);
    let names: Vec<String> = tool_rows(&mut h).into_iter().map(|r| r.0).collect();
    assert_eq!(names, all, "the same set, in the pie's order");
    for want in [
        "Grid",
        "Drum Kit",
        "WaveForge",
        "FM-4",
        "Chip FX",
        "Noise",
        "Wavetable",
        "Analog",
        "Synth",
        "Drum Synth",
        "Voice",
        crate::strings::STRINGS_DISPLAY_NAME,
    ] {
        assert!(names.iter().any(|n| n == want), "{want}");
    }
    // first in the Category column and at the top of the unfiltered list
    h.state_mut().br.filter.category = None;
    frames(&mut h, 3);
    let first = h.state().br.row_rects.first().unwrap().0.clone();
    assert_eq!(first, format!("tool:{}", all[0]));
    let cats = crate::browser::column_values(
        &h.state().br.items,
        &h.state().br.filter,
        &[],
        |i| &i.category,
        true,
        false,
    );
    assert_eq!(cats[0].0, crate::browser::TOOLS_CAT);
    // every one starts on a playable preset, not a bare Init
    for b in Builtin::ALL {
        let (name, _) = b.make().unwrap();
        assert!(!name.to_lowercase().contains("init"), "{b:?}: {name}");
        assert!(
            b.tip().contains(
                name.trim_start_matches("WaveForge ")
                    .trim_start_matches(concat!(crate::strings_name!(), " "))
            ),
            "{b:?}"
        );
    }
}

#[test]
fn a_chipwave_instrument_dropped_on_a_track_plays_it_and_opens_its_editor() {
    let mut h = tl_harness();
    h.state_mut().undo_reset();
    let before = song(&mut h, |s| s.clone());
    let rows = tool_rows(&mut h);
    let (_, p) = rows.iter().find(|r| r.0 == "FM-4").cloned().unwrap();
    let (head, _) = h.state().tlv.track_rects(2).expect("track 3 header");
    drag(&mut h, p, head.center());
    frames(&mut h, 3);
    let n = song(&mut h, |s| s.instruments.len());
    assert_eq!(n, before.instruments.len() + 1, "one new instrument");
    let (name, slot) = song(&mut h, |s| {
        (
            s.instruments[n - 1].name.clone(),
            s.timeline.as_ref().unwrap().tracks[2].instr,
        )
    });
    assert_eq!(name, "E-Piano");
    assert_eq!(slot as usize, n, "given to track 3");
    assert_eq!(
        song(&mut h, |s| s.channels),
        before.channels,
        "no new track"
    );
    // the track is selected, its full editor open below the timeline
    assert_eq!(h.state().ch, 2);
    assert_eq!(h.state().tl_cur_track(), 2);
    assert_eq!(h.state().instr as usize, n);
    assert_eq!(h.state().view, View::Timeline);
    assert!(h.state().extra.lower_inst, "the instrument editor is open");
    // one undo step takes it all back
    ctrl(&mut h, Key::Z);
    frames(&mut h, 2);
    assert_eq!(
        song(&mut h, |s| s.instruments.len()),
        before.instruments.len()
    );
    assert_eq!(
        song(&mut h, |s| s.timeline.as_ref().unwrap().tracks[2].instr),
        before.timeline.as_ref().unwrap().tracks[2].instr
    );
    assert_eq!(song(&mut h, |s| s.patterns.clone()), before.patterns);
}

#[test]
fn a_chipwave_instrument_dropped_below_the_tracks_makes_a_new_track() {
    let mut h = tl_harness();
    h.state_mut().set_channels(4);
    frames(&mut h, 3);
    h.state_mut().undo_reset();
    let n0 = song(&mut h, |s| s.channels);
    let i0 = song(&mut h, |s| s.instruments.len());
    let rows = tool_rows(&mut h);
    let (_, p) = rows.iter().find(|r| r.0 == "Drum Kit").cloned().unwrap();
    let y = h.state().tlv.tracks_bottom() + 30.0;
    let x = h.state().tlv.heads.center().x;
    drag(&mut h, p, Pos2::new(x, y));
    frames(&mut h, 3);
    assert_eq!(song(&mut h, |s| s.channels), n0 + 1);
    assert_eq!(song(&mut h, |s| s.channel_names[n0].clone()), "Drum Kit");
    let slot = song(&mut h, |s| s.instruments.len());
    assert_eq!(slot, i0 + 1);
    assert!(song(&mut h, |s| matches!(
        &s.instruments[slot - 1].kind,
        InstrumentKind::Gen(g) if matches!(g.engine, crate::gen::GenEngine::Drums(_))
    )));
    assert_eq!(
        song(&mut h, |s| s.timeline.as_ref().unwrap().tracks[n0].instr) as usize,
        slot
    );
    assert_eq!(h.state().ch, n0);
    assert!(h.state().extra.lower_inst);
    // the new track and its instrument are one undo step
    ctrl(&mut h, Key::Z);
    frames(&mut h, 2);
    assert_eq!(
        song(&mut h, |s| (s.channels, s.instruments.len())),
        (n0, i0)
    );
    // double-click: a new track at the end too
    let rows = tool_rows(&mut h);
    let (_, p) = rows.iter().find(|r| r.0 == "Analog").cloned().unwrap();
    dbl(&mut h, p);
    frames(&mut h, 3);
    assert_eq!(song(&mut h, |s| s.channels), n0 + 1);
    assert_eq!(
        song(&mut h, |s| s.instruments.last().unwrap().name.clone()),
        "Warm Lead"
    );
}
