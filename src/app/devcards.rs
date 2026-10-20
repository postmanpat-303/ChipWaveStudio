//! The compact Devices cards (cleanup D, picked by Del (postmanpat)): every
//! device card has the same width, one graph and 2-4 key knobs; the rest of
//! its parameters sit under "…" (per card, remembered while ChipWave runs).
//! This module holds the per-kind choices (key knobs, graph) and the small
//! painters and the parameter row the cards use; the row itself, drag and
//! drop, bypass and remove stay in devices.rs. Original ChipWave code.
use super::*;
use crate::fx::{self, Device, FxKind};

/// width of every device card (and the instrument card)
pub(super) const CARD_W: f32 = 256.0;
/// extra width of a card with its "…" open
pub(super) const MORE_W: f32 = 214.0;
/// space between cards (the signal chevron sits in it)
pub(super) const GAP: f32 = 12.0;
/// the narrowest a card gets when many share the row (three knobs)
pub(super) const CARD_MIN_W: f32 = 196.0;
/// height of a parameter row under "…"
pub(super) const ROW_H: f32 = 22.0;

/// the parameter indices shown as knobs on a card (2-4); choices and the
/// rest go under "…". Plugins show their thumbnail instead.
pub(crate) fn key_params(d: &Device) -> Vec<usize> {
    use FxKind as K;
    let keys: &[&str] = match d.kind {
        K::Eq => &["lo_gain", "m1_gain", "hi_gain"],
        K::Filter => &["cutoff", "resonance", "mix"],
        K::Chorus => &["rate", "depth", "mix"],
        K::Compressor => &["threshold", "ratio", "makeup"],
        K::Reverb => &["size", "damping", "mix"],
        // a tempo-synced delay ignores its time knob
        K::Delay if d.real(0) as usize == 0 => &["time", "feedback", "mix"],
        K::Delay => &["feedback", "lp", "mix"],
        K::Drive => &["drive", "tone", "mix"],
        K::Bitcrusher => &["bits", "rate", "mix"],
        K::Resonator => &["decay", "damp", "mix"],
        K::Digital => &["drive", "bits", "mix"],
        K::Sub => &["drive", "growl", "mix"],
        K::Phaser if d.real(1) as usize == 0 => &["rate", "depth", "mix"],
        K::Phaser => &["depth", "feedback", "mix"],
        K::FreqShift => &["coarse", "feedback", "mix"],
        K::Grid => &["m1", "m2", "m3", "m4"],
        K::Plugin => &[],
    };
    let info = fx::params(d.kind);
    keys.iter()
        .filter_map(|k| info.iter().position(|p| p.key == *k))
        .filter(|&i| info[i].choices.is_empty())
        .collect()
}

/// the parameters under "…": everything that isn't a key knob, in order
pub(crate) fn more_params(d: &Device) -> Vec<usize> {
    let keys = key_params(d);
    (0..fx::params(d.kind).len())
        .filter(|i| !keys.contains(i))
        .collect()
}

/// the graph's background: a dark well with a faint grid
pub(super) fn well(p: &egui::Painter, r: Rect) {
    let t = th();
    if t.ember {
        crate::ember::well(p, r, 4);
        let g = Stroke::new(1.0, crate::ember::GRID);
        for k in 1..4 {
            p.vline(
                r.left() + r.width() * k as f32 / 4.0,
                r.y_range().shrink(1.0),
                g,
            );
        }
        return;
    }
    p.rect_filled(
        r,
        4,
        Color32::from_black_alpha(if t.light { 22 } else { 70 }),
    );
    let g = Stroke::new(1.0, t.ink2.gamma_multiply(0.14));
    for k in 1..4 {
        p.vline(r.left() + r.width() * k as f32 / 4.0, r.y_range(), g);
    }
}

fn line(p: &egui::Painter, pts: Vec<Pos2>, col: Color32) {
    let t = th();
    if t.ember && col == t.accent {
        crate::ember::glow_line(p, &pts, 1.8, t.acc_hot);
        return;
    }
    p.add(egui::Shape::line(pts, Stroke::new(1.8, col)));
}

/// `f(u)` for u in 0..1 across the graph, -1..1 up the middle
fn wave(r: Rect, f: impl Fn(f32) -> f32) -> Vec<Pos2> {
    (0..=96)
        .map(|k| {
            let u = k as f32 / 96.0;
            Pos2::new(
                r.left() + 4.0 + u * (r.width() - 8.0),
                r.center().y - f(u).clamp(-1.0, 1.0) * (r.height() * 0.5 - 6.0),
            )
        })
        .collect()
}

/// chorus: the two delay lines' LFOs, a little apart (rate and depth)
pub(super) fn chorus_graph(p: &egui::Painter, r: Rect, d: &Device) {
    let t = th();
    well(p, r);
    let (rate, depth) = (d.real(0), d.real(1));
    let cycles = (rate.clamp(0.05, 10.0) * 2.0).clamp(0.6, 8.0);
    let amp = (depth / fx::params(d.kind)[1].max).clamp(0.15, 1.0);
    let tau = std::f32::consts::TAU;
    line(
        p,
        wave(r, |u| (u * cycles * tau + 1.6).sin() * amp),
        t.vol.gamma_multiply(0.75),
    );
    line(p, wave(r, |u| (u * cycles * tau).sin() * amp), t.accent);
}

/// drive: the waveshaper's transfer curve (input across, output up)
pub(super) fn drive_graph(p: &egui::Painter, r: Rect, d: &Device) {
    let t = th();
    well(p, r);
    let info = &fx::params(d.kind)[1];
    let k = 1.0 + 9.0 * info.to_norm(d.real(1));
    p.line_segment(
        [r.left_bottom(), r.right_top()],
        Stroke::new(1.0, t.ink2.gamma_multiply(0.3)),
    );
    line(
        p,
        wave(r, |u| ((u * 2.0 - 1.0) * k).tanh() / k.tanh()),
        t.accent,
    );
}

/// bit crusher (and Digital): a sine through the bit depth's steps
pub(super) fn steps_graph(p: &egui::Painter, r: Rect, bits: f32) {
    let t = th();
    well(p, r);
    let levels = 2f32.powf(bits.clamp(1.0, 16.0)).min(64.0) * 0.5;
    let tau = std::f32::consts::TAU;
    line(
        p,
        wave(r, |u| (u * tau).sin() * 0.85),
        t.ink2.gamma_multiply(0.35),
    );
    line(
        p,
        wave(r, |u| ((u * tau).sin() * 0.85 * levels).round() / levels),
        t.accent,
    );
}

/// phaser: the notches over frequency, centred where the sweep sits
pub(super) fn phaser_graph(p: &egui::Painter, r: Rect, d: &Device) {
    let t = th();
    well(p, r);
    let info = fx::params(d.kind);
    let notches = [2.0, 3.0, 4.0, 6.0][(d.real(0) as usize).min(3)];
    let c = info[4].to_norm(d.real(4));
    let depth = d.real(3).clamp(0.0, 1.0);
    line(
        p,
        wave(r, |u| {
            let x = (u - c * 0.6) * notches * std::f32::consts::PI;
            0.6 - depth * 1.3 * x.cos().powi(8)
        }),
        t.accent,
    );
}

/// frequency shifter: a spectrum line and the same line moved by the shift
pub(super) fn shift_graph(p: &egui::Painter, r: Rect, d: &Device) {
    let t = th();
    well(p, r);
    let info = &fx::params(d.kind)[0];
    let sh = (info.to_norm(d.real(0)) - 0.5) * 0.5;
    let peaks = |u: f32| -0.7 + 1.3 * (-(((u * 4.0).fract() - 0.5) * 9.0).powi(2)).exp();
    line(p, wave(r, peaks), t.ink2.gamma_multiply(0.35));
    line(p, wave(r, |u| peaks(u - sh)), t.accent);
}

/// resonator: decaying ringing partials
pub(super) fn ring_graph(p: &egui::Painter, r: Rect, d: &Device) {
    let t = th();
    well(p, r);
    let info = &fx::params(d.kind)[5];
    let dec = 0.15 + 0.85 * info.to_norm(d.real(5));
    let tau = std::f32::consts::TAU;
    line(
        p,
        wave(r, |u| (u * 9.0 * tau).sin() * (-u / dec * 2.2).exp()),
        t.accent,
    );
}

/// sub: the input and the sub octave under it
pub(super) fn sub_graph(p: &egui::Painter, r: Rect) {
    let t = th();
    well(p, r);
    let tau = std::f32::consts::TAU;
    line(
        p,
        wave(r, |u| (u * 4.0 * tau).sin() * 0.45),
        t.ink2.gamma_multiply(0.4),
    );
    line(p, wave(r, |u| (u * 2.0 * tau).sin() * 0.85), t.accent);
}

/// a compressor's gain-reduction meter (dB, 0..24) at the graph's right
pub(super) fn gr_meter(p: &egui::Painter, r: Rect, gr: f32) {
    let t = th();
    let m = Rect::from_min_max(
        Pos2::new(r.right() - 8.0, r.top() + 2.0),
        Pos2::new(r.right() - 2.0, r.bottom() - 2.0),
    );
    p.rect_filled(m, 2, Color32::from_black_alpha(110));
    let f = (gr / 24.0).clamp(0.0, 1.0);
    if f > 0.0 {
        p.rect_filled(
            Rect::from_min_size(m.min, egui::vec2(m.width(), m.height() * f)),
            2,
            t.play,
        );
    }
}

/// a grid of dots for a Grid patch (its module count)
pub(super) fn grid_graph(p: &egui::Painter, r: Rect, modules: usize) {
    let t = th();
    well(p, r);
    let n = modules.clamp(1, 12);
    let cols = n.min(4);
    for k in 0..n {
        let (cx, cy) = ((k % cols) as f32, (k / cols) as f32);
        let c = Pos2::new(
            r.left() + r.width() * (cx + 0.5) / cols as f32,
            r.top() + 14.0 + cy * 20.0,
        );
        if c.y < r.bottom() - 6.0 {
            p.rect_filled(
                Rect::from_center_size(c, egui::vec2(18.0, 10.0)),
                2,
                t.accent.gamma_multiply(0.5),
            );
        }
    }
}

/// one parameter under "…": name, a bar to drag, the value. Returns the new
/// normalised value and whether this starts an undo step.
pub(super) fn param_row(
    ui: &mut egui::Ui,
    id: egui::Id,
    name: &str,
    norm: f32,
    text: &str,
    def: f32,
) -> (egui::Response, Option<(f32, bool)>) {
    let t = th();
    let w = ui.available_width();
    let (r, resp) = ui.allocate_exact_size(egui::vec2(w, ROW_H), Sense::hover());
    let bar = Rect::from_min_max(
        Pos2::new(r.left() + 80.0, r.center().y - 3.0),
        Pos2::new(r.right() - 58.0, r.center().y + 3.0),
    );
    let hit = ui
        .interact(
            bar.expand2(egui::vec2(4.0, 7.0)),
            id,
            Sense::click_and_drag(),
        )
        .on_hover_cursor(egui::CursorIcon::ResizeHorizontal)
        .on_hover_text(format!(
            "{name}: {text}\ndrag (Ctrl: fine) · double-click: default · right-click: automate"
        ));
    let p = ui.painter();
    p.with_clip_rect(Rect::from_min_max(
        r.min,
        Pos2::new(bar.left() - 6.0, r.bottom()),
    ))
    .text(
        r.left_center() + egui::vec2(2.0, 0.0),
        egui::Align2::LEFT_CENTER,
        name,
        FontId::proportional(11.5),
        t.ink,
    );
    p.rect_filled(
        bar,
        3,
        Color32::from_black_alpha(if t.light { 40 } else { 110 }),
    );
    let f = norm.clamp(0.0, 1.0);
    p.rect_filled(
        Rect::from_min_size(bar.min, egui::vec2(bar.width() * f, bar.height())),
        3,
        if hit.hovered() || hit.dragged() {
            t.accent
        } else {
            t.accent.gamma_multiply(0.8)
        },
    );
    p.text(
        r.right_center() - egui::vec2(2.0, 0.0),
        egui::Align2::RIGHT_CENTER,
        text,
        FontId::monospace(11.0),
        t.value,
    );
    let mut out = None;
    if hit.double_clicked() {
        out = Some((def, true));
    } else if hit.dragged() {
        let fine = ui.input(|i| i.modifiers.ctrl || i.modifiers.command);
        let dx = hit.drag_delta().x / bar.width().max(1.0) * if fine { 0.1 } else { 1.0 };
        if dx != 0.0 {
            out = Some(((f + dx).clamp(0.0, 1.0), hit.drag_started()));
        }
    } else if hit.clicked() {
        if let Some(q) = hit.interact_pointer_pos() {
            out = Some((((q.x - bar.left()) / bar.width()).clamp(0.0, 1.0), true));
        }
    }
    let _ = resp;
    (hit, out)
}

/// how many knobs fit side by side in `w`
pub(super) fn knobs_fit(w: f32) -> usize {
    ((w / super::knob::CELL.x).floor() as usize).max(2)
}

/// UI-only identity of device `i` for remembering its "…": a plugin's uid,
/// else its kind and which of that kind it is (built-ins keep uid 0 so the
/// song file doesn't change)
pub(super) fn open_key(chain: &[Device], i: usize) -> u64 {
    let d = &chain[i];
    if d.uid != 0 {
        return 1 << 63 | d.uid as u64;
    }
    let kind = FxKind::ALL.iter().position(|k| *k == d.kind).unwrap_or(0) as u64;
    let nth = chain[..i]
        .iter()
        .filter(|e| e.kind == d.kind && e.uid == 0)
        .count() as u64;
    kind << 16 | nth
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_has_two_to_four_key_knobs_and_the_rest_is_under_more() {
        for k in FxKind::ALL {
            let d = Device::new(k);
            let keys = key_params(&d);
            assert!((2..=4).contains(&keys.len()), "{k:?}: {keys:?}");
            let more = more_params(&d);
            assert_eq!(keys.len() + more.len(), fx::params(k).len(), "{k:?}");
            assert!(
                keys.iter().all(|i| fx::params(k)[*i].choices.is_empty()),
                "{k:?}"
            );
        }
        assert!(key_params(&Device::new(FxKind::Plugin)).is_empty());
    }
}
