//! ChipWave's own metronome symbol, painted (no image assets): a small
//! rounded pyramid with a pendulum, its sliding weight and an accent tip.
//! While the metronome plays, the arm swings with the transport's beat
//! position (at an end of its swing on every beat) and the downbeat
//! flashes the case; off, the arm rests centred and dimmed. During a
//! record count-in it swings with the count-in clicks and carries the beats
//! left as a small badge.
//! It is the toolbar Metronome toggle's icon; `metronome-mockup` is its
//! screenshot board.
use super::*;

/// what the symbol shows
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct MetroLook {
    /// the metronome is on (lit); off: dimmed, arm at rest
    pub on: bool,
    /// beats since the bar line (0.0 ..< per_bar) while the transport
    /// plays; None: stopped (arm at rest)
    pub beat: Option<f32>,
    /// beats per bar (the downbeat flashes)
    pub per_bar: u32,
    /// record count-in: beats left (shown as a badge)
    pub count: Option<u32>,
}

/// largest swing either side of centre (radians, about 23 degrees)
const SWING: f32 = 0.40;
/// how long (in beats) the downbeat flash and the beat pulse last
const FLASH: f32 = 0.22;

/// the arm's angle (0 up, + to the right): an end of its swing exactly
/// on every beat, alternating sides, so the click lands as it turns
pub(super) fn arm_angle(beat: f32) -> f32 {
    -SWING * (std::f32::consts::PI * beat).cos()
}

/// paint the symbol into `r` (designed at 18x18, scales with r's height)
pub(super) fn paint_metronome(p: &egui::Painter, r: Rect, look: MetroLook) {
    let t = th();
    let s = r.height() / 18.0;
    let c = r.center();
    let at = |x: f32, y: f32| c + egui::vec2(x * s, y * s);
    let live = look.on || look.count.is_some();
    let ink = if live {
        t.ink
    } else {
        t.ink2.gamma_multiply(0.75)
    };
    let accent = if live {
        t.accent
    } else {
        t.ink2.gamma_multiply(0.6)
    };
    let playing = look.beat.filter(|_| live);
    let (flash, pulse) = match playing {
        Some(b) => {
            let n = look.per_bar.max(1) as f32;
            let in_bar = b.rem_euclid(n);
            let frac = in_bar.fract();
            let k = (1.0 - frac / FLASH).max(0.0);
            (if in_bar < 1.0 { k } else { 0.0 }, k)
        }
        None => (0.0, 0.0),
    };
    // the case: a pyramid with chamfered (softened) corners, a thin
    // outline (1 px at 18 px) with its flat top and bottom on the pixel
    // grid so they stay crisp
    let case_w = 1.0 * s;
    let ppp = p.pixels_per_point();
    let grid = |y: f32| {
        let off = if (case_w * ppp).round() as i32 % 2 == 1 {
            0.5
        } else {
            0.0
        };
        ((y * ppp - off).round() + off) / ppp
    };
    let (top, bot) = (grid(c.y - 7.0 * s), grid(c.y + 7.0 * s));
    let ch = 0.6 * s; // the chamfer's height
    let xy = |x: f32, y: f32| Pos2::new(c.x + x * s, y);
    let case = vec![
        xy(-6.0, bot),
        xy(-6.6, bot - ch),
        xy(-2.9, top + ch),
        xy(-2.2, top),
        xy(2.2, top),
        xy(2.9, top + ch),
        xy(6.6, bot - ch),
        xy(6.0, bot),
    ];
    let fill = if flash > 0.0 {
        accent.gamma_multiply(0.25 + 0.55 * flash)
    } else {
        ink.gamma_multiply(if live { 0.14 } else { 0.08 })
    };
    p.add(egui::Shape::convex_polygon(
        case,
        fill,
        Stroke::new(case_w, ink),
    ));
    // the scale window behind the pendulum
    p.line_segment(
        [at(0.0, -4.6), at(0.0, 2.0)],
        Stroke::new(1.6 * s, ink.gamma_multiply(0.25)),
    );
    // the plinth
    // the plinth: as thin as the case, a pixel row below it
    let base = grid(bot + 2.0 * s);
    p.line_segment([xy(-7.4, base), xy(7.4, base)], Stroke::new(case_w, ink));
    // the pendulum from its pivot low in the case
    let a = playing.map(arm_angle).unwrap_or(0.0);
    let pivot = at(0.0, 4.6);
    let dir = egui::vec2(a.sin(), -a.cos());
    let along = |d: f32| pivot + dir * d * s;
    let tip = along(12.6);
    p.line_segment([pivot, tip], Stroke::new(1.2 * s, ink));
    // the sliding weight
    p.line_segment([along(6.0), along(8.2)], Stroke::new(3.0 * s, ink));
    // the accent tip: pulses on each beat
    p.circle_filled(tip, (1.5 + 0.8 * pulse) * s, accent);
    p.circle_filled(pivot, 1.1 * s, ink);
    // count-in: the beats left on a small badge, bottom right (clear of
    // the swinging arm)
    if let Some(n) = look.count {
        let bc = at(6.8, 3.6);
        p.circle_filled(bc, 4.9 * s, t.brick);
        p.text(
            bc + egui::vec2(0.0, 0.5 * s),
            egui::Align2::CENTER_CENTER,
            n.min(9).to_string(),
            egui::FontId::monospace(8.5 * s),
            Color32::WHITE,
        );
    }
}

/// `--view metronome-mockup`: the symbol's states at 1:1 in a toolbar
/// button, painted large, and a strip of animation frames (120 bpm)
pub(super) fn metronome_mockup(ctx: &egui::Context) {
    let t = th();
    egui::Area::new(egui::Id::unique("metronome_mockup"))
        .order(egui::Order::Foreground)
        .fixed_pos(Pos2::ZERO)
        .show(ctx, |ui| {
            let full = ctx.content_rect();
            ui.set_min_size(full.size());
            let p = ui.painter().clone();
            p.rect_filled(full, 0.0, t.bg_bot);
            let text = |pos: Pos2, s: &str, size: f32, col: Color32| {
                p.text(
                    pos,
                    egui::Align2::LEFT_TOP,
                    s,
                    egui::FontId::proportional(size),
                    col,
                );
            };
            let off = MetroLook {
                on: false,
                beat: None,
                per_bar: 4,
                count: None,
            };
            let on = MetroLook { on: true, ..off };
            let states: [(&str, MetroLook); 10] = [
                ("off", off),
                ("on, stopped", on),
                (
                    "beat 1 (flash)",
                    MetroLook {
                        beat: Some(0.0),
                        ..on
                    },
                ),
                (
                    "1 + 1/4",
                    MetroLook {
                        beat: Some(0.25),
                        ..on
                    },
                ),
                (
                    "1 + 1/2",
                    MetroLook {
                        beat: Some(0.5),
                        ..on
                    },
                ),
                (
                    "beat 2",
                    MetroLook {
                        beat: Some(1.0),
                        ..on
                    },
                ),
                (
                    "2 + 1/2",
                    MetroLook {
                        beat: Some(1.5),
                        ..on
                    },
                ),
                (
                    "beat 3",
                    MetroLook {
                        beat: Some(2.0),
                        ..on
                    },
                ),
                (
                    "count-in 3",
                    MetroLook {
                        beat: Some(1.05),
                        count: Some(3),
                        ..off
                    },
                ),
                (
                    "count-in 1",
                    MetroLook {
                        beat: Some(3.0),
                        count: Some(1),
                        ..off
                    },
                ),
            ];
            text(
                Pos2::new(24.0, 14.0),
                "METRONOME SYMBOL · ChipWave's own, painted · 1:1 in the toolbar button",
                15.0,
                t.ink,
            );
            // row A: 1:1, in a toolbar-style button (icon + label)
            let font = egui::TextStyle::Button.resolve(ui.style());
            let vis = ui.visuals().clone();
            for (i, (name, look)) in states.iter().enumerate() {
                let x = 24.0 + i as f32 * 132.0;
                let br = Rect::from_min_size(Pos2::new(x, 48.0), egui::vec2(124.0, 26.0));
                let wv = if look.on && look.beat.is_some() {
                    &vis.widgets.active
                } else {
                    &vis.widgets.inactive
                };
                p.rect(
                    br,
                    5,
                    wv.weak_bg_fill,
                    wv.bg_stroke,
                    egui::StrokeKind::Inside,
                );
                let ir = Rect::from_min_size(br.min + egui::vec2(8.0, 4.0), egui::vec2(18.0, 18.0));
                paint_metronome(&p, ir, *look);
                p.text(
                    Pos2::new(ir.right() + 6.0, br.center().y),
                    egui::Align2::LEFT_CENTER,
                    "Metronome",
                    font.clone(),
                    wv.text_color(),
                );
                text(Pos2::new(x, 80.0), name, 11.0, t.ink2);
            }
            // row B: painted at 4x (vector, the shapes)
            text(
                Pos2::new(24.0, 108.0),
                "painted at 4x (72 px)",
                12.0,
                t.ink2,
            );
            for (i, (_, look)) in states.iter().enumerate() {
                let x = 24.0 + i as f32 * 132.0;
                let r = Rect::from_min_size(Pos2::new(x + 22.0, 132.0), egui::vec2(72.0, 72.0));
                p.rect_filled(r.expand(6.0), 6, vis.widgets.inactive.weak_bg_fill);
                paint_metronome(&p, r, *look);
            }
            // row C: animation frames, one bar at 120 bpm, 25 fps (50
            // frames); 28x28 cells, 25 a row (cropped into the GIF)
            text(
                Pos2::new(24.0, 228.0),
                "frames: one 4/4 bar at 120 bpm, 25 fps (left to right, two rows)",
                12.0,
                t.ink2,
            );
            for f in 0..50 {
                let beat = f as f32 * (2.0 / 25.0); // 2 beats a second
                let cell = frame_cell(f);
                p.rect_filled(cell, 5, vis.widgets.inactive.weak_bg_fill);
                paint_metronome(
                    &p,
                    Rect::from_center_size(cell.center(), egui::vec2(18.0, 18.0)),
                    MetroLook {
                        beat: Some(beat),
                        ..on
                    },
                );
            }
        });
}

/// where animation frame `f` sits on the mockup board (for cropping)
pub(super) fn frame_cell(f: usize) -> Rect {
    Rect::from_min_size(
        Pos2::new(
            24.0 + (f % 25) as f32 * 32.0,
            250.0 + (f / 25) as f32 * 32.0,
        ),
        egui::vec2(28.0, 28.0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_arm_turns_on_every_beat_and_rests_centred_between() {
        // an end of the swing on each beat, alternating sides
        assert!((arm_angle(0.0) + SWING).abs() < 1e-6);
        assert!((arm_angle(1.0) - SWING).abs() < 1e-6);
        assert!((arm_angle(2.0) + SWING).abs() < 1e-6);
        // through the centre half way
        assert!(arm_angle(0.5).abs() < 1e-6);
        assert!(arm_angle(1.5).abs() < 1e-6);
    }
}
