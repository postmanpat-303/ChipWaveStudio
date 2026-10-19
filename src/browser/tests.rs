use super::*;

fn tmpdir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("cw-browser-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn every_tab_has_built_in_items_and_badges_have_colours() {
    let v = builtin_items();
    for t in [
        Tab::Instruments,
        Tab::Presets,
        Tab::Grid,
        Tab::Wavetables,
        Tab::Songs,
    ] {
        assert!(v.iter().any(|i| i.tab == t), "{t:?} is empty");
    }
    assert_eq!(v.iter().filter(|i| i.tab == Tab::Wavetables).count(), 10);
    assert!(
        v.iter().any(|i| matches!(i.src, Src::GridFx(_))),
        "Grid FX patches"
    );
    assert!(
        v.iter().any(|i| matches!(i.src, Src::Demo(_))),
        "demo songs"
    );
    // keys are unique (favourites and ratings use them)
    let mut keys: Vec<&str> = v.iter().map(|i| i.key.as_str()).collect();
    keys.sort();
    let n = keys.len();
    keys.dedup();
    assert_eq!(keys.len(), n);
    // distinct colours per badge
    let mut cols: Vec<u32> = Fmt::ALL.iter().map(|f| f.colour()).collect();
    cols.sort();
    cols.dedup();
    assert_eq!(cols.len(), Fmt::ALL.len());
    // every built-in instrument item makes an instrument
    let song = Song::empty();
    for it in v
        .iter()
        .filter(|i| matches!(i.tab, Tab::Instruments | Tab::Presets | Tab::Wavetables))
    {
        instrument_of(it, &song).unwrap_or_else(|e| panic!("{}: {e}", it.name));
    }
    for it in v.iter().filter(|i| matches!(i.src, Src::GridFx(_))) {
        let Src::GridFx(n) = &it.src else {
            unreachable!()
        };
        assert!(grid_fx_device(n).is_some());
    }
}

#[test]
fn tags_search_filters_and_sorting() {
    assert!(tags_for("Acid Moog", "Analog").contains(&"Lead"));
    assert!(tags_for("Octave Bass", "").contains(&"Bass"));
    assert!(tags_for("909 Kick", "").contains(&"Drums"));
    let v = builtin_items();
    let favs = vec![];
    let r = BTreeMap::new();
    let mut f = Filter {
        tab: Tab::Instruments,
        ..Default::default()
    };
    let all = filter(&v, &f, &favs, &r);
    assert!(all.len() > 10);
    // sorted by name
    let names: Vec<String> = all.iter().map(|&i| v[i].name.to_lowercase()).collect();
    let mut s = names.clone();
    s.sort();
    assert_eq!(names, s);
    f.query = "kick".into();
    let k = filter(&v, &f, &favs, &r);
    assert!(!k.is_empty() && k.len() < all.len());
    assert!(k.iter().all(|&i| matches_query(&v[i], "kick")));
    f.query.clear();
    f.tags = vec!["Drums"];
    let d = filter(&v, &f, &favs, &r);
    assert!(d.iter().all(|&i| v[i].tags.contains(&"Drums")));
    // category column with counts; picking one narrows the list
    let cats = column_values(&v, &f, &favs, |i| &i.category, true, false);
    let (c, n) = cats[0].clone();
    f.category = Some(c);
    assert_eq!(filter(&v, &f, &favs, &r).len(), n);
    // favourites and rating sort
    f = Filter {
        tab: Tab::Wavetables,
        loc: LocFilter::Favourites,
        ..Default::default()
    };
    let fav = vec!["table:Wavefold".to_string()];
    let got = filter(&v, &f, &fav, &r);
    assert_eq!(got.len(), 1);
    assert_eq!(v[got[0]].name, "Wavefold");
    f.loc = LocFilter::All;
    f.sort = Sort::Rating;
    let mut r = BTreeMap::new();
    r.insert("table:Bit Crush".to_string(), 3u8);
    let got = filter(&v, &f, &favs, &r);
    assert_eq!(v[got[0]].name, "Bit Crush");
}

#[test]
fn folder_scan_classifies_files_skips_hidden_and_bundles() {
    let d = tmpdir("scan");
    std::fs::create_dir_all(d.join("Drums/Kicks")).unwrap();
    std::fs::create_dir_all(d.join("Wavetables")).unwrap();
    std::fs::create_dir_all(d.join(".hidden")).unwrap();
    std::fs::create_dir_all(d.join("Synth.vst3/Contents")).unwrap();
    let wav = crate::audiofile::wav_f32(22050, 1, &[0.0, 0.5, -0.5, 0.0]);
    std::fs::write(d.join("Drums/Kicks/kick.wav"), &wav).unwrap();
    std::fs::write(d.join("Wavetables/saw.wav"), &wav).unwrap();
    std::fs::write(d.join(".hidden/x.wav"), &wav).unwrap();
    std::fs::write(d.join("Synth.vst3/Contents/a.wav"), &wav).unwrap();
    for f in [
        "loop.flac",
        "vox.mp3",
        "rain.ogg",
        "a.cwt",
        "b.cwt.toml",
        "c.mid",
        "d.mod",
        "e.cwi",
        "notes.txt",
    ] {
        std::fs::write(d.join(f), b"x").unwrap();
    }
    let n = AtomicUsize::new(0);
    let hits = scan_folder(&d, &n, &AtomicBool::new(false));
    let fmt = |name: &str| hits.iter().find(|h| h.path.ends_with(name)).map(|h| h.fmt);
    assert_eq!(fmt("kick.wav"), Some(Fmt::Wav));
    assert_eq!(fmt("saw.wav"), Some(Fmt::Wavetable));
    assert_eq!(fmt("loop.flac"), Some(Fmt::Flac));
    assert_eq!(fmt("vox.mp3"), Some(Fmt::Mp3));
    assert_eq!(fmt("rain.ogg"), Some(Fmt::Ogg));
    assert_eq!(fmt("a.cwt"), Some(Fmt::Song));
    assert_eq!(fmt("b.cwt.toml"), Some(Fmt::Song));
    assert_eq!(fmt("c.mid"), Some(Fmt::Midi));
    assert_eq!(fmt("d.mod"), Some(Fmt::Mod));
    assert_eq!(fmt("e.cwi"), Some(Fmt::Cwi));
    assert_eq!(fmt("notes.txt"), None);
    assert_eq!(fmt("x.wav"), None, "hidden folder");
    assert_eq!(fmt("a.wav"), None, "inside a plugin bundle");
    assert!(n.load(Ordering::Relaxed) >= 3);
    // items: tabs, categories from the sub-folder, the location
    let root = d.display().to_string();
    let items = file_items(&root, &hits);
    let kick = items.iter().find(|i| i.name == "kick.wav").unwrap();
    assert_eq!(kick.tab, Tab::Samples);
    assert_eq!(
        kick.category,
        Path::new("Drums").join("Kicks").display().to_string()
    );
    assert_eq!(kick.loc, Loc::Folder(root.clone()));
    assert!(kick.tags.contains(&"Drums"));
    let song = items.iter().find(|i| i.name == "b").unwrap();
    assert_eq!(song.tab, Tab::Songs);
    assert!(items
        .iter()
        .find(|i| i.name == "c.mid")
        .unwrap()
        .detail
        .contains("planned"));
    // a sample and a wavetable WAV load as instruments
    let s = Song::empty();
    let k = instrument_of(kick, &s).unwrap();
    assert!(matches!(k.kind, InstrumentKind::Sample { .. }));
    let wt = items.iter().find(|i| i.name == "saw.wav").unwrap();
    assert_eq!(wt.tab, Tab::Wavetables);
    // a stop request ends the walk
    let none = scan_folder(&d, &n, &AtomicBool::new(true));
    assert!(none.is_empty());
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn preview_song_plays_one_note_through_the_tracks_channel() {
    let mut base = Song::empty();
    base.mixer.channels[2].volume = 0.3;
    base.mixer.channels[2].mute = true;
    let inst = instrument_of(&builtin_items()[0], &base).unwrap();
    let p = preview_song(&inst, &base, 2);
    assert_eq!(p.mixer.channels[0].volume, 0.3);
    assert!(!p.mixer.channels[0].mute);
    let notes: Vec<u8> = p.patterns[0]
        .cells
        .iter()
        .map(|r| r[0].note)
        .filter(|&n| n > 0 && n != crate::model::NOTE_OFF)
        .collect();
    assert_eq!(notes, vec![crate::model::parse_note("C-3").unwrap()]);
    let out = crate::engine::render_song(&p, 22050);
    assert!(out.iter().any(|x| x.abs() > 1e-4), "it sounds");
}

#[test]
fn index_round_trips_through_json() {
    let mut i = Index::default();
    i.folders.insert(
        "/x".into(),
        vec![FileHit {
            path: "/x/a.wav".into(),
            fmt: Fmt::Wav,
            size: 10,
        }],
    );
    let t = serde_json::to_string(&i).unwrap();
    let back: Index = serde_json::from_str(&t).unwrap();
    assert_eq!(back, i);
}

#[test]
fn plugin_folder_search_finds_folders_and_skips_bundles() {
    use crate::plugin::scan::find_plugin_folders;
    use std::sync::atomic::{AtomicBool, AtomicUsize};
    let d = tmpdir("findplug");
    let a = d.join("Audio").join("VST3");
    let b = d.join("Tools").join("clap stuff");
    std::fs::create_dir_all(a.join("Synth.vst3").join("Contents").join("deep.vst3")).unwrap();
    std::fs::write(a.join("Fx.vst3"), b"x").unwrap();
    std::fs::create_dir_all(&b).unwrap();
    std::fs::write(b.join("one.clap"), b"x").unwrap();
    std::fs::create_dir_all(d.join(".hidden")).unwrap();
    std::fs::write(d.join(".hidden").join("no.clap"), b"x").unwrap();
    std::fs::create_dir_all(d.join("empty").join("deeper")).unwrap();
    let visited = AtomicUsize::new(0);
    let now = std::sync::Mutex::new(String::new());
    let stop = AtomicBool::new(false);
    let found = find_plugin_folders(std::slice::from_ref(&d), 8, 10_000, &visited, &now, &stop);
    let mut got: Vec<(PathBuf, usize)> = found;
    got.sort();
    assert_eq!(got, vec![(a.clone(), 2), (b.clone(), 1)]);
    assert!(visited.load(std::sync::atomic::Ordering::Relaxed) >= 4);
    // a stop request ends the search at once
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    assert!(
        find_plugin_folders(std::slice::from_ref(&d), 8, 10_000, &visited, &now, &stop).is_empty()
    );
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn plugin_catalogue_sets_category_creator_and_showcase_tag() {
    use crate::plugin::clap::Desc;
    use crate::plugin::scan::{Cache, Entry as ScanEntry, Found};
    let mk = |id: &str, name: &str, vendor: &str, format: &str, instrument: bool| Found {
        path: format!("/tmp/{name}.{format}"),
        format: format.into(),
        desc: Desc {
            id: id.into(),
            name: name.into(),
            vendor: vendor.into(),
            version: "1".into(),
            features: if instrument {
                vec!["instrument".into()]
            } else {
                vec!["audio-effect".into()]
            },
        },
    };
    let cache = Cache {
        version: 2,
        entries: vec![ScanEntry {
            path: "/tmp".into(),
            format: "dir".into(),
            mtime: 0,
            size: 0,
            status: "ok".into(),
            message: String::new(),
            plugins: vec![
                mk(
                    "org.surge-synth-team.surge-xt",
                    "Surge XT",
                    "Surge Synth Team",
                    "clap",
                    true,
                ),
                mk(
                    "com.digital-suburban.dexed",
                    "Dexed",
                    "Digital Suburban",
                    "clap",
                    true,
                ),
                mk(
                    "AABBCCDDEEFF00112233445566778899",
                    "Geonkick",
                    "Quamplex",
                    "vst3",
                    true,
                ),
                mk(
                    "deadbeefdeadbeefdeadbeefdeadbeef",
                    "Mystery",
                    "Acme",
                    "vst3",
                    true,
                ),
                mk(
                    "org.surge-synth-team.surge-xt-fx",
                    "Surge XT Effects",
                    "Surge Synth Team",
                    "clap",
                    false,
                ),
            ],
        }],
    };
    let items = plugin_items(&cache);
    let surge = items.iter().find(|i| i.name == "Surge XT").unwrap();
    assert_eq!(surge.category, "Hybrid");
    assert_eq!(surge.creator, "Surge Synth Team");
    assert!(surge.tags.contains(&"Showcase"));
    let dexed = items.iter().find(|i| i.name == "Dexed").unwrap();
    assert_eq!(dexed.category, "FM");
    assert!(dexed.tags.contains(&"Showcase"));
    let kick = items.iter().find(|i| i.name == "Geonkick").unwrap();
    assert_eq!(kick.category, "Drums");
    assert!(kick.tags.contains(&"Showcase"));
    let mystery = items.iter().find(|i| i.name == "Mystery").unwrap();
    assert_eq!(mystery.category, "Instrument");
    assert!(!mystery.tags.contains(&"Showcase"));
    let fx = items.iter().find(|i| i.name == "Surge XT Effects").unwrap();
    assert_eq!(fx.category, "Effect");
    assert!(fx.tags.contains(&"Showcase"));
}
