//! Every symbol the drop preview, the pies, the launcher and the other
//! new workflow views draw exists in the embedded fonts (or egui's
//! fallbacks), so nothing shows as a missing-glyph box.
use super::ui_tests::{frame, harness_with};
use super::*;

/// the UI files whose string literals are checked
const SOURCES: [(&str, &str); 12] = [
    ("dropplan.rs", include_str!("dropplan.rs")),
    ("objpie.rs", include_str!("objpie.rs")),
    ("cliparr.rs", include_str!("cliparr.rs")),
    ("radial.rs", include_str!("radial.rs")),
    ("instpanel.rs", include_str!("instpanel.rs")),
    ("trackpie.rs", include_str!("trackpie.rs")),
    ("inspector.rs", include_str!("inspector.rs")),
    ("pianoview.rs", include_str!("pianoview.rs")),
    ("timeline.rs", include_str!("timeline.rs")),
    ("timeline/edit.rs", include_str!("timeline/edit.rs")),
    ("drumkitview.rs", include_str!("drumkitview.rs")),
    ("dialogs.rs", include_str!("dialogs.rs")),
];

/// the non-ASCII characters inside string literals (comment lines and
/// trailing comments skipped)
fn literal_chars(src: &str) -> Vec<char> {
    let mut v = vec![];
    for line in src.lines() {
        if line.trim_start().starts_with("//") {
            continue;
        }
        let (mut inside, mut esc, mut prev) = (false, false, ' ');
        for c in line.chars() {
            if inside {
                if esc {
                    esc = false;
                } else if c == '\\' {
                    esc = true;
                } else if c == '"' {
                    inside = false;
                } else if !c.is_ascii() && !v.contains(&c) {
                    v.push(c);
                }
            } else if c == '"' {
                inside = true;
            } else if c == '/' && prev == '/' {
                break;
            }
            prev = c;
        }
    }
    v
}

#[test]
fn every_symbol_in_the_new_views_has_a_glyph() {
    for theme in crate::theme::THEMES {
        let mut h = harness_with(fileio::demo_song());
        h.state_mut().theme = theme;
        let ctx = h.ctx.clone();
        h.state_mut().apply_theme(&ctx);
        frame(&mut h);
        frame(&mut h);
        let mut missing = vec![];
        for (name, src) in SOURCES {
            for c in literal_chars(src) {
                for fam in [
                    egui::FontFamily::Proportional,
                    egui::FontFamily::Monospace,
                    egui::FontFamily::Name("label".into()),
                ] {
                    let id = egui::FontId::new(12.0, fam.clone());
                    if !h.ctx.fonts_mut(|f| f.has_glyph(&id, c)) {
                        missing.push(format!("{name}: {c:?} (U+{:04X}) in {fam:?}", c as u32));
                    }
                }
            }
        }
        assert!(
            missing.is_empty(),
            "{}: missing glyphs:\n{}",
            theme.name,
            missing.join("\n")
        );
    }
}
