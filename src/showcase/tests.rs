use super::*;
use std::collections::HashMap;
use std::io::Cursor;

/// serves canned bytes for URLs (no network in tests)
struct Canned(HashMap<String, Vec<u8>>);

impl Fetch for Canned {
    fn fetch(
        &self,
        url: &str,
        _size: u64,
        out: &mut dyn Write,
        on: &mut dyn FnMut(u64),
        cancel: &AtomicBool,
    ) -> Result<(), String> {
        let b = self.0.get(url).ok_or("HTTP 404")?;
        for c in b.chunks(7) {
            if cancel.load(Ordering::Relaxed) {
                return Err(CANCELLED.into());
            }
            out.write_all(c).map_err(|e| e.to_string())?;
            on(c.len() as u64);
        }
        Ok(())
    }
}

fn hex(b: &[u8]) -> String {
    Sha256::digest(b).iter().map(|x| format!("{x:02x}")).collect()
}

fn zip_of(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut w = zip::ZipWriter::new(Cursor::new(vec![]));
    let o = zip::write::SimpleFileOptions::default().unix_permissions(0o755);
    for (n, b) in files {
        if n.ends_with('/') {
            w.add_directory(*n, o).unwrap();
        } else {
            w.start_file(*n, o).unwrap();
            w.write_all(b).unwrap();
        }
    }
    w.finish().unwrap().into_inner()
}

fn targz_of(files: &[(&str, &[u8])]) -> Vec<u8> {
    let gz = flate2::write::GzEncoder::new(vec![], flate2::Compression::fast());
    let mut t = tar::Builder::new(gz);
    for (n, b) in files {
        let mut h = tar::Header::new_gnu();
        h.set_size(b.len() as u64);
        h.set_mode(0o644);
        h.set_cksum();
        t.append_data(&mut h, n, *b).unwrap();
    }
    t.into_inner().unwrap().finish().unwrap()
}

fn leak<T>(v: Vec<T>) -> &'static [T] {
    Box::leak(v.into_boxed_slice())
}

fn test_item(version: &'static str, assets: &'static [Asset]) -> Item {
    Item {
        id: "test-synth",
        name: "Test Synth",
        creator: "Tester",
        category: "FM",
        licence: "GPL-3.0",
        version,
        formats: "CLAP + VST3",
        page: "https://github.com/x/y/releases",
        source: "https://github.com/x/y",
        licence_text: None,
        big: false,
        note: "",
        windows: Get::Auto(assets),
        macos: Get::Auto(assets),
        linux: Get::Auto(assets),
    }
}

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("cw-showcase-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

const URL: &str = "https://github.com/x/y/releases/download/v1/TestSynth-win.zip";

fn synth_zip(v: &str) -> Vec<u8> {
    zip_of(&[
        ("TestSynth.vst3/", b""),
        ("TestSynth.vst3/Contents/x86_64-win/TestSynth.vst3", v.as_bytes()),
        ("TestSynth.clap", b"clap"),
        ("TestSynth.exe", b"standalone: must not be installed"),
        ("LICENSE", b"GPL text"),
        ("../evil.txt", b"escape attempt"),
    ])
}

fn synth_assets(bytes: &[u8]) -> &'static [Asset] {
    leak(vec![Asset {
        url: URL,
        sha256: Box::leak(hex(bytes).into_boxed_str()),
        size: bytes.len() as u64,
        kind: Kind::Zip,
        picks: leak(vec![
            p("TestSynth.vst3", Dest::Vst3, "TestSynth.vst3"),
            p("TestSynth.clap", Dest::Clap, "TestSynth.clap"),
            p("LICENSE", Dest::Licence, "LICENSE.txt"),
        ]),
    }])
}

#[test]
fn installs_only_the_bundles_with_licence_and_skips_same_version() {
    let base = tmp("ok");
    let roots = Roots::under(&base);
    let z = synth_zip("v1");
    let it = test_item("1.0", synth_assets(&z));
    let f = Canned(HashMap::from([(URL.to_string(), z)]));
    let cancel = AtomicBool::new(false);
    let mut seen = vec![];
    let r = install(&it, &roots, &f, &cancel, &mut |s, _| seen.push(s));
    assert_eq!(r, Ok(true));
    assert!(seen.contains(&State::Downloading) && seen.contains(&State::Installing));
    let bin = roots.vst3.join("TestSynth.vst3/Contents/x86_64-win/TestSynth.vst3");
    assert_eq!(std::fs::read_to_string(&bin).unwrap(), "v1");
    assert!(roots.clap.join("TestSynth.clap").is_file());
    // licence + source note next to the bundles in both folders
    for d in [&roots.vst3, &roots.clap] {
        assert_eq!(std::fs::read_to_string(d.join("TestSynth-LICENSE.txt")).unwrap(), "GPL text");
        let src = std::fs::read_to_string(d.join("TestSynth-SOURCE.txt")).unwrap();
        assert!(src.contains("https://github.com/x/y") && src.contains("SHA-256"));
    }
    // nothing else: no standalone exe, no escape, no staging left, download deleted
    assert!(!base.join("evil.txt").exists() && !roots.vst3.parent().unwrap().join("evil.txt").exists());
    let names: Vec<String> = std::fs::read_dir(&roots.vst3)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    assert!(!names.iter().any(|n| n.contains("exe") || n.contains("staging")), "{names:?}");
    assert!(std::fs::read_dir(&roots.cache).unwrap().next().is_none());
    assert!(is_installed(&roots, &it));
    // same version again: skipped without downloading
    let none = Canned(HashMap::new());
    assert_eq!(install(&it, &roots, &none, &cancel, &mut |_, _| {}), Ok(false));
    // a new version replaces the old bundle
    let z2 = synth_zip("v2");
    let it2 = test_item("2.0", synth_assets(&z2));
    let f2 = Canned(HashMap::from([(URL.to_string(), z2)]));
    assert_eq!(install(&it2, &roots, &f2, &cancel, &mut |_, _| {}), Ok(true));
    assert_eq!(std::fs::read_to_string(&bin).unwrap(), "v2");
    assert_eq!(load_record(&roots)["test-synth"].version, "2.0");
    // deleting a bundle makes it "not installed" again
    std::fs::remove_dir_all(roots.vst3.join("TestSynth.vst3")).unwrap();
    assert!(!is_installed(&roots, &it2));
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn checksum_mismatch_installs_nothing() {
    let base = tmp("bad");
    let roots = Roots::under(&base);
    let z = synth_zip("v1");
    let it = test_item("1.0", synth_assets(&z));
    let f = Canned(HashMap::from([(URL.to_string(), synth_zip("tampered"))]));
    let e = install(&it, &roots, &f, &AtomicBool::new(false), &mut |_, _| {}).unwrap_err();
    assert!(e.contains("checksum"), "{e}");
    assert!(!roots.vst3.join("TestSynth.vst3").exists());
    assert!(!roots.record.exists());
    // the bad download isn't kept
    assert!(std::fs::read_dir(&roots.cache).unwrap().next().is_none());
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn missing_download_and_unexpected_layout_are_clear_errors() {
    let base = tmp("404");
    let roots = Roots::under(&base);
    let z = synth_zip("v1");
    let it = test_item("1.0", synth_assets(&z));
    let e = install(&it, &roots, &Canned(HashMap::new()), &AtomicBool::new(false), &mut |_, _| {})
        .unwrap_err();
    assert!(e.contains("404"), "{e}");
    // archive without the expected bundle
    let other = zip_of(&[("Something.vst3/x", b"1")]);
    let it2 = test_item("1.0", leak(vec![Asset { sha256: Box::leak(hex(&other).into_boxed_str()), size: other.len() as u64, ..synth_assets(&z)[0] }]));
    let f = Canned(HashMap::from([(URL.to_string(), other)]));
    let e = install(&it2, &roots, &f, &AtomicBool::new(false), &mut |_, _| {}).unwrap_err();
    assert!(e.contains("didn't contain TestSynth.vst3"), "{e}");
    let left: Vec<_> = std::fs::read_dir(&roots.vst3).map(|r| r.count()).into_iter().collect();
    assert_eq!(left, vec![0], "staging cleaned up");
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn cancel_stops_without_installing() {
    let base = tmp("cancel");
    let roots = Roots::under(&base);
    let z = synth_zip("v1");
    let it = test_item("1.0", synth_assets(&z));
    let f = Canned(HashMap::from([(URL.to_string(), z)]));
    let cancel = AtomicBool::new(true);
    let e = install(&it, &roots, &f, &cancel, &mut |_, _| {}).unwrap_err();
    assert_eq!(e, CANCELLED);
    assert!(!roots.vst3.join("TestSynth.vst3").exists());
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn tar_gz_data_folder_and_embedded_licence() {
    let base = tmp("tgz");
    let roots = Roots::under(&base);
    let t = targz_of(&[
        ("./README.txt", b"readme"),
        ("./Vendor/SynthData/wavetables/a.wt", b"wt"),
        ("./Vendor/SynthData/patches/b.fxp", b"fxp"),
    ]);
    let z = zip_of(&[("Synth.clap", b"clap")]);
    let assets = leak(vec![
        Asset {
            url: "https://github.com/x/y/releases/download/v1/s.zip",
            sha256: Box::leak(hex(&z).into_boxed_str()),
            size: z.len() as u64,
            kind: Kind::Zip,
            picks: leak(vec![p("Synth.clap", Dest::Clap, "Synth.clap")]),
        },
        Asset {
            url: "https://github.com/x/y/releases/download/v1/c.tar.gz",
            sha256: Box::leak(hex(&t).into_boxed_str()),
            size: t.len() as u64,
            kind: Kind::TarGz,
            picks: leak(vec![p("Vendor/SynthData", Dest::Data("Synth"), "")]),
        },
    ]);
    let mut it = test_item("1", assets);
    it.licence_text = Some("embedded licence");
    let f = Canned(HashMap::from([
        (assets[0].url.to_string(), z),
        (assets[1].url.to_string(), t),
    ]));
    let prog = Mutex::new(vec![Progress { id: "test-synth", state: State::Waiting, done: 0, total: it.size() }]);
    // run() through a one-item list needs a manifest id, so call install directly
    let r = install(&it, &roots, &f, &AtomicBool::new(false), &mut |s, d| {
        let mut g = prog.lock().unwrap();
        g[0].state = s;
        if let Some(d) = d {
            g[0].done = d;
        }
    });
    assert_eq!(r, Ok(true));
    assert_eq!(prog.lock().unwrap()[0].fraction(), 1.0);
    assert_eq!(std::fs::read_to_string(roots.data.join("Synth/wavetables/a.wt")).unwrap(), "wt");
    assert!(roots.data.join("Synth/patches/b.fxp").is_file());
    assert!(!roots.data.join("README.txt").exists());
    assert_eq!(
        std::fs::read_to_string(roots.clap.join("TestSynth-LICENSE.txt")).unwrap(),
        "embedded licence"
    );
    // licence only where a plugin went (no VST3 here)
    assert!(!roots.vst3.join("TestSynth-LICENSE.txt").exists());
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn archive_paths_cannot_escape() {
    assert_eq!(clean("./a\\b/c").as_deref(), Some("a/b/c"));
    assert_eq!(clean("../x"), None);
    assert_eq!(clean("a/../../x"), None);
    assert_eq!(clean("/etc/passwd"), None);
}

#[test]
fn manifest_is_pinned_official_and_matches_the_catalogue() {
    assert_eq!(ITEMS.len(), 8);
    let mut ids = std::collections::HashSet::new();
    for it in ITEMS {
        assert!(ids.insert(it.id));
        // the Browser files every showcase plugin under the same category
        let e = crate::browser::catalogue::lookup("", it.name).unwrap_or_else(|| panic!("{}", it.name));
        assert_eq!(e.category, it.category, "{}", it.name);
        assert!(e.tags.contains(&"Showcase"));
        assert!(it.source.starts_with("https://") && it.page.starts_with("https://"));
        for g in [it.windows, it.macos, it.linux] {
            if let Get::Auto(a) = g {
                assert!(!a.is_empty());
                for a in a {
                    assert!(official(a.url), "{}", a.url);
                    assert_eq!(a.sha256.len(), 64);
                    assert!(a.sha256.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
                    assert!(a.size > 0 && !a.picks.is_empty());
                    for pk in a.picks {
                        assert!(clean(pk.from).as_deref() == Some(pk.from));
                    }
                }
                // every auto install ships a licence: from the archive or embedded
                let has = a.iter().any(|a| a.picks.iter().any(|p| p.dest == Dest::Licence));
                assert!(has || it.licence_text.is_some(), "{} needs a licence", it.name);
                // only plugin bundles (and data folders) are taken
                for a in a {
                    for pk in a.picks {
                        if matches!(pk.dest, Dest::Vst3) {
                            assert!(pk.name.ends_with(".vst3"));
                        }
                        if matches!(pk.dest, Dest::Clap) {
                            assert!(pk.name.ends_with(".clap"));
                        }
                    }
                }
            }
        }
    }
    // the small ones are ticked by default, Surge XT is the big one
    assert!(item("surge-xt").unwrap().big);
    assert!(ITEMS.iter().filter(|i| i.big).count() == 1);
    // Windows: Dexed and ADLplug are among the auto installs
    assert!(matches!(item("dexed").unwrap().windows, Get::Auto(_)));
    assert!(matches!(item("adlplug").unwrap().windows, Get::Auto(_)));
    assert!(matches!(item("odin2").unwrap().windows, Get::Page(_)));
}

#[test]
fn only_github_release_downloads_are_fetched() {
    assert!(official("https://github.com/asb2m10/dexed/releases/download/v1.0.1/Dexed-1.0.1-win.zip"));
    assert!(!official("https://example.com/releases/download/x.zip"));
    assert!(!official("http://github.com/a/b/releases/download/x.zip"));
    let e = Http
        .fetch("https://evil.example/x.zip", 1, &mut std::io::sink(), &mut |_| {}, &AtomicBool::new(false))
        .unwrap_err();
    assert!(e.contains("not an official"));
}

/// real archives, when a download cache is around (CHIPWAVE_SHOWCASE_DL=<dir>)
#[test]
fn real_archives_extract_when_available() {
    let Some(dir) = std::env::var_os("CHIPWAVE_SHOWCASE_DL").map(PathBuf::from) else {
        return;
    };
    for (it, a) in [(item("adlplug").unwrap(), ADL_WIN[0]), (item("dexed").unwrap(), DEXED_WIN[0])] {
        let f = dir.join(a.url.rsplit('/').next().unwrap());
        if !f.exists() {
            continue;
        }
        assert_eq!(sha256_file(&f).unwrap(), a.sha256, "{}", it.name);
        let base = tmp(it.id);
        let targets: Vec<_> = a.picks.iter().map(|pk| (*pk, base.join(pk.name))).collect();
        let n = extract(&f, a.kind, &targets, &AtomicBool::new(false)).unwrap();
        assert!(n > 0);
        for (_, t) in &targets {
            assert!(t.exists(), "{}", t.display());
        }
        let _ = std::fs::remove_dir_all(&base);
    }
}
