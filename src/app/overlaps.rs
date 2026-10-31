//! Developer check for screenshots (`CHIPWAVE_SHOT_OVERLAPS=<report.txt>`
//! with `--screenshot`): at the capture frame it walks every painted text
//! shape and lists the text that overlaps other text, is painted over by a
//! later filled widget, or is cut off sideways by its clip rect. Nothing
//! here runs outside screenshot mode.
use eframe::egui;
use egui::{Rect, Shape};
use std::sync::atomic::{AtomicBool, Ordering};

static ARMED: AtomicBool = AtomicBool::new(false);

/// a text row as painted: its visible rect, the full rect, the text
struct Row {
    vis: Rect,
    full: Rect,
    clip: Rect,
    text: String,
    idx: usize,
}

/// install the end-of-pass check once (when the env var is set)
pub fn install(ctx: &egui::Context) {
    let Ok(path) = std::env::var("CHIPWAVE_SHOT_OVERLAPS") else {
        return;
    };
    ctx.on_end_pass(
        "overlaps",
        std::sync::Arc::new(move |ui: &mut egui::Ui| {
            if ARMED.swap(false, Ordering::SeqCst) {
                let r = report(ui.ctx());
                let _ = std::fs::write(&path, r);
            }
        }),
    );
}

/// check this pass's shapes (call on the capture frame)
pub fn arm() {
    ARMED.store(true, Ordering::SeqCst);
}

fn rows_of(shapes: &[(Rect, Shape)]) -> (Vec<Row>, Vec<(usize, Rect)>) {
    let mut rows = vec![];
    let mut fills = vec![];
    fn walk(
        clip: Rect,
        s: &Shape,
        idx: usize,
        rows: &mut Vec<Row>,
        fills: &mut Vec<(usize, Rect)>,
    ) {
        match s {
            Shape::Vec(v) => v.iter().for_each(|s| walk(clip, s, idx, rows, fills)),
            Shape::Text(t) if t.angle == 0.0 && t.opacity_factor > 0.05 => {
                for row in &t.galley.rows {
                    let text: String = row.row.glyphs.iter().map(|g| g.chr).collect();
                    if text.trim().is_empty() {
                        continue;
                    }
                    // the inked part of the row (glyph boxes, not line gaps
                    // or leading space)
                    let ink = row.row.glyphs.iter().filter(|g| !g.chr.is_whitespace());
                    let (x0, x1) = ink.fold((f32::MAX, f32::MIN), |(a, b), g| {
                        (a.min(g.pos.x), b.max(g.pos.x + g.advance_width))
                    });
                    let o = t.pos + row.pos.to_vec2();
                    let h = row.row.size.y;
                    let full = Rect::from_min_max(
                        egui::pos2(o.x + x0, o.y + h * 0.15),
                        egui::pos2(o.x + x1, o.y + h * 0.85),
                    );
                    let vis = full.intersect(clip);
                    if vis.width() > 1.0 && vis.height() > 1.0 {
                        rows.push(Row {
                            vis,
                            full,
                            clip,
                            text,
                            idx,
                        });
                    }
                }
            }
            Shape::Rect(r) if r.fill.a() > 200 && r.rect.width() > 3.0 && r.rect.height() > 3.0 => {
                fills.push((idx, r.rect.intersect(clip)));
            }
            _ => {}
        }
    }
    for (k, (clip, s)) in shapes.iter().enumerate() {
        walk(*clip, s, k, &mut rows, &mut fills);
    }
    (rows, fills)
}

fn report(ctx: &egui::Context) -> String {
    let layers: Vec<egui::LayerId> = ctx.memory(|m| m.layer_ids().collect());
    let mut out = String::new();
    let mut n = 0;
    for l in layers {
        let shapes: Vec<(Rect, Shape)> = ctx.graphics(|g| {
            g.get(l)
                .map(|p| {
                    p.all_entries()
                        .map(|c| (c.clip_rect, c.shape.clone()))
                        .collect()
                })
                .unwrap_or_default()
        });
        let (rows, fills) = rows_of(&shapes);
        let lname = l.short_debug_format();
        let at = |r: Rect| {
            format!(
                "@{:.0},{:.0} {:.0}x{:.0}",
                r.left(),
                r.top(),
                r.width(),
                r.height()
            )
        };
        for (i, a) in rows.iter().enumerate() {
            // cut off sideways by its clip rect (a hidden overflow)
            let cut =
                (a.full.left() < a.clip.left() - 2.0) || (a.full.right() > a.clip.right() + 2.0);
            if cut && a.vis.width() > 4.0 {
                n += 1;
                out += &format!("CLIPPED [{lname}] {:?} {}\n", a.text, at(a.full));
            }
            for b in &rows[i + 1..] {
                // the same text a few px off is a glow or shadow, not a clash
                // (an LCD glow is the same text at another size: compare centres)
                let d = a.full.center() - b.full.center();
                if a.text == b.text && d.x.abs() <= 4.0 && d.y.abs() <= 4.0 {
                    continue;
                }
                let x = a.vis.intersect(b.vis);
                if x.width() > 2.0 && x.height() > 2.0 {
                    n += 1;
                    out += &format!(
                        "TEXT/TEXT [{lname}] {:?} x {:?} {}\n",
                        a.text,
                        b.text,
                        at(x)
                    );
                }
            }
            // a later filled rect painted over this text
            if let Some((_, f)) = fills.iter().find(|(k, f)| {
                *k > a.idx && {
                    let x = a.vis.intersect(*f);
                    x.width() > 3.0 && x.height() > 3.0 && !f.contains_rect(a.vis)
                }
            }) {
                n += 1;
                out += &format!(
                    "COVERED [{lname}] {:?} under {} by {}\n",
                    a.text,
                    at(a.vis),
                    at(*f)
                );
            }
        }
    }
    format!("overlaps: {n}\n{out}")
}
