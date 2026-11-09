// Copyright (C) 2026 the OpenRB-338 authors.
// SPDX-License-Identifier: GPL-3.0-only
//
// Adapted from OpenRB-338 commit cbca4492c0224dffaa477346bc8860559f3d853f.
// https://github.com/pingw33n/openrb-338
// What was kept and what was left out is listed in NOTICE.

//! Bass-line voice adapted from OpenRB-338. GPL-3.0-only.
//! See `NOTICE` for the file list and the licence consequence of linking it.

mod tables;
mod voice;

pub use tables::Tables;
pub use voice::{Bass, Block, Params, ACCENT, CUTOFF, DECAY, ENVMOD, RESONANCE, TUNE, WAVEFORM};

use std::sync::OnceLock;

/// The pitch, filter and smoother tables. Built on the first call, then only read.
pub fn tables() -> &'static Tables {
    static T: OnceLock<Tables> = OnceLock::new();
    T.get_or_init(Tables::build)
}

/// Build the tables now, so the audio thread only reads them.
pub fn warm() {
    let _ = tables();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn play(vintage: bool, accent: bool) -> [f32; 64] {
        let mut v = Bass::new();
        v.set_sound_model(vintage);
        v.set_param(CUTOFF as u8, 40);
        v.set_param(RESONANCE as u8, 90);
        v.set_param(ENVMOD as u8, 80);
        v.set_param(DECAY as u8, 80);
        v.set_param(ACCENT as u8, 100);
        v.set_param(TUNE as u8, 0x40);
        v.note_on([12, 0, u8::from(accent), 0, 0, 0]);
        let mut bus = [0.0; 64];
        v.process(&mut bus, tables());
        bus
    }

    #[test]
    fn a_note_is_finite_and_audible_in_both_models() {
        for vintage in [false, true] {
            let b = play(vintage, false);
            assert!(b.iter().all(|x| x.is_finite()), "vintage {vintage}");
            let peak = b.iter().map(|x| x.abs()).fold(0.0_f32, f32::max);
            assert!(peak > 1.0e-3, "vintage {vintage} peak {peak}");
        }
    }

    #[test]
    fn accent_changes_the_block() {
        let a = play(false, false);
        let b = play(false, true);
        assert!(a.iter().zip(b).any(|(x, y)| (x - y).abs() > 1.0e-4));
    }

    #[test]
    fn pitch_table_origin_is_110_hz() {
        let t = tables();
        // The stored increment times 44100 is an octave above the note the
        // oscillator plays, because the phase cycle is 2.0 wide.
        let inc = t.tb303_pitch[3072] * 44100.0;
        assert!((inc - 130.81).abs() < 0.05, "{inc}");
    }

    #[test]
    fn two_voices_do_not_share_state() {
        let mut a = Bass::new();
        let mut b = Bass::new();
        a.note_on([0, 0, 0, 0, 0, 0]);
        b.note_on([24, 0, 0, 0, 0, 0]);
        let t = tables();
        let mut ba = [0.0; 64];
        let mut bb = [0.0; 64];
        a.process(&mut ba, t);
        b.process(&mut bb, t);
        assert!(ba.iter().zip(bb).any(|(x, y)| (x - y).abs() > 1.0e-3));
        let mut a2 = Bass::new();
        a2.note_on([0, 0, 0, 0, 0, 0]);
        let mut ba2 = [0.0; 64];
        a2.process(&mut ba2, t);
        assert_eq!(ba, ba2);
    }
}
