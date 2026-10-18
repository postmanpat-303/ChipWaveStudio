//! Out-of-process plugin scanner and command-line rendering with the
//! ChipWave Test Synth CLAP fixture (plugins/cw-test-synth).
use std::path::{Path, PathBuf};
use std::process::Command;

const EXE: &str = env!("CARGO_BIN_EXE_chipwave");

fn fixture_lib() -> PathBuf {
    static BUILT: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    BUILT
        .get_or_init(|| {
            let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            let target = root.join("target").join("fixture");
            let st = Command::new(option_env!("CARGO").unwrap_or("cargo"))
                .args(["build", "--release", "--quiet", "--manifest-path"])
                .arg(root.join("plugins/cw-test-synth/Cargo.toml"))
                .env("CARGO_TARGET_DIR", &target)
                .status()
                .expect("cargo build of the test plugin");
            assert!(st.success());
            let f = if cfg!(windows) {
                "cw_test_synth.dll"
            } else if cfg!(target_os = "macos") {
                "libcw_test_synth.dylib"
            } else {
                "libcw_test_synth.so"
            };
            target.join("release").join(f)
        })
        .clone()
}

/// a fresh folder holding copies of the fixture under the given names
fn folder(tag: &str, names: &[&str]) -> PathBuf {
    let d = std::env::temp_dir().join(format!("cw-plugtest-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    for n in names {
        std::fs::copy(fixture_lib(), d.join(n)).unwrap();
    }
    d
}

fn scan_one(p: &Path) -> serde_json::Value {
    let out = Command::new(EXE)
        .arg("--scan-plugin")
        .arg(p)
        .output()
        .unwrap();
    assert!(out.status.success());
    serde_json::from_slice(&out.stdout).expect("scanner JSON")
}

#[test]
fn scanner_child_reports_the_instrument_as_json() {
    let d = folder("json", &["synth.clap"]);
    let v = scan_one(&d.join("synth.clap"));
    assert_eq!(v["ok"], true, "{v}");
    let p = &v["plugins"][0];
    assert_eq!(p["id"], "org.chipwave.test-synth");
    assert_eq!(p["format"], "clap");
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn crashing_and_hanging_plugins_do_not_take_the_scan_down() {
    let d = folder("guard", &["good.clap", "crash.clap", "hang.clap"]);
    let cache = d.join("cache.json");
    let out = Command::new(EXE)
        .arg("--scan-plugins")
        .arg(&d)
        .env("CHIPWAVE_PLUGIN_CACHE", &cache)
        .env("CHIPWAVE_SCAN_TIMEOUT", "3")
        .env("CLAP_PATH", "")
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "{text}");
    let line = |name: &str| {
        text.lines()
            .find(|l| l.contains(name) && l.trim_start().starts_with("clap"))
            .unwrap_or_else(|| panic!("{name} missing in:\n{text}"))
            .to_string()
    };
    assert!(line("good.clap").contains(" ok:"), "{text}");
    assert!(line("crash.clap").contains(" crashed:"), "{text}");
    assert!(line("hang.clap").contains(" timeout:"), "{text}");
    // the cache is written and lists only the good instrument
    let c: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&cache).unwrap()).unwrap();
    let ok: Vec<_> = c["entries"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| {
            e["status"] == "ok" && e["path"].as_str().unwrap().contains(d.to_str().unwrap())
        })
        .collect();
    assert_eq!(ok.len(), 1);
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn render_command_includes_plugin_instruments() {
    let d = folder("render", &["synth.clap"]);
    let song = d.join("plug.cwt.toml");
    std::fs::write(
        &song,
        format!(
            "[song]\ntitle = \"p\"\nbpm = 120\nspeed = 6\ntracks = 2\n\
             [[instrument]]\nslot = 1\nname = \"Plug\"\npatch = \"clap:org.chipwave.test-synth\"\n\
             plugin_path = '{}'\n\
             [[block]]\nname = \"b\"\nlines = 4\nrows = \"\"\"\n00 | C-4 01 .. ....\n\"\"\"\n",
            d.join("synth.clap").display()
        ),
    )
    .unwrap();
    let wav = d.join("out.wav");
    let st = Command::new(EXE)
        .arg("--render")
        .arg(&song)
        .arg(&wav)
        .env("CHIPWAVE_PLUGIN_CACHE", d.join("cache.json"))
        .status()
        .unwrap();
    assert!(st.success());
    let bytes = std::fs::read(&wav).unwrap();
    assert!(bytes.len() > 44 + 1000);
    // 16- or 32-bit data: any non-zero byte past the header means sound
    let loud = bytes[44..].chunks(2).filter(|c| c != &[0, 0]).count();
    assert!(loud > 1000, "rendered silence ({loud} non-zero samples)");
    let _ = std::fs::remove_dir_all(d);
}
