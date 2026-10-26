//! Controller maps: a MIDI learn list saved as a small TOML file, one per
//! controller (docs/MIDI.md). The file holds only CC numbers and knob ids
//! from the knob list (`crate::knobs`), so it is data, not code:
//!
//! ```toml
//! name = "My controller"
//! version = 1
//!
//! [[map]]
//! cc = 74
//! id = "instrument.1.cutoff"
//! ```

use crate::automation::Target;
use crate::model::Song;
use serde::{Deserialize, Serialize};

/// file extension of a controller map
pub const EXT: &str = "toml";
/// the version this build writes; newer files still load what they can
pub const VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CtlMap {
    /// the controller's name (the MIDI device, or what the user typed)
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub map: Vec<Entry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    pub cc: u8,
    /// a knob id, e.g. "instrument.1.cutoff" or "mixer.ch.2.volume"
    pub id: String,
}

/// What loading a map gave: the new learn list and what was left out.
#[derive(Debug, Default, PartialEq)]
pub struct Loaded {
    pub learn: Vec<(u8, String)>,
    /// learned knobs this song does not have (kept: another song may)
    pub absent: usize,
    /// entries skipped: an id that is no knob, a CC above 127, a repeat
    pub skipped: Vec<String>,
}

/// The TOML text of a learn list.
pub fn to_text(name: &str, learn: &[(u8, String)]) -> Result<String, String> {
    let m = CtlMap {
        name: name.to_string(),
        version: VERSION,
        map: learn
            .iter()
            .map(|(cc, id)| Entry {
                cc: *cc,
                id: id.clone(),
            })
            .collect(),
    };
    toml::to_string(&m).map_err(|e| e.to_string())
}

/// Read a map. Ids are checked against the knob list of `song`: a knob of
/// this song is kept under its own id, a well-formed id of a knob this song
/// lacks is kept as written, anything else is skipped.
pub fn from_text(text: &str, song: &Song) -> Result<(String, Loaded), String> {
    let m: CtlMap = toml::from_str(text).map_err(|e| e.to_string())?;
    let mut out = Loaded::default();
    for e in m.map {
        if e.cc > 127 {
            out.skipped.push(format!("CC {}", e.cc));
            continue;
        }
        let id = match crate::knobs::resolve(song, &e.id) {
            Some(t) => t.id(),
            None if Target::parse(&e.id).is_some() => {
                out.absent += 1;
                e.id.clone()
            }
            None => {
                out.skipped.push(e.id);
                continue;
            }
        };
        // one target per CC and one CC per target, as MIDI learn keeps it
        if out.learn.iter().any(|(c, x)| *c == e.cc || *x == id) {
            out.skipped.push(format!("CC {} {}", e.cc, e.id));
            continue;
        }
        out.learn.push((e.cc, id));
    }
    Ok((m.name, out))
}

/// where controller maps are kept by default
pub fn dir() -> Option<std::path::PathBuf> {
    crate::settings::config_dir().map(|d| d.join("Controllers"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_map_round_trips() {
        let song = crate::fileio::demo_song();
        let learn = vec![
            (74u8, "instrument.1.cutoff".to_string()),
            (7, "mixer.ch.2.volume".to_string()),
            (20, "tempo".to_string()),
        ];
        let text = to_text("Desk keys", &learn).unwrap();
        assert!(text.contains("name = \"Desk keys\""), "{text}");
        assert!(text.contains("[[map]]"), "{text}");
        let (name, l) = from_text(&text, &song).unwrap();
        assert_eq!(name, "Desk keys");
        assert_eq!(l.learn, learn);
        assert_eq!(l.absent, 0);
        assert!(l.skipped.is_empty());
    }

    #[test]
    fn unknown_ids_are_skipped_and_absent_knobs_kept() {
        let song = crate::fileio::demo_song();
        let text = r#"
name = "Pads"
version = 99
future_field = "ignored"

[[map]]
cc = 10
id = "instrument.99.cutoff"

[[map]]
cc = 11
id = "rm -rf /"

[[map]]
cc = 200
id = "tempo"

[[map]]
cc = 12
id = "mixer.master.volume"

[[map]]
cc = 12
id = "tempo"
"#;
        let (name, l) = from_text(text, &song).unwrap();
        assert_eq!(name, "Pads");
        // the long form is stored under the knob's own id
        assert_eq!(
            l.learn,
            vec![
                (10, "instrument.99.cutoff".to_string()),
                (12, Target::Master.id()),
            ]
        );
        assert_eq!(l.absent, 1);
        assert_eq!(l.skipped.len(), 3, "{:?}", l.skipped);
    }

    #[test]
    fn bad_files_say_why() {
        let song = crate::fileio::demo_song();
        assert!(from_text("[[map]]\ncc = \"x\"", &song).is_err());
        assert!(from_text("not toml at all ===", &song).is_err());
        let (_, l) = from_text("", &song).unwrap();
        assert!(l.learn.is_empty());
    }
}
