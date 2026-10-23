//! Track edits from the timeline headers and the Browser: add, reorder
//! (docs/TIMELINE.md "Tracks"). Tracks are mixer channels; the work is
//! done by `crate::chanops`, here: undo, the cursor and status.
use super::*;

impl App {
    /// insert an empty track at `at` named `name` (no snapshot; callers
    /// take one). False with a message when all 32 are used.
    pub(super) fn track_insert(&mut self, at: usize, name: &str) -> bool {
        let ok = self.with(|s| {
            let at = at.min(s.song.channels);
            crate::chanops::insert_channel(&mut s.song, at, name)
        });
        if !ok {
            self.status = "All 32 tracks are used (the channel limit)".into();
            return false;
        }
        let n = self.with(|s| s.song.channels);
        self.ch = at.min(n - 1);
        // the new track is the current one, not a lit Song / Master
        self.tlv.song_sel = None;
        self.dirty = true;
        crate::plugin::sync(&self.shared, None);
        true
    }

    /// "+ Add track" / Add track above / below (undoable)
    pub(super) fn track_add(&mut self, at: usize) -> bool {
        if self.with(|s| s.song.channels) >= MAX_CHANNELS {
            self.status = "All 32 tracks are used (the channel limit)".into();
            return false;
        }
        self.snapshot();
        let name = format!("Track {}", at + 1);
        self.track_insert(at, &name)
    }

    /// move track `from` to index `to` (header drag; undoable). The
    /// channel's pattern columns move with it, so playback is the same.
    pub(super) fn track_move(&mut self, from: usize, to: usize) {
        self.snapshot();
        let ok = self.with(|s| {
            let ok = crate::chanops::move_channel(&mut s.song, from, to);
            if ok && s.engine.playing {
                // the channels' running voices belong to the old order
                s.engine.ensure_plan(&s.song);
            }
            ok
        });
        if !ok {
            return;
        }
        // the cursor channel follows the track it was on
        let c = self.ch;
        self.ch = if c == from {
            to
        } else if from < c && c <= to {
            c - 1
        } else if to <= c && c < from {
            c + 1
        } else {
            c
        };
        self.dirty = true;
        crate::plugin::sync(&self.shared, None);
        self.status = format!("Moved track {:02} to {:02}", from + 1, to + 1);
    }

    /// "Add track with instrument…": the Browser's Instruments tab opens;
    /// the next instrument loaded there makes the track at `at`
    pub(super) fn track_add_with(&mut self, at: usize) {
        if self.with(|s| s.song.channels) >= MAX_CHANNELS {
            self.status = "All 32 tracks are used (the channel limit)".into();
            return;
        }
        self.br.new_track_at = Some(at);
        self.br.new_track_pattern = false;
        self.open_browser_tab(crate::browser::Tab::Instruments);
        self.status = format!(
            "Pick an instrument in the Browser (double-click or Enter) for the new track {:02}",
            at + 1
        );
    }

    /// the pie's Change instrument…: the Browser's Instruments, Favourites
    /// first when there are any; the next pick replaces track `tr`'s
    /// instrument (`br_assign_slot`, one undo step)
    pub(super) fn track_change_instrument(&mut self, tr: usize) {
        self.open_browser_tab(crate::browser::Tab::Instruments);
        if !self.extra.browser_favs.is_empty() {
            self.br.filter.loc = crate::browser::LocFilter::Favourites;
        }
        self.br.track_target = Some((tr, crate::browser::Tab::Instruments));
        self.status = format!(
            "Pick the new instrument for track {:02} in the Browser (double-click or Enter)",
            tr + 1
        );
    }

    /// the pie's Add › WaveForge, Drum Kit, …: a new built-in instrument
    /// (its Init or first preset) plays track `tr`; one undo step, and the
    /// Rack shows its compact card (the full editor outside the Timeline)
    pub(crate) fn track_new_instrument(&mut self, tr: usize, b: super::trackpie::Builtin) {
        let Some((name, kind)) = b.make() else {
            self.status = format!("No {} preset to start from", b.label());
            return;
        };
        if self.with(|s| s.song.instruments.len() >= 255) {
            self.status = "All 255 instrument slots are used".into();
            return;
        }
        self.snapshot();
        self.undo_hold += 1;
        self.add_instrument(&name, kind);
        let slot = self.instr;
        let n = self.with(|s| crate::chanops::assign_track_instrument(&mut s.song, tr, slot));
        self.undo_hold -= 1;
        // the track is selected (headers, lanes, the lower panel)
        self.tl_select_track(tr, false);
        self.instr = slot;
        self.dirty = true;
        // the Rack: the instrument's compact card with room beside it for
        // effects (Del (postmanpat) 23 Oct 2026), not the full editor
        if matches!(self.view, View::Timeline | View::Pattern) {
            self.set_lower(4);
        } else {
            self.show_instrument_panel();
        }
        self.status = format!(
            "Track {:02} now plays {slot:02X} {name}{}",
            tr + 1,
            if n > 0 {
                format!(" ({n} note{} changed)", if n == 1 { "" } else { "s" })
            } else {
                String::new()
            }
        );
    }

    /// Browser › ChipWave Instruments dropped below the tracks (or loaded):
    /// a new track at `at`, named after the instrument, playing a new `b`;
    /// one undo step
    pub(crate) fn track_new_with_instrument(&mut self, at: usize, b: super::trackpie::Builtin) {
        if self.with(|s| s.song.channels) >= MAX_CHANNELS {
            self.status =
                "All 32 tracks are used (the channel limit): drop onto a track instead".into();
            return;
        }
        if self.with(|s| s.song.instruments.len() >= 255) {
            self.status = "All 255 instrument slots are used".into();
            return;
        }
        self.snapshot();
        self.undo_hold += 1;
        if self.track_insert(at, b.label()) {
            let at = self.ch;
            self.track_new_instrument(at, b);
        }
        self.undo_hold -= 1;
    }

    /// the pie's Add › Drum pattern › <instrument> (`kit` None) or › Drum
    /// Kit › <kit>: a new track below `tr` playing a new instrument, with an
    /// EMPTY 1-bar pattern at the play marker's bar, open in the step
    /// sequencer (a Drum Kit) or the piano roll. One undo step.
    pub(crate) fn track_new_pattern_track(
        &mut self,
        tr: usize,
        b: super::trackpie::Builtin,
        kit: Option<&str>,
    ) -> bool {
        if self.with(|s| s.song.timeline.is_none()) {
            self.status = "Drum patterns go on the Timeline (timeline mode)".into();
            return false;
        }
        if self.with(|s| s.song.channels) >= MAX_CHANNELS {
            self.status = "All 32 tracks are used (the channel limit)".into();
            return false;
        }
        if self.with(|s| s.song.instruments.len() >= 255) {
            self.status = "All 255 instrument slots are used".into();
            return false;
        }
        let inst = match kit {
            Some(k) => crate::drumpat::drumkit_instrument(k),
            None => b.make().map(|(name, kind)| Instrument {
                name,
                volume: 0.8,
                transpose: 0,
                fx: vec![],
                kind,
            }),
        };
        let Some(inst) = inst else {
            self.status = format!("No {} preset to start from", kit.unwrap_or(b.label()));
            return false;
        };
        let name = inst.name.clone();
        let at = (tr + 1).min(self.with(|s| s.song.channels));
        self.snapshot();
        self.undo_hold += 1;
        let ok = self.track_insert(at, &name)
            && self.with(|s| {
                let Some(k) = s.song.add_instrument(inst) else {
                    return false;
                };
                crate::chanops::assign_track_instrument(&mut s.song, at, k as u8 + 1);
                true
            });
        let id = if ok {
            self.track_add_empty_pattern(at)
        } else {
            None
        };
        self.undo_hold -= 1;
        let Some((id, bar)) = id else {
            if ok {
                self.status = "Can't add a pattern there".into();
            }
            return false;
        };
        self.status = format!(
            "New track {:02} {name}: an empty 1-bar pattern at bar {bar}{} (Ctrl+Z undoes it all)",
            at + 1,
            if self.dk.clip == Some(id) {
                ", in the step sequencer: click the steps"
            } else {
                ", in the piano roll"
            }
        );
        true
    }

    /// the track pie's Add › Drum pattern › <instrument> (`kit` None) or ›
    /// Drum Kit › <kit> on track `tr` ITSELF (never a new track): a new
    /// instrument plays it, its pattern (the selected or first note clip on
    /// the track, else a new 1-bar one at the play marker) opens in the step
    /// sequencer or the piano roll, and an empty pattern gets the kit's
    /// starter groove (as Kits… does). One undo step.
    pub(crate) fn track_set_pattern_instrument(
        &mut self,
        tr: usize,
        b: super::trackpie::Builtin,
        kit: Option<&str>,
    ) -> bool {
        if self.with(|s| s.song.timeline.is_none()) {
            self.status = "Drum patterns go on the Timeline (timeline mode)".into();
            return false;
        }
        if tr >= self.with(|s| s.song.channels) {
            return false;
        }
        if self.with(|s| s.song.instruments.len() >= 255) {
            self.status = "All 255 instrument slots are used".into();
            return false;
        }
        let inst = match kit {
            Some(k) => crate::drumpat::drumkit_instrument(k),
            None => b.make().map(|(name, kind)| Instrument {
                name,
                volume: 0.8,
                transpose: 0,
                fx: vec![],
                kind,
            }),
        };
        let Some(inst) = inst else {
            self.status = format!("No {} preset to start from", kit.unwrap_or(b.label()));
            return false;
        };
        let name = inst.name.clone();
        let sel = self.tlv.sel;
        self.snapshot();
        self.undo_hold += 1;
        // the instrument plays the track; a default name follows it
        let (slot, clip) = self.with(|s| {
            let Some(k) = s.song.add_instrument(inst) else {
                return (None, None);
            };
            let slot = k as u8 + 1;
            crate::chanops::assign_track_instrument(&mut s.song, tr, slot);
            if let Some(n) = s.song.channel_names.get_mut(tr) {
                let t = n.trim();
                if t.is_empty() || t.starts_with("Track ") {
                    *n = name.chars().take(24).collect();
                }
            }
            // its pattern: the selected note clip on the track, else the first
            let t = s.song.timeline.as_deref();
            let notes_on = |c: &crate::timeline::Clip| {
                c.track as usize == tr && matches!(c.kind, crate::timeline::ClipKind::Notes { .. })
            };
            let clip = t.and_then(|t| {
                sel.and_then(|id| t.clips.iter().find(|c| c.id == id && notes_on(c)))
                    .or_else(|| t.clips.iter().find(|c| notes_on(c)))
                    .map(|c| c.id)
            });
            (Some(slot), clip)
        });
        let Some(slot) = slot else {
            self.undo_hold -= 1;
            self.status = "All 255 instrument slots are used".into();
            return false;
        };
        self.instr = slot;
        let id = match clip {
            Some(id) => {
                self.track_open_pattern(tr, id);
                Some(id)
            }
            None => self.track_add_empty_pattern(tr).map(|(id, _)| id),
        };
        // an empty pattern gets the kit's starter groove
        let mut groove = None;
        if let (Some(id), Some(k)) = (id, kit) {
            let data = self.with(|s| {
                let t = s.song.timeline.as_deref()?;
                let c = t.clips.iter().find(|c| c.id == id)?;
                match c.kind {
                    crate::timeline::ClipKind::Notes { data } => {
                        let d = t.notes.get(data as usize)?;
                        Some((data as usize, (**d).clone()))
                    }
                    _ => None,
                }
            });
            if let Some((data, mut d)) = data {
                if let Some(g) =
                    super::drumkitview::starter_groove(k).filter(|_| d.notes.is_empty())
                {
                    d.notes = g.notes(0);
                    d.len = g.len();
                    d.name = g.name.to_string();
                    d.drum = Some(crate::drumkit::DrumMeta { swing: g.swing });
                    groove = Some((g.name, d.notes.len()));
                    self.dk_write(data, d);
                } else if d.drum.is_none() {
                    d.drum = Some(crate::drumkit::DrumMeta::default());
                    self.dk_write(data, d);
                }
            }
        }
        self.undo_hold -= 1;
        self.with(|s| {
            let crate::audio::Shared { song, engine, .. } = s;
            engine.ensure_plan(song);
        });
        self.dirty = true;
        crate::plugin::sync(&self.shared, None);
        self.status = match groove {
            Some((g, hits)) => format!(
                "Track {:02} now plays {slot:02X} {name}, with its starter groove {g} ({hits} hits)",
                tr + 1
            ),
            None => format!("Track {:02} now plays {slot:02X} {name}", tr + 1),
        };
        true
    }

    /// select track `tr` and its note clip `id`, open in the step sequencer
    /// (a Drum Kit) or the piano roll
    pub(crate) fn track_open_pattern(&mut self, tr: usize, id: u32) {
        let slot = self.track_slot(tr);
        let kit = self.with(|s| {
            s.song
                .instruments
                .get((slot as usize).wrapping_sub(1))
                .is_some_and(|i| crate::drumkit::kit_of(i).is_some())
        });
        self.tl_select_track(tr, false);
        self.ch = tr;
        if slot > 0 {
            self.instr = slot;
        }
        self.tlv.sel = Some(id);
        self.tlv.sels = vec![id];
        if kit {
            self.dk.inst = slot as usize - 1;
            self.dk.clip = Some(id);
            self.dk.page = 0;
            self.show_instrument_editor();
        } else {
            self.timeline_open_roll();
        }
    }

    /// an empty 1-bar note clip on track `at` at the play marker's bar
    /// (no snapshot; callers take one), opened in the step sequencer when
    /// the track plays a Drum Kit, else the piano roll: (clip id, bar)
    pub(crate) fn track_add_empty_pattern(&mut self, at: usize) -> Option<(u32, i64)> {
        let cue = self.tlv.cue;
        let slot = self.track_slot(at);
        let res = self.with(|s| {
            let song = &mut s.song;
            let kit = song
                .instruments
                .get((slot as usize).wrapping_sub(1))
                .is_some_and(|i| crate::drumkit::kit_of(i).is_some());
            let t = song.timeline.as_deref()?;
            let sig = t.sig_at(cue);
            let bar = sig.bar_ticks().max(1);
            let from = if cue >= sig.tick { sig.tick } else { 0 };
            let start = from + (cue - from).max(0) / bar * bar;
            let id = crate::timeline::new_note_clip(song, at, start)?;
            let t = song.timeline.as_deref_mut()?;
            let k = t.clips.iter().position(|c| c.id == id)?;
            if let crate::timeline::ClipKind::Notes { data } = t.clips[k].kind {
                let d = t.notes[data as usize].make_mut();
                d.name = "Pattern".into();
                d.len = bar;
                if kit {
                    d.drum = Some(crate::drumkit::DrumMeta::default());
                }
            }
            t.clips[k].len = bar;
            let pos = t.bar_pos(start);
            let crate::audio::Shared { song, engine, .. } = s;
            engine.ensure_plan(song);
            Some((id, pos.bar, kit))
        });
        let (id, bar, _) = res?;
        self.dirty = true;
        crate::plugin::sync(&self.shared, None);
        self.track_open_pattern(at, id);
        Some((id, bar))
    }

    /// the track pie's Dice kit on a drum track: its Drum Kit's editor opens
    /// on its first pattern and rolls (as the kit's own Dice kit; one undo)
    pub(crate) fn track_dice_kit(&mut self, tr: usize) {
        let slot = self.track_slot(tr);
        let kit = self.with(|s| {
            s.song
                .instruments
                .get((slot as usize).wrapping_sub(1))
                .is_some_and(|i| crate::drumkit::kit_of(i).is_some())
        });
        if !kit {
            self.status = format!("Track {:02}: Dice kit needs a Drum Kit", tr + 1);
            return;
        }
        match self.track_first_note_clip(tr) {
            Some((id, _)) => self.track_open_pattern(tr, id),
            None => {
                self.tl_select_track(tr, false);
                self.ch = tr;
                self.instr = slot;
                self.show_instrument_editor();
            }
        }
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(1, |d| d.subsec_nanos() ^ d.as_secs() as u32);
        self.dk_queue_dice(slot as usize - 1, seed);
        self.status = format!("Track {:02}: Dice kit (Undo takes it back)", tr + 1);
    }

    /// the track pie's Sounds › pick: the track's instrument becomes preset
    /// `k` of its type (keeps its name and effects), one undo
    pub(crate) fn track_sound(&mut self, tr: usize, k: usize) {
        let slot = self.track_slot(tr);
        let Some(inst) = self.with(|s| {
            s.song
                .instruments
                .get((slot as usize).wrapping_sub(1))
                .cloned()
        }) else {
            return;
        };
        let Some((_, name, _, kind)) = super::instpanel::instcard::preset_list(&inst)
            .into_iter()
            .nth(k)
        else {
            return;
        };
        self.snapshot();
        self.with(|s| {
            if let Some(x) = s.song.instruments.get_mut(slot as usize - 1) {
                x.kind = kind;
            }
        });
        self.dirty = true;
        crate::plugin::sync(&self.shared, None);
        self.status = format!("Track {:02}: {name}", tr + 1);
    }

    /// the track pie's Chords… on a melodic track: its first pattern (a new
    /// empty 1-bar one when it has none) opens in the piano roll with the
    /// chord generator on its first bar
    pub(crate) fn track_chords(&mut self, tr: usize) {
        if self.track_gen(tr) {
            self.status = format!("Track {:02}: the chord generator", tr + 1);
        }
    }

    /// the track pie's Melody…: as [`Self::track_chords`], on the
    /// Generators panel's Melody tab
    pub(crate) fn track_melody(&mut self, tr: usize) {
        if self.track_gen(tr) {
            let at = self.cl.gen.opts.start_bar as i64 * super::chordlane::BAR;
            self.gen_open_melody(at);
            self.status = format!("Track {:02}: the melody generator", tr + 1);
        }
    }

    /// open the track's first pattern (a new one if it has none) with the
    /// Generators panel on its Chords tab; false: no pattern
    fn track_gen(&mut self, tr: usize) -> bool {
        let (id, at) = match self.track_first_note_clip(tr) {
            Some(c) => c,
            None => {
                self.snapshot();
                self.undo_hold += 1;
                let r = self.track_add_empty_pattern(tr);
                self.undo_hold -= 1;
                let Some((id, _)) = r else {
                    self.status = "Can't add a pattern there".into();
                    return false;
                };
                let at = self.with(|s| {
                    s.song
                        .timeline
                        .as_deref()
                        .and_then(|t| t.clips.iter().find(|c| c.id == id))
                        .map_or(0, |c| c.start)
                });
                (id, at)
            }
        };
        self.track_open_pattern(tr, id);
        self.cl_open_gen_at(at);
        true
    }

    /// the earliest note clip on track `tr`: (id, start)
    fn track_first_note_clip(&self, tr: usize) -> Option<(u32, i64)> {
        self.with(|s| {
            let t = s.song.timeline.as_deref()?;
            t.clips
                .iter()
                .filter(|c| c.track as usize == tr && c.notes().is_some())
                .min_by_key(|c| c.start)
                .map(|c| (c.id, c.start))
        })
    }
}
