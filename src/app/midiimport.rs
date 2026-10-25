//! .mid import (docs/MIDI.md): a MIDI file becomes one note clip, with each
//! note's velocity and recorded key pressure. One undo step.
use super::*;
use crate::timeline as tl;

impl App {
    /// import the .mid at `path` as a note clip on track `tr` at `at`
    pub(crate) fn import_midi(&mut self, path: std::path::PathBuf, tr: usize, at: tl::Tick) {
        let f = match std::fs::read(&path)
            .map_err(|e| e.to_string())
            .and_then(|b| crate::smf::parse(&b))
        {
            Ok(f) => f,
            Err(e) => {
                self.status = format!("Can't import that MIDI file: {e}");
                return;
            }
        };
        self.import_midi_file(f, &path, tr, at);
    }

    pub(crate) fn import_midi_file(
        &mut self,
        f: crate::smf::MidiFile,
        path: &std::path::Path,
        tr: usize,
        at: tl::Tick,
    ) {
        if self.with(|s| s.song.timeline.is_none() || tr >= s.song.channels) {
            self.status = "MIDI files go onto a track in the Timeline".into();
            return;
        }
        let name = f.name.clone().unwrap_or_else(|| {
            path.file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "MIDI".into())
        });
        let (count, pressed) = (
            f.notes.len(),
            f.notes.iter().filter(|n| n.pressure().is_some()).count(),
        );
        self.snapshot();
        let res = self.with(|s| {
            let id = tl::new_note_clip(&mut s.song, tr, at.max(0))?;
            let t = s.song.timeline.as_deref_mut()?;
            let k = t.clips.iter().position(|c| c.id == id)?;
            if let tl::ClipKind::Notes { data } = t.clips[k].kind {
                let d = t.notes[data as usize].make_mut();
                d.name = name.clone();
                d.notes = f.notes;
                d.len = f.len;
            }
            t.clips[k].len = f.len;
            let crate::audio::Shared { song, engine, .. } = s;
            engine.ensure_plan(song);
            Some(id)
        });
        self.dirty = true;
        match res {
            Some(id) => {
                self.tlv.sels = vec![id];
                self.tlv.sel = Some(id);
                self.ch = tr;
                self.status = format!(
                    "{name}: {count} notes on track {:02}{}",
                    tr + 1,
                    if pressed > 0 {
                        format!(", {pressed} with key pressure (the piano roll's Pressure lane)")
                    } else {
                        String::new()
                    }
                );
            }
            None => self.status = "No track there".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::ui_tests::harness_with;
    use super::*;

    #[test]
    fn a_midi_file_becomes_one_note_clip_with_its_pressure() {
        let mut s = crate::model::Song::clean();
        crate::chanops::insert_channel(&mut s, 0, "Keys");
        let mut h = harness_with(s);
        h.state_mut().undo_reset();
        let clips = |h: &egui_kittest::Harness<'static, App>| {
            h.state()
                .with(|s| s.song.timeline.as_ref().map_or(0, |t| t.clips.len()))
        };
        let before = clips(&h);
        let mut n = tl::Note {
            start: 0,
            len: tl::PPQ,
            key: 49,
            vel: tl::vel_from_7(100),
            ..Default::default()
        };
        n.press_push(tl::PPQ / 2, 0.5);
        let f = crate::smf::MidiFile {
            notes: vec![n.clone()],
            len: 4 * tl::PPQ,
            name: Some("Lead".into()),
        };
        h.state_mut()
            .import_midi_file(f, std::path::Path::new("x.mid"), 0, 8 * tl::PPQ);
        assert_eq!(clips(&h), before + 1);
        let id = h.state().tlv.sel.unwrap();
        let (start, notes, name) = h.state().with(|s| {
            let t = s.song.timeline.as_ref().unwrap();
            let c = t.clips.iter().find(|c| c.id == id).unwrap();
            let tl::ClipKind::Notes { data } = c.kind else {
                unreachable!()
            };
            let d = &t.notes[data as usize];
            (c.start, d.notes.clone(), d.name.clone())
        });
        assert_eq!((start, name.as_str()), (8 * tl::PPQ, "Lead"));
        assert_eq!(notes, vec![n]);
        assert!(
            h.state().status.contains("key pressure"),
            "{}",
            h.state().status
        );
        // one undo step
        h.state_mut().undo(false);
        assert_eq!(clips(&h), before);
    }
}
