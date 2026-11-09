// Copyright (C) 2026 the OpenRB-338 authors.
// SPDX-License-Identifier: GPL-3.0-only
//
// Adapted from OpenRB-338
// https://github.com/pingw33n/openrb-338
// commit cbca4492c0224dffaa477346bc8860559f3d853f
// file crates/orb-engine/src/tables/mod.rs
//
// Only the three bass-line tables: pitch, filter and the cutoff smoother.
// The pan, level, PCF, distortion and drum tables are not included.

//! Bass-line lookup tables. Built once with pinned `libm` 0.2.16.
#![allow(clippy::all, non_snake_case)]

use libm::{cos, exp, pow};

/// An `fstp dword`: the f64 value rounded to f32.
#[inline]
fn st(x: f64) -> f32 {
    x as f32
}

fn boxed<T: Copy, const N: usize>(v: T) -> Box<[T; N]> {
    vec![v; N].into_boxed_slice().try_into().ok().unwrap()
}

/// The bass-line phase increment per sample, indexed by `pitch >> 4`.
pub type PitchTable = [f32; 6144];
/// Filter `FILT[reso >> 4][cutoff] = [c0, c1, c2, c3]`.
pub type FilterTable = [[[f32; 4]; 1024]; 8];

/// A 2-pole cutoff smoother `y = (a*y1 + x*g) - b2*y2`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Smoother {
    pub b2: f32,
    pub a: f32,
    pub g: f32,
}

/// The three tables the voice reads. Built once, then only read.
pub struct Tables {
    pub tb303_pitch: Box<PitchTable>,
    pub tb303_filter: Box<FilterTable>,
    pub tb303_smoother: Smoother,
}

impl Tables {
    pub fn build() -> Self {
        Self {
            tb303_pitch: tb303_pitch(),
            tb303_filter: tb303_filter(),
            tb303_smoother: tb303_smoother(),
        }
    }
}

/// `pow(2.0, 1/12)`: 2.0 high dword at 0x10023A8E, 1/12 at 0x10023A93 (low) / 0x10023A9B (high).
const PITCH_SEMI_EXP: f64 = f64::from_bits(0x3FB5_5555_5555_5555);
const C_10153A90: f64 = 1.0;
const C_10153A98: f64 = 55.0;
const C_10153A80: f64 = 2.0;
/// 1/44100.
const C_10153AB0: f64 = f64::from_bits(0x3EF7_C6F8_C751_F177);
/// `pow(2.0, 1/768)`: 1/768 at 0x10023AF0 (low) / 0x10023B02 (high).
const PITCH_STEP_EXP: f64 = f64::from_bits(0x3F55_5555_5555_5555);
/// 3072.0, high dword at 0x10023B0E.
const PITCH_ORIGIN: f64 = 3072.0;

/// The TB-303 pitch table, 303 ctor `sub_10023920` (guard `byte_10124704`; at 0x10125364):
/// `k = (((1*s)*(s*s))*55*2 * (1/44100)) / pow(p, 3072)` with `s = pow(2, 1/12)` (the cube by
/// the compiler's squaring loop) and `p = pow(2, 1/768)`, then `tab[i] = f32(pow(p, i) * k)`.
pub fn tb303_pitch() -> Box<PitchTable> {
    let s = pow(2.0, PITCH_SEMI_EXP);
    // exponent 3 by squaring (0x10023AC3..0x10023AE3): acc = 1*s, then acc *= s*s
    let cube = (C_10153A90 * s) * (s * s);
    let v18 = (cube * C_10153A98) * C_10153A80;
    let p = pow(2.0, PITCH_STEP_EXP);
    let k = (v18 * C_10153AB0) / pow(p, PITCH_ORIGIN);
    let mut tab: Box<PitchTable> = boxed(0.0);
    for (i, e) in tab.iter_mut().enumerate() {
        *e = st(pow(p, i as f64) * k);
    }
    tab
}

/// `pow(1602.0, 1/1023)`: 1602.0 at 0x100234BC (high) / 0x100234B7 (low), 1/1023 at
/// 0x100234C1 (low) / 0x100234C9 (high).
const FILT_SPAN: f64 = 1602.0;
const FILT_SPAN_EXP: f64 = f64::from_bits(0x3F50_0401_0040_1004);
/// 1023.0, high dword at 0x100234DA.
const FILT_TOP: f64 = 1023.0;
/// f32 1.78 widened: 0x10023507 (low) / 0x10023513 (high); the same at 0x100172FB / 0x10017307
/// (PCF LP) and 0x100175AB / 0x100175B7 (PCF BP).
const Q_BASE: f64 = f64::from_bits(0x3FFC_7AE1_4000_0000);
const C_101539C8: f64 = 0.4;
const C_101539D8: f64 = 0.52;
/// f32 1/7.
const C_10153A58: f32 = f32::from_bits(0x3E12_4925);
const C_101539E0: f64 = f64::from_bits(0x3FC5_5555_5555_5555);
const C_101539E8: f64 = 0.5;
const C_101539F0: f64 = 0.6;
const C_101539F8: f64 = 128.0;
const C_10153A00: f64 = 1.0 / 1024.0;
const C_10153A08: f64 = 0.000125;
const C_10153A10: f64 = 0.3;
const C_10153A5C: f32 = 1.0;
const C_10153A60: f32 = 0.0;
const C_10153A18: f64 = 1.0;
/// The decimal literal 6.283185308.
const C_10153A20: f64 = f64::from_bits(0x4019_21FB_5452_4550);
const C_101539A8: f64 = 1.0;
/// -3.141592654.
const C_10153A28: f64 = f64::from_bits(0xC009_21FB_5452_4550);
const C_10153A30: f64 = 2.0;
/// -6.283185308.
const C_10153A38: f64 = f64::from_bits(0xC019_21FB_5452_4550);
const C_10153A40: f64 = 0.85;
const C_10153A48: f64 = 0.15;
const C_10153A64: f32 = 4.0;
const C_10153A68: f32 = 3.0;
const C_10153A50: f64 = 0.8;

/// The TB-303 filter table FILT, `sub_100234A0` (every 303 ctor, guard `byte_10124700`; at
/// 0x1012B364). See `docs/initial_investigation.md` section 6.2 for the formula.
pub fn tb303_filter() -> Box<FilterTable> {
    let k = pow(FILT_SPAN, FILT_SPAN_EXP);
    let base = C_101539C8 / pow(k, FILT_TOP);
    let mut tab: Box<FilterTable> = boxed([[0.0; 4]; 1024]);
    for (r, row) in tab.iter_mut().enumerate() {
        let rf = r as f64;
        let pm = pow(Q_BASE, rf) * C_101539D8;
        let t = rf * C_10153A58 as f64;
        let m94 = rf * C_101539E0 + C_101539E8;
        let aw = t * C_101539C8 + C_101539F0;
        for (i, e) in row.iter_mut().enumerate() {
            let fi = i as f64;
            let w = (pow(k, fi) * base) * aw;
            let u = (fi * C_101539F8) * C_10153A00;
            let mut q = ((((u * fi) * C_101539F8) * C_10153A00) * C_10153A08) - C_10153A10;
            if q > C_10153A5C as f64 {
                q = C_10153A5C as f64;
            }
            if q < C_10153A60 as f64 {
                q = C_10153A60 as f64;
            }
            let d = q * pm + (C_10153A18 - q) * m94;
            let inv_d = C_101539A8 / d;
            let cs = cos(w * C_10153A20);
            let e1 = exp((w * C_10153A28) * inv_d);
            let c0 = e1 * (cs * C_10153A30);
            let w2 = w * C_10153A38;
            let c1 = exp(w2 * inv_d);
            let one_c0 = C_10153A18 - c0;
            let c3 = exp((t * C_10153A40 + C_10153A48) * w2);
            let num = (C_10153A18 - c3) * (c1 + one_c0);
            let dq = if d > C_10153A64 as f64 {
                C_10153A68 as f64
            } else {
                d
            };
            let c2 = num / ((dq + C_10153A5C as f64) * C_10153A50);
            *e = [st(c0), st(c1), st(c2), st(c3)];
        }
    }
    tab
}

/// The smoothers' angle w = f32 0.0628 widened (0x10153DE0, 0x10152490).
const SMOOTH_W: f64 = f64::from_bits(0x3FB0_15BF_8C18_0485);
/// -w/2 (0x10153DE8, 0x10152498).
const SMOOTH_HALF_NEG_W: f64 = f64::from_bits(0xBFA0_15BF_8C18_0485);
/// 2.0 (0x10153DF0, 0x101524A0).
const SMOOTH_TWO: f64 = 2.0;
/// -w (0x10153DF8, 0x101524A8).
const SMOOTH_NEG_W: f64 = f64::from_bits(0xBFB0_15BF_8C18_0485);
/// 1.0 (0x10153E00, 0x101524B0).
const SMOOTH_ONE: f64 = 1.0;

/// The static initializers `sub_10026BB0` (303, at 0x1012470C) and `sub_10017840` (PCF, at
/// 0x10100260) compute the same constants: `A = exp(-w/2) * (cos(w)*2)`, `b2 = exp(-w)`,
/// `g = (1 - A) + b2`, stored as f32 in the order b2, A, g.
fn smoother() -> Smoother {
    let cs = cos(SMOOTH_W);
    let e1 = exp(SMOOTH_HALF_NEG_W);
    let a = e1 * (cs * SMOOTH_TWO);
    let b2 = exp(SMOOTH_NEG_W);
    let g = (SMOOTH_ONE - a) + b2;
    Smoother {
        b2: st(b2),
        a: st(a),
        g: st(g),
    }
}

/// The TB-303 cutoff smoother (`sub_10026BB0`, at 0x1012470C).
pub fn tb303_smoother() -> Smoother {
    smoother()
}
