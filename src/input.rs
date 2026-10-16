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
}

/// Arranger shortcuts. Ctrl+D / Ctrl+Shift+D work whenever the arranger is
/// visible (docked or full); the others only in the full arranger view,
/// where they don't clash with the pattern editor.
pub fn arranger_shortcut(e: &Event, full_view: bool) -> Option<ArrAction> {
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
    if !full_view || m.command || m.alt {
        return None;
    }
    Some(match key {
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
