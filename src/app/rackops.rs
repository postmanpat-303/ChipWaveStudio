//! Rack edits from the panel, the Instruments list and the track menu
//! (docs/racks/PLAN.md 4.7): make a rack, add / remove layers, a layer's
//! settings. Each is one undo step.
use super::*;
use crate::model::{Layer, MAX_LAYERS};

/// why instrument `idx` can't become a rack (None = it can)
pub(crate) fn no_rack_reason(song: &Song, idx: usize) -> Option<&'static str> {
    let inst = song.instruments.get(idx)?;
    match inst.kind {
        InstrumentKind::Rack(_) => Some("it is a rack already"),
        InstrumentKind::Plugin(_) => Some("plugin instruments can't play as a rack layer yet"),
        _ if song.rack_of(idx).is_some() => Some("it is a layer of a rack"),
        _ if song.instruments.len() >= 255 => Some("all 255 instrument slots are used"),
        _ => None,
    }
}

/// why slot `slot` can't be added to rack `rack` as a layer
pub(crate) fn no_layer_reason(song: &Song, rack: usize, slot: usize) -> Option<&'static str> {
    let InstrumentKind::Rack(r) = &song.instruments.get(rack)?.kind else {
        return Some("not a rack");
    };
    let inst = song.instruments.get(slot)?;
    if r.layers.len() >= MAX_LAYERS {
        Some("a rack has at most 4 layers")
    } else if slot == rack || matches!(inst.kind, InstrumentKind::Rack(_)) {
        Some("a rack can't go inside a rack")
    } else if matches!(inst.kind, InstrumentKind::Plugin(_)) {
        Some("plugin instruments can't play as a rack layer yet")
    } else if r.layers.iter().any(|l| l.slot == slot) {
        Some("it is a layer of this rack already")
    } else {
        None
    }
}

impl App {
    /// "Make a rack from this instrument": it becomes layer A of a new rack
    /// in its own slot, sounding the same
    pub(crate) fn make_rack(&mut self, idx: usize) {
        if let Some(why) = self.with(|s| no_rack_reason(&s.song, idx)) {
            self.status = format!("Can't make a rack: {why}");
            return;
        }
        self.snapshot();
        let slot = self.with(|s| s.song.make_rack(idx));
        if let Some(slot) = slot {
            self.dirty = true;
            self.instr = (idx + 1) as u8;
            self.dev.sel = Some(super::chains::Sel::Layer(idx, 0));
            self.status = format!(
                "Made a rack: its sound is layer A (slot {:02X}); + Add layer for more",
                slot + 1
            );
        }
    }

    /// Add layer: a new init synth (`existing` None) or an instrument
    /// already in the song
    pub(crate) fn add_layer(&mut self, rack: usize, existing: Option<usize>) {
        let full = self.with(|s| match &s.song.instruments.get(rack).map(|i| &i.kind) {
            Some(InstrumentKind::Rack(r)) => r.layers.len() >= MAX_LAYERS,
            _ => true,
        });
        if full {
            self.status = "A rack has at most 4 layers".into();
            return;
        }
        if let Some(slot) = existing {
            if let Some(why) = self.with(|s| no_layer_reason(&s.song, rack, slot)) {
                self.status = format!("Can't add that layer: {why}");
                return;
            }
        } else if self.with(|s| s.song.instruments.len()) >= 255 {
            self.status = "All 255 instrument slots are used".into();
            return;
        }
        self.snapshot();
        let k = self.with(|s| {
            let slot = match existing {
                Some(x) => x,
                None => {
                    let n = s.song.instruments.len();
                    s.song.instruments.push(Instrument {
                        arp: None,
                        name: format!("Layer {}", n + 1),
                        volume: 0.8,
                        transpose: 0,
                        fx: vec![],
                        kind: InstrumentKind::Synth(Default::default()),
                    });
                    n
                }
            };
            match &mut s.song.instruments[rack].kind {
                InstrumentKind::Rack(r) => {
                    r.layers.push(Layer::new(slot));
                    Some((r.layers.len() - 1, slot))
                }
                _ => None,
            }
        });
        if let Some((k, slot)) = k {
            self.dirty = true;
            self.dev.sel = Some(super::chains::Sel::Layer(rack, k));
            let name = self.with(|s| s.song.instruments[slot].name.clone());
            self.status = format!(
                "Added layer {} ({slot:02X} {name})",
                layer_letter(k),
                slot = slot + 1
            );
        }
    }

    /// take layer `k` out of rack `rack`; its instrument stays in the song
    pub(crate) fn remove_layer(&mut self, rack: usize, k: usize) {
        let ok = self.with(|s| {
            matches!(&s.song.instruments.get(rack).map(|i| &i.kind),
                Some(InstrumentKind::Rack(r)) if k < r.layers.len())
        });
        if !ok {
            return;
        }
        self.snapshot();
        let slot = self.with(|s| match &mut s.song.instruments[rack].kind {
            InstrumentKind::Rack(r) => r.layers.remove(k).slot,
            _ => 0,
        });
        self.dirty = true;
        self.dev.sel = Some(super::chains::Sel::Inst(rack));
        self.status = format!(
            "Removed layer {} from the rack (instrument {:02X} is still in the song; Ctrl+Z brings the layer back)",
            layer_letter(k),
            slot + 1
        );
    }

    /// a layer's settings changed (`step`: starts an undo step; slider
    /// drags after the first frame don't)
    pub(crate) fn set_layer(&mut self, rack: usize, k: usize, l: Layer, step: bool) {
        if step {
            self.snapshot();
        }
        self.with(|s| {
            if let Some(InstrumentKind::Rack(r)) =
                s.song.instruments.get_mut(rack).map(|i| &mut i.kind)
            {
                if let Some(x) = r.layers.get_mut(k) {
                    *x = l;
                }
            }
        });
        self.dirty = true;
    }
}

/// "A".."D"
pub(crate) fn layer_letter(k: usize) -> char {
    (b'A' + k.min(25) as u8) as char
}
