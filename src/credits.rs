//! Credits shown in Help > About (Credits and Licences tabs). One structured list,
//! so the README and the website can be kept in sync with it. Nothing here means
//! code was copied: the `note` says when it was an idea or a design reference
//! only, and THIRD_PARTY_LICENSES.txt holds the full notices.

/// headed, collapsible sections of the Credits tab (in this order)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Group {
    Voice,
    Synthesis,
    Drums,
    Effects,
    Plugins,
    Libraries,
    Inspiration,
    Research,
    Heritage,
}

impl Group {
    pub const ALL: [Group; 9] = [
        Group::Voice,
        Group::Synthesis,
        Group::Drums,
        Group::Effects,
        Group::Plugins,
        Group::Libraries,
        Group::Inspiration,
        Group::Research,
        Group::Heritage,
    ];
    pub fn title(self) -> &'static str {
        match self {
            Group::Voice => "Voice & speech",
            Group::Synthesis => "Synthesis & generators",
            Group::Drums => "Drums & Chip FX",
            Group::Effects => "Effects, mixing & loudness",
            Group::Plugins => "Plugin hosting",
            Group::Libraries => "Libraries",
            Group::Inspiration => "Inspiration",
            Group::Research => "Research & papers",
            Group::Heritage => "Tracker heritage",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Credit {
    pub group: Group,
    /// project, paper (with its year) or person; bold, and a link when `url` is set
    pub name: &'static str,
    /// author(s) or organisation ("" if `name` already says it)
    pub by: &'static str,
    /// what ChipWave uses it for
    pub what: &'static str,
    /// SPDX-style licence for the badge ("" for papers, products and standards)
    pub licence: &'static str,
    /// muted remark: "idea only, no code copied", journal, …
    pub note: &'static str,
    pub url: &'static str,
}

const fn c(
    group: Group,
    name: &'static str,
    by: &'static str,
    what: &'static str,
    licence: &'static str,
    note: &'static str,
    url: &'static str,
) -> Credit {
    Credit {
        group,
        name,
        by,
        what,
        licence,
        note,
        url,
    }
}

use Group::*;

#[rustfmt::skip]
pub const CREDITS: &[Credit] = &[
    // Voice & speech
    c(Voice, "Mutable Instruments Plaits", "Emilie Gillet",
        "Voice engine inspired by its speech synth (\u{201c}naive speech\u{201d})", "MIT",
        "No code copied; MIT notice in THIRD_PARTY_LICENSES",
        "https://github.com/pichenettes/eurorack/tree/master/plaits/dsp/speech"),
    c(Voice, "Csound manual formant table", "Csound community",
        "Vowel formant data", "LGPL", "",
        "https://csound.com/docs/manual/MiscFormants.html"),
    c(Voice, "Pink Trombone", "Neil Thapen",
        "Throat mode inspired by its vocal-tract design (tube, tongue, lips, nose)", "MIT",
        "Independent implementation, no code copied",
        "https://dood.al/pinktrombone/"),
    // Synthesis & generators
    c(Synthesis, "Minimoog", "R. Moog",
        "Analog generator: the design it follows", "", "Design reference", ""),
    c(Synthesis, "polyBLEP oscillators", "after Välimäki & Huovilainen (2007)",
        "Analog generator oscillators", "", "",
        "https://doi.org/10.1109/MSP.2007.323276"),
    c(Synthesis, "Vital", "Matt Tytel",
        "Analog / FM-4 inspiration: oscillator drift and macro ideas", "GPL-3",
        "No code copied", "https://github.com/mtytel/vital"),
    c(Synthesis, "Surge XT", "Surge Synth Team",
        "Analog / Noise inspiration; also used to test plugin hosting (not bundled)", "GPL-3",
        "No code copied", "https://github.com/surge-synthesizer/surge"),
    c(Synthesis, "Dexed", "Pascal Gauthier",
        "FM-4 inspiration", "GPL-3", "No code copied",
        "https://github.com/asb2m10/dexed"),
    c(Synthesis, "MSFA", "Raph Levien",
        "The FM engine Dexed is built on; FM-4 inspiration", "Apache-2.0", "No code copied",
        "https://github.com/google/music-synthesizer-for-android"),
    c(Synthesis, "DaisySP ladder", "R. van Hoesel, Infrasonic Audio",
        "Moog ladder cross-checks in the tests", "MIT", "",
        "https://github.com/electro-smith/DaisySP"),
    c(Synthesis, "fundsp", "S. Perttu",
        "Moog ladder cross-checks in the tests", "MIT/Apache-2.0", "",
        "https://github.com/SamiPerttu/fundsp"),
    c(Synthesis, "MoogLadders", "D. Diakopoulos",
        "Moog ladder cross-checks in the tests", "", "",
        "https://github.com/ddiakopoulos/MoogLadders"),
    c(Synthesis, "Pink noise filter", "Paul Kellet (musicdsp.org)",
        "Noise generator: pink noise", "", "",
        "https://www.musicdsp.org/en/latest/Filters/76-pink-noise-filter.html"),
    c(Synthesis, "SuperCollider Dust / DaisySP Dust", "SuperCollider, Electrosmith",
        "Noise generator: crackle after them", "", "No code copied",
        "https://github.com/supercollider/supercollider"),
    // Drums & Chip FX
    c(Drums, "Sonic Pi sc808", "S. Aaron et al.",
        "Drum Synth recipes, after Yoshinosuke Horiuchi's SC-808", "MIT", "",
        "https://github.com/sonic-pi-net/sonic-pi"),
    c(Drums, "Plaits drum models", "Emilie Gillet; DaisySP port by Electrosmith",
        "Drum models", "MIT", "", "https://github.com/electro-smith/DaisySP"),
    c(Drums, "sfxr", "Tomas Pettersson (DrPetter)",
        "Chip FX generator", "MIT", "", "https://www.drpetter.se/project_sfxr.html"),
    c(Drums, "sfxr-rs", "Teemu Erkkola",
        "Chip FX: the Rust port of sfxr it follows", "MIT", "",
        "https://github.com/bzar/sfxr-rs"),
    // Effects, mixing & loudness
    c(Effects, "libebur128", "Jan Kokemüller",
        "K-weighting formulas for the loudness meter", "MIT", "No code copied",
        "https://github.com/jiixyj/libebur128"),
    c(Effects, "pyloudnorm", "C. Steinmetz",
        "K-weighting formulas for the loudness meter", "MIT", "No code copied",
        "https://github.com/csteinmetz1/pyloudnorm"),
    c(Effects, "Designing a straightforward limiter", "Geraint Luff, Signalsmith Audio",
        "Lookahead limiter design of the master limiter", "", "",
        "https://signalsmith-audio.co.uk/writing/2022/limiter/"),
    c(Effects, "Analog octave pedals", "e.g. Boss OC-2",
        "Sub bass: the octave divider follows them", "", "Idea only", ""),
    c(Effects, "Analog phaser pedals", "e.g. MXR Phase 90, Electro-Harmonix Small Stone",
        "Phaser: swept all-pass stages mixed with the dry signal", "",
        "Idea only, no circuit or code copied", ""),
    c(Effects, "Bode frequency shifter", "Harald Bode",
        "Frequency shifter: the single-sideband (Hilbert + quadrature) design", "",
        "Idea only, no circuit or code copied", "https://en.wikipedia.org/wiki/Harald_Bode"),
    c(Research, "Niemitalo, Hilbert transform by IIR all-pass filters (2001)", "Olli Niemitalo",
        "Frequency shifter: the all-pass coefficients of its 90° phase splitter", "",
        "Published coefficients only, no code copied", "https://yehar.com/blog/?p=368"),
    // Plugin hosting
    c(Plugins, "CLAP", "the free-audio project (Alexandre Bique et al.)",
        "Plugin hosting: the plugin standard ChipWave hosts", "MIT", "",
        "https://github.com/free-audio/clap"),
    c(Plugins, "clap-sys", "micahrj et al.",
        "CLAP bindings", "MIT/Apache-2.0", "", "https://github.com/micahrj/clap-sys"),
    c(Plugins, "VST 3 SDK", "Steinberg Media Technologies GmbH",
        "VST3 hosting: the VST 3 API (interface definitions) ChipWave hosts; VST is a registered trademark of Steinberg Media Technologies GmbH",
        "MIT", "Since SDK 3.8.0; the API through the vst3 crate",
        "https://github.com/steinbergmedia/vst3sdk"),
    c(Plugins, "vst3 / com-scrape-types", "Micah Johnston (coupler-rs)",
        "VST3 bindings generated from the SDK headers, COM helpers", "MIT/Apache-2.0", "",
        "https://github.com/coupler-rs/vst3-rs"),
    c(Plugins, "clack", "prokopyl",
        "Host design reference", "MIT/Apache-2.0", "No code copied",
        "https://github.com/prokopyl/clack"),
    c(Plugins, "Carla", "Filipe Coelho (falkTX)",
        "Plugin scanning ideas", "GPL-2.0+", "No code copied",
        "https://github.com/falkTX/Carla"),
    c(Plugins, "Ardour", "Ardour community",
        "Plugin scanning ideas", "GPL-2.0+", "No code copied", "https://ardour.org"),
    c(Plugins, "libloading", "S. Kazlauskas",
        "Plugin loading", "ISC", "", "https://github.com/nagisa/rust_libloading"),
    c(Plugins, "windows-sys", "Microsoft",
        "Plugin editor windows on Windows", "MIT/Apache-2.0", "",
        "https://github.com/microsoft/windows-rs"),
    c(Plugins, "base64", "Rust community",
        "Plugin state in saved songs", "MIT/Apache-2.0", "",
        "https://github.com/marshallpierce/rust-base64"),
    // Libraries
    c(Libraries, "egui / eframe", "Emil Ernerfeldt and contributors",
        "The user interface", "MIT/Apache-2.0", "", "https://github.com/emilk/egui"),
    c(Libraries, "cpal", "RustAudio", "Audio output", "Apache-2.0", "",
        "https://github.com/RustAudio/cpal"),
    c(Libraries, "midir", "Patrick Reisert", "MIDI input", "MIT", "",
        "https://github.com/Boddlnagg/midir"),
    c(Libraries, "hound", "Ruud van Asseldonk", "WAV import and export", "Apache-2.0", "",
        "https://github.com/ruuda/hound"),
    c(Libraries, "mp3lame-encoder / LAME", "LAME project", "MP3 export", "LGPL-3.0", "",
        "https://lame.sourceforge.io/"),
    c(Libraries, "toml, serde, zip, rfd, ureq, png", "Rust community",
        "Song files, file dialogs, the update check and screenshots; more crates in THIRD_PARTY_LICENSES",
        "MIT/Apache-2.0", "", ""),
    // Inspiration
    c(Inspiration, "Scaler", "Plugin Boutique",
        "Chord helper inspired by its chord pads and progressions", "",
        "Idea only, no material used",
        "https://www.pluginboutique.com/product/2-Effects/26-Chords/12468-Scaler-3"),
    c(Inspiration, "Bitwig Studio", "Bitwig",
        "Generators: the macro + dice workflow idea", "", "Idea only, no code or assets", ""),
    c(Inspiration, "Yamaha DX21 / TX81Z", "Yamaha",
        "FM-4 generator: algorithms after them", "", "", ""),
    c(Inspiration, "Roland TR-808", "Roland",
        "Transform: the \u{201c}metal\u{201d} ratios are its cymbal oscillator frequencies", "", "", ""),
    c(Inspiration, "Roland TB-303", "Roland",
        "Glide and accent (the 3C / 3D effects) imitate it", "", "", ""),
    // Research & papers
    c(Research, "Peterson & Barney (1952)", "G. E. Peterson, H. L. Barney",
        "Vowel data (the ER and AE vowels)", "", "JASA 24(2)",
        "https://doi.org/10.1121/1.1906875"),
    c(Research, "Rosenberg (1971)", "A. E. Rosenberg",
        "Glottal pulse shape", "", "JASA", "https://doi.org/10.1121/1.1912389"),
    c(Research, "Fant, Liljencrants & Lin (1985)", "",
        "LF glottal flow model: the voice's pulse and tension", "", "", ""),
    c(Research, "Klatt (1980)", "Dennis H. Klatt",
        "Parallel formant synthesis", "", "JASA 67(3)", "https://doi.org/10.1121/1.383940"),
    c(Research, "Delattre, Liberman & Cooper (1955)", "",
        "Consonant loci (formant transitions)", "", "JASA",
        "https://doi.org/10.1121/1.1908024"),
    c(Research, "Stevens, Acoustic Phonetics (1998)", "K. N. Stevens",
        "Consonant noise bands", "", "MIT Press", ""),
    c(Research, "Kelly & Lochbaum (1962)", "J. L. Kelly, C. C. Lochbaum",
        "The vocal-tract tube model of Throat mode", "", "", ""),
    c(Research, "ITU-R BS.1770-4 (2015)", "ITU",
        "Loudness meter + true peak", "", "Standard", "https://www.itu.int/rec/R-REC-BS.1770"),
    c(Research, "EBU R 128 (2020)", "EBU",
        "Loudness meter: gating and momentary / short-term / integrated LUFS", "",
        "Standard; with EBU Tech 3341", "https://tech.ebu.ch/publications/r128"),
    c(Research, "Smith & Angell (1982)", "J. O. Smith, J. B. Angell",
        "Transform resonators: the constant-gain resonator", "",
        "Intro to Digital Filters (CCRMA)", "https://ccrma.stanford.edu/~jos/filters/"),
    c(Research, "Fletcher & Rossing (1998)", "N. H. Fletcher, T. D. Rossing",
        "Bell and bar mode ratios", "", "The Physics of Musical Instruments",
        "https://doi.org/10.1007/978-0-387-21603-4"),
    c(Research, "Linkwitz (1976)", "S. H. Linkwitz",
        "Sub crossover: Linkwitz-Riley", "", "", "https://www.linkwitzlab.com/filters.htm"),
    c(Research, "Zavalishin, The Art of VA Filter Design (2018)", "V. Zavalishin",
        "Moog ladder, the TPT state-variable filter and the Phaser's all-pass stages", "", "",
        "https://www.native-instruments.com/fileadmin/ni_media/downloads/pdf/VAFilterDesign_2.1.0.pdf"),
    c(Research, "Huovilainen (2004)", "A. Huovilainen",
        "Moog ladder (non-linear model)", "", "DAFx-04", ""),
    c(Research, "Välimäki & Huovilainen (2006)", "V. Välimäki, A. Huovilainen",
        "Moog ladder (vintage mode)", "", "Computer Music Journal", ""),
    c(Research, "Krumhansl & Kessler (1982)", "C. L. Krumhansl, E. J. Kessler",
        "Key profiles for key finding (Krumhansl–Schmuckler algorithm)", "",
        "Psychological Review", "https://doi.org/10.1037/0033-295X.89.4.334"),
    // Tracker heritage
    c(Heritage, "OctaMED", "Teijo Kinnunen and Ray Burt-Frost",
        "The workflow ChipWave follows", "", "", "https://en.wikipedia.org/wiki/OctaMED"),
];

/// all text of a credit, for the search box and the tests
pub fn haystack(c: &Credit) -> String {
    format!(
        "{} {} {} {} {} {}",
        c.name,
        c.by,
        c.what,
        c.licence,
        c.note,
        c.group.title()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Everyone named in the About credits before the 0.2.4.x redesign (the old
    /// one-line-per-credit list) must still be credited, and every old link kept.
    #[test]
    fn every_previous_credit_is_still_present() {
        const OLD_NAMES: &[&str] = &[
            "Mutable Instruments Plaits",
            "Emilie Gillet",
            "Csound",
            "LGPL",
            "Peterson & Barney (1952)",
            "JASA 24(2)",
            "Rosenberg (1971)",
            "Fant, Liljencrants & Lin (1985)",
            "Klatt (1980)",
            "JASA 67(3)",
            "ITU-R BS.1770-4",
            "EBU R 128",
            "libebur128",
            "Jan Kokemüller",
            "pyloudnorm",
            "C. Steinmetz",
            "Geraint Luff",
            "Signalsmith Audio",
            "Pink Trombone",
            "Neil Thapen",
            "Delattre, Liberman & Cooper (1955)",
            "K. N. Stevens",
            "Acoustic Phonetics",
            "J. O. Smith",
            "J. B. Angell",
            "(1982)",
            "N. H. Fletcher",
            "T. D. Rossing",
            "The Physics of Musical Instruments",
            "Linkwitz-Riley",
            "S. H. Linkwitz",
            "octave pedals",
            "Zavalishin",
            "The Art of VA Filter Design",
            "Huovilainen",
            "DAFx-04",
            "Välimäki & Huovilainen (2006)",
            "DaisySP",
            "R. van Hoesel",
            "Infrasonic Audio",
            "fundsp",
            "S. Perttu",
            "MoogLadders",
            "D. Diakopoulos",
            "Sonic Pi",
            "sc808",
            "S. Aaron",
            "Yoshinosuke Horiuchi",
            "SC-808",
            "Electrosmith",
            "sfxr",
            "Tomas Pettersson",
            "DrPetter",
            "sfxr-rs",
            "Teemu Erkkola",
            "Minimoog",
            "R. Moog",
            "polyBLEP",
            "Välimäki & Huovilainen (2007)",
            "Vital",
            "Matt Tytel",
            "Surge XT",
            "Surge Synth Team",
            "Dexed",
            "Pascal Gauthier",
            "MSFA",
            "Raph Levien",
            "Apache-2.0",
            "DX21 / TX81Z",
            "CLAP",
            "free-audio",
            "Alexandre Bique",
            "clap-sys",
            "micahrj",
            "VST 3 SDK",
            "Steinberg Media Technologies GmbH",
            "vst3 / com-scrape-types",
            "Micah Johnston (coupler-rs)",
            "clack",
            "prokopyl",
            "Carla",
            "Filipe Coelho (falkTX)",
            "Ardour",
            "GPL-2.0+",
            "libloading",
            "S. Kazlauskas",
            "ISC",
            "windows-sys",
            "Microsoft",
            "base64",
            "Paul Kellet",
            "musicdsp.org",
            "SuperCollider Dust",
            "DaisySP Dust",
            "Krumhansl–Schmuckler",
            "Krumhansl & Kessler (1982)",
            "Scaler",
            "Plugin Boutique",
            "idea only, no material used",
            "OctaMED",
            "Teijo Kinnunen",
            "Ray Burt-Frost",
            "The workflow ChipWave follows",
            "No code copied",
            "GPL-3",
        ];
        const OLD_URLS: &[&str] = &[
            "https://github.com/pichenettes/eurorack/tree/master/plaits/dsp/speech",
            "https://csound.com/docs/manual/MiscFormants.html",
            "https://doi.org/10.1121/1.1906875",
            "https://doi.org/10.1121/1.1912389",
            "https://doi.org/10.1121/1.383940",
            "https://www.itu.int/rec/R-REC-BS.1770",
            "https://github.com/jiixyj/libebur128",
            "https://signalsmith-audio.co.uk/writing/2022/limiter/",
            "https://dood.al/pinktrombone/",
            "https://doi.org/10.1121/1.1908024",
            "https://ccrma.stanford.edu/~jos/filters/",
            "https://doi.org/10.1007/978-0-387-21603-4",
            "https://www.linkwitzlab.com/filters.htm",
            "https://www.native-instruments.com/fileadmin/ni_media/downloads/pdf/VAFilterDesign_2.1.0.pdf",
            "https://github.com/ddiakopoulos/MoogLadders",
            "https://github.com/sonic-pi-net/sonic-pi",
            "https://github.com/electro-smith/DaisySP",
            "https://github.com/bzar/sfxr-rs",
            "https://doi.org/10.1109/MSP.2007.323276",
            "https://github.com/mtytel/vital",
            "https://github.com/surge-synthesizer/surge",
            "https://github.com/asb2m10/dexed",
            "https://github.com/free-audio/clap",
            "https://github.com/micahrj/clap-sys",
            "https://github.com/falkTX/Carla",
            "https://github.com/nagisa/rust_libloading",
            "https://www.musicdsp.org/en/latest/Filters/76-pink-noise-filter.html",
            "https://doi.org/10.1037/0033-295X.89.4.334",
            "https://www.pluginboutique.com/product/2-Effects/26-Chords/12468-Scaler-3",
            "https://en.wikipedia.org/wiki/OctaMED",
        ];
        let all: String = CREDITS.iter().map(haystack).collect::<Vec<_>>().join("\n");
        let lower = all.to_lowercase();
        for n in OLD_NAMES {
            assert!(
                all.contains(n) || lower.contains(&n.to_lowercase()),
                "credit lost: {n}"
            );
        }
        for u in OLD_URLS {
            assert!(CREDITS.iter().any(|c| c.url == *u), "credit link lost: {u}");
        }
        // never the full name
        assert!(!lower.contains("derek") && !lower.contains("heasley"));
    }

    #[test]
    fn every_group_has_credits_and_entries_are_tidy() {
        for g in Group::ALL {
            assert!(CREDITS.iter().any(|c| c.group == g), "{g:?} is empty");
        }
        for c in CREDITS {
            assert!(!c.name.is_empty() && !c.what.is_empty(), "{c:?}");
            assert!(c.url.is_empty() || c.url.starts_with("https://"), "{c:?}");
        }
        // research entries carry their year
        for c in CREDITS.iter().filter(|c| c.group == Group::Research) {
            assert!(
                c.name.contains("(1") || c.name.contains("(2"),
                "no year: {}",
                c.name
            );
        }
    }
}
