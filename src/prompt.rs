//! Unsaved-changes prompt logic (docs/SAFETY.md §5), kept UI-free for tests.
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq)]
pub enum Pending {
    Quit,
    New,
    /// Open... (file dialog)
    Open,
    /// open this file (Recent, drag and drop)
    OpenPath(PathBuf),
    /// load the built-in demo song (File > New from demo, Help > Load demo song)
    Demo,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Choice {
    Save,
    DontSave,
    Cancel,
}

#[derive(Debug, PartialEq)]
pub enum Step {
    /// go ahead now
    Run(Pending),
    /// ask Save / Don't save / Cancel first
    Ask(Pending),
}

pub fn request(dirty: bool, p: Pending) -> Step {
    if dirty {
        Step::Ask(p)
    } else {
        Step::Run(p)
    }
}

#[derive(Debug, PartialEq)]
pub enum Outcome {
    /// run the action; `discarded` = the user threw changes away on purpose
    Run { discarded: bool },
    /// stay (cancelled, or the save failed / was cancelled)
    Stay,
}

/// `saved` = whether the save (for Choice::Save) succeeded
pub fn resolve(c: Choice, saved: bool) -> Outcome {
    match c {
        Choice::Save if saved => Outcome::Run { discarded: false },
        Choice::Save | Choice::Cancel => Outcome::Stay,
        Choice::DontSave => Outcome::Run { discarded: true },
    }
}

/// Clean exit: the recovery copy may go only when nothing unsaved is lost.
pub fn may_delete_recovery(dirty: bool, discarded: bool) -> bool {
    !dirty || discarded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_flow() {
        assert_eq!(request(false, Pending::Quit), Step::Run(Pending::Quit));
        assert_eq!(request(true, Pending::New), Step::Ask(Pending::New));
        assert_eq!(
            resolve(Choice::Save, true),
            Outcome::Run { discarded: false }
        );
        assert_eq!(resolve(Choice::Save, false), Outcome::Stay);
        assert_eq!(resolve(Choice::Cancel, true), Outcome::Stay);
        assert_eq!(
            resolve(Choice::DontSave, false),
            Outcome::Run { discarded: true }
        );
        assert!(may_delete_recovery(false, false));
        assert!(may_delete_recovery(true, true));
        assert!(!may_delete_recovery(true, false));
    }
}
