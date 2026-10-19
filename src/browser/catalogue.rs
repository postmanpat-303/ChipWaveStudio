//! Built-in metadata for well-known open-source plugins so the Browser
//! can show a sensible category, creator and a Showcase tag once they are
//! scanned. Generic: any plugin id / name that matches is enriched; unknown
//! plugins stay on the plain Instrument / Effect categories.
//!
//! The binaries themselves are **not** shipped in the ChipWave repo. Local
//! copies for testing live under `/workspace/showcase-plugins/` (see that
//! folder's README). Shipping options are documented in docs/BROWSER.md.

/// one known plugin (or a family of ids / names that share metadata)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    /// Browser category column (e.g. "FM", "Drums")
    pub category: &'static str,
    /// shown in the Creator column (overrides the scan vendor when set)
    pub creator: &'static str,
    /// extra tags merged into the item (always includes "Showcase" for these)
    pub tags: &'static [&'static str],
}

/// id substrings (lowercase) that identify a known plugin, then name
/// needles used when the id did not match. Longer / more specific id
/// needles are listed first.
const BY_ID: &[(&str, Entry)] = &[
    (
        "org.surge-synth-team.surge-xt-fx",
        Entry {
            category: "Effect",
            creator: "Surge Synth Team",
            tags: &["Showcase", "FX"],
        },
    ),
    (
        "org.surge-synth-team.surge-xt",
        Entry {
            category: "Hybrid",
            creator: "Surge Synth Team",
            tags: &["Showcase", "Evolving"],
        },
    ),
    (
        "com.digital-suburban.dexed",
        Entry {
            category: "FM",
            creator: "Digital Suburban",
            tags: &["Showcase", "FM", "Keys"],
        },
    ),
    (
        "org.surge-synth-team.ob-xf",
        Entry {
            category: "Analog",
            creator: "Surge Synth Team",
            tags: &["Showcase", "Analog"],
        },
    ),
    (
        "org.surge-synth-team.monique",
        Entry {
            category: "Bass",
            creator: "Surge Synth Team",
            tags: &["Showcase", "Bass", "Analog"],
        },
    ),
    (
        "com.thewavewarden.odin2",
        Entry {
            category: "Pad",
            creator: "The Wave Warden",
            tags: &["Showcase", "Pad", "Evolving"],
        },
    ),

    (
        "github.com/berarma/aymidi",
        Entry {
            category: "Chip",
            creator: "Bernat Arlandis (berarma)",
            tags: &["Showcase", "Chip", "PSG"],
        },
    ),

    // VST3 class ids (32 hex digits, ChipWave text form)
    (
        "abcdef019182faeb566d624153675854", // Surge XT
        Entry {
            category: "Hybrid",
            creator: "Surge Synth Team",
            tags: &["Showcase", "Evolving"],
        },
    ),
    (
        "abcdef019182faeb566d624153465854", // Surge XT Effects
        Entry {
            category: "Effect",
            creator: "Surge Synth Team",
            tags: &["Showcase", "FX"],
        },
    ),
    (
        "abcdef019182faeb4447534244657864", // Dexed
        Entry {
            category: "FM",
            creator: "Digital Suburban",
            tags: &["Showcase", "FM", "Keys"],
        },
    ),
    (
        "abcdef019182faeb535354784f425846", // OB-Xf
        Entry {
            category: "Analog",
            creator: "Surge Synth Team",
            tags: &["Showcase", "Analog"],
        },
    ),
    (
        "abcdef019182faeb574157414f44494e", // Odin2
        Entry {
            category: "Pad",
            creator: "The Wave Warden",
            tags: &["Showcase", "Pad", "Evolving"],
        },
    ),
    (
        "abcdef019182faeb566d62416d6f6e69", // Monique
        Entry {
            category: "Bass",
            creator: "Surge Synth Team",
            tags: &["Showcase", "Bass", "Analog"],
        },
    ),
    (
        "e2cc953290e863f06405e13924e34b24", // Geonkick
        Entry {
            category: "Drums",
            creator: "Quamplex",
            tags: &["Showcase", "Drums"],
        },
    ),
    (
        "e8fab71815ed46e38b5983101e12993f", // sfizz
        Entry {
            category: "Sampler",
            creator: "SFZTools",
            tags: &["Showcase"],
        },
    ),
    (
        "abcdef019182faeb4a50436d41444c4d", // ADLplug
        Entry {
            category: "Chip",
            creator: "Jean Pierre Cimalando",
            tags: &["Showcase", "Chip", "FM"],
        },
    ),
];

/// name matchers (lowercase, compared to the whole name) when id is unknown
/// (typical for VST3 class-id plugins that don't expose a URI)
const BY_NAME: &[(&str, Entry)] = &[
    (
        "surge xt effects",
        Entry {
            category: "Effect",
            creator: "Surge Synth Team",
            tags: &["Showcase", "FX"],
        },
    ),
    (
        "surge xt",
        Entry {
            category: "Hybrid",
            creator: "Surge Synth Team",
            tags: &["Showcase", "Evolving"],
        },
    ),
    (
        "dexed",
        Entry {
            category: "FM",
            creator: "Digital Suburban",
            tags: &["Showcase", "FM", "Keys"],
        },
    ),
    (
        "ob-xf",
        Entry {
            category: "Analog",
            creator: "Surge Synth Team",
            tags: &["Showcase", "Analog"],
        },
    ),
    (
        "odin2",
        Entry {
            category: "Pad",
            creator: "The Wave Warden",
            tags: &["Showcase", "Pad", "Evolving"],
        },
    ),
    (
        "odin 2",
        Entry {
            category: "Pad",
            creator: "The Wave Warden",
            tags: &["Showcase", "Pad", "Evolving"],
        },
    ),
    (
        "moniquemonosynth",
        Entry {
            category: "Bass",
            creator: "Surge Synth Team",
            tags: &["Showcase", "Bass", "Analog"],
        },
    ),
    (
        "monique",
        Entry {
            category: "Bass",
            creator: "Surge Synth Team",
            tags: &["Showcase", "Bass", "Analog"],
        },
    ),
    (
        "geonkick",
        Entry {
            category: "Drums",
            creator: "Quamplex",
            tags: &["Showcase", "Drums"],
        },
    ),
    (
        "sfizz",
        Entry {
            category: "Sampler",
            creator: "SFZTools",
            tags: &["Showcase"],
        },
    ),
    (
        "adlplug",
        Entry {
            category: "Chip",
            creator: "Jean Pierre Cimalando",
            tags: &["Showcase", "Chip", "FM"],
        },
    ),
    (
        "aymidi",
        Entry {
            category: "Chip",
            creator: "Bernat Arlandis (berarma)",
            tags: &["Showcase", "Chip", "PSG"],
        },
    ),
];

/// look up metadata for a scanned plugin by its id and display name
pub fn lookup(id: &str, name: &str) -> Option<Entry> {
    let id_l = id.to_lowercase();
    for (needle, e) in BY_ID {
        if id_l.contains(needle) {
            return Some(*e);
        }
    }
    let name_l = name.to_lowercase();
    // strip common suffixes / whitespace for matching
    let compact: String = name_l.chars().filter(|c| !c.is_whitespace()).collect();
    for (needle, e) in BY_NAME {
        let n: String = needle.chars().filter(|c| !c.is_whitespace()).collect();
        if name_l == *needle || compact == n {
            return Some(*e);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_ids_and_names_map_to_categories() {
        let s = lookup("org.surge-synth-team.surge-xt", "Surge XT").unwrap();
        assert_eq!(s.category, "Hybrid");
        assert!(s.tags.contains(&"Showcase"));
        // effects id must not steal the instrument entry
        let fx = lookup("org.surge-synth-team.surge-xt-fx", "Surge XT Effects").unwrap();
        assert_eq!(fx.category, "Effect");
        assert_eq!(
            lookup("com.digital-suburban.dexed", "Dexed").unwrap().category,
            "FM"
        );
        assert_eq!(
            lookup("org.surge-synth-team.OB-Xf", "OB-Xf").unwrap().category,
            "Analog"
        );
        assert_eq!(
            lookup("com.thewavewarden.odin2", "Odin2").unwrap().category,
            "Pad"
        );
        assert_eq!(
            lookup("org.surge-synth-team.monique-monosynth", "Monique")
                .unwrap()
                .category,
            "Bass"
        );
        // VST3 class ids: fall back to the name
        assert_eq!(lookup("ABCDEF0123456789", "Geonkick").unwrap().category, "Drums");
        assert_eq!(lookup("deadbeef", "sfizz").unwrap().category, "Sampler");
        assert_eq!(lookup("deadbeef", "ADLplug").unwrap().category, "Chip");
        assert_eq!(lookup("deadbeef", "Odin 2").unwrap().category, "Pad");
        assert_eq!(lookup("github.com/berarma/aymidi", "AyMidi").unwrap().category, "Chip");
        assert_eq!(lookup("deadbeef", "AyMidi").unwrap().creator, "Bernat Arlandis (berarma)");
        // VST3 class id (hex) matches without relying on the name
        assert_eq!(
            lookup("ABCDEF019182FAEB566D624153675854", "Anything")
                .unwrap()
                .category,
            "Hybrid"
        );
    }

    #[test]
    fn unknown_plugins_are_untouched() {
        assert!(lookup("com.example.unknown", "Mystery Synth").is_none());
        assert!(lookup("", "Some Random Name").is_none());
    }
}
