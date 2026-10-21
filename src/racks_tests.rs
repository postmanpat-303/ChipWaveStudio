//! .cwr files and the My Racks library (docs/racks/PLAN.md Stage 5-6):
//! a saved rack loads back exactly and sounds the same; damaged or newer
//! files are refused before anything changes, with one plain sentence;
//! loading gives the new slots to one track only; rename / duplicate /
//! update (with backups) / delete (to Deleted) in a temp folder.
use super::*;
use crate::engine::racks_tests::{clip_song, diff, one_note_song, render};
use crate::fx::{Device, FxKind, FxPath};
use crate::model::{Instrument, InstrumentKind, Layer, RackP, Song};

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("cw-racks-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// slot 1: a rack of a saw (with a Split: Drive | dry) and a sample layer;
/// the rack has a Filter
pub(crate) fn song_with_rack() -> Song {
    let mut s = one_note_song();
    s.mixer.master = 0.05;
    s.mixer.limiter.enabled = false;
    let saw = s.instruments[0].clone();
    let mut split = Device::split();
    split.paths[0] = FxPath {
        devices: vec![Device::new(FxKind::Drive)],
        ..split.paths[0].clone()
    };
    let mut a = saw.clone();
    a.name = "Saw".into();
    a.fx = vec![split];
    let data: Vec<f32> = (0..4800).map(|k| ((k as f32) * 0.05).sin() * 0.5).collect();
    let b = Instrument {
        name: "Blip".into(),
        volume: 0.7,
        transpose: 0,
        fx: vec![],
        kind: InstrumentKind::Sample {
            file: String::new(),
            rate: 48000,
            data: data.into(),
            base_note: 49,
            lp: Default::default(),
        },
    };
    let mut l2 = Layer::new(2);
    l2.level_db = -4.0;
    l2.pan = 0.3;
    let rack = Instrument {
        name: "Glass Keys".into(),
        volume: 0.9,
        transpose: 0,
        fx: vec![Device::new(FxKind::Filter)],
        kind: InstrumentKind::Rack(RackP {
            layers: vec![Layer::new(1), l2],
        }),
    };
    s.instruments = vec![rack, a, b];
    s
}

pub(crate) fn info(name: &str) -> RackInfo {
    RackInfo {
        name: name.into(),
        description: "bright keys with a driven edge".into(),
        sound_type: "Keys".into(),
        tags: vec!["bright".into(), "glass".into()],
        ..Default::default()
    }
}

#[test]
fn a_saved_rack_loads_back_exactly_and_sounds_the_same() {
    let d = tmp("roundtrip");
    let s = song_with_rack();
    let r = from_song(&s, 0, info("Glass Keys")).unwrap();
    assert_eq!(r.instruments.len(), 3);
    assert_eq!(r.info.created, today());
    let p = d.join(file_name(&r.info.name));
    save(&r, &p).unwrap();
    let back = load(&p).unwrap();
    assert_eq!(back.info, r.info);
    // the sample came back bit for bit (the file name inside differs)
    for (x, y) in back.instruments.iter().zip(&r.instruments) {
        match (&x.kind, &y.kind) {
            (InstrumentKind::Sample { data: a, .. }, InstrumentKind::Sample { data: b, .. }) => {
                assert_eq!(**a, **b)
            }
            _ => assert_eq!(x, y),
        }
    }
    // loaded into a new song it renders like the original rack
    let mut fresh = one_note_song();
    fresh.mixer.master = 0.05;
    fresh.mixer.limiter.enabled = false;
    fresh.instruments.truncate(1);
    let slot = load_into(&mut fresh, &back, Some(0)).unwrap();
    assert_eq!(slot, 1);
    let orig = render(&clip_song(s, 1, &[37, 44]), 0.8);
    let again = render(&clip_song(fresh, (slot + 1) as u8, &[37, 44]), 0.8);
    assert!(diff(&orig[8192..], &again[8192..]) < 1e-7);
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn a_plain_instrument_saves_as_a_one_layer_rack() {
    let s = one_note_song();
    let r = from_song(&s, 0, info("")).unwrap();
    assert_eq!(
        r.info.name, s.instruments[0].name,
        "an empty name: the instrument's"
    );
    assert_eq!(r.instruments.len(), 2);
    match &r.instruments[0].kind {
        InstrumentKind::Rack(x) => assert_eq!(x.layers, vec![Layer::new(1)]),
        _ => panic!("not a rack"),
    }
    assert_eq!(r.instruments[1].kind, s.instruments[0].kind);
    // a plugin instrument can't (it can't play as a layer yet)
    let mut p = s.clone();
    p.instruments[0].kind = InstrumentKind::Plugin(crate::plugin::PluginPatch {
        format: "clap".into(),
        id: "org.example.x".into(),
        name: "X".into(),
        vendor: String::new(),
        path: String::new(),
        state: String::new(),
        params: vec![],
        macros: vec![],
    });
    assert!(from_song(&p, 0, info("x")).unwrap_err().contains("plugin"));
}

fn write_toml(p: &Path, toml: &str) {
    let f = std::fs::File::create(p).unwrap();
    let mut z = zip::ZipWriter::new(f);
    let o = zip::write::SimpleFileOptions::default();
    z.start_file("mimetype", o).unwrap();
    z.write_all(MIME).unwrap();
    z.start_file("rack.toml", o).unwrap();
    z.write_all(toml.as_bytes()).unwrap();
    z.finish().unwrap();
}

fn layers(r: &mut Rack) -> &mut Vec<Layer> {
    match &mut r.instruments[0].kind {
        InstrumentKind::Rack(x) => &mut x.layers,
        _ => unreachable!(),
    }
}

#[test]
fn newer_damaged_and_wrong_racks_are_refused_with_one_sentence() {
    let d = tmp("bad");
    let p = d.join("Glass Keys.cwr");
    write_toml(
        &p,
        "format = \"chipwave-rack\"\nformat_version = 2\n[rack]\nname = \"x\"\n",
    );
    assert_eq!(
        load(&p).unwrap_err(),
        "Glass Keys.cwr was made by a newer ChipWave (rack format 2). Please update to load it."
    );
    let p = d.join("Reese Wall.cwr");
    write_toml(&p, "this is = = not toml");
    assert_eq!(
        load(&p).unwrap_err(),
        "Reese Wall.cwr is damaged (rack.toml can't be read)"
    );
    std::fs::write(d.join("junk.cwr"), b"hello").unwrap();
    assert!(load(&d.join("junk.cwr"))
        .unwrap_err()
        .contains("is damaged"));
    // valid files whose content breaks the rules
    let good = from_song(&song_with_rack(), 0, info("Rules")).unwrap();
    let check = |f: &dyn Fn(&mut Rack), want: &str| {
        let mut r = good.clone();
        f(&mut r);
        let e = validate(&r).unwrap_err();
        assert!(e.contains(want), "{e}");
    };
    check(&|r| layers(r).clear(), "0 layers");
    check(&|r| layers(r)[1].slot = 9, "isn't there");
    check(&|r| layers(r)[1].slot = 1, "repeats");
    check(
        &|r| r.instruments[2].kind = InstrumentKind::Rack(RackP::default()),
        "rack inside a rack",
    );
    check(
        &|r| {
            for _ in 0..3 {
                r.instruments.push(r.instruments[1].clone());
                let n = r.instruments.len() - 1;
                layers(r).push(Layer::new(n));
            }
        },
        "5 layers",
    );
    // nothing is written for a rack that doesn't pass
    let mut bad = good.clone();
    layers(&mut bad).clear();
    assert!(save(&bad, &d.join("bad.cwr")).is_err());
    assert!(!d.join("bad.cwr").exists() && !d.join("bad.cwr.tmp").exists());
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn loading_a_rack_gives_new_slots_to_one_track_only() {
    let mut song = crate::fileio::demo_song();
    let before = song.clone();
    let r = from_song(&song_with_rack(), 0, info("Glass Keys")).unwrap();
    let n0 = song.instruments.len();
    let slot = load_into(&mut song, &r, Some(1)).unwrap();
    assert_eq!(slot, n0);
    assert_eq!(song.instruments.len(), n0 + 3);
    assert_eq!(
        &song.instruments[..n0],
        &before.instruments[..n0],
        "old slots untouched"
    );
    match &song.instruments[slot].kind {
        InstrumentKind::Rack(x) => assert_eq!(
            x.layers.iter().map(|l| l.slot).collect::<Vec<_>>(),
            vec![n0 + 1, n0 + 2]
        ),
        _ => panic!(),
    }
    // track 2's notes now play the rack; every other track's are as before
    for (p, q) in song.patterns.iter().zip(&before.patterns) {
        for (row, old) in p.cells.iter().zip(&q.cells) {
            for (ch, (c, o)) in row.iter().zip(old).enumerate() {
                if ch == 1 && (1..=120).contains(&o.note) {
                    assert_eq!(c.instr as usize, slot + 1);
                } else {
                    assert_eq!(c, o);
                }
            }
        }
    }
    // full song: refused, nothing changed
    let mut full = before.clone();
    while full.instruments.len() < 254 {
        full.instruments.push(full.instruments[0].clone());
    }
    let keep = full.instruments.len();
    assert!(load_into(&mut full, &r, Some(0))
        .unwrap_err()
        .contains("slots"));
    assert_eq!(full.instruments.len(), keep);
}

#[test]
fn my_racks_rename_duplicate_update_with_backups_and_delete_to_deleted() {
    let d = tmp("library");
    let r = from_song(&song_with_rack(), 0, info("Glass Keys")).unwrap();
    let p = d.join("Glass Keys.cwr");
    save(&r, &p).unwrap();
    let mut other = r.clone();
    other.info = info("Reese Wall");
    other.info.tags = vec!["bass".into()];
    save(&other, &d.join("Reese Wall.cwr")).unwrap();
    std::fs::write(d.join("Broken.cwr"), b"x").unwrap();
    let all = scan(&d);
    assert_eq!(all.len(), 3);
    assert!(
        all.iter().any(|e| e.error.is_some()),
        "a broken file is listed with its reason"
    );
    let hits: Vec<_> = all.iter().filter(|e| matches(e, "glass bright")).collect();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].layers, 2);
    assert!(all.iter().filter(|e| matches(e, "bass")).count() == 1);
    // rename: the file follows the name
    let p2 = rename(&p, "Ice Keys").unwrap();
    assert_eq!(p2, d.join("Ice Keys.cwr"));
    assert!(!p.exists());
    assert_eq!(read_info(&p2).unwrap().0.name, "Ice Keys");
    // duplicate
    let p3 = duplicate(&p2).unwrap();
    assert_eq!(read_info(&p3).unwrap().0.name, "Ice Keys copy");
    // update 4 times: the newest 3 backups stay; name and tags kept
    for k in 0..4 {
        let mut n = r.clone();
        n.info = info(&format!("ignored {k}"));
        if let InstrumentKind::Rack(x) = &mut n.instruments[0].kind {
            x.layers[0].level_db = -(k as f32);
        }
        update(&p2, n).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(3));
    }
    let i = read_info(&p2).unwrap().0;
    assert_eq!(i.name, "Ice Keys");
    assert_eq!(i.tags, vec!["bright".to_string(), "glass".to_string()]);
    let l = load(&p2).unwrap();
    match &l.instruments[0].kind {
        InstrumentKind::Rack(x) => assert_eq!(x.layers[0].level_db, -3.0),
        _ => panic!(),
    }
    let backups = std::fs::read_dir(d.join(".backup")).unwrap().count();
    assert_eq!(backups, BACKUPS);
    // delete: to Deleted/, out of the list
    let gone = delete(&p3).unwrap();
    assert!(gone.starts_with(d.join("Deleted")) && gone.exists() && !p3.exists());
    assert!(scan(&d).iter().all(|e| e.path != p3));
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn plugin_effects_in_a_rack_are_listed_for_the_missing_plugin_warning() {
    let mut s = song_with_rack();
    let mut d = Device::new(FxKind::Plugin);
    d.plugin = Some(Box::new(crate::plugin::PluginPatch {
        format: "clap".into(),
        id: "com.example.verb".into(),
        name: "Example Verb".into(),
        vendor: "Example".into(),
        path: String::new(),
        state: String::new(),
        params: vec![],
        macros: vec![],
    }));
    s.instruments[2].fx.push(d);
    let r = from_song(&s, 0, info("With plugin")).unwrap();
    assert_eq!(r.plugins.len(), 1);
    assert_eq!(r.plugins[0].id, "com.example.verb");
    assert_eq!(r.plugins[0].used_by, "layer B › effects");
}
