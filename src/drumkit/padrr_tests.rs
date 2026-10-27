//! Pad Round Robin (docs/ROUND_ROBIN.md, part B).
use super::padrr::*;
use super::*;
use crate::gen::{DrumKind, GenEngine, GenPatch};
use crate::timeline::rr::Order;

const SR: f32 = 48000.0;
/// the default kit's snare pad and its key
const SNARE: usize = 1;

fn kit(f: impl Fn(&mut Pad)) -> GenPatch {
    let mut g = GenPatch {
        engine: GenEngine::Drums(DrumKitP::default()),
        ..Default::default()
    };
    if let GenEngine::Drums(k) = &mut g.engine {
        f(&mut k.pads[SNARE]);
    }
    g
}

fn snare_key(g: &GenPatch) -> u8 {
    match &g.engine {
        GenEngine::Drums(k) => k.pads[SNARE].key,
        _ => unreachable!(),
    }
}

/// hit `hit` on the snare pad, sample by sample
fn render(g: &GenPatch, hit: u32) -> Vec<(f32, f32)> {
    let freq = crate::model::note_freq(snare_key(g));
    let mut v = crate::synth::SynthVoice::seeded(9);
    v.gen.hit = hit;
    (0..(SR * 0.3) as usize)
        .map(|_| v.render_gen(&g.base, g, freq, SR, 120.0))
        .collect()
}

fn rr(alts: usize, variation: f32) -> PadRr {
    PadRr {
        alts: (0..alts)
            .map(|i| crate::gen::DrumP::new(DrumKind::Snare, (i + 1) as u8 % 3, &[]))
            .collect(),
        variation,
        ..Default::default()
    }
}

#[test]
fn off_by_default_and_not_saved_until_used() {
    let p = Pad::default();
    assert!(p.rr.is_none());
    let t = toml::to_string(&p).unwrap();
    assert!(!t.contains("rr"), "{t}");
    let mut q = p.clone();
    q.rr = Some(rr(2, 0.4));
    let back: Pad = toml::from_str(&toml::to_string(&q).unwrap()).unwrap();
    assert_eq!(back, q);
    // the default order is Random no repeat
    assert_eq!(PadRr::default().order, Order::RandomNoRepeat);
}

#[test]
fn each_order_rotates_the_sounds_hit_by_hit() {
    let mut r = rr(3, 0.0);
    r.order = Order::InOrder;
    assert_eq!(preview(&r, 0, 8), vec![0, 1, 2, 3, 0, 1, 2, 3]);
    r.order = Order::PingPong;
    assert_eq!(preview(&r, 0, 8), vec![0, 1, 2, 3, 2, 1, 0, 1]);
    r.order = Order::EveryN { n: 4, which: 2 };
    assert_eq!(preview(&r, 0, 8), vec![0, 0, 0, 2, 0, 0, 0, 2]);
    r.order = Order::RandomNoRepeat;
    let s = preview(&r, 0, 64);
    assert!(s.windows(2).all(|w| w[0] != w[1]), "{s:?}");
    for i in 0..4 {
        assert!(s.contains(&i));
    }
    r.order = Order::Random;
    let s = preview(&r, 0, 64);
    assert!(s.iter().all(|x| *x < 4));
    // pure: any window of hits is the same as the full run's
    assert_eq!(preview(&r, 20, 8), s[20..28].to_vec());
    // another seed, another run
    let mut r2 = r.clone();
    r2.seed ^= 1;
    assert_ne!(preview(&r2, 0, 64), s);
}

#[test]
fn variation_stays_inside_its_ranges_and_zero_means_none() {
    let r = rr(0, 0.0);
    for k in 0..32 {
        let m = hit_mod(&r, k);
        assert_eq!(
            (m.semis, m.gain, m.decay, m.delay, m.pan, m.tone),
            (0.0, 1.0, 1.0, 0.0, 0.0, 0.0)
        );
    }
    let r = rr(0, 0.5);
    let d = r.drift;
    let mut moved = 0;
    for k in 0..256 {
        let m = hit_mod(&r, k);
        assert!(m.semis.abs() <= d.pitch * 0.5 + 1e-6);
        assert!((20.0 * m.gain.log10()).abs() <= d.level * 0.5 + 1e-4);
        assert!(m.tone.abs() <= d.tone * 0.5 + 1e-6);
        assert!((m.decay - 1.0).abs() <= d.decay * 0.5 + 1e-6);
        assert!(
            m.delay >= 0.0 && m.delay <= d.timing * 0.5 / 1000.0 + 1e-6,
            "late only"
        );
        assert!(m.pan.abs() <= d.pan * 0.5 + 1e-6);
        moved += (m.semis != 0.0) as u32;
        assert_eq!(m, hit_mod(&r, k), "pure");
    }
    assert!(moved > 200);
}

#[test]
fn a_pad_with_one_sound_and_no_variation_renders_bit_identically() {
    let plain = kit(|_| {});
    let on = kit(|p| p.rr = Some(rr(0, 0.0)));
    for hit in [0, 1, 7] {
        assert_eq!(render(&on, hit), render(&plain, 0), "hit {hit}");
    }
}

#[test]
fn hits_play_their_sound_and_drift_the_same_every_time() {
    let g = kit(|p| {
        let mut r = rr(1, 0.0);
        r.order = Order::InOrder;
        p.rr = Some(r);
    });
    let plain = kit(|_| {});
    assert_eq!(
        render(&g, 0),
        render(&plain, 0),
        "hit 0: the pad's own sound"
    );
    assert_ne!(render(&g, 1), render(&plain, 0), "hit 1: the other sound");
    assert_eq!(render(&g, 2), render(&g, 0));
    // timing drift: the hit starts later, and silent until then
    let late = kit(|p| {
        let mut r = rr(0, 1.0);
        r.drift = Drift {
            pitch: 0.0,
            level: 0.0,
            tone: 0.0,
            decay: 0.0,
            timing: 20.0,
            pan: 0.0,
        };
        p.rr = Some(r);
    });
    let k = (0..16)
        .find(|k| hit_mod(late_rr(&late), *k).delay > 0.005)
        .unwrap();
    let o = render(&late, k);
    let wait = (hit_mod(late_rr(&late), k).delay * SR).ceil() as usize;
    assert!(o[..wait].iter().all(|(l, r)| *l == 0.0 && *r == 0.0));
    assert_eq!(
        o[wait..],
        render(&plain, 0)[..o.len() - wait],
        "the same hit, later"
    );
}

fn late_rr(g: &GenPatch) -> &PadRr {
    match &g.engine {
        GenEngine::Drums(k) => k.pads[SNARE].rr.as_ref().unwrap(),
        _ => unreachable!(),
    }
}

#[test]
fn timeline_hits_are_numbered_per_key_so_a_seek_plays_the_same_hit() {
    use crate::timeline::{self as tl, Note, ROW};
    let mut s = crate::fileio::demo_song();
    s.channels = 2;
    s.mixer.channels.resize(2, Default::default());
    tl::enter_timeline_mode(&mut s);
    let id = tl::new_note_clip(&mut s, 1, 0).unwrap();
    let t = s.timeline.as_deref_mut().unwrap();
    let c = t.clips.iter_mut().find(|c| c.id == id).unwrap();
    c.loop_len = Some(4 * ROW);
    c.len = 4 * 4 * ROW;
    let data = c.notes().unwrap();
    t.notes[data].make_mut().notes = [(0, 50), (ROW, 52), (2 * ROW, 50)]
        .iter()
        .map(|&(start, key)| Note {
            start,
            len: ROW,
            key,
            vel: 50000,
            ..Default::default()
        })
        .collect();
    let v = &tl::note_spans(s.timeline.as_deref().unwrap(), 2)[1];
    let hits = |key| {
        v.iter()
            .filter(|n| n.key == key)
            .map(|n| n.hit)
            .collect::<Vec<_>>()
    };
    assert_eq!(hits(50), (0..8).collect::<Vec<_>>());
    assert_eq!(hits(52), (0..4).collect::<Vec<_>>());
}

/// a timeline song: 8 snare hits through `g`, rendered to samples
fn kit_song(g: GenPatch) -> Vec<u32> {
    use crate::model::{Instrument, InstrumentKind};
    use crate::timeline::{self as tl, Note, ROW};
    let mut s = crate::fileio::demo_song();
    s.automation.clear();
    s.channels = 2;
    s.mixer.channels.resize(2, Default::default());
    let key = snare_key(&g);
    s.instruments.push(Instrument {
        arp: None,
        name: "Kit".into(),
        volume: 0.8,
        transpose: 0,
        fx: vec![],
        kind: InstrumentKind::Gen(g),
    });
    let slot = s.instruments.len() as u8;
    tl::enter_timeline_mode(&mut s);
    let id = tl::new_note_clip(&mut s, 1, 0).unwrap();
    let t = s.timeline.as_deref_mut().unwrap();
    let c = t.clips.iter_mut().find(|c| c.id == id).unwrap();
    c.len = 8 * ROW;
    let data = c.notes().unwrap();
    t.notes[data].make_mut().notes = (0..8)
        .map(|i| Note {
            start: i * ROW,
            len: ROW,
            key,
            vel: 50000,
            instr: slot,
            ..Default::default()
        })
        .collect();
    let o = crate::export::Opts {
        format: crate::export::Format::Wav,
        sample_rate: 48000,
        tail_secs: 0.0,
        normalise: false,
        limiter: false,
        ..Default::default()
    };
    let a = crate::export::render(&s, &o, &Default::default()).unwrap();
    assert!(a.iter().any(|x| x.abs() > 1e-3));
    a.iter().map(|x| x.to_bits()).collect()
}

#[test]
fn a_round_robin_kit_song_renders_the_same_twice_and_rotates() {
    let g = kit(|p| p.rr = Some(rr(2, 0.7)));
    assert_eq!(kit_song(g.clone()), kit_song(g));
    // the engine numbers the hits: with a second sound in order, the song
    // is not the plain one; with one sound and no variation it is
    let plain = kit_song(kit(|_| {}));
    let two = kit(|p| {
        let mut r = rr(1, 0.0);
        r.order = Order::InOrder;
        p.rr = Some(r);
    });
    assert_ne!(kit_song(two), plain);
    assert_eq!(kit_song(kit(|p| p.rr = Some(rr(0, 0.0)))), plain);
}
