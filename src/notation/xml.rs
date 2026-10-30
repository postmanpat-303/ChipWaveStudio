//! The MusicXML 4.0 (partwise) writer.
use super::{value_name, Content, Drum, Item, KeySig, Part, Score, MIN_VALUE};
use crate::timeline::{Tick, PPQ};
use std::fmt::Write;

/// divisions per quarter note (a 32nd is one)
pub const DIVISIONS: Tick = PPQ / MIN_VALUE;

fn esc(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn div(t: Tick) -> Tick {
    t / MIN_VALUE
}

/// the drums a part plays, in order
fn drums_used(p: &Part) -> Vec<Drum> {
    let mut v: Vec<Drum> = p
        .staves
        .iter()
        .flat_map(|s| s.bars.iter().flatten())
        .filter_map(|it| match &it.content {
            Content::Drums(d) => Some(d.clone()),
            _ => None,
        })
        .flatten()
        .collect();
    v.sort();
    v.dedup();
    v
}

fn inst_id(pid: &str, d: Drum) -> String {
    format!("{pid}-I{}", d.info().gm as u32 + 1)
}

#[allow(clippy::too_many_arguments)]
fn note(
    s: &mut String,
    key: KeySig,
    pid: &str,
    it: &Item,
    bar_len: Tick,
    staff: usize,
    voice: usize,
    grand: bool,
) {
    let staff_el = if grand {
        format!("<staff>{staff}</staff>")
    } else {
        String::new()
    };
    if it.bar_rest {
        let _ = writeln!(
            s,
            "      <note><rest measure=\"yes\"/><duration>{}</duration><voice>{voice}</voice>{staff_el}</note>",
            div(bar_len)
        );
        return;
    }
    let (_, ty, dot) = value_name(it.dur);
    let dot = if dot { "<dot/>" } else { "" };
    let d = div(it.dur);
    match &it.content {
        Content::Rest => {
            let _ = writeln!(
                s,
                "      <note><rest/><duration>{d}</duration><voice>{voice}</voice><type>{ty}</type>{dot}{staff_el}</note>"
            );
        }
        Content::Notes(ps) => {
            for (i, &p) in ps.iter().enumerate() {
                let (l, a, oct) = key.spell(p);
                let chord = if i > 0 { "<chord/>" } else { "" };
                let alter = if a != 0 {
                    format!("<alter>{a}</alter>")
                } else {
                    String::new()
                };
                let mut ties = String::new();
                let mut tied = String::new();
                if it.tied {
                    ties.push_str("<tie type=\"stop\"/>");
                    tied.push_str("<tied type=\"stop\"/>");
                }
                if it.tie {
                    ties.push_str("<tie type=\"start\"/>");
                    tied.push_str("<tied type=\"start\"/>");
                }
                let notations = if tied.is_empty() {
                    String::new()
                } else {
                    format!("<notations>{tied}</notations>")
                };
                let _ = writeln!(
                    s,
                    "      <note>{chord}<pitch><step>{l}</step>{alter}<octave>{oct}</octave></pitch><duration>{d}</duration>{ties}<voice>{voice}</voice><type>{ty}</type>{dot}{staff_el}{notations}</note>"
                );
            }
        }
        Content::Drums(ds) => {
            for (i, &dr) in ds.iter().enumerate() {
                let inf = dr.info();
                let chord = if i > 0 { "<chord/>" } else { "" };
                let head = if inf.x_head {
                    "<notehead>x</notehead>"
                } else {
                    ""
                };
                let _ = writeln!(
                    s,
                    "      <note>{chord}<unpitched><display-step>{}</display-step><display-octave>{}</display-octave></unpitched><duration>{d}</duration><instrument id=\"{}\"/><voice>{voice}</voice><type>{ty}</type>{dot}<stem>up</stem>{head}{staff_el}</note>",
                    inf.step,
                    inf.octave,
                    inst_id(pid, dr)
                );
            }
        }
    }
}

/// the score as a MusicXML 4.0 partwise file
pub fn to_musicxml(sc: &Score) -> String {
    let mut s = String::new();
    s.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"no\"?>\n");
    s.push_str("<!DOCTYPE score-partwise PUBLIC \"-//Recordare//DTD MusicXML 4.0 Partwise//EN\" \"http://www.musicxml.org/dtds/partwise.dtd\">\n");
    s.push_str("<score-partwise version=\"4.0\">\n");
    if !sc.title.trim().is_empty() {
        let _ = writeln!(
            s,
            "  <work><work-title>{}</work-title></work>",
            esc(sc.title.trim())
        );
    }
    s.push_str("  <identification>\n");
    if !sc.composer.trim().is_empty() {
        let _ = writeln!(
            s,
            "    <creator type=\"composer\">{}</creator>",
            esc(sc.composer.trim())
        );
    }
    s.push_str(
        "    <encoding><software>ChipWave Studio</software></encoding>\n  </identification>\n",
    );
    s.push_str("  <part-list>\n");
    let empty = Part {
        name: "Music".into(),
        drums: false,
        staves: vec![super::Staff {
            clef: super::Clef::Treble,
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
        }],
    };
    let parts: Vec<&Part> = if sc.parts.is_empty() {
        vec![&empty]
    } else {
        sc.parts.iter().collect()
    };
    for (pi, p) in parts.iter().enumerate() {
        let pid = format!("P{}", pi + 1);
        let _ = writeln!(
            s,
            "    <score-part id=\"{pid}\"><part-name>{}</part-name>",
            esc(&p.name)
        );
        if p.drums {
            let used = drums_used(p);
            for &d in &used {
                let _ = writeln!(
                    s,
                    "      <score-instrument id=\"{}\"><instrument-name>{}</instrument-name></score-instrument>",
                    inst_id(&pid, d),
                    d.info().name
                );
            }
            for &d in &used {
                let _ = writeln!(
                    s,
                    "      <midi-instrument id=\"{}\"><midi-channel>10</midi-channel><midi-unpitched>{}</midi-unpitched></midi-instrument>",
                    inst_id(&pid, d),
                    d.info().gm as u32 + 1
                );
            }
        }
        s.push_str("    </score-part>\n");
    }
    s.push_str("  </part-list>\n");
    for (pi, p) in parts.iter().enumerate() {
        let pid = format!("P{}", pi + 1);
        let grand = p.staves.len() > 1;
        let _ = writeln!(s, "  <part id=\"{pid}\">");
        let mut sig = (0u8, 0u8);
        for (bi, b) in sc.bars.iter().enumerate() {
            let _ = writeln!(s, "    <measure number=\"{}\">", bi + 1);
            let new_sig = (b.num, b.den) != sig;
            if bi == 0 || new_sig {
                s.push_str("      <attributes>");
                if bi == 0 {
                    let _ = write!(s, "<divisions>{DIVISIONS}</divisions>");
                    if !p.drums {
                        let _ = write!(
                            s,
                            "<key><fifths>{}</fifths><mode>{}</mode></key>",
                            sc.key.fifths,
                            if sc.key.minor { "minor" } else { "major" }
                        );
                    }
                }
                if new_sig {
                    let _ = write!(
                        s,
                        "<time><beats>{}</beats><beat-type>{}</beat-type></time>",
                        b.num, b.den
                    );
                    sig = (b.num, b.den);
                }
                if bi == 0 {
                    if grand {
                        let _ = write!(s, "<staves>{}</staves>", p.staves.len());
                    }
                    for (si, st) in p.staves.iter().enumerate() {
                        let (sign, line, oc) = st.clef.xml();
                        let num = if grand {
                            format!(" number=\"{}\"", si + 1)
                        } else {
                            String::new()
                        };
                        let _ = write!(s, "<clef{num}><sign>{sign}</sign><line>{line}</line>");
                        if oc != 0 {
                            let _ = write!(s, "<clef-octave-change>{oc}</clef-octave-change>");
                        }
                        s.push_str("</clef>");
                    }
                }
                s.push_str("</attributes>\n");
            }
            if pi == 0 {
                if let Some((_, name)) = sc.marks.iter().find(|m| m.0 == bi) {
                    let _ = writeln!(
                        s,
                        "      <direction placement=\"above\"><direction-type><rehearsal>{}</rehearsal></direction-type></direction>",
                        esc(name)
                    );
                }
                if let Some((bpm, text)) = b.tempo {
                    let bpm = bpm.round().max(1.0) as i32;
                    if !text.is_empty() {
                        let _ = writeln!(
                            s,
                            "      <direction placement=\"above\"><direction-type><words>{text}</words></direction-type></direction>"
                        );
                    }
                    let _ = writeln!(
                        s,
                        "      <direction placement=\"above\"><direction-type><metronome><beat-unit>quarter</beat-unit><per-minute>{bpm}</per-minute></metronome></direction-type><sound tempo=\"{bpm}\"/></direction>"
                    );
                }
            }
            for (si, st) in p.staves.iter().enumerate() {
                if si > 0 {
                    let _ = writeln!(
                        s,
                        "      <backup><duration>{}</duration></backup>",
                        div(b.len)
                    );
                }
                for it in &st.bars[bi] {
                    note(&mut s, sc.key, &pid, it, b.len, si + 1, si * 4 + 1, grand);
                }
            }
            if bi + 1 == sc.bars.len() {
                s.push_str("      <barline location=\"right\"><bar-style>light-heavy</bar-style></barline>\n");
            }
            s.push_str("    </measure>\n");
        }
        s.push_str("  </part>\n");
    }
    s.push_str("</score-partwise>\n");
    s
}
