//! Unified Filter card (design A §3.8): one mapping table so every engine
//! that has a filter shows the same labels (Cutoff, Reso, Env amt, …) and
//! the same automation targets where they exist. Presentation only — no DSP
//! or file-format change.
use crate::automation::Target;
use crate::model::{Instrument, InstrumentKind};
use crate::synth::{FilterMode, Patch};
use eframe::egui;

/// which filter controls an instrument exposes through the unified card
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum FilterKind {
    /// synth / voice / moog-style patch filter
    Patch,
    /// Analog / Wave / Chip FX / Noise / Drum / Kit / FM instrument filter
    Generator,
    /// none (sample, plugin, …)
    None,
}

#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
pub struct FilterMap {
    pub kind: FilterKind,
    /// automation targets for Cutoff / Reso when they map 1:1
    pub cutoff: Option<Target>,
    pub reso: Option<Target>,
    /// pattern command hint (38xx / 39xx)
    pub cmd_cut: &'static str,
    pub cmd_res: &'static str,
}

impl FilterMap {
    #[allow(dead_code)]
    pub fn for_instrument(i: usize, inst: &Instrument) -> Self {
        let cutoff = Some(Target::Cutoff(i));
        let reso = Some(Target::Resonance(i));
        match &inst.kind {
            InstrumentKind::Synth(_) | InstrumentKind::Voice(_) => Self {
                kind: FilterKind::Patch,
                cutoff,
                reso,
                cmd_cut: "38xx",
                cmd_res: "39xx",
            },
            InstrumentKind::Gen(_) => Self {
                kind: FilterKind::Generator,
                cutoff,
                reso,
                cmd_cut: "38xx",
                cmd_res: "39xx",
            },
            _ => Self {
                kind: FilterKind::None,
                cutoff: None,
                reso: None,
                cmd_cut: "",
                cmd_res: "",
            },
        }
    }

    pub fn has_filter(self) -> bool {
        self.kind != FilterKind::None
    }
}

/// shared labels (design A): same names everywhere
pub const LABEL_CUTOFF: &str = "Cutoff";
pub const LABEL_RESO: &str = "Reso";
pub const LABEL_ENV: &str = "Env amt";
pub const LABEL_DRIVE: &str = "Drive";

/// draw a compact Filter header row (A / ~ / Cmd) above existing controls
#[allow(dead_code)]
pub fn header_buttons(ui: &mut egui::Ui, map: FilterMap, on_auto: &mut bool, on_mod: &mut bool) {
    if !map.has_filter() {
        return;
    }
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Filter").strong());
        if ui
            .small_button("A")
            .on_hover_text("open Cutoff and Reso automation lanes on the track")
            .clicked()
        {
            *on_auto = true;
        }
        if ui
            .small_button("~")
            .on_hover_text("add an LFO → cutoff modulation route")
            .clicked()
        {
            *on_mod = true;
        }
        if !map.cmd_cut.is_empty() {
            ui.label(
                egui::RichText::new(format!("Cmd {}", map.cmd_cut))
                    .small()
                    .weak(),
            )
            .on_hover_text(format!(
                "{} = cutoff, {} = resonance (command wins at its row)",
                map.cmd_cut, map.cmd_res
            ));
        }
    });
}

/// mode names matching the unified card (only engine-supported modes)
pub fn mode_name(m: FilterMode) -> &'static str {
    match m {
        FilterMode::Lp => "LP",
        FilterMode::Bp => "BP",
        FilterMode::Hp => "HP",
    }
}

/// bind patch filter fields for the unified labels (Synth / Voice)
#[allow(dead_code)]
pub fn patch_fields(p: &mut Patch) -> (&mut FilterMode, &mut f32, &mut f32, &mut f32, &mut f32) {
    (
        &mut p.filter,
        &mut p.cutoff,
        &mut p.resonance,
        &mut p.env_amount,
        &mut p.drive,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Instrument, InstrumentKind};

    #[test]
    fn synth_maps_to_patch_filter_and_38xx() {
        let inst = Instrument {
            name: "Bass".into(),
            volume: 1.0,
            transpose: 0,
            fx: vec![],
            kind: InstrumentKind::Synth(Default::default()),
        };
        let m = FilterMap::for_instrument(0, &inst);
        assert_eq!(m.kind, FilterKind::Patch);
        assert_eq!(m.cutoff, Some(Target::Cutoff(0)));
        assert_eq!(m.cmd_cut, "38xx");
        assert_eq!(LABEL_CUTOFF, "Cutoff");
        assert_eq!(LABEL_RESO, "Reso");
    }

    #[test]
    fn sample_has_no_filter_card() {
        let inst = Instrument {
            name: "Smp".into(),
            volume: 1.0,
            transpose: 0,
            fx: vec![],
            kind: InstrumentKind::Sample {
                file: String::new(),
                rate: 44100,
                data: crate::model::SampleData::from(vec![]),
                base_note: 60,
                lp: Default::default(),
            },
        };
        assert!(!FilterMap::for_instrument(0, &inst).has_filter());
    }
}
