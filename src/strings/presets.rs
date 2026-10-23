//! StringWave factory presets. Masters are level-matched so a chord peaks
//! around -3.5 dBFS (checked by the tests).
use super::*;

type Layer = (bool, Wave, i32, f32, f32, f32, f32);

fn layers(p: &mut StringsP, l: [Layer; 3]) {
    for (k, (on, wave, oct, det, tone, lvl, pan)) in l.into_iter().enumerate() {
        p.layer_on[k] = on;
        p.wave[k] = wave;
        p.layer(k, oct, det, tone, lvl, pan);
    }
}
fn amp(p: &mut StringsP, a: f32, d: f32, s: f32, r: f32) {
    p.set(&[(AMP_A, a), (AMP_D, d), (AMP_S, s), (AMP_R, r)]);
}
fn fenv(p: &mut StringsP, a: f32, d: f32, s: f32, r: f32) {
    p.set(&[(F_A, a), (F_D, d), (F_S, s), (F_R, r)]);
}
fn lfo(p: &mut StringsP, n: usize, wave: LfoWave, rate: f32, depth: f32, dest: Dest) {
    p.lfo_wave[n] = wave;
    p.lfo_dest[n] = dest;
    p.set(&[(LFO[n], rate), (LFO[n] + 1, depth)]);
}
/// chorus (on, rate, depth, mix), echo (on, time, feedback, tone, mix),
/// reverb (on, size, damp, mix)
fn fx(
    p: &mut StringsP,
    cho: (bool, f32, f32, f32),
    echo: (bool, f32, f32, f32, f32),
    rev: (bool, f32, f32, f32),
) {
    p.cho_on = cho.0;
    p.echo_on = echo.0;
    p.rev_on = rev.0;
    p.set(&[
        (CHO_RATE, cho.1),
        (CHO_DEPTH, cho.2),
        (CHO_MIX, cho.3),
        (ECHO_TIME, echo.1),
        (ECHO_FB, echo.2),
        (ECHO_TONE, echo.3),
        (ECHO_MIX, echo.4),
        (REV_SIZE, rev.1),
        (REV_DAMP, rev.2),
        (REV_MIX, rev.3),
    ]);
}

/// (name, tip, patch)
pub fn presets() -> Vec<(&'static str, &'static str, StringsP)> {
    use Wave::*;
    let mut v = vec![(
        concat!(strings_name!(), " Velvet Ensemble"),
        concat!(
            strings_name!(),
            ": three warm layers, gentle vibrato, chorus and a big hall"
        ),
        StringsP::default(),
    )];

    let mut p = StringsP::default();
    p.set(&[(MASTER, -2.4)]);
    layers(
        &mut p,
        [
            (true, Bowed, 0, 0.45, 0.6, 0.7, -0.4),
            (true, Bowed, -1, 0.3, 0.4, 0.45, 0.4),
            (true, Ensemble, 1, 0.25, 0.5, 0.2, 0.0),
        ],
    );
    p.set(&[
        (SUB_LVL, 0.15),
        (DRONE_LVL, 0.1),
        (CUTOFF, 2400.0),
        (RESO, 0.1),
    ]);
    amp(&mut p, 0.9, 1.5, 0.9, 2.8);
    lfo(&mut p, 0, LfoWave::Sine, 4.8, 0.08, Dest::Pitch);
    lfo(&mut p, 1, LfoWave::Sine, 0.08, 0.3, Dest::LayerMix);
    fx(
        &mut p,
        (true, 0.4, 0.5, 0.35),
        (false, 0.45, 0.25, 0.5, 0.1),
        (true, 0.95, 0.35, 0.5),
    );
    v.push((
        concat!(strings_name!(), " Bowed Cathedral"),
        concat!(strings_name!(), ": slow bowed strings in a long reverb"),
        p,
    ));

    let mut p = StringsP::default();
    p.set(&[(MASTER, -7.2)]);
    layers(
        &mut p,
        [
            (true, Plucked, 0, 0.25, 0.75, 0.8, -0.3),
            (true, Plucked, 1, 0.35, 0.6, 0.4, 0.3),
            (true, Ensemble, 1, 0.3, 0.5, 0.12, 0.0),
        ],
    );
    p.set(&[
        (SUB_LVL, 0.1),
        (DRONE_LVL, 0.0),
        (CUTOFF, 7000.0),
        (F_ENV, 0.3),
    ]);
    fenv(&mut p, 0.002, 0.8, 0.3, 1.0);
    amp(&mut p, 0.002, 3.0, 0.25, 2.5);
    lfo(&mut p, 0, LfoWave::Sine, 5.0, 0.03, Dest::Pitch);
    lfo(&mut p, 1, LfoWave::Triangle, 0.2, 0.15, Dest::Cutoff);
    fx(
        &mut p,
        (true, 0.6, 0.4, 0.3),
        (true, 0.33, 0.35, 0.65, 0.22),
        (true, 0.8, 0.3, 0.38),
    );
    v.push((
        concat!(strings_name!(), " Harp Shimmer"),
        concat!(strings_name!(), ": plucked strings with a shimmering echo"),
        p,
    ));

    let mut p = StringsP::default();
    p.set(&[(MASTER, -4.6)]);
    layers(
        &mut p,
        [
            (true, Ensemble, -1, 0.55, 0.5, 0.55, -0.3),
            (true, Body, -1, 0.2, 0.7, 0.4, 0.3),
            (false, Ensemble, 0, 0.3, 0.5, 0.2, 0.0),
        ],
    );
    p.sub_oct = SubOct::Down2;
    p.f24 = true;
    p.set(&[
        (SUB_LVL, 0.45),
        (DRONE_LVL, 0.5),
        (DRONE_TONE, 0.25),
        (CUTOFF, 900.0),
        (RESO, 0.3),
    ]);
    amp(&mut p, 1.5, 2.0, 0.9, 3.5);
    lfo(&mut p, 0, LfoWave::Sine, 0.12, 0.35, Dest::Cutoff);
    lfo(&mut p, 1, LfoWave::SampleHold, 0.6, 0.12, Dest::Reverb);
    fx(
        &mut p,
        (true, 0.25, 0.7, 0.5),
        (true, 0.7, 0.45, 0.3, 0.15),
        (true, 0.85, 0.6, 0.4),
    );
    v.push((
        concat!(strings_name!(), " Undertow Drone"),
        concat!(strings_name!(), ": dark low strings over a deep drone"),
        p,
    ));

    let mut p = StringsP::default();
    p.set(&[(MASTER, 1.8)]);
    layers(
        &mut p,
        [
            (true, Body, 0, 0.4, 0.75, 0.6, -0.35),
            (true, Body, 1, 0.3, 0.55, 0.35, 0.35),
            (true, Ensemble, 0, 0.3, 0.5, 0.25, 0.0),
        ],
    );
    p.set(&[(CUTOFF, 2600.0), (RESO, 0.25)]);
    amp(&mut p, 0.6, 1.2, 0.8, 2.0);
    lfo(&mut p, 0, LfoWave::Triangle, 0.3, 0.4, Dest::LayerMix);
    lfo(&mut p, 1, LfoWave::Sine, 0.11, 0.2, Dest::Cutoff);
    fx(
        &mut p,
        (true, 0.5, 0.7, 0.55),
        (false, 0.4, 0.3, 0.5, 0.1),
        (true, 0.7, 0.5, 0.33),
    );
    v.push((
        concat!(strings_name!(), " Hollow Body Pad"),
        concat!(strings_name!(), ": a woody, hollow pad"),
        p,
    ));

    let mut p = StringsP::default();
    p.set(&[(MASTER, -2.1)]);
    layers(
        &mut p,
        [
            (true, Ensemble, 0, 0.4, 0.5, 0.6, -0.3),
            (true, Plucked, 0, 0.3, 0.65, 0.55, 0.3),
            (false, Bowed, 1, 0.3, 0.5, 0.25, 0.0),
        ],
    );
    p.set(&[(CUTOFF, 4200.0)]);
    amp(&mut p, 0.15, 1.0, 0.8, 1.2);
    fx(
        &mut p,
        (true, 0.5, 0.5, 0.4),
        (true, 0.5, 0.55, 0.45, 0.35),
        (true, 0.6, 0.45, 0.25),
    );
    v.push((
        concat!(strings_name!(), " Echo Strings"),
        concat!(strings_name!(), ": ensemble and plucks through a long echo"),
        p,
    ));

    let mut p = StringsP::default();
    p.set(&[(MASTER, -3.0)]);
    layers(
        &mut p,
        [
            (true, Ensemble, 0, 0.5, 0.5, 0.65, -0.45),
            (true, Bowed, 0, 0.35, 0.5, 0.5, 0.45),
            (true, Ensemble, -1, 0.4, 0.5, 0.35, 0.0),
        ],
    );
    p.set(&[(SUB_LVL, 0.18), (CUTOFF, 1500.0), (F_ENV, 0.55)]);
    fenv(&mut p, 2.5, 3.0, 0.7, 3.0);
    amp(&mut p, 2.5, 2.0, 0.9, 3.0);
    lfo(&mut p, 0, LfoWave::Sine, 5.4, 0.07, Dest::Pitch);
    lfo(&mut p, 1, LfoWave::Sine, 0.07, 0.2, Dest::Amp);
    fx(
        &mut p,
        (true, 0.45, 0.65, 0.5),
        (true, 0.6, 0.25, 0.4, 0.1),
        (true, 0.9, 0.4, 0.42),
    );
    v.push((
        concat!(strings_name!(), " Slow Swell Orchestra"),
        concat!(strings_name!(), ": a slow orchestral swell that opens up"),
        p,
    ));

    let mut p = StringsP::default();
    p.set(&[(MASTER, -7.4)]);
    p.layer_on[1] = false;
    p.layer_on[2] = false;
    p.wave[1] = Bowed;
    p.wave[2] = Ensemble;
    p.layer(1, 0, 0.3, 0.55, 0.55, 0.0);
    p.layer(2, 1, 0.35, 0.5, 0.28, 0.35);
    p.set(&[(SUB_LVL, 0.0), (DRONE_LVL, 0.0)]);
    lfo(&mut p, 0, LfoWave::Sine, 5.2, 0.0, Dest::Pitch);
    lfo(&mut p, 1, LfoWave::Triangle, 0.15, 0.0, Dest::Cutoff);
    fx(
        &mut p,
        (false, 0.5, 0.6, 0.5),
        (false, 0.38, 0.3, 0.5, 0.15),
        (false, 0.75, 0.45, 0.3),
    );
    v.push((
        concat!(strings_name!(), " Init"),
        concat!(strings_name!(), ": one plain ensemble layer, no effects"),
        p,
    ));
    v
}
