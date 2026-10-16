//! Loads factory .cwpatch/.toml presets (presets/factory) into the simple synth.
//! Only the subset of keys our MVP synth supports is read; the rest is ignored.
use crate::synth::{Adsr, FilterMode, Patch, Wave};
use toml::Value;

pub const FACTORY: &[(&str, &str)] = &[
    (
        "psytrance/punchy-psy-kick",
        include_str!("../presets/factory/psytrance/punchy-psy-kick.toml"),
    ),
    (
        "psytrance/rolling-offbeat-bass",
        include_str!("../presets/factory/psytrance/rolling-offbeat-bass.toml"),
    ),
    (
        "psytrance/acid-squelch",
        include_str!("../presets/factory/psytrance/acid-squelch.toml"),
    ),
    (
        "psytrance/psy-pluck",
        include_str!("../presets/factory/psytrance/psy-pluck.toml"),
    ),
    (
        "psytrance/supersaw-lead",
        include_str!("../presets/factory/psytrance/supersaw-lead.toml"),
    ),
    (
        "psytrance/trance-gate-pad",
        include_str!("../presets/factory/psytrance/trance-gate-pad.toml"),
    ),
    (
        "psytrance/zap",
        include_str!("../presets/factory/psytrance/zap.toml"),
    ),
    (
        "alien/flutter-warble",
        include_str!("../presets/factory/alien/flutter-warble.toml"),
    ),
];

fn f(v: Option<&Value>, d: f32) -> f32 {
    match v {
        Some(Value::Float(x)) => *x as f32,
        Some(Value::Integer(i)) => *i as f32,
        _ => d,
    }
}
fn ms(v: Option<&Value>, d: f32) -> f32 {
    match v {
        Some(Value::Float(_)) | Some(Value::Integer(_)) => f(v, d * 1000.0) / 1000.0,
        _ => d,
    }
}
fn wave(s: Option<&Value>) -> Wave {
    match s.and_then(|v| v.as_str()).unwrap_or("saw") {
        "square" => Wave::Square,
        "pulse" => Wave::Pulse,
        "triangle" => Wave::Triangle,
        s if s.contains("sine") => Wave::Sine,
        _ => Wave::Saw,
    }
}
fn adsr(t: Option<&Value>, d: Adsr) -> Adsr {
    let Some(t) = t else { return d };
    Adsr {
        a: ms(t.get("attack_ms"), d.a) + ms(t.get("hold_ms"), 0.0),
        d: ms(t.get("decay_ms"), d.d),
        s: f(t.get("sustain"), d.s),
        r: ms(t.get("release_ms"), d.r),
    }
}

pub fn parse(name: &str, text: &str) -> Result<Patch, String> {
    let v: Value = toml::from_str(text).map_err(|e| e.to_string())?;
    let mut p = Patch {
        name: v
            .get("name")
            .and_then(|x| x.as_str())
            .unwrap_or(name)
            .to_string(),
        ..Default::default()
    };
    if let Some(o) = v.get("osc1") {
        p.osc1 = wave(o.get("wave"));
        p.osc1_oct = f(o.get("octave"), 0.0) as i32;
        p.osc1_level = f(o.get("level"), 0.8);
        p.pulse_width = f(o.get("pulse_width"), 0.5);
        if let Some(u) = o.get("unison") {
            p.unison = f(u.get("voices"), 1.0) as u32;
            p.detune_cents = f(u.get("detune"), 0.0);
        }
    }
    if let Some(o) = v.get("osc2") {
        let semi = f(o.get("semi"), 0.0) as i32 + 12 * f(o.get("octave"), 0.0) as i32;
        p.osc2 = Some((wave(o.get("wave")), semi, f(o.get("level"), 0.5)));
    }
    if let Some(o) = v.get("sub") {
        p.sub_level = f(o.get("level"), 0.0);
    }
    if let Some(o) = v.get("noise") {
        p.noise_level = f(o.get("level"), 0.0);
    }
    if let Some(fl) = v.get("filter1") {
        let mode = fl.get("mode").and_then(|x| x.as_str()).unwrap_or("lp");
        p.filter = if mode.starts_with("hp") {
            FilterMode::Hp
        } else if mode.starts_with("bp") {
            FilterMode::Bp
        } else {
            FilterMode::Lp
        };
        p.cutoff = f(fl.get("cutoff_hz"), 4000.0);
        p.resonance = f(fl.get("resonance"), 0.2);
        p.key_track = f(fl.get("key_track"), 0.0);
        p.env_amount = f(fl.get("env_amount"), 0.0);
        p.drive = f(fl.get("drive"), 0.0);
    }
    let env = v.get("env");
    p.amp = adsr(env.and_then(|e| e.get("amp")), p.amp);
    p.fenv = adsr(env.and_then(|e| e.get("filter")), p.fenv);
    // pitch envelope: any [[mod]] with source env.mod -> osc1.pitch
    if let Some(Value::Array(mods)) = v.get("mod") {
        for m in mods {
            if m.get("source").and_then(|x| x.as_str()) == Some("env.mod")
                && m.get("target").and_then(|x| x.as_str()) == Some("osc1.pitch")
            {
                let decay = ms(
                    env.and_then(|e| e.get("mod"))
                        .and_then(|e| e.get("decay_ms")),
                    0.05,
                );
                p.pitch_env = (f(m.get("amount"), 0.0) * 60.0, decay.max(0.005) / 2.0);
            }
        }
    }
    Ok(p)
}

pub fn factory(name: &str) -> Option<Patch> {
    FACTORY
        .iter()
        .find(|(n, _)| *n == name)
        .and_then(|(n, t)| parse(n, t).ok())
}

/// Synthetic hi-hat (factory drum samples are still to be made).
pub fn hat(open: bool) -> Patch {
    Patch {
        name: if open { "Open Hat" } else { "Closed Hat" }.into(),
        osc1_level: 0.0,
        noise_level: 0.9,
        filter: FilterMode::Hp,
        cutoff: 7000.0,
        resonance: 0.3,
        amp: Adsr {
            a: 0.0,
            d: if open { 0.25 } else { 0.05 },
            s: 0.0,
            r: 0.03,
        },
        gain: 0.5,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_factory_presets_parse() {
        for (n, t) in super::FACTORY {
            super::parse(n, t).unwrap_or_else(|e| panic!("{n}: {e}"));
        }
    }
}
