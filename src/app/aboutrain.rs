//! Help > About opens with digital rain: columns of falling glyphs cover the
//! window, and as each column's head falls past it reveals the page beneath,
//! cell by cell; then the rain fades out (about 1.6 s in all). Painter only:
//! the columns and glyph galleys are worked out once per opening, so a frame
//! allocates nothing of its own. Any click, key or scroll skips it, and
//! Settings › Look › Reduce motion turns it off.
use eframe::egui::{self, Color32, Rect};
use std::sync::Arc;

/// one rain cell (points)
pub(super) const CELL_W: f32 = 14.0;
pub(super) const CELL_H: f32 = 16.0;
/// glyph size: decoration only, the page underneath keeps its own sizes
const GLYPH_PX: f32 = 13.0;
/// every revealing head is past the bottom edge by now (s)
pub(super) const REVEAL_END: f32 = 1.15;
/// the rain has faded out and the page is fully readable (s)
pub(super) const TOTAL: f32 = 1.6;
/// glyphs in a falling trail, head included
const TRAIL: i32 = 12;
/// rows behind a head that dissolve in, pixel by pixel
const BAND: i32 = 3;
/// glyphs the bundled mono font has (no katakana there): digits, a few
/// letters, maths and music signs
const GLYPHS: [&str; 34] = [
    "0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "Z", "X", "C", "V", "W", "A", "E", "H", "I",
    "P", "<", ">", "=", "*", "+", ":", "Δ", "Ω", "π", "♪", "◊", "¦", "†", "‡",
];
/// the dark "screen" over cells not yet revealed
const COVER: Color32 = Color32::from_rgb(2, 9, 5);
/// classic digital-rain green, and the near-white head
const GREEN: Color32 = Color32::from_rgb(40, 255, 110);
const HEAD: Color32 = Color32::from_rgb(215, 255, 225);

/// a falling drop: starts `delay` s in, crosses its distance in `fall` s
#[derive(Clone, Copy)]
struct Fall {
    delay: f32,
    fall: f32,
}

impl Fall {
    /// how far down (0 = top, 1 = done) it is at `t`; < 0 before it starts
    fn at(self, t: f32) -> f32 {
        (t - self.delay) / self.fall
    }
}

#[derive(Clone, Copy)]
struct Col {
    /// the drop whose head reveals the page
    reveal: Fall,
    /// a fainter, quicker drop for depth (reveals nothing)
    ghost: Fall,
    seed: u32,
}

pub(super) struct Rain {
    /// context time of the first frame it was painted (None until then)
    start: Option<f64>,
    seed: u32,
    cols: Vec<Col>,
    glyphs: Vec<Arc<egui::Galley>>,
}

/// small integer hash (for glyph picks and the dissolve), no state
fn hash(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^ (x >> 16)
}

/// 0..1 from a hash
fn unit(x: u32) -> f32 {
    (hash(x) & 0xffff) as f32 / 65535.0
}

fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let k = ((x - a) / (b - a)).clamp(0.0, 1.0);
    k * k * (3.0 - 2.0 * k)
}

impl Rain {
    pub(super) fn new(seed: u32) -> Self {
        Self {
            start: None,
            seed: hash(seed | 1),
            cols: Vec::new(),
            glyphs: Vec::new(),
        }
    }

    /// columns for a window `w` points wide, worked out once
    fn ensure_cols(&mut self, w: f32) {
        let n = (w / CELL_W).ceil().max(1.0) as usize;
        if self.cols.len() >= n {
            return;
        }
        let s = self.seed;
        for i in self.cols.len()..n {
            let k = s.wrapping_add((i as u32).wrapping_mul(0x9e37_79b9));
            let delay = 0.35 * unit(k ^ 0x11);
            // finishes by REVEAL_END at the latest
            let fall = 0.55 + (REVEAL_END - 0.35 - 0.55) * unit(k ^ 0x22);
            let gdelay = 0.05 + 0.55 * unit(k ^ 0x33);
            let gfall = 0.35 + 0.25 * unit(k ^ 0x44);
            self.cols.push(Col {
                reveal: Fall { delay, fall },
                ghost: Fall {
                    delay: gdelay,
                    fall: gfall,
                },
                seed: hash(k),
            });
        }
    }

    fn ensure_glyphs(&mut self, p: &egui::Painter) {
        if self.glyphs.is_empty() {
            let font = crate::theme::mono(GLYPH_PX);
            self.glyphs = GLYPHS
                .iter()
                .map(|g| p.layout_no_wrap((*g).to_string(), font.clone(), Color32::PLACEHOLDER))
                .collect();
        }
    }

    /// Paint the rain over the window `rect` on its layer. `now` = context
    /// time. Returns false once it has finished.
    pub(super) fn paint(
        &mut self,
        ctx: &egui::Context,
        layer: egui::LayerId,
        rect: Rect,
        now: f64,
    ) -> bool {
        let t = (now - *self.start.get_or_insert(now)) as f32;
        if t >= TOTAL || rect.width() < 1.0 || rect.height() < 1.0 {
            return false;
        }
        let p = ctx.layer_painter(layer).with_clip_rect(rect);
        self.ensure_cols(rect.width());
        self.ensure_glyphs(&p);
        let rows = (rect.height() / CELL_H).ceil() as i32;
        let fade = 1.0 - smooth(REVEAL_END, TOTAL, t);
        // the glyphs change now and then (about 14 times a second)
        let tick = (t * 14.0) as u32;
        let ng = self.glyphs.len() as u32;
        let glyph = |p: &egui::Painter, x: f32, row: i32, pick: u32, col: Color32| {
            let g = &self.glyphs[(hash(pick) % ng) as usize];
            let y = rect.top() + row as f32 * CELL_H;
            let pos = egui::pos2(
                (x + (CELL_W - g.size().x) * 0.5).round(),
                (y + (CELL_H - g.size().y) * 0.5).round(),
            );
            p.galley(pos, g.clone(), col);
        };
        // distance a revealing head travels: from just above the window to
        // past its bottom with the dissolve band
        let span = (rows + BAND + 1) as f32;
        let gspan = (rows + TRAIL) as f32;
        for (i, c) in self.cols.iter().enumerate() {
            let x = rect.left() + i as f32 * CELL_W;
            if x >= rect.right() {
                break;
            }
            let cs = c.seed;
            // where the revealing head is, in rows (may be < 0 or > rows)
            let head = (c.reveal.at(t) * span).floor() as i32 - 1;
            if t < REVEAL_END && head < rows + BAND {
                // not yet revealed: everything from the head down
                let top = rect.top() + head.max(0) as f32 * CELL_H;
                if top < rect.bottom() {
                    p.rect_filled(
                        Rect::from_min_max(
                            egui::pos2(x, top),
                            egui::pos2(x + CELL_W, rect.bottom()),
                        ),
                        0.0,
                        COVER,
                    );
                    // a faint screen of still glyphs on the dark part
                    for row in (head + 1).max(0)..rows {
                        let h = hash(cs ^ (row as u32).wrapping_mul(0x2c1b_3c6d));
                        if h.is_multiple_of(6) {
                            glyph(&p, x, row, h ^ (tick / 3), GREEN.gamma_multiply(0.16));
                        }
                    }
                }
                // the dissolve band just behind the head: cells fade in
                // unevenly, so the page appears pixel by pixel
                for k in 1..=BAND {
                    let row = head - k;
                    if row < 0 || row >= rows {
                        continue;
                    }
                    let jit = unit(cs ^ (row as u32).wrapping_mul(0x5bd1_e995)) * 0.5 - 0.25;
                    let a = (1.0 - k as f32 / (BAND + 1) as f32 + jit).clamp(0.0, 1.0);
                    let y = rect.top() + row as f32 * CELL_H;
                    p.rect_filled(
                        Rect::from_min_size(egui::pos2(x, y), egui::vec2(CELL_W, CELL_H)),
                        0.0,
                        COVER.gamma_multiply(a),
                    );
                }
            }
            // the falling trails: the revealing drop, then a fainter ghost
            for (head, len, bright) in [
                (head, TRAIL, 1.0f32),
                ((c.ghost.at(t) * gspan).floor() as i32 - 1, TRAIL - 4, 0.5),
            ] {
                for k in 0..len {
                    let row = head - k;
                    if row < 0 || row >= rows {
                        continue;
                    }
                    let a = (1.0 - k as f32 / len as f32).powf(1.4) * bright * fade;
                    if a <= 0.01 {
                        continue;
                    }
                    let col = if k == 0 { HEAD } else { GREEN };
                    // the head flickers every tick, the trail now and then
                    let flick = if k == 0 { tick } else { tick / 4 };
                    let pick = cs ^ (row as u32).wrapping_mul(0x27d4_eb2f) ^ flick;
                    glyph(&p, x, row, pick, col.gamma_multiply(a));
                }
            }
        }
        ctx.request_repaint();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_column_reveals_before_the_fade() {
        let mut r = Rain::new(7);
        r.ensure_cols(700.0);
        assert_eq!(r.cols.len(), 50);
        for c in &r.cols {
            assert!(c.reveal.delay + c.reveal.fall <= REVEAL_END + 1e-4);
            assert!(c.reveal.at(REVEAL_END) >= 1.0 - 1e-4);
            assert!(c.ghost.delay + c.ghost.fall <= TOTAL);
        }
        assert!(REVEAL_END < TOTAL && (1.2..=1.8).contains(&TOTAL));
    }
}
