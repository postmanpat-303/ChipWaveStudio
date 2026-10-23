//! The toolbar's LCD display and the compact Master (0.2.10).
//!
//! A segment display on dark glass: tempo (drag, double-click to type,
//! TAP), Speed (Tracker view), time signature, position, clock, the
//! LOOP / CLICK / REC / FOLLOW lamps and the engine load with the audio
//! buffer. Beside it a compact Master meter; a click opens the Master
//! panel (`ca_master`, unchanged) in a popover. Everything is painted with
//! egui shapes (7-segment digits of our own), no fonts or crates added.
//! The colour is a setting (presets or any colour); Settings → Look can
//! switch back to the classic toolbar with the Master under the Clip
//! Arranger.
use super::*;
use std::sync::atomic::Ordering;

/// the display height (the toolbar groups are 32)
const H: f32 = 42.0;

/// the colour presets: (name, colour); the first is the default
pub(super) const PRESETS: [(&str, u32); 5] = [
    ("Cyan", 0x7FE3FF),
    ("Amber", 0xFFA23A),
    ("Green", 0x8CFF7A),
    ("Red", 0xFF6450),
    ("Ice", 0xDDEEFF),
];

/// the colour for a saved setting: a preset name or "#RRGGBB"; anything
/// else (or "") is the default cyan
pub(super) fn colour_of(setting: &str) -> Color32 {
    let custom = setting
        .strip_prefix('#')
        .filter(|h| h.len() == 6)
        .and_then(|h| u32::from_str_radix(h, 16).ok())
        .map(hex);
    custom
        .or_else(|| {
            PRESETS
                .iter()
                .find(|p| p.0.eq_ignore_ascii_case(setting))
                .map(|p| hex(p.1))
        })
        .unwrap_or(hex(PRESETS[0].1))
}

/// "#RRGGBB" for a custom colour
pub(super) fn colour_setting(c: Color32) -> String {
    format!("#{:02X}{:02X}{:02X}", c.r(), c.g(), c.b())
}

/// tempo from tap times (seconds): the mean of the gaps, 30..300 BPM;
/// None until there are two taps
pub(super) fn tap_bpm(taps: &[f64]) -> Option<f32> {
    if taps.len() < 2 {
        return None;
    }
    let span = taps[taps.len() - 1] - taps[0];
    let gap = span / (taps.len() - 1) as f64;
    (gap > 0.0).then(|| ((60.0 / gap) as f32).clamp(30.0, 300.0))
}

/// taps further apart than this start a new count
const TAP_RESET: f64 = 2.0;

pub(super) struct LcdUi {
    /// the Master popover is open
    pub master_open: bool,
    /// recent TAP times (seconds)
    pub taps: Vec<f64>,
    /// typing a tempo / a speed (double-click)
    pub bpm_edit: Option<String>,
    pub spd_edit: Option<String>,
    /// the Master meter's peak hold per side: (level, since)
    hold: [(f32, f64); 2],
    /// last drawn: the display, the Master cell, the popover (tests)
    pub rect: Rect,
    pub master_rect: Rect,
    pub pop_rect: Rect,
}

impl Default for LcdUi {
    fn default() -> Self {
        Self {
            master_open: false,
            taps: vec![],
            bpm_edit: None,
            spd_edit: None,
            hold: [(0.0, 0.0); 2],
            rect: Rect::NOTHING,
            master_rect: Rect::NOTHING,
            pop_rect: Rect::NOTHING,
        }
    }
}

pub(super) struct Pal {
    bg_top: Color32,
    bg_bot: Color32,
    pub lit: Color32,
    hot: Color32,
    ghost: Color32,
    label: Color32,
    rec: Color32,
    warn: Color32,
}

fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

/// the display's palette from its colour: dark glass tinted by it, the
/// lit segments, a brighter "hot" (playing), faint ghost segments and
/// dimmer captions; REC and warnings keep red / yellow
pub(super) fn pal(c: Color32) -> Pal {
    let glass = hex(0x0A1116);
    Pal {
        bg_top: mix(glass, c, 0.07),
        bg_bot: mix(hex(0x04080A), c, 0.025),
        lit: c,
        hot: mix(c, Color32::WHITE, 0.6),
        ghost: Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 26),
        label: mix(glass, c, 0.66),
        rec: hex(0xFF5A4A),
        warn: hex(0xFFD04A),
    }
}

// ---- 7-segment digits ----
// segments a b c d e f g (bit 0 = a)
fn segs(c: char) -> u8 {
    match c {
        '0' => 0b0111111,
        '1' => 0b0000110,
        '2' => 0b1011011,
        '3' => 0b1001111,
        '4' => 0b1100110,
        '5' => 0b1101101,
        '6' => 0b1111101,
        '7' => 0b0000111,
        '8' => 0b1111111,
        '9' => 0b1101111,
        '-' => 0b1000000,
        _ => 0,
    }
}

fn seg_shapes(p: &egui::Painter, o: Pos2, h: f32, on: u8, col: Color32, ghost: Color32) {
    let w = h * 0.50;
    let t = (h * 0.15).max(2.0);
    let sl = h * 0.10; // italic slant
    let hh = h / 2.0;
    let pt = |x: f32, y: f32| Pos2::new(o.x + x + sl * (1.0 - y / h), o.y + y);
    // (horizontal?, x, y)
    let def: [(bool, f32, f32); 7] = [
        (true, 0.0, 0.0),
        (false, w, 0.0),
        (false, w, hh),
        (true, 0.0, h),
        (false, 0.0, hh),
        (false, 0.0, 0.0),
        (true, 0.0, hh),
    ];
    for (k, (hor, x, y)) in def.iter().enumerate() {
        let c = if on >> k & 1 == 1 { col } else { ghost };
        let g = t * 0.55;
        let pts = if *hor {
            let (x0, x1) = (x + g, x + w - g);
            vec![
                pt(x0, *y),
                pt(x0 + t / 2.0, y - t / 2.0),
                pt(x1 - t / 2.0, y - t / 2.0),
                pt(x1, *y),
                pt(x1 - t / 2.0, y + t / 2.0),
                pt(x0 + t / 2.0, y + t / 2.0),
            ]
        } else {
            let (y0, y1) = (y + g, y + hh - g);
            vec![
                pt(*x, y0),
                pt(x + t / 2.0, y0 + t / 2.0),
                pt(x + t / 2.0, y1 - t / 2.0),
                pt(*x, y1),
                pt(x - t / 2.0, y1 - t / 2.0),
                pt(x - t / 2.0, y0 + t / 2.0),
            ]
        };
        p.add(egui::Shape::convex_polygon(pts, c, Stroke::NONE));
    }
}

/// (digit width, gap between digits, '.' slot, ':' slot)
fn seg_metrics(h: f32) -> (f32, f32, f32, f32) {
    (h * 0.50, h * 0.36, h * 0.30, h * 0.40)
}

/// draw `s` in segments, digits `h` tall (space: an unlit digit); returns
/// the width
fn seg_text(p: &egui::Painter, at: Pos2, s: &str, h: f32, col: Color32, ghost: Color32) -> f32 {
    let mut x = at.x;
    let (w, gap, dot_slot, colon_slot) = seg_metrics(h);
    let t = (h * 0.15).max(2.0);
    let mut first = true;
    for c in s.chars() {
        match c {
            '.' => {
                let r = Rect::from_min_size(
                    Pos2::new(x + gap * 0.3, at.y + h - t * 0.6),
                    egui::vec2(t, t),
                );
                p.rect_filled(r, 0.8, col);
                x += dot_slot;
                continue;
            }
            ':' => {
                for fy in [0.30, 0.74] {
                    let sl = h * 0.10 * (1.0 - fy);
                    let r = Rect::from_min_size(
                        Pos2::new(x + gap * 0.45 + sl, at.y + h * fy - t * 0.5),
                        egui::vec2(t, t),
                    );
                    p.rect_filled(r, 0.8, col);
                }
                x += colon_slot;
                continue;
            }
            _ => {}
        }
        if !first {
            x += gap;
        }
        first = false;
        if c == '/' {
            p.line_segment(
                [
                    Pos2::new(x + w * 0.95, at.y + 1.0),
                    Pos2::new(x + w * 0.05, at.y + h - 1.0),
                ],
                Stroke::new(t * 0.9, col),
            );
        } else {
            seg_shapes(p, Pos2::new(x, at.y), h, segs(c), col, ghost);
        }
        x += w;
    }
    x - at.x
}

fn seg_w(s: &str, h: f32) -> f32 {
    let (w, gap, dot_slot, colon_slot) = seg_metrics(h);
    let mut x = 0.0;
    let mut first = true;
    for c in s.chars() {
        match c {
            '.' => x += dot_slot,
            ':' => x += colon_slot,
            _ => {
                if !first {
                    x += gap;
                }
                first = false;
                x += w;
            }
        }
    }
    x
}

const BIG: f32 = 18.0;
const MID: f32 = 13.0;

fn lab(p: &egui::Painter, at: Pos2, s: &str, col: Color32) -> f32 {
    let g = p.layout_no_wrap(s.to_string(), theme::label_font(9.5), col);
    let w = g.size().x;
    p.galley(at, g, col);
    w
}

/// one status lamp: a small LED and its caption
fn lamp(p: &egui::Painter, at: Pos2, s: &str, on: bool, col: Color32, pl: &Pal) -> f32 {
    let c = at + egui::vec2(3.0, 6.0);
    if on {
        p.circle_filled(c, 5.0, col.gamma_multiply(0.18));
        p.circle_filled(c, 2.6, col);
    } else {
        p.circle_filled(c, 2.6, pl.ghost.gamma_multiply(2.2));
    }
    let cap = if on {
        col
    } else {
        pl.label.gamma_multiply(0.7)
    };
    9.0 + lab(p, at + egui::vec2(9.0, 0.0), s, cap)
}

/// recessed dark glass: outer bevel, gradient, top sheen. The glass stays
/// dark in every theme; light themes (Amiga) get a light bevel so it sits
/// in the grey bar.
fn glass(p: &egui::Painter, r: Rect, pl: &Pal, hot: bool) {
    let light = th().light;
    let bevel = if light {
        Color32::from_white_alpha(170)
    } else {
        Color32::from_white_alpha(16)
    };
    p.rect_filled(r.expand(1.5), 7, bevel);
    p.rect_filled(
        r.translate(egui::vec2(0.0, -1.0)),
        6,
        Color32::from_black_alpha(140),
    );
    let mut mesh = egui::Mesh::default();
    theme::round_mesh(&mut mesh, r.shrink(1.0), [5.0; 4], pl.bg_top, pl.bg_bot);
    p.add(egui::Shape::mesh(mesh));
    let sheen = Rect::from_min_max(
        r.min + egui::vec2(2.0, 2.0),
        Pos2::new(r.right() - 2.0, r.top() + r.height() * 0.42),
    );
    let mut sm = egui::Mesh::default();
    theme::round_mesh(
        &mut sm,
        sheen,
        [4.0, 4.0, 0.0, 0.0],
        Color32::from_white_alpha(10),
        Color32::from_white_alpha(1),
    );
    p.add(egui::Shape::mesh(sm));
    p.rect_stroke(
        r.shrink(0.5),
        6,
        Stroke::new(1.0, Color32::from_black_alpha(200)),
        egui::StrokeKind::Inside,
    );
    if hot {
        p.rect_stroke(
            r.expand(0.5),
            6.5,
            Stroke::new(1.0, pl.lit.gamma_multiply(0.5)),
            egui::StrokeKind::Outside,
        );
    }
}

/// what the display shows this frame
struct LcdState {
    playing: bool,
    tl_mode: bool,
    bpm: f32,
    speed: u32,
    sig: (u8, u8),
    /// Timeline: bar, beat, step (16th in the beat); Tracker: pos, row, -
    pos: (i64, i64, i64),
    secs: f64,
    loop_on: bool,
    loop_bars: Option<(i64, i64)>,
    metro: bool,
    count_in_bars: u8,
    count_left: Option<u32>,
    rec: bool,
    armed: bool,
    follow: bool,
    /// (DSP %, buffer ms, underruns, buffer at its maximum); None: no audio
    load: Option<(f32, f32, u32, bool)>,
}

impl App {
    /// the LCD toolbar is on (Settings → Look)
    pub(super) fn lcd_on(&self) -> bool {
        self.extra.lcd_toolbar
    }

    /// the compact Master meter is in the toolbar: the LCD toolbar, or the
    /// classic one while the Clip Arranger (and its Master panel) is hidden
    pub(super) fn master_in_toolbar(&self) -> bool {
        self.lcd_on() || self.side_hidden()
    }

    pub(super) fn lcd_pal(&self) -> Pal {
        pal(colour_of(&self.extra.lcd_colour))
    }

    fn lcd_state(&mut self) -> LcdState {
        let playing = self.with(|s| s.engine.playing);
        let cue_tl = self.tlv.cue;
        let cue = self.cue;
        let st = self.with(|s| {
            let bpm = s.song.bpm;
            match s.song.timeline.as_deref() {
                Some(t) => {
                    let tick = match s.engine.tl_ticks().filter(|_| playing) {
                        Some(tk) => tk as i64,
                        None => cue_tl,
                    };
                    let sg = t.sig_at(tick);
                    let bp = t.bar_pos(tick);
                    let step_t = (sg.beat_ticks() / 4).max(1);
                    let lb = t
                        .loop_range
                        .map(|(a, b)| (t.bar_pos(a).bar, t.bar_pos(b - 1).bar));
                    (
                        true,
                        t.bpm_at(tick, bpm),
                        (sg.num, sg.den),
                        (bp.bar, bp.beat, bp.tick / step_t + 1),
                        t.seconds_at(tick, bpm),
                        t.loop_on,
                        lb,
                        t.tracks.iter().any(|x| x.armed),
                    )
                }
                None => {
                    let (o, r) = if playing {
                        (s.engine.order_pos, s.engine.row)
                    } else {
                        cue
                    };
                    let loop_on = playing && s.engine.loop_pattern;
                    (
                        false,
                        bpm,
                        (4, 4),
                        (o as i64, r as i64, 0),
                        0.0,
                        loop_on,
                        None,
                        false,
                    )
                }
            }
        });
        let (tl_mode, bpm, sig, pos, secs, loop_on, loop_bars, armed) = st;
        let speed = self.with(|s| s.song.speed);
        let load = match &self.audio {
            Some(a) => Some((
                a.load_pct(),
                a.latency_ms(),
                a.stats.underruns.load(Ordering::Relaxed),
                a.latency_maxed(),
            )),
            // screenshots run without a sound card: CHIPWAVE_LCD_LOAD=23,12,0
            // gives stand-in values (screenshot runs only)
            None if self.shot.is_some() => std::env::var("CHIPWAVE_LCD_LOAD").ok().and_then(|v| {
                let x: Vec<f32> = v.split(',').filter_map(|s| s.parse().ok()).collect();
                (x.len() == 3).then(|| (x[0], x[1], x[2] as u32, false))
            }),
            None => None,
        };
        LcdState {
            playing,
            tl_mode,
            bpm,
            speed,
            sig,
            pos,
            secs,
            loop_on,
            loop_bars,
            metro: self.extra.metronome,
            count_in_bars: self.extra.count_in_bars(),
            count_left: self.with(|s| s.engine.count_in_left()),
            rec: self.recording(),
            armed,
            follow: self.follow,
            load,
        }
    }

    /// the LCD display in the toolbar
    /// the Tracker view's toolbar has more in it (octave, edit mode, step):
    /// under 1560 px wide the display and the Master cell go compact
    fn lcd_narrow(&self, ctx: &egui::Context) -> bool {
        self.view == View::Pattern && ctx.content_rect().width() < 1560.0
    }

    pub(super) fn lcd_display(&mut self, ui: &mut egui::Ui) {
        let st = self.lcd_state();
        let pl = self.lcd_pal();
        let time = ui.input(|i| i.time);
        let blink = ((time * 4.0) as u64).is_multiple_of(2);
        if st.playing || st.count_left.is_some() {
            ui.ctx().request_repaint();
        }
        // the Speed cell shows in the Tracker view (ticks per row is a
        // tracker setting; the Song menu has it everywhere)
        let show_spd = self.view == View::Pattern;
        // the Tracker view has no clock (TIME), and its toolbar has more
        // (octave, edit mode, step): narrower than 1560 px it drops the
        // signature and draws the load cell without its bar
        let show_time = !show_spd;
        let narrow = self.lcd_narrow(ui.ctx());
        let show_sig = !narrow;
        let w_bpm = seg_w("000.00", BIG) + 4.0;
        let w_spd = if show_spd {
            seg_w("00", BIG) + 6.0
        } else {
            0.0
        };
        let w_sig = if show_sig {
            seg_w("00/0", BIG).max(30.0)
        } else {
            0.0
        };
        let w_pos = if st.tl_mode {
            seg_w("000.0.0", BIG)
        } else {
            seg_w("000.00", BIG)
        };
        let w_time = if show_time {
            seg_w("00:00.00", MID).max(64.0)
        } else {
            0.0
        };
        let lamp_col = if narrow { 70.0 } else { 84.0 };
        let w_state = lamp_col + 66.0;
        let w_load = if narrow { 66.0 } else { 96.0 };
        let gap = if narrow { 9.0 } else { 11.0 };
        let cells = 3.0
            + [show_spd, show_sig, show_time]
                .iter()
                .filter(|b| **b)
                .count() as f32;
        let w =
            10.0 + w_bpm + w_spd + w_sig + w_pos + w_time + w_state + w_load + gap * cells + 10.0;
        let id = ui.make_persistent_id("lcd_display");
        let (r, resp) = ui.allocate_exact_size(egui::vec2(w, H), Sense::click());
        self.lcd.rect = r;
        let p = ui.painter_at(r.expand(3.0));
        glass(&p, r, &pl, false);
        let top = r.top() + 4.0;
        let val_y = |h: f32| r.bottom() - 6.0 - h;
        let mut x = r.left() + 10.0;
        let div = |p: &egui::Painter, x: f32| {
            p.vline(
                x,
                (r.top() + 6.0)..=(r.bottom() - 6.0),
                Stroke::new(1.0, pl.label.gamma_multiply(0.3)),
            );
        };
        // 1. tempo: drag up / down, double-click to type, TAP
        let bpm_r = Rect::from_min_size(Pos2::new(x - 4.0, r.top()), egui::vec2(w_bpm + 8.0, H));
        lab(&p, Pos2::new(x, top), "BPM", pl.label);
        let tap_r = Rect::from_min_size(Pos2::new(x + w_bpm - 26.0, top), egui::vec2(26.0, 12.0));
        let tapping = self.lcd.taps.last().is_some_and(|t0| time - t0 < 0.15);
        p.rect_stroke(
            tap_r,
            2,
            Stroke::new(
                1.0,
                if tapping {
                    pl.hot
                } else {
                    pl.label.gamma_multiply(0.8)
                },
            ),
            egui::StrokeKind::Inside,
        );
        p.text(
            tap_r.center(),
            egui::Align2::CENTER_CENTER,
            "TAP",
            theme::label_font(8.0),
            if tapping { pl.hot } else { pl.label },
        );
        let bpm_val = Rect::from_min_max(
            Pos2::new(x, val_y(BIG) - 2.0),
            Pos2::new(x + w_bpm, r.bottom() - 4.0),
        );
        if self.lcd.bpm_edit.is_none() {
            let bs = format!("{:6.2}", st.bpm);
            let bw = seg_w(&bs, BIG);
            seg_text(
                &p,
                Pos2::new(x + w_bpm - 4.0 - bw, val_y(BIG)),
                &bs,
                BIG,
                pl.lit,
                pl.ghost,
            );
        }
        x += w_bpm + gap / 2.0;
        div(&p, x);
        x += gap / 2.0;
        // 1b. speed (Tracker view)
        let mut spd_r = Rect::NOTHING;
        let mut spd_val = Rect::NOTHING;
        if show_spd {
            spd_r = Rect::from_min_size(Pos2::new(x - 4.0, r.top()), egui::vec2(w_spd + 8.0, H));
            spd_val = Rect::from_min_max(
                Pos2::new(x, val_y(BIG) - 2.0),
                Pos2::new(x + w_spd, r.bottom() - 4.0),
            );
            lab(&p, Pos2::new(x, top), "SPD", pl.label);
            if self.lcd.spd_edit.is_none() {
                seg_text(
                    &p,
                    Pos2::new(x, val_y(BIG)),
                    &format!("{:>2}", st.speed),
                    BIG,
                    pl.lit,
                    pl.ghost,
                );
            }
            x += w_spd + gap / 2.0;
            div(&p, x);
            x += gap / 2.0;
        }
        // 2. time signature
        if show_sig {
            lab(&p, Pos2::new(x, top), "SIG", pl.label);
            let ss = format!("{}/{}", st.sig.0, st.sig.1);
            seg_text(&p, Pos2::new(x, val_y(BIG)), &ss, BIG, pl.lit, pl.ghost);
            x += w_sig + gap / 2.0;
            div(&p, x);
            x += gap / 2.0;
        }
        // 3. position (bar.beat.step; the Tracker: pos.row), or the count-in
        let pos_r = Rect::from_min_size(Pos2::new(x - 4.0, r.top()), egui::vec2(w_pos + 8.0, H));
        let pos_name = match st.count_left {
            Some(n) => format!("Count-in {n}"),
            None if st.tl_mode => format!("Position {}.{}.{}", st.pos.0, st.pos.1, st.pos.2),
            None => format!("Position {}.{:02}", st.pos.0, st.pos.1),
        };
        if let Some(n) = st.count_left {
            let cap = if blink {
                pl.rec
            } else {
                pl.rec.gamma_multiply(0.45)
            };
            lab(&p, Pos2::new(x, top), "COUNT-IN", cap);
            let s = format!("{n}");
            let cw = seg_w(&s, BIG);
            seg_text(
                &p,
                Pos2::new(x + (w_pos - cw) / 2.0, val_y(BIG)),
                &s,
                BIG,
                pl.rec,
                pl.ghost,
            );
            // the count-in's beats as pips
            let total = (st.count_in_bars.max(1) as u32) * st.sig.0.max(1) as u32;
            let done = total.saturating_sub(n);
            let shown = total.min(8);
            for k in 0..shown {
                let c = Pos2::new(x + w_pos - 4.0 - (shown - 1 - k) as f32 * 7.0, top + 5.0);
                let col = if k < done {
                    pl.rec
                } else {
                    pl.ghost.gamma_multiply(2.5)
                };
                p.circle_filled(c, 2.2, col);
            }
        } else {
            let (cap, s) = if st.tl_mode {
                (
                    "BAR · BEAT · STEP",
                    format!("{:>3}.{}.{}", st.pos.0, st.pos.1, st.pos.2),
                )
            } else {
                ("POS · ROW", format!("{:>3}.{:02}", st.pos.0, st.pos.1))
            };
            lab(&p, Pos2::new(x, top), cap, pl.label);
            let col = if st.rec {
                pl.rec
            } else if st.playing {
                pl.hot
            } else {
                pl.lit
            };
            seg_text(&p, Pos2::new(x, val_y(BIG)), &s, BIG, col, pl.ghost);
        }
        x += w_pos + gap / 2.0;
        div(&p, x);
        x += gap / 2.0;
        // 4. clock mm:ss.cs (the Timeline's time)
        if show_time {
            lab(&p, Pos2::new(x, top), "TIME", pl.label);
            let ts = if st.tl_mode {
                let cs = (st.secs * 100.0).floor() as u64;
                format!("{:02}:{:02}.{:02}", cs / 6000, cs / 100 % 60, cs % 100)
            } else {
                "--:--.--".to_string()
            };
            seg_text(
                &p,
                Pos2::new(x, val_y(MID) - 2.0),
                &ts,
                MID,
                pl.lit,
                pl.ghost,
            );
            x += w_time + gap / 2.0;
            div(&p, x);
            x += gap / 2.0;
        }
        // 5. lamps: LOOP (range), CLICK / count-in, REC / ARMED, FOLLOW
        let loop_txt = match st.loop_bars {
            Some((a, b)) => format!("LOOP {a}–{b}"),
            None => "LOOP".to_string(),
        };
        let y1 = top + 1.0;
        let y2 = r.bottom() - 17.0;
        let lr_loop = Rect::from_min_size(Pos2::new(x - 2.0, y1 - 3.0), egui::vec2(lamp_col, 17.0));
        lamp(&p, Pos2::new(x, y1), &loop_txt, st.loop_on, pl.lit, &pl);
        let click_txt = match st.count_in_bars {
            0 => "CLICK".to_string(),
            n => format!("CLICK +{n}"),
        };
        // the click lamp flashes on the beat (the transport's own beats)
        let on_beat = st.metro
            && st.playing
            && self.with(|s| {
                s.engine
                    .tl_ticks()
                    .zip(s.song.timeline.as_deref())
                    .is_some_and(|(tk, t)| {
                        let sg = t.sig_at(tk as i64);
                        let bp = t.bar_pos(tk as i64);
                        (bp.tick as f64 + tk.fract()) < sg.beat_ticks().max(1) as f64 * 0.25
                    })
            });
        let lr_click = Rect::from_min_size(
            Pos2::new(x + lamp_col - 2.0, y1 - 3.0),
            egui::vec2(68.0, 17.0),
        );
        lamp(
            &p,
            Pos2::new(x + lamp_col, y1),
            &click_txt,
            st.metro || st.count_left.is_some(),
            if on_beat { pl.hot } else { pl.lit },
            &pl,
        );
        let (rec_txt, rec_on) = if st.count_left.is_some() {
            ("REC", blink)
        } else if st.rec {
            ("REC", true)
        } else if st.armed {
            ("ARMED", true)
        } else {
            ("REC", false)
        };
        lamp(&p, Pos2::new(x, y2), rec_txt, rec_on, pl.rec, &pl);
        let lr_follow = Rect::from_min_size(
            Pos2::new(x + lamp_col - 2.0, y2 - 3.0),
            egui::vec2(68.0, 17.0),
        );
        lamp(
            &p,
            Pos2::new(x + lamp_col, y2),
            "FOLLOW",
            st.follow,
            pl.lit,
            &pl,
        );
        x += w_state + gap / 2.0;
        div(&p, x);
        x += gap / 2.0;
        // 6. engine load and the audio buffer / underruns
        lab(&p, Pos2::new(x, y1), "DSP", pl.label);
        let bar = Rect::from_min_size(
            Pos2::new(x + 24.0, y1 + 3.0),
            egui::vec2(if narrow { 0.0 } else { 36.0 }, 6.0),
        );
        let (dsp, buf, xr, maxed) = st.load.unwrap_or((0.0, 0.0, 0, false));
        let n = 9;
        let segw = bar.width() / n as f32;
        for k in 0..if narrow { 0 } else { n } {
            let on = st.load.is_some() && (k as f32 + 0.5) / n as f32 <= dsp / 100.0;
            let c = if !on {
                pl.ghost.gamma_multiply(2.5)
            } else if k >= 7 {
                pl.rec
            } else if k >= 5 {
                pl.warn
            } else {
                pl.lit
            };
            let sr = Rect::from_min_size(
                Pos2::new(bar.left() + k as f32 * segw, bar.top()),
                egui::vec2(segw - 1.0, bar.height()),
            );
            p.rect_filled(sr, 0.5, c);
        }
        let pct = if st.load.is_some() {
            format!("{dsp:.0}%")
        } else {
            "--".into()
        };
        p.text(
            Pos2::new(r.right() - 9.0, y1),
            egui::Align2::RIGHT_TOP,
            pct,
            theme::label_font(9.5),
            pl.lit,
        );
        let bt = match st.load {
            Some(_) if maxed => format!("BUF {buf:.0} MAX"),
            Some(_) if narrow => format!("BUF {buf:.0}"),
            Some(_) => format!("BUF {buf:.0} ms"),
            None => "NO AUDIO".into(),
        };
        lab(&p, Pos2::new(x, y2), &bt, pl.label);
        // the underrun light: dim; amber after an underrun
        let xc = Pos2::new(r.right() - 12.0, y2 + 6.0);
        if xr > 0 {
            p.circle_filled(xc, 5.0, pl.warn.gamma_multiply(0.2));
            p.circle_filled(xc, 2.6, pl.warn);
        } else {
            p.circle_filled(xc, 2.6, pl.ghost.gamma_multiply(2.2));
        }
        let lr_load = Rect::from_min_max(Pos2::new(x - 2.0, r.top()), r.right_bottom());

        // ---- interaction ----
        // the position (or the count-in) for screen readers
        ui.interact(pos_r, id.with("pos"), Sense::hover())
            .widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, &pos_name));
        // the display: right-click picks its colour
        let dresp = resp
            .on_hover_text("The display · right-click: its colour (Settings → Look has it too)");
        self.lcd_colour_popup(&dresp);
        // tempo: drag up / down, double-click to type; right-click: MIDI
        // learn / automation for the tempo (as the old BPM box)
        let bresp = ui
            .interact(bpm_r, id.with("bpm"), Sense::click_and_drag())
            .on_hover_text(
                "Tempo (BPM): drag up / down (Shift: fine), double-click to type, TAP to tap it in · right-click: MIDI learn",
            );
        bresp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, "Tempo"));
        if bresp.dragged() {
            let dy = -bresp.drag_delta().y;
            if dy != 0.0 {
                let fine = ui.input(|i| i.modifiers.shift);
                let nb = (st.bpm + dy * if fine { 0.05 } else { 0.5 }).clamp(30.0, 300.0);
                self.set_tempo((nb * 100.0).round() / 100.0, st.speed);
            }
        }
        if bresp.double_clicked() {
            self.lcd.bpm_edit = Some(format!("{:.2}", st.bpm));
        }
        self.learn_menu(&bresp, crate::automation::Target::Tempo);
        let tresp = ui
            .interact(tap_r.expand(1.0), id.with("tap"), Sense::click())
            .on_hover_text("TAP: click in time (four taps or more) to set the tempo");
        tresp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, "TAP"));
        if tresp.clicked() {
            self.lcd_tap(time);
        }
        // typing a tempo
        if let Some(mut s) = self.lcd.bpm_edit.take() {
            match lcd_edit(ui, bpm_val, id.with("bpm_edit"), &mut s, &pl) {
                Some(true) => {
                    if let Ok(v) = s.trim().replace(',', ".").parse::<f32>() {
                        self.set_tempo(v.clamp(30.0, 300.0), st.speed);
                    }
                }
                Some(false) => {}
                None => self.lcd.bpm_edit = Some(s),
            }
        }
        // speed: the same as the tempo
        if show_spd {
            let sresp = ui
                .interact(spd_r, id.with("spd"), Sense::click_and_drag())
                .on_hover_text("Speed: ticks per row (effect 09xx sets it in the Tracker) · drag up / down, double-click to type");
            sresp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, "Speed"));
            if sresp.dragged() {
                let acc = id.with("spd_acc");
                let mut a: f32 = ui.data(|d| d.get_temp(acc)).unwrap_or(0.0);
                a -= sresp.drag_delta().y;
                let steps = (a / 8.0).trunc();
                if steps != 0.0 {
                    a -= steps * 8.0;
                    let ns = (st.speed as i32 + steps as i32).clamp(1, 32) as u32;
                    self.set_tempo(st.bpm, ns);
                }
                ui.data_mut(|d| d.insert_temp(acc, a));
            }
            if sresp.double_clicked() {
                self.lcd.spd_edit = Some(st.speed.to_string());
            }
            if let Some(mut s) = self.lcd.spd_edit.take() {
                match lcd_edit(ui, spd_val, id.with("spd_edit"), &mut s, &pl) {
                    Some(true) => {
                        if let Ok(v) = s.trim().parse::<u32>() {
                            self.set_tempo(st.bpm, v.clamp(1, 32));
                        }
                    }
                    Some(false) => {}
                    None => self.lcd.spd_edit = Some(s),
                }
            }
        }
        // the lamps switch what they show
        let lamp_btn = |ui: &mut egui::Ui, r: Rect, salt: &str, name: &str, tip: &str| {
            let resp = ui
                .interact(r, id.with(salt), Sense::click())
                .on_hover_text(tip);
            let name = name.to_string();
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &name));
            resp
        };
        let lresp = if st.tl_mode {
            lamp_btn(ui, lr_loop, "loop", "LOOP", "Loop the loop region (L) · Shift-drag the ruler to set it · Ctrl+L: around the selection")
        } else {
            lamp_btn(
                ui,
                lr_loop,
                "loop",
                "LOOP",
                "Lit while Play block loops the current block (the Timeline has the loop region)",
            )
        };
        if lresp.clicked() && st.tl_mode {
            self.tl_apply(super::timeline::TlOp::ToggleLoop);
        }
        self.lcd_colour_popup(&lresp);
        let cresp = lamp_btn(
            ui,
            lr_click,
            "click",
            "CLICK",
            "Metronome: click to switch it · +1 / +2: the record count-in in bars (right-click to set it)",
        );
        if cresp.clicked() {
            self.extra.metronome = !self.extra.metronome;
            self.save_settings();
        }
        cresp.context_menu(|ui| self.count_in_menu(ui));
        let fresp = lamp_btn(
            ui,
            lr_follow,
            "follow",
            "FOLLOW",
            "Follow: the view pages along with the playhead (click to switch)",
        );
        if fresp.clicked() {
            self.follow = !self.follow;
        }
        self.lcd_colour_popup(&fresp);
        let audio_tip = match &self.audio {
            Some(a) => super::topbar::audio_tip(a),
            None => "No audio device (Settings → Audio)".to_string(),
        };
        let load_resp = ui
            .interact(lr_load, id.with("load"), Sense::click())
            .on_hover_text(format!(
                "DSP: engine load · BUF: latency queued ahead of the device; the light turns amber after an underrun{}\n\n{audio_tip}",
                if xr > 0 { format!(" ({xr} so far)") } else { String::new() }
            ));
        self.lcd_colour_popup(&load_resp);
    }

    /// a TAP at `time`: taps more than two seconds apart start again; the
    /// tempo follows from the last few
    pub(super) fn lcd_tap(&mut self, time: f64) {
        if self.lcd.taps.last().is_some_and(|t0| time - t0 > TAP_RESET) {
            self.lcd.taps.clear();
        }
        self.lcd.taps.push(time);
        if self.lcd.taps.len() > 6 {
            self.lcd.taps.remove(0);
        }
        if let Some(b) = tap_bpm(&self.lcd.taps) {
            let speed = self.with(|s| s.song.speed);
            self.set_tempo((b * 100.0).round() / 100.0, speed);
        }
    }

    /// the colour menu on a right-click; it stays open while picking (a
    /// click away closes it), so the custom colour's picker works in it
    fn lcd_colour_popup(&mut self, resp: &egui::Response) {
        egui::Popup::context_menu(resp)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| self.lcd_colour_menu(ui));
    }

    /// the display's colour: the presets, then any colour
    pub(super) fn lcd_colour_menu(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("Display colour").strong());
        if self.lcd_colour_pick(ui) {
            self.save_settings();
        }
    }

    /// the colour swatches and a custom picker (the menu and Settings);
    /// true when changed
    pub(super) fn lcd_colour_pick(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;
        let cur = colour_of(&self.extra.lcd_colour);
        ui.horizontal(|ui| {
            for (n, c) in PRESETS {
                let (r, resp) = ui.allocate_exact_size(egui::vec2(22.0, 22.0), Sense::click());
                ui.painter().circle_filled(r.center(), 9.5, hex(0x0A1116));
                ui.painter().circle_filled(r.center(), 6.5, hex(c));
                if cur == hex(c) {
                    ui.painter().circle_stroke(
                        r.center(),
                        11.0,
                        Stroke::new(2.0, ui.visuals().text_color()),
                    );
                }
                resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, n));
                if resp.on_hover_text(n).clicked() {
                    self.extra.lcd_colour = if n == PRESETS[0].0 {
                        String::new()
                    } else {
                        n.to_string()
                    };
                    changed = true;
                }
            }
            ui.add_space(4.0);
            let mut rgb = [cur.r(), cur.g(), cur.b()];
            if egui::color_picker::color_edit_button_srgb(ui, &mut rgb)
                .on_hover_text("any colour")
                .changed()
            {
                self.extra.lcd_colour = colour_setting(Color32::from_rgb(rgb[0], rgb[1], rgb[2]));
                changed = true;
            }
        });
        changed
    }
}

/// a small text field over a readout; Some(true) on Enter, Some(false) on
/// Esc or a click away, None while typing
fn lcd_edit(ui: &mut egui::Ui, r: Rect, id: egui::Id, s: &mut String, pl: &Pal) -> Option<bool> {
    let resp = ui.put(
        r,
        egui::TextEdit::singleline(s)
            .id(id)
            .font(egui::FontId::monospace(14.0))
            .text_color(pl.hot)
            .background_color(pl.bg_bot)
            .align(egui::Align2::RIGHT_CENTER),
    );
    if !resp.has_focus() && !resp.lost_focus() {
        resp.request_focus();
        // first frame: select it all so typing replaces it
        if let Some(mut st) = egui::TextEdit::load_state(ui.ctx(), id) {
            st.cursor.set_char_range(Some(egui::text::CCursorRange::two(
                egui::text::CCursor::new(0),
                egui::text::CCursor::new(s.chars().count()),
            )));
            st.store(ui.ctx(), id);
        }
        return None;
    }
    if resp.lost_focus() {
        let enter = ui.input(|i| i.key_pressed(Key::Enter));
        return Some(enter);
    }
    None
}

// ---- the compact Master meter and its popover ----
impl App {
    /// the Master in the toolbar: L / R level with peak hold, limiter
    /// reduction, the clip light and the volume; a click opens the Master
    /// panel; the wheel nudges the volume
    pub(super) fn lcd_master(&mut self, ui: &mut egui::Ui) {
        let (vol, lim, peaks, loud, playing) = self.with(|s| {
            (
                s.song.mixer.master,
                s.song.mixer.limiter,
                s.engine.peak_lr,
                s.engine.loud,
                s.engine.playing,
            )
        });
        let now = ui.input(|i| i.time);
        // peak hold: 1.5 s, then it falls
        for (h, &p) in self.lcd.hold.iter_mut().zip(peaks.iter()) {
            if p >= h.0 || now - h.1 > 1.5 {
                *h = (p, now);
            }
        }
        // the LEVEL history fills while playing, popover open or not
        if playing && self.extra.level_hist != 1 {
            let dt = ui.input(|i| i.stable_dt);
            self.ca.hist_step(dt, peaks[0].max(peaks[1]));
        }
        let hold = self.lcd.hold;
        let w = if self.lcd_narrow(ui.ctx()) {
            150.0
        } else {
            176.0
        };
        let (r, resp) = ui.allocate_exact_size(egui::vec2(w, H), Sense::click());
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, "Master meter"));
        self.lcd.master_rect = r;
        let mut open = self.lcd.master_open;
        let pl = self.lcd_pal();
        {
            let p = ui.painter();
            glass(p, r, &pl, resp.hovered() || open);
            let dim = pl.label;
            p.text(
                r.left_top() + egui::vec2(8.0, 4.0),
                egui::Align2::LEFT_TOP,
                "MASTER",
                theme::label_font(9.5),
                dim,
            );
            let db = if vol <= 0.0001 {
                "-inf".to_string()
            } else {
                format!("{:+.1}", 20.0 * vol.log10())
            };
            p.text(
                Pos2::new(r.right() - 22.0, r.top() + 4.0),
                egui::Align2::RIGHT_TOP,
                "dB",
                theme::label_font(9.5),
                dim,
            );
            if vol > 0.0001 {
                // (11 px segments are too small to read: a mono readout)
                p.text(
                    Pos2::new(r.right() - 40.0, r.top() + 3.0),
                    egui::Align2::RIGHT_TOP,
                    &db,
                    egui::FontId::monospace(11.0),
                    pl.lit,
                );
            } else {
                p.text(
                    Pos2::new(r.right() - 40.0, r.top() + 4.0),
                    egui::Align2::RIGHT_TOP,
                    "-inf",
                    theme::label_font(10.0),
                    pl.lit,
                );
            }
            // the caret: it opens
            let c = Pos2::new(r.right() - 10.0, r.top() + 9.5);
            let tri = if open {
                vec![
                    c + egui::vec2(-4.0, 2.0),
                    c + egui::vec2(4.0, 2.0),
                    c + egui::vec2(0.0, -3.0),
                ]
            } else {
                vec![
                    c + egui::vec2(-4.0, -2.0),
                    c + egui::vec2(4.0, -2.0),
                    c + egui::vec2(0.0, 3.0),
                ]
            };
            p.add(egui::Shape::convex_polygon(
                tri,
                if open { pl.lit } else { dim },
                Stroke::NONE,
            ));
            // L / R bars: dB scale -48..+3 in segments, the peak hold lit
            let frac = |v: f32| {
                if v <= 0.0 {
                    0.0
                } else {
                    ((20.0 * v.log10() + 48.0) / 51.0).clamp(0.0, 1.0)
                }
            };
            let bar_l = r.left() + 20.0;
            let bar_r = r.right() - 34.0;
            let n = 26;
            let sw = (bar_r - bar_l) / n as f32;
            for (k, (name, y)) in [("L", r.top() + 21.0), ("R", r.top() + 30.0)]
                .iter()
                .enumerate()
            {
                p.text(
                    Pos2::new(r.left() + 9.0, *y + 3.0),
                    egui::Align2::LEFT_CENTER,
                    *name,
                    theme::label_font(9.0),
                    dim,
                );
                let lit = (frac(peaks[k]) * n as f32).round() as usize;
                let hk = ((frac(hold[k].0) * n as f32).round() as usize).min(n);
                for s in 0..n {
                    let sr = Rect::from_min_size(
                        Pos2::new(bar_l + s as f32 * sw + 0.5, *y),
                        egui::vec2(sw - 1.0, 6.0),
                    );
                    let zone = if s >= n - 2 {
                        pl.rec
                    } else if s >= n - 6 {
                        pl.warn
                    } else {
                        pl.lit
                    };
                    let on = s < lit || (hk > 0 && s + 1 == hk);
                    p.rect_filled(
                        sr,
                        0.5,
                        if on {
                            zone
                        } else {
                            pl.ghost.gamma_multiply(2.0)
                        },
                    );
                }
            }
            // 0 dB tick
            let x0 = bar_l + (48.0 / 51.0) * (bar_r - bar_l);
            p.vline(
                x0,
                (r.top() + 18.5)..=(r.top() + 39.0),
                Stroke::new(1.0, dim.gamma_multiply(0.6)),
            );
            // limiter reduction: a short bar growing down
            let gx = r.right() - 27.0;
            let gr_r = Rect::from_min_max(
                Pos2::new(gx, r.top() + 20.0),
                Pos2::new(gx + 5.0, r.top() + 36.0),
            );
            p.rect_filled(gr_r, 1, pl.ghost.gamma_multiply(2.0));
            let f = (-loud.gain_red / 12.0).clamp(0.0, 1.0);
            if f > 0.0 && lim.enabled {
                p.rect_filled(
                    Rect::from_min_size(gr_r.min, egui::vec2(5.0, gr_r.height() * f)),
                    1,
                    pl.warn,
                );
            }
            p.text(
                Pos2::new(gx + 2.5, r.top() + 37.0),
                egui::Align2::CENTER_TOP,
                "GR",
                theme::label_font(7.0),
                dim,
            );
            // clip light: a true peak over 0 dBTP since Play
            let clip = loud.true_peak.is_finite() && loud.true_peak > 0.0;
            let cc = Pos2::new(r.right() - 11.0, r.top() + 27.0);
            if clip {
                p.circle_filled(cc, 6.0, pl.rec.gamma_multiply(0.25));
                p.circle_filled(cc, 3.2, pl.rec);
            } else {
                p.circle_filled(cc, 3.2, pl.ghost.gamma_multiply(2.4));
            }
            p.text(
                Pos2::new(cc.x, r.top() + 37.0),
                egui::Align2::CENTER_TOP,
                "CLIP",
                theme::label_font(7.0),
                dim,
            );
        }
        let resp = resp.on_hover_text(
            "Master: level L / R with peak hold, limiter reduction (GR) and the clip light · \
             click: volume, drive, ceiling, limiter, loudness and the level history · \
             scroll: volume ±0.5 dB",
        );
        // the wheel nudges the volume
        if resp.hovered() {
            let sc = ui.input(|i| i.smooth_scroll_delta.y);
            if sc.abs() > 0.5 {
                let db = if vol > 0.0001 {
                    20.0 * vol.log10()
                } else {
                    -60.0
                };
                let nd = (db + 0.5 * sc.signum()).clamp(-60.0, 6.0);
                self.with(|s| s.song.mixer.master = 10f32.powf(nd / 20.0));
                self.dirty = true;
            }
        }
        if resp.clicked() {
            open = !open;
        }
        self.lcd.pop_rect = Rect::NOTHING;
        // (the Master panel's rect: the popover's while open)
        self.ca.master = Rect::NOTHING;
        if open {
            let size = egui::vec2(322.0, 318.0);
            let screen = ui.ctx().content_rect();
            let x = (r.right() - size.x).clamp(4.0, (screen.right() - size.x - 4.0).max(4.0));
            let pos = Pos2::new(x, r.bottom() + 6.0);
            let id = ui.make_persistent_id("lcd_master_pop");
            let area = egui::Area::new(id)
                .order(egui::Order::Foreground)
                .fixed_pos(pos)
                .show(ui.ctx(), |ui| {
                    let (pr, _) = ui.allocate_exact_size(size, Sense::hover());
                    ui.painter().rect_filled(
                        pr.translate(egui::vec2(0.0, 4.0)).expand(2.0),
                        10,
                        Color32::from_black_alpha(90),
                    );
                    self.ca_master(ui, pr);
                    pr
                });
            let pop = area.inner;
            self.lcd.pop_rect = pop;
            // closes on Esc or a click outside (not while one of its own
            // menus is open: the level history's right-click menu)
            let menu_open = egui::Popup::is_any_open(ui.ctx());
            let esc =
                !menu_open && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Escape));
            let outside = !menu_open
                && ui.input(|i| {
                    i.pointer.any_pressed()
                        && i.pointer
                            .interact_pos()
                            .is_some_and(|q| !pop.contains(q) && !r.contains(q))
                });
            if esc || outside {
                open = false;
            }
        }
        self.lcd.master_open = open;
    }
}
