//! The LilyPond (.ly) writer.
use super::{value_name, Bar, Clef, Content, Item, KeySig, Part, Score, Staff};
use std::fmt::Write;

fn esc(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
}

fn letter(l: char, a: i8) -> String {
    let mut s = l.to_ascii_lowercase().to_string();
    let acc = match a {
        2 => "isis",
        1 => "is",
        -1 => "es",
        -2 => "eses",
        _ => "",
    };
    s.push_str(acc);
    s
}

fn pitch(k: KeySig, p: u8) -> String {
    let (l, a, oct) = k.spell(p);
    let mut s = letter(l, a);
    let marks = oct - 3;
    for _ in 0..marks.max(0) {
        s.push('\'');
    }
    for _ in 0..(-marks).max(0) {
        s.push(',');
    }
    s
}

fn bar_rest(b: &Bar) -> String {
    match (b.num, b.den) {
        (4, 4) => "R1".into(),
        (2, 4) => "R2".into(),
        (3, 4) => "R2.".into(),
        (6, 8) => "R2.".into(),
        (n, d) => format!("R1*{n}/{d}"),
    }
}

fn item(k: KeySig, it: &Item, b: &Bar) -> String {
    if it.bar_rest {
        return bar_rest(b);
    }
    let (d, _, _) = value_name(it.dur);
    let tie = if it.tie { "~" } else { "" };
    match &it.content {
        Content::Rest => format!("r{d}"),
        Content::Notes(ps) if ps.len() == 1 => format!("{}{d}{tie}", pitch(k, ps[0])),
        Content::Notes(ps) => {
            let inner: Vec<String> = ps.iter().map(|&p| pitch(k, p)).collect();
            format!("<{}>{d}{tie}", inner.join(" "))
        }
        Content::Drums(ds) if ds.len() == 1 => format!("{}{d}", ds[0].info().ly),
        Content::Drums(ds) => {
            let inner: Vec<&str> = ds.iter().map(|d| d.info().ly).collect();
            format!("<{}>{d}", inner.join(" "))
        }
    }
}

/// the music of one staff; `lead` = the first staff, which carries the
/// tempo and rehearsal marks
fn staff_music(sc: &Score, st: &Staff, drums: bool, lead: bool) -> String {
    let mut s = String::new();
    if !drums {
        let (l, a) = sc.key.home();
        let _ = write!(
            s,
            "\\clef {} \\key {} {}\n    ",
            st.clef.ly(),
            letter(l, a),
            if sc.key.minor { "\\minor" } else { "\\major" }
        );
    }
    let mut sig = (0u8, 0u8);
    for (i, (b, items)) in sc.bars.iter().zip(&st.bars).enumerate() {
        if (b.num, b.den) != sig {
            sig = (b.num, b.den);
            let _ = write!(s, "\\time {}/{} ", b.num, b.den);
        }
        if lead {
            if let Some((_, name)) = sc.marks.iter().find(|m| m.0 == i) {
                let _ = write!(s, "\\mark \\markup \\box \"{}\" ", esc(name));
            }
            if let Some((bpm, text)) = b.tempo {
                let bpm = bpm.round().max(1.0) as i32;
                if text.is_empty() {
                    let _ = write!(s, "\\tempo 4 = {bpm} ");
                } else {
                    let _ = write!(s, "\\tempo \"{text}\" 4 = {bpm} ");
                }
            }
        }
        let parts: Vec<String> = items.iter().map(|it| item(sc.key, it, b)).collect();
        let _ = write!(s, "{} | % {}\n    ", parts.join(" "), i + 1);
    }
    s.push_str("\\bar \"|.\"");
    s
}

fn part(sc: &Score, p: &Part, lead: bool, out: &mut String) {
    let name = esc(&p.name);
    if p.drums {
        let _ = write!(
            out,
            "  \\new DrumStaff \\with {{ instrumentName = \"{name}\" }} \\drummode {{\n    {}\n  }}\n",
            staff_music(sc, &p.staves[0], true, lead)
        );
    } else if p.staves.len() > 1 {
        let _ = writeln!(
            out,
            "  \\new PianoStaff \\with {{ instrumentName = \"{name}\" }} <<"
        );
        for (i, st) in p.staves.iter().enumerate() {
            let _ = write!(
                out,
                "    \\new Staff {{\n    {}\n    }}\n",
                staff_music(sc, st, false, lead && i == 0)
            );
        }
        out.push_str("  >>\n");
    } else {
        let _ = write!(
            out,
            "  \\new Staff \\with {{ instrumentName = \"{name}\" }} {{\n    {}\n  }}\n",
            staff_music(sc, &p.staves[0], false, lead)
        );
    }
}

/// the score as a LilyPond file
pub fn to_ly(sc: &Score) -> String {
    let mut s = String::new();
    s.push_str("% Exported from ChipWave Studio\n\\version \"2.24.0\"\n\n\\header {\n");
    if !sc.title.trim().is_empty() {
        let _ = writeln!(s, "  title = \"{}\"", esc(sc.title.trim()));
    }
    if !sc.composer.trim().is_empty() {
        let _ = writeln!(s, "  composer = \"{}\"", esc(sc.composer.trim()));
    }
    s.push_str("  tagline = \"Exported from ChipWave Studio\"\n}\n\n");
    s.push_str("\\paper {\n  #(set-paper-size \"a4\")\n}\n\n\\score {\n  <<\n");
    if sc.parts.is_empty() {
        // an empty score still shows the bars
        let st = Staff {
            clef: Clef::Treble,
            bars: sc
                .bars
                .iter()
                .map(|b| {
                    vec![Item {
                        dur: b.len,
                        content: Content::Rest,
                        tie: false,
                        tied: false,
                        bar_rest: true,
                    }]
                })
                .collect(),
        };
        let p = Part {
            name: String::new(),
            drums: false,
            staves: vec![st],
        };
        part(sc, &p, true, &mut s);
    }
    for (i, p) in sc.parts.iter().enumerate() {
        part(sc, p, i == 0, &mut s);
    }
    s.push_str("  >>\n  \\layout {\n    \\context { \\Staff \\RemoveEmptyStaves }\n    \\context { \\DrumStaff \\RemoveEmptyStaves }\n  }\n}\n");
    s
}
