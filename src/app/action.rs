//! One front door for edits: named actions on top of the snapshot undo
//! (`app/undo.rs`, docs/ARCHITECTURE.md "Edits and undo").
//!
//! An edit says what it is with an [`Action`]; the history still stores
//! whole-song snapshots, the action only names the step, so the Edit menu
//! reads "Undo: Move clip". [`App::act`] runs a one-shot edit as its own
//! named step; [`App::name_edit`] names a gesture that is still going (a
//! drag, which becomes one step when the mouse is released). Edits not
//! routed here yet keep working and undo as plain "Undo".
use super::*;

/// A named edit. The label is what Undo / Redo show.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Action {
    /// an instrument / effect knob (its name, e.g. "Cutoff")
    SetKnob(String),
    /// clips dragged to a new place (how many)
    MoveClips(usize),
    /// Alt-dragged copies of clips (how many)
    CopyClips(usize),
    ResizeClip,
    FadeClip,
    DeleteClips(usize),
    DuplicateClips(usize),
    SplitClip,
    PasteClips,
    AddNote,
    ResizeNotes,
    MoveNotes,
    CopyNotes,
    DeleteNotes(usize),
    EraseNotes,
    QuantiseNotes,
    PasteNotes,
    /// a typed Tracker cell (note, instrument, volume or effect)
    EditCell,
    ClearCells,
    PasteCells,
    Transpose,
    InsertRow,
    DeleteRow,
    AddTrack,
    DeleteTrack,
    MoveTrack,
    RenameTrack,
}

fn many(n: usize, one: &str, more: &str) -> String {
    if n == 1 {
        one.to_string()
    } else {
        format!("{more} ({n})")
    }
}

impl Action {
    /// the step name the Edit menu shows after "Undo: " / "Redo: "
    pub(crate) fn label(&self) -> String {
        match self {
            Action::SetKnob(name) if !name.trim().is_empty() => format!("Set {}", name.trim()),
            Action::SetKnob(_) => "Set knob".into(),
            Action::MoveClips(n) => many(*n, "Move clip", "Move clips"),
            Action::CopyClips(n) => many(*n, "Copy clip", "Copy clips"),
            Action::ResizeClip => "Resize clip".into(),
            Action::FadeClip => "Fade clip".into(),
            Action::DeleteClips(n) => many(*n, "Delete clip", "Delete clips"),
            Action::DuplicateClips(n) => many(*n, "Duplicate clip", "Duplicate clips"),
            Action::SplitClip => "Split clip".into(),
            Action::PasteClips => "Paste clips".into(),
            Action::AddNote => "Add note".into(),
            Action::ResizeNotes => "Resize notes".into(),
            Action::MoveNotes => "Move notes".into(),
            Action::CopyNotes => "Copy notes".into(),
            Action::DeleteNotes(n) => many(*n, "Delete note", "Delete notes"),
            Action::EraseNotes => "Erase notes".into(),
            Action::QuantiseNotes => "Quantise notes".into(),
            Action::PasteNotes => "Paste notes".into(),
            Action::EditCell => "Edit cell".into(),
            Action::ClearCells => "Clear cells".into(),
            Action::PasteCells => "Paste cells".into(),
            Action::Transpose => "Transpose".into(),
            Action::InsertRow => "Insert row".into(),
            Action::DeleteRow => "Delete row".into(),
            Action::AddTrack => "Add track".into(),
            Action::DeleteTrack => "Delete track".into(),
            Action::MoveTrack => "Move track".into(),
            Action::RenameTrack => "Rename track".into(),
        }
    }
}

impl App {
    /// Run `f` on the song as the named edit `a`: its own undo step (like
    /// `snapshot` + `with`), shown as "Undo: <label>".
    pub(super) fn act<R>(&mut self, a: Action, f: impl FnOnce(&mut Shared) -> R) -> R {
        self.snapshot();
        let r = self.with(f);
        self.name_edit(a);
        r
    }

    /// Name the edit in progress (the changes since the last step
    /// boundary, or still to come in this gesture). Inside an undo hold
    /// (several edits made one step) the step stays unnamed.
    pub(super) fn name_edit(&mut self, a: Action) {
        if self.undo_hold == 0 {
            self.hist.name(a.label());
        }
    }

    /// Name the gesture in progress unless it has a name already (a drag
    /// that resizes the note its press just drew stays "Add note").
    pub(super) fn name_edit_once(&mut self, a: Action) {
        if !self.hist.is_named() {
            self.name_edit(a);
        }
    }

    /// the Edit menu's Undo / Redo text: "Undo: Move clip", or plain
    /// "Undo" for an unnamed step
    pub(super) fn undo_text(&self, redo: bool) -> String {
        if redo {
            step_text("Redo", self.hist.redo_label())
        } else {
            step_text("Undo", self.hist.undo_label())
        }
    }
}

/// "Undo: Move clip", or the bare word for an unnamed step
pub(super) fn step_text(word: &str, label: Option<&str>) -> String {
    match label {
        Some(l) => format!("{word}: {l}"),
        None => word.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_read_well() {
        assert_eq!(Action::MoveClips(1).label(), "Move clip");
        assert_eq!(Action::MoveClips(3).label(), "Move clips (3)");
        assert_eq!(Action::SetKnob("Cutoff".into()).label(), "Set Cutoff");
        assert_eq!(Action::SetKnob(" ".into()).label(), "Set knob");
        assert_eq!(Action::DeleteNotes(2).label(), "Delete notes (2)");
    }
}
