//! Input mapping that does not depend on app state, so it can be unit-tested
//! with synthetic events.
//!
//! On Windows (egui-winit), Ctrl+C / Ctrl+X / Ctrl+V are delivered as
//! `Event::Copy`, `Event::Cut` and `Event::Paste(text)` *instead of* key
//! events, and `Paste` is only sent when the system clipboard holds text.
//! Injected keys (xdotool under Wine) may arrive as plain key events, so both
//! forms are accepted here and de-duplicated.
use crate::fileio::{cell_from_text, cell_to_text};
use crate::model::Cell;
use eframe::egui::{Event, Key};

pub const CLIP_HEADER: &str = "ChipWave cells v1";
/// arranger clipboard (song-order entries); distinct so the two never mix
pub const ARR_HEADER: &str = "ChipWave song-order v1";

pub fn arr_clip_to_text(blocks: &[usize]) -> String {
    let ids: Vec<String> = blocks.iter().map(|b| b.to_string()).collect();
    format!("{ARR_HEADER}\n{}\n", ids.join(" "))
}

pub fn arr_clip_from_text(t: &str) -> Option<Vec<usize>> {
    let mut lines = t.lines();
    if lines.next()?.trim() != ARR_HEADER {
        return None;
    }
    let v: Vec<usize> = lines
        .flat_map(|l| l.split_whitespace())
        .filter_map(|x| x.parse().ok())
        .collect();
    if v.is_empty() {
        None
    } else {
        Some(v)
    }
}

/// Where a clipboard action goes, given which view has keyboard focus.
#[derive(Clone, Debug, PartialEq)]
pub enum Routed {
    Pattern(ClipAction),
    Arranger(ArrAction, Option<String>),
}

pub fn route_clip(a: ClipAction, arranger_focus: bool, shift: bool) -> Routed {
    if !arranger_focus {
        return Routed::Pattern(a);
    }
    match a {
        ClipAction::Copy => Routed::Arranger(ArrAction::Copy, None),
        ClipAction::Cut => Routed::Arranger(ArrAction::Cut, None),
        ClipAction::Paste(t) => Routed::Arranger(
            if shift {
                ArrAction::PasteUnique
            } else {
                ArrAction::Paste
            },
            t,
        ),
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ClipAction {
    Copy,
    Cut,
    /// text from the system clipboard, if egui delivered any
    Paste(Option<String>),
}

fn cmd_key(e: &Event, k: Key) -> bool {
    matches!(e, Event::Key { key, pressed: true, modifiers, .. }
        if *key == k && modifiers.command && !modifiers.alt)
}

/// Clipboard actions in one frame's events (event form preferred, key form
/// as a fallback; never both for the same action).
pub fn clipboard_actions(events: &[Event]) -> Vec<ClipAction> {
    let mut out = vec![];
    let (mut ec, mut ex, mut ev) = (false, false, false);
    for e in events {
        match e {
            Event::Copy => {
                ec = true;
                out.push(ClipAction::Copy)
            }
            Event::Cut => {
                ex = true;
                out.push(ClipAction::Cut)
            }
            Event::Paste(t) => {
                ev = true;
                out.push(ClipAction::Paste(Some(t.clone())))
            }
            _ => {}
        }
    }
    for e in events {
        if !ec && cmd_key(e, Key::C) {
            out.push(ClipAction::Copy);
            ec = true;
        } else if !ex && cmd_key(e, Key::X) {
            out.push(ClipAction::Cut);
            ex = true;
        } else if !ev && cmd_key(e, Key::V) {
            out.push(ClipAction::Paste(None));
            ev = true;
        }
    }
    out
}

/// key events already handled by `clipboard_actions`
pub fn is_clipboard_key(e: &Event) -> bool {
    cmd_key(e, Key::C) || cmd_key(e, Key::X) || cmd_key(e, Key::V)
}

/// Text form for the system clipboard (also readable by people).
pub fn clip_to_text(clip: &[Vec<Cell>]) -> String {
    let mut s = String::from(CLIP_HEADER);
    s.push('\n');
    for row in clip {
        let cells: Vec<String> = row.iter().map(cell_to_text).collect();
        s.push_str(&cells.join(" | "));
        s.push('\n');
    }
    s
}

pub fn clip_from_text(t: &str) -> Option<Vec<Vec<Cell>>> {
    let mut lines = t.lines();
    if lines.next()?.trim() != CLIP_HEADER {
        return None;
    }
    let rows: Vec<Vec<Cell>> = lines
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.split('|').map(cell_from_text).collect())
        .collect();
    if rows.is_empty() {
        None
    } else {
        Some(rows)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ArrAction {
    Duplicate,
    DuplicateUnique,
    Insert,
    Delete,
    Prev,
    Next,
    ZoomIn,
    ZoomOut,
    ExtendPrev,
    ExtendNext,
    SelectAll,
    Cut,
    Copy,
    Paste,
    PasteUnique,
}

/// Arranger shortcuts. Ctrl+D / Ctrl+Shift+D work whenever the arranger is
/// visible (docked or full); the others only while the arranger has keyboard
/// focus (clicked, or the full arranger view).
pub fn arranger_shortcut(e: &Event, focused: bool) -> Option<ArrAction> {
    let Event::Key {
        key,
        pressed: true,
        modifiers: m,
        ..
    } = e
    else {
        return None;
    };
    if *key == Key::D && m.command && !m.alt {
        return Some(if m.shift {
            ArrAction::DuplicateUnique
        } else {
            ArrAction::Duplicate
        });
    }
    if !focused || m.alt {
        return None;
    }
    if m.command {
        return (*key == Key::A).then_some(ArrAction::SelectAll);
    }
    Some(match key {
        Key::ArrowLeft if m.shift => ArrAction::ExtendPrev,
        Key::ArrowRight if m.shift => ArrAction::ExtendNext,
        Key::ArrowLeft => ArrAction::Prev,
        Key::ArrowRight => ArrAction::Next,
        Key::Delete | Key::Backspace => ArrAction::Delete,
        Key::Insert => ArrAction::Insert,
        Key::Plus | Key::Equals => ArrAction::ZoomIn,
        Key::Minus => ArrAction::ZoomOut,
        _ => return None,
    })
}

/// short text for the debug key log (Ctrl+Shift+K)
pub fn describe(e: &Event) -> Option<String> {
    Some(match e {
        Event::Key {
            key,
            pressed: true,
            modifiers: m,
            repeat,
            ..
        } => format!(
            "{}{}{}{}{}",
            if m.ctrl { "Ctrl+" } else { "" },
            if m.alt { "Alt+" } else { "" },
            if m.shift { "Shift+" } else { "" },
            key.name(),
            if *repeat { "(r)" } else { "" }
        ),
        Event::Copy => "Event::Copy".into(),
        Event::Cut => "Event::Cut".into(),
        Event::Paste(t) => format!("Event::Paste({} chars)", t.chars().count()),
        Event::Text(t) => format!("Text({t:?})"),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::Modifiers;

    fn key(k: Key, m: Modifiers) -> Event {
        Event::Key {
            key: k,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: m,
        }
    }
    const CTRL: Modifiers = Modifiers::COMMAND;

    #[test]
    fn windows_style_events_map_to_actions() {
        // what egui-winit sends on Windows: no Key event at all
        let a = clipboard_actions(&[Event::Copy, Event::Cut, Event::Paste("x".into())]);
        assert_eq!(
            a,
            vec![
                ClipAction::Copy,
                ClipAction::Cut,
                ClipAction::Paste(Some("x".into()))
            ]
        );
    }

    #[test]
    fn key_fallback_and_no_duplicates() {
        assert_eq!(
            clipboard_actions(&[key(Key::C, CTRL)]),
            vec![ClipAction::Copy]
        );
        assert_eq!(
            clipboard_actions(&[key(Key::V, CTRL)]),
            vec![ClipAction::Paste(None)]
        );
        // both forms in the same frame -> one action
        assert_eq!(
            clipboard_actions(&[Event::Copy, key(Key::C, CTRL)]),
            vec![ClipAction::Copy]
        );
        // plain C is a note, not copy
        assert!(clipboard_actions(&[key(Key::C, Modifiers::NONE)]).is_empty());
        assert!(is_clipboard_key(&key(Key::X, CTRL)));
    }

    #[test]
    fn clip_text_roundtrip() {
        let a = Cell {
            note: 49,
            instr: 2,
            vol: 0x21,
            fx: 0x0C,
            fxp: 0x20,
        };
        let clip = vec![vec![a, Cell::default()], vec![Cell::default(), a]];
        let t = clip_to_text(&clip);
        assert_eq!(clip_from_text(&t), Some(clip));
        assert_eq!(clip_from_text("hello"), None);
    }

    #[test]
    fn ctrl_d_works_docked_and_full() {
        let d = key(Key::D, CTRL);
        assert_eq!(arranger_shortcut(&d, false), Some(ArrAction::Duplicate));
        assert_eq!(arranger_shortcut(&d, true), Some(ArrAction::Duplicate));
        let sd = key(Key::D, Modifiers::COMMAND | Modifiers::SHIFT);
        assert_eq!(
            arranger_shortcut(&sd, false),
            Some(ArrAction::DuplicateUnique)
        );
        // docked: arrows stay with the pattern editor
        assert_eq!(
            arranger_shortcut(&key(Key::ArrowLeft, Modifiers::NONE), false),
            None
        );
        assert_eq!(
            arranger_shortcut(&key(Key::ArrowLeft, Modifiers::NONE), true),
            Some(ArrAction::Prev)
        );
    }

    #[test]
    fn clipboard_routes_to_focused_view() {
        assert_eq!(
            route_clip(ClipAction::Copy, false, false),
            Routed::Pattern(ClipAction::Copy)
        );
        assert_eq!(
            route_clip(ClipAction::Cut, true, false),
            Routed::Arranger(ArrAction::Cut, None)
        );
        let t = arr_clip_to_text(&[0, 2]);
        assert_eq!(
            route_clip(ClipAction::Paste(Some(t.clone())), true, false),
            Routed::Arranger(ArrAction::Paste, Some(t.clone()))
        );
        assert_eq!(
            route_clip(ClipAction::Paste(None), true, true),
            Routed::Arranger(ArrAction::PasteUnique, None)
        );
        // Windows-style events, end to end
        let acts = clipboard_actions(&[Event::Copy, Event::Paste(t)]);
        let r: Vec<Routed> = acts
            .into_iter()
            .map(|a| route_clip(a, true, false))
            .collect();
        assert!(matches!(r[0], Routed::Arranger(ArrAction::Copy, None)));
        assert!(matches!(r[1], Routed::Arranger(ArrAction::Paste, Some(_))));
    }

    #[test]
    fn clipboards_do_not_mix() {
        let a = arr_clip_to_text(&[3, 1, 1]);
        assert_eq!(arr_clip_from_text(&a), Some(vec![3, 1, 1]));
        assert_eq!(clip_from_text(&a), None);
        let p = clip_to_text(&[vec![Cell::default()]]);
        assert_eq!(arr_clip_from_text(&p), None);
    }

    #[test]
    fn arranger_keys_need_focus() {
        let del = key(Key::Delete, Modifiers::NONE);
        assert_eq!(arranger_shortcut(&del, false), None);
        assert_eq!(arranger_shortcut(&del, true), Some(ArrAction::Delete));
        let sr = key(Key::ArrowRight, Modifiers::SHIFT);
        assert_eq!(arranger_shortcut(&sr, true), Some(ArrAction::ExtendNext));
        assert_eq!(
            arranger_shortcut(&key(Key::A, CTRL), true),
            Some(ArrAction::SelectAll)
        );
        assert_eq!(arranger_shortcut(&key(Key::A, CTRL), false), None);
        // Ctrl+C never comes through here (handled as a clipboard action)
        assert_eq!(arranger_shortcut(&key(Key::C, CTRL), true), None);
    }

    #[test]
    fn paste_event_text_restores_cells() {
        let clip = vec![vec![Cell {
            note: 30,
            instr: 1,
            ..Default::default()
        }]];
        let ev = Event::Paste(clip_to_text(&clip));
        match &clipboard_actions(&[ev])[0] {
            ClipAction::Paste(Some(t)) => assert_eq!(clip_from_text(t), Some(clip)),
            _ => panic!(),
        }
    }
}
