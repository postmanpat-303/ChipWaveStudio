//! Poly synth factory presets (original patches).
use super::*;

fn init() -> PolyP {
    PolyP::default()
}

/// adsr (attack, decay, sustain, release) of the filter or amp envelope
fn env(p: &mut PolyP, base: usize, a: f32, d: f32, s: f32, r: f32) {
    p.set(&[(base, a), (base + 1, d), (base + 2, s), (base + 3, r)]);
}

/// (name, tip, patch); concat!(poly_name!(), " Init") first
pub fn presets() -> Vec<(&'static str, &'static str, PolyP)> {
    let mut v = vec![(
        concat!(poly_name!(), " Init"),
        concat!(poly_name!(), ": the Init patch, one plain saw"),
        init(),
    )];

    let mut p = init();
    p.filter = PolyFilter::Lp4;
    p.set(&[
        (CUTOFF, 2200.0),
        (RES, 0.25),
        (ENV_AMT, 2.5),
        (KEYTRK, 0.5),
        (VEL_CUT, 1.0),
        (GLIDE, 0.04),
    ]);
    env(&mut p, FEG, 0.002, 0.45, 0.3, 0.3);
    env(&mut p, AEG, 0.004, 0.3, 0.85, 0.25);
    v.push((
        concat!(poly_name!(), " Saw Lead"),
        concat!(
            poly_name!(),
            ": one saw through a 24 dB low-pass with a short filter envelope; a plain lead"
        ),
        p,
    ));

    let mut p = init();
    p.mode = VoiceMode::Mono;
    p.set_osc(0, -1.0, 0.5, 0.0, -1, 0.7, 1, 0.0, 0.0);
    p.set(&[
        (CUTOFF, 260.0),
        (RES, 0.1),
        (ENV_AMT, 1.6),
        (KEYTRK, 0.6),
        (VEL_CUT, 0.6),
        (GLIDE, 0.03),
    ]);
    env(&mut p, FEG, 0.001, 0.25, 0.2, 0.15);
    env(&mut p, AEG, 0.002, 0.4, 0.9, 0.1);
    v.push((
        concat!(poly_name!(), " Sub Bass"),
        concat!(
            poly_name!(),
            ": a square one octave down plus its sub, dark and round; mono"
        ),
        p,
    ));

    let mut p = init();
    p.mode = VoiceMode::Legato;
    p.reset = [false, false];
    p.set_osc(0, 0.0, 0.5, 0.0, 0, 0.0, 7, 25.0, 0.6);
    p.set_osc(1, 0.0, 0.5, 0.0, 1, 0.0, 3, 12.0, 0.4);
    p.set(&[
        (BLEND, 0.35),
        (CUTOFF, 3800.0),
        (RES, 0.2),
        (ENV_AMT, 2.0),
        (KEYTRK, 0.5),
        (GLIDE, 0.08),
    ]);
    env(&mut p, FEG, 0.005, 0.6, 0.4, 0.4);
    env(&mut p, AEG, 0.005, 0.4, 0.85, 0.35);
    v.push((
        concat!(poly_name!(), " Unison Lead"),
        concat!(
            poly_name!(),
            ": 7 detuned saws with an octave stack, legato glide"
        ),
        p,
    ));

    let mut p = init();
    p.reset = [false, false];
    p.filter = PolyFilter::Lp2;
    p.set_osc(0, 0.0, 0.5, 0.0, 0, 0.0, 16, 45.0, 1.0);
    p.set_osc(1, 1.0, 0.5, 0.0, 0, 0.0, 8, 30.0, 1.0);
    p.set(&[
        (BLEND, 0.3),
        (CUTOFF, 2800.0),
        (RES, 0.1),
        (ENV_AMT, 1.0),
        (KEYTRK, 0.3),
        (GAIN, 0.9),
    ]);
    env(&mut p, FEG, 0.8, 2.0, 0.6, 1.2);
    env(&mut p, AEG, 0.6, 1.0, 0.85, 1.4);
    v.push((
        concat!(poly_name!(), " Supersaw Pad"),
        concat!(
            poly_name!(),
            ": 16 + 8 detuned saws spread wide, slow attack"
        ),
        p,
    ));

    let mut p = init();
    p.set_osc(1, -1.0, 0.3, 7.0, 0, 0.0, 1, 0.0, 0.0);
    p.set(&[
        (BLEND, 0.3),
        (CUTOFF, 380.0),
        (RES, 0.3),
        (ENV_AMT, 4.5),
        (KEYTRK, 0.5),
        (VEL_CUT, 1.5),
        (VEL_AMP, 0.7),
    ]);
    env(&mut p, FEG, 0.001, 0.25, 0.0, 0.2);
    env(&mut p, AEG, 0.001, 0.6, 0.0, 0.3);
    v.push((
        concat!(poly_name!(), " Pluck"),
        concat!(
            poly_name!(),
            ": saw and a fifth, the filter snaps shut; velocity opens it"
        ),
        p,
    ));

    let mut p = init();
    p.reset = [false, false];
    p.filter = PolyFilter::Lp2;
    p.set_osc(0, -0.35, 0.4, 0.0, 0, 0.0, 4, 12.0, 0.7);
    p.set_osc(1, 0.0, 0.5, 0.0, -1, 0.0, 2, 8.0, 0.5);
    p.set(&[
        (BLEND, 0.25),
        (CUTOFF, 1100.0),
        (RES, 0.1),
        (ENV_AMT, 1.0),
        (KEYTRK, 0.4),
        (NOISE, 0.03),
        (COLOUR, -0.6),
        (VEL_AMP, 0.3),
    ]);
    env(&mut p, FEG, 1.5, 3.0, 0.5, 2.0);
    env(&mut p, AEG, 1.2, 2.0, 0.8, 2.0);
    p.route(0, ModSrc::Feg, OSC[0] + PW, 0.3);
    v.push((
        concat!(poly_name!(), " Soft Pad"),
        concat!(
            poly_name!(),
            ": mellow pulse-saw voices with a little noise, slow filter swell"
        ),
        p,
    ));

    let mut p = init();
    p.mode = VoiceMode::Mono;
    p.reset = [false, false];
    p.set_osc(0, -1.0, 0.5, 0.0, -1, 0.0, 2, 9.0, 0.3);
    p.set(&[
        (CUTOFF, 520.0),
        (RES, 0.2),
        (ENV_AMT, 2.2),
        (KEYTRK, 0.5),
        (GLIDE, 0.02),
    ]);
    env(&mut p, FEG, 0.001, 0.35, 0.25, 0.2);
    env(&mut p, AEG, 0.002, 0.5, 0.8, 0.12);
    p.route(0, ModSrc::Feg, OSC[0] + PW, -0.35);
    p.route(1, ModSrc::Key, OSC[0] + PW, 0.2);
    v.push((
        concat!(poly_name!(), " PWM Bass"),
        concat!(
            poly_name!(),
            ": two pulses whose width follows the filter envelope; mono"
        ),
        p,
    ));

    let mut p = init();
    p.mode = VoiceMode::Mono;
    p.set_osc(0, 0.0, 0.5, 0.0, 0, 0.0, 1, 0.0, 0.0);
    p.set_osc(1, 0.0, 0.5, 0.0, -1, 0.0, 2, 10.0, 0.3);
    p.set(&[
        (OSC[0] + SYNC, 14.0),
        (BLEND, 0.2),
        (CUTOFF, 5200.0),
        (RES, 0.15),
        (ENV_AMT, 1.0),
        (KEYTRK, 0.5),
        (GLIDE, 0.05),
    ]);
    env(&mut p, FEG, 0.002, 0.8, 0.2, 0.3);
    env(&mut p, AEG, 0.003, 0.3, 0.9, 0.25);
    p.route(0, ModSrc::Feg, OSC[0] + SYNC, 0.45);
    v.push((
        concat!(poly_name!(), " Sync Lead"),
        concat!(
            poly_name!(),
            ": hard-synced saw swept by the filter envelope; mono glide"
        ),
        p,
    ));

    // two identical oscillators cancel with NEG at 50 %: only the noise is left
    let mut p = init();
    p.blend = BlendOp::Neg;
    p.filter = PolyFilter::Bp2;
    p.set(&[
        (BLEND, 0.5),
        (NOISE, 1.0),
        (GAIN, 1.6),
        (COLOUR, 0.3),
        (CUTOFF, 300.0),
        (RES, 0.5),
        (ENV_AMT, 5.0),
        (KEYTRK, 1.0),
        (FM_AMT, 0.12),
        (FM_FREQ, 27.0),
    ]);
    env(&mut p, FEG, 2.5, 1.0, 1.0, 1.5);
    env(&mut p, AEG, 0.8, 1.0, 1.0, 1.5);
    v.push((concat!(poly_name!(), " Noise Riser"), concat!(poly_name!(), ": noise through a resonant band-pass that opens over seconds (the oscillators cancel in NEG)"), p));

    let mut p = init();
    p.mode = VoiceMode::Mono;
    p.blend = BlendOp::Max;
    p.shaper = ShaperMode::Fold;
    p.set_osc(0, 0.0, 0.5, 0.0, -1, 0.3, 1, 0.0, 0.0);
    p.set_osc(1, -1.0, 0.3, 0.1, -1, 0.0, 2, 14.0, 0.2);
    p.set(&[
        (BLEND, 0.5),
        (DRIVE, 0.55),
        (FEEDBACK, 0.3),
        (CUTOFF, 700.0),
        (RES, 0.45),
        (ENV_AMT, 3.0),
        (KEYTRK, 0.5),
        (SHAPER, 0.3),
        (SHAPER_MIX, 0.6),
        (GAIN, 0.8),
    ]);
    env(&mut p, FEG, 0.001, 0.3, 0.2, 0.2);
    env(&mut p, AEG, 0.002, 0.4, 0.85, 0.12);
    v.push((
        concat!(poly_name!(), " Dirty Bass"),
        concat!(
            poly_name!(),
            ": MAX-blended saw and pulse, driven, fed back and folded; mono"
        ),
        p,
    ));

    v
}

/// a factory preset by name
#[allow(dead_code)] // WIP: wired up in the next commits
pub fn by_name(name: &str) -> Option<PolyP> {
    presets()
        .into_iter()
        .find(|(n, _, _)| *n == name)
        .map(|(_, _, p)| p)
}
