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

// (the display height is `lcd_h`)

/// the backlight presets: (name, glass colour); the first is the default.
/// (Until 0.2.12 these were digit colours on dark glass; the same names
/// are now tints of the light glass, so saved settings keep working.)
pub(super) const PRESETS: [(&str, u32); 6] = [
    ("Mint", 0xC9EDE0),
    ("Cyan", 0xC4E9F3),
    ("Amber", 0xF6DDB2),
    ("Green", 0xD3EFC2),
    ("Red", 0xF6D0C8),
    ("Ice", 0xE3EAF3),
];

/// the colour for a saved setting: a preset name or "#RRGGBB"; anything
/// else (or "") is the default mint
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
    /// screenshots only: the LOOP and REC pills show lit (CHIPWAVE_SHOT_LCD=on)
    pub shot_lit: bool,
    /// the display and the Master cell are drawn compact (the top bar
    /// sets it each frame when the full ones would not fit)
    pub narrow: bool,
    /// the in-between display (1366 px windows): the signature, the loop's
    /// bars and the buffer's ms stay; tighter cells, no load meters, and
    /// the signature beside TIME's caption (the top bar sets it each frame)
    pub mid: bool,
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
            shot_lit: false,
            narrow: false,
            mid: false,
        }
    }
}

pub(super) struct Pal {
    /// the backlit glass, top and bottom of its gradient
    pub bg_top: Color32,
    pub bg_bot: Color32,
    /// the digits: near-black, tinted by the backlight
    pub lit: Color32,
    /// the digits while typing / tapping (as dark)
    hot: Color32,
    /// captions above the values
    pub label: Color32,
    /// the thin lines between the cells
    div: Color32,
    /// an off pill: its text and outline
    pub off_fg: Color32,
    off_bd: Color32,
    /// meters: an unlit segment and a lit one
    track: Color32,
    meter: Color32,
    /// REC (the red pill, the meters' top) and its text; warnings (amber)
    pub rec: Color32,
    rec_fg: Color32,
    warn: Color32,
    /// the glass's frame in this theme
    pub edge: Color32,
}

fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

/// WCAG relative luminance
pub(super) fn luminance(c: Color32) -> f32 {
    let f = |v: u8| {
        let v = v as f32 / 255.0;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * f(c.r()) + 0.7152 * f(c.g()) + 0.0722 * f(c.b())
}

/// WCAG contrast ratio (1..21)
pub(super) fn contrast(a: Color32, b: Color32) -> f32 {
    let (x, y) = (luminance(a), luminance(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

/// the display's palette from its backlight colour: light glass (any
/// colour is lightened until it is a backlight), near-black digits and
/// darker-tinted captions, each kept at 4.5:1 or more against the glass's
/// darker edge; REC is a red pill, warnings amber
pub(super) fn pal(c: Color32) -> Pal {
    let mut top = c;
    for _ in 0..24 {
        if luminance(top) >= 0.62 {
            break;
        }
        top = mix(top, Color32::WHITE, 0.12);
    }
    let bot = mix(top, Color32::BLACK, 0.12);
    let ink = |t: f32| mix(top, Color32::BLACK, t);
    // darker until readable (4.5:1 on the glass's darker end)
    let readable = |mut x: Color32| {
        for _ in 0..20 {
            if contrast(x, bot) >= 4.5 {
                break;
            }
            x = mix(x, Color32::BLACK, 0.12);
        }
        x
    };
    let edge = if th().light {
        hex(0x3E4652)
    } else {
        hex(0x08090A)
    };
    Pal {
        bg_top: top,
        bg_bot: bot,
        lit: readable(ink(0.9)),
        hot: readable(ink(0.9)),
        label: readable(ink(0.66)),
        div: ink(0.3),
        off_fg: readable(ink(0.62)),
        off_bd: ink(0.45),
        track: ink(0.24),
        meter: readable(ink(0.64)),
        rec: hex(0xB3261E),
        rec_fg: Color32::WHITE,
        warn: readable(hex(0x9A5B00)),
        edge,
    }
}

/// the captions (11 pt, the 10 pt floor and the Text size via `px`)
const CAP: f32 = crate::theme::ts::SMALL;
/// BPM and the position; SIG / SPD / TIME
const BIG: f32 = 21.0;
const MID: f32 = 15.0;

/// the display's inner margin left and right (full / compact; in-between)
const PAD: f32 = 10.0;
const PAD_MID: f32 = 7.0;

/// the display's height: a caption row and a big value row (follows the
/// Text size; 42 at 100 %)
pub(super) fn lcd_h() -> f32 {
    let px = crate::theme::ts::px;
    (px(CAP) * 1.25 + px(BIG) * 1.0 + 7.0).max(42.0).round()
}

/// the compact Master cell's width (full / compact)
pub(super) fn master_w(narrow: bool) -> f32 {
    if narrow {
        150.0
    } else {
        176.0
    }
}

/// the width of `s` in the display's digits (Atkinson Hyperlegible Mono)
fn num_w(p: &egui::Painter, s: &str, size: f32) -> f32 {
    p.layout_no_wrap(s.to_string(), theme::num_font(size), Color32::WHITE)
        .size()
        .x
}

/// a value: its baseline sits `bottom` (the galley's foot), from `x`
/// (`right`: right-aligned there); returns its width
fn num(
    p: &egui::Painter,
    x: f32,
    bottom: f32,
    s: &str,
    size: f32,
    col: Color32,
    right: bool,
) -> f32 {
    let g = p.layout_no_wrap(s.to_string(), theme::num_font(size), col);
    let sz = g.size();
    // the font's descent is empty for digits: sit them on the cell's foot
    let at = Pos2::new(
        if right { x - sz.x } else { x },
        bottom - sz.y + crate::theme::ts::px(size) * 0.16,
    );
    p.galley(at.round(), g, col);
    sz.x
}

fn lab(p: &egui::Painter, at: Pos2, s: &str, col: Color32) -> f32 {
    let g = p.layout_no_wrap(s.to_string(), theme::label_font(CAP), col);
    let w = g.size().x;
    p.galley(at, g, col);
    w
}

fn lab_w(p: &egui::Painter, s: &str) -> f32 {
    p.layout_no_wrap(s.to_string(), theme::label_font(CAP), Color32::WHITE)
        .size()
        .x
}

/// one toggle pill: filled dark with a light dot when on (REC: red),
/// an outline with a hollow dot when off
fn pill(p: &egui::Painter, r: Rect, s: &str, on: bool, red: bool, flash: bool, pl: &Pal) {
    let rad = r.height() / 2.0;
    let dot = Pos2::new(r.left() + rad + 1.0, r.center().y);
    let fg = if on {
        let (bg, fg) = if red {
            (pl.rec, pl.rec_fg)
        } else if flash {
            (pl.meter, pl.bg_top)
        } else {
            (pl.lit, pl.bg_top)
        };
        p.rect_filled(r, rad, bg);
        p.circle_filled(dot, 2.6, fg);
        fg
    } else {
        p.rect_stroke(
            r,
            rad,
            Stroke::new(1.0, pl.off_bd),
            egui::StrokeKind::Inside,
        );
        p.circle_stroke(dot, 2.3, Stroke::new(1.0, pl.off_bd));
        pl.off_fg
    };
    p.text(
        Pos2::new(dot.x + 6.0, r.center().y),
        egui::Align2::LEFT_CENTER,
        s,
        theme::label_font(CAP),
        fg,
    );
}

/// a rounded rect coloured by any function of the position (a fan mesh
/// like `theme::round_mesh`, for the backlight glow)
fn fan(p: &egui::Painter, r: Rect, rad: f32, f: impl Fn(Pos2) -> Color32) {
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(r.center(), f(r.center()));
    let rad = rad.clamp(0.0, r.width().min(r.height()) * 0.5);
    let corners = [
        (Pos2::new(r.left() + rad, r.top() + rad), 180.0f32),
        (Pos2::new(r.right() - rad, r.top() + rad), 270.0),
        (Pos2::new(r.right() - rad, r.bottom() - rad), 0.0),
        (Pos2::new(r.left() + rad, r.bottom() - rad), 90.0),
    ];
    let mut n = 0u32;
    for (o, a0) in corners {
        for i in 0..=4 {
            let a = (a0 + 22.5 * i as f32).to_radians();
            let q = o + egui::vec2(a.cos(), a.sin()) * rad;
            mesh.colored_vertex(q, f(q));
            n += 1;
        }
    }
    for i in 0..n {
        mesh.add_triangle(0, 1 + i, 1 + (i + 1) % n);
    }
    p.add(egui::Shape::mesh(mesh));
}

/// the backlight's highlight colour at `a` (0..255): the glass's own tint,
/// lightened, so the glow and shine follow the chosen colour
pub(super) fn glow_col(pl: &Pal, a: f32) -> Color32 {
    let c = mix(pl.bg_top, Color32::WHITE, 0.7);
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a.clamp(0.0, 255.0) as u8)
}

/// backlit glass: a dark frame (the theme's edge), the light gradient, a
/// soft backlight glow in the middle, an inner shadow along the top, a
/// faint inner highlight and a glassy shine across the top
fn glass(p: &egui::Painter, r: Rect, pl: &Pal, hot: bool) {
    if th().light {
        p.rect_filled(r.expand(1.5), 7, Color32::from_white_alpha(150));
    }
    p.rect_filled(r, 6, pl.edge);
    let inner = r.shrink(2.0);
    let mut mesh = egui::Mesh::default();
    theme::round_mesh(&mut mesh, inner, [4.5; 4], pl.bg_top, pl.bg_bot);
    p.add(egui::Shape::mesh(mesh));
    // the backlight glow: brightest in the middle, fading to the edges
    let (c, hw, hh) = (inner.center(), inner.width() * 0.5, inner.height() * 0.5);
    fan(p, inner, 4.5, |q| {
        let (dx, dy) = ((q.x - c.x) / hw, (q.y - c.y) / hh);
        glow_col(
            pl,
            (1.0 - (dx * dx * 0.9 + dy * dy * 0.3).sqrt()).max(0.0) * 170.0,
        )
    });
    let shade = Rect::from_min_max(inner.min, Pos2::new(inner.right(), inner.top() + 5.0));
    let mut sm = egui::Mesh::default();
    theme::round_mesh(
        &mut sm,
        shade,
        [4.5, 4.5, 0.0, 0.0],
        Color32::from_rgba_unmultiplied(0, 40, 30, 60),
        Color32::from_rgba_unmultiplied(0, 40, 30, 0),
    );
    p.add(egui::Shape::mesh(sm));
    p.rect_stroke(
        inner,
        4.5,
        Stroke::new(1.0, Color32::from_white_alpha(70)),
        egui::StrokeKind::Inside,
    );
    // the shine: a glassy strip over the top 42 %
    let strip = Rect::from_min_max(
        inner.min + egui::vec2(1.0, 1.0),
        Pos2::new(inner.right() - 1.0, inner.top() + inner.height() * 0.42),
    );
    let mut sm = egui::Mesh::default();
    let (a, b) = (glow_col(pl, 150.0), glow_col(pl, 25.0));
    theme::round_mesh(&mut sm, strip, [3.5, 3.5, 0.0, 0.0], a, b);
    p.add(egui::Shape::mesh(sm));
    if hot {
        p.rect_stroke(
            r.expand(0.5),
            6.5,
            Stroke::new(1.5, pl.meter),
            egui::StrokeKind::Outside,
        );
    }
}

/// what the display shows this frame
/// the display's cells and widths (`lcd_cols`)
struct LcdCols {
    show_spd: bool,
    show_time: bool,
    show_sig: bool,
    /// the signature on TIME's caption row (the in-between display)
    sig_in_time: bool,
    w_bpm: f32,
    w_spd: f32,
    w_sig: f32,
    w_pos: f32,
    w_time: f32,
    ph: f32,
    col1: f32,
    col2: f32,
    w_state: f32,
    gap: f32,
    w: f32,
}

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
            loop_on: loop_on || self.lcd.shot_lit,
            loop_bars,
            metro: self.extra.metronome,
            count_in_bars: self.extra.count_in_bars(),
            count_left: self.with(|s| s.engine.count_in_left()),
            rec: self.recording() || self.lcd.shot_lit,
            armed,
            follow: self.follow,
            load,
        }
    }

    /// the display and the Master cell go compact when the full ones do not
    /// fit beside the centred top-bar panel (the top bar decides, each frame)
    fn lcd_narrow(&self, _ctx: &egui::Context) -> bool {
        self.lcd.narrow
    }

    /// the display's width, full, in-between (`mid`) or compact (the top
    /// bar lays out with it)
    pub(super) fn lcd_display_w(&self, p: &egui::Painter, narrow: bool, mid: bool) -> f32 {
        let tl_mode = self.with(|s| s.song.timeline.is_some());
        self.lcd_cols(p, narrow, mid && !narrow, tl_mode).w
    }

    /// the display's cells and their widths
    fn lcd_cols(&self, mp: &egui::Painter, narrow: bool, mid: bool, tl_mode: bool) -> LcdCols {
        // the Speed cell shows in the Tracker view (ticks per row is a
        // tracker setting; the Song menu has it everywhere)
        let show_spd = self.view == View::Pattern;
        // the Tracker view has no clock (TIME); compact (`lcd_narrow`) drops
        // the signature and draws the load cell without its bar
        let show_time = !show_spd;
        // in-between: the signature rides on TIME's caption row (its own
        // cell when there is no TIME)
        let sig_in_time = mid && show_time;
        let show_sig = !narrow && !sig_in_time;
        let h = lcd_h();
        let w_bpm = num_w(mp, "000.00", BIG) + 4.0;
        let w_spd = if show_spd {
            num_w(mp, "00", BIG) + 6.0
        } else {
            0.0
        };
        let w_sig = if show_sig {
            num_w(mp, "00/0", MID).max(lab_w(mp, "SIG") + 4.0)
        } else {
            0.0
        };
        let w_pos = if tl_mode {
            num_w(mp, "000.0.0", BIG).max(lab_w(mp, "BAR · BEAT · STEP"))
        } else {
            num_w(mp, "000.00", BIG).max(lab_w(mp, "POS · ROW"))
        };
        let w_time = if show_time {
            num_w(mp, "00:00.00", MID)
        } else {
            0.0
        };
        // the pills: two columns (LOOP / REC, CLICK / FOLLOW)
        let ph = ((h - 11.0) / 2.0).floor();
        let pill_w = |s: &str| lab_w(mp, s) + ph + 8.0;
        let col1 = pill_w(if narrow { "LOOP" } else { "LOOP 00–00" }).max(pill_w("ARMED"));
        let col2 = pill_w("CLICK +0").max(pill_w("FOLLOW"));
        let w_state = col1 + 4.0 + col2;
        let w_load = if mid {
            lab_w(mp, "BUF 000 ms") + 4.0
        } else if narrow {
            lab_w(mp, "BUF 000 MAX") + 4.0
        } else {
            lab_w(mp, "BUF") + 34.0 + lab_w(mp, "000 ms") + 10.0
        };
        // in-between: tighter cells and edges, so 1366 px windows keep it
        let gap = if mid {
            8.0
        } else if narrow {
            9.0
        } else {
            13.0
        };
        let pad = if mid { PAD_MID } else { PAD };
        let cells = 3.0
            + [show_spd, show_sig, show_time]
                .iter()
                .filter(|b| **b)
                .count() as f32;
        let w = pad + w_bpm + w_spd + w_sig + w_pos + w_time + w_state + w_load + gap * cells + pad;
        LcdCols {
            show_spd,
            show_time,
            show_sig,
            sig_in_time,
            w_bpm,
            w_spd,
            w_sig,
            w_pos,
            w_time,
            ph,
            col1,
            col2,
            w_state,
            gap,
            w,
        }
    }

    pub(super) fn lcd_display(&mut self, ui: &mut egui::Ui) {
        let st = self.lcd_state();
        let pl = self.lcd_pal();
        let time = ui.input(|i| i.time);
        let blink = ((time * 4.0) as u64).is_multiple_of(2);
        if st.playing || st.count_left.is_some() {
            ui.ctx().request_repaint();
        }
        let narrow = self.lcd_narrow(ui.ctx());
        let mid = self.lcd.mid && !narrow;
        let h = lcd_h();
        let mp = ui.painter().clone();
        let LcdCols {
            show_spd,
            show_time,
            show_sig,
            sig_in_time,
            w_bpm,
            w_spd,
            w_sig,
            w_pos,
            w_time,
            ph,
            col1,
            col2,
            w_state,
            gap,
            w,
        } = self.lcd_cols(&mp, narrow, mid, st.tl_mode);
        let id = ui.make_persistent_id("lcd_display");
        let (r, resp) = ui.allocate_exact_size(egui::vec2(w, h), Sense::click());
        self.lcd.rect = r;
        let p = ui.painter_at(r.expand(3.0));
        glass(&p, r, &pl, false);
        let top = r.top() + 4.0;
        // the values sit on the display's foot
        let foot = r.bottom() - 4.0;
        let mut x = r.left() + if mid { PAD_MID } else { PAD };
        let div = |p: &egui::Painter, x: f32| {
            p.vline(
                x,
                (r.top() + 7.0)..=(r.bottom() - 7.0),
                Stroke::new(1.0, pl.div),
            );
        };
        let val = |x0: f32, x1: f32, size: f32| {
            Rect::from_min_max(
                Pos2::new(x0, foot - crate::theme::ts::px(size) * 0.9),
                Pos2::new(x1, foot),
            )
        };
        // 1. tempo: drag up / down, double-click to type, TAP
        let bpm_r = Rect::from_min_size(Pos2::new(x - 4.0, r.top()), egui::vec2(w_bpm + 8.0, h));
        lab(&p, Pos2::new(x, top), "BPM", pl.label);
        let tw = lab_w(&mp, "TAP") + 10.0;
        let tap_r = Rect::from_min_size(
            Pos2::new(x + w_bpm - tw, top),
            egui::vec2(tw, crate::theme::ts::px(CAP) + 2.0),
        );
        let tapping = self.lcd.taps.last().is_some_and(|t0| time - t0 < 0.15);
        if tapping {
            p.rect_filled(tap_r, tap_r.height() / 2.0, pl.lit);
        } else {
            p.rect_stroke(
                tap_r,
                tap_r.height() / 2.0,
                Stroke::new(1.0, pl.off_bd),
                egui::StrokeKind::Inside,
            );
        }
        p.text(
            tap_r.center(),
            egui::Align2::CENTER_CENTER,
            "TAP",
            theme::label_font(CAP),
            if tapping { pl.bg_top } else { pl.off_fg },
        );
        let bpm_val = val(x, x + w_bpm, BIG);
        if self.lcd.bpm_edit.is_none() {
            let bs = format!("{:6.2}", st.bpm);
            num(&p, x + w_bpm - 2.0, foot, bs.trim(), BIG, pl.lit, true);
        }
        x += w_bpm + gap / 2.0;
        div(&p, x);
        x += gap / 2.0;
        // 1b. speed (Tracker view)
        let mut spd_r = Rect::NOTHING;
        let mut spd_val = Rect::NOTHING;
        if show_spd {
            spd_r = Rect::from_min_size(Pos2::new(x - 4.0, r.top()), egui::vec2(w_spd + 8.0, h));
            spd_val = val(x, x + w_spd, BIG);
            lab(&p, Pos2::new(x, top), "SPD", pl.label);
            if self.lcd.spd_edit.is_none() {
                num(
                    &p,
                    x + w_spd - 2.0,
                    foot,
                    &st.speed.to_string(),
                    BIG,
                    pl.lit,
                    true,
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
            num(&p, x, foot - 1.0, &ss, MID, pl.lit, false);
            x += w_sig + gap / 2.0;
            div(&p, x);
            x += gap / 2.0;
        }
        // 3. position (bar.beat.step; the Tracker: pos.row), or the count-in
        let pos_r = Rect::from_min_size(Pos2::new(x - 4.0, r.top()), egui::vec2(w_pos + 8.0, h));
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
            let cw = num_w(&mp, &s, BIG);
            num(&p, x + (w_pos - cw) / 2.0, foot, &s, BIG, pl.rec, false);
            // the count-in's beats as pips
            let total = (st.count_in_bars.max(1) as u32) * st.sig.0.max(1) as u32;
            let done = total.saturating_sub(n);
            let shown = total.min(8);
            for k in 0..shown {
                let c = Pos2::new(x + w_pos - 4.0 - (shown - 1 - k) as f32 * 7.0, top + 6.0);
                if k < done {
                    p.circle_filled(c, 2.4, pl.rec);
                } else {
                    p.circle_stroke(c, 2.2, Stroke::new(1.0, pl.off_bd));
                }
            }
        } else {
            let (cap, s) = if st.tl_mode {
                (
                    "BAR · BEAT · STEP",
                    format!("{}.{}.{}", st.pos.0, st.pos.1, st.pos.2),
                )
            } else {
                ("POS · ROW", format!("{}.{:02}", st.pos.0, st.pos.1))
            };
            lab(&p, Pos2::new(x, top), cap, pl.label);
            let col = if st.rec { pl.rec } else { pl.lit };
            num(&p, x, foot, &s, BIG, col, false);
        }
        x += w_pos + gap / 2.0;
        div(&p, x);
        x += gap / 2.0;
        // 4. clock mm:ss.cs (the Timeline's time)
        if show_time {
            lab(&p, Pos2::new(x, top), "TIME", pl.label);
            // the in-between display: the signature at the caption's end
            if sig_in_time {
                p.text(
                    Pos2::new(x + w_time, top),
                    egui::Align2::RIGHT_TOP,
                    format!("{}/{}", st.sig.0, st.sig.1),
                    theme::label_font(CAP),
                    pl.lit,
                );
            }
            let ts = if st.tl_mode {
                let cs = (st.secs * 100.0).floor() as u64;
                format!("{:02}:{:02}.{:02}", cs / 6000, cs / 100 % 60, cs % 100)
            } else {
                "--:--.--".to_string()
            };
            num(&p, x, foot - 1.0, &ts, MID, pl.lit, false);
            x += w_time + gap / 2.0;
            div(&p, x);
            x += gap / 2.0;
        }
        // 5. pills: LOOP (range), CLICK / count-in, REC / ARMED, FOLLOW
        let loop_txt = match st.loop_bars {
            Some((a, b)) if !narrow => format!("LOOP {a}–{b}"),
            _ => "LOOP".to_string(),
        };
        let y1 = r.top() + 5.0;
        let y2 = r.bottom() - 5.0 - ph;
        let x2 = x + col1 + 4.0;
        let lr_loop = Rect::from_min_size(Pos2::new(x, y1), egui::vec2(col1, ph));
        pill(&p, lr_loop, &loop_txt, st.loop_on, false, false, &pl);
        let click_txt = match st.count_in_bars {
            0 => "CLICK".to_string(),
            n => format!("CLICK +{n}"),
        };
        // the click pill flashes on the beat (the transport's own beats)
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
        let lr_click = Rect::from_min_size(Pos2::new(x2, y1), egui::vec2(col2, ph));
        pill(
            &p,
            lr_click,
            &click_txt,
            st.metro || st.count_left.is_some(),
            false,
            on_beat,
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
        let rec_r = Rect::from_min_size(Pos2::new(x, y2), egui::vec2(col1, ph));
        pill(&p, rec_r, rec_txt, rec_on, true, false, &pl);
        let lr_follow = Rect::from_min_size(Pos2::new(x2, y2), egui::vec2(col2, ph));
        pill(&p, lr_follow, "FOLLOW", st.follow, false, false, &pl);
        x += w_state + gap / 2.0;
        div(&p, x);
        x += gap / 2.0;
        // 6. engine load and the audio buffer / underruns, each with a meter
        let (dsp, buf, xr, maxed) = st.load.unwrap_or((0.0, 0.0, 0, false));
        let row = |y: f32| y + ph / 2.0;
        let meter = |p: &egui::Painter, x0: f32, y: f32, frac: f32, warn_at: f32| {
            let n = 8;
            let mw = 32.0;
            let segw = mw / n as f32;
            for k in 0..n {
                let on = st.load.is_some() && (k as f32 + 0.5) / n as f32 <= frac;
                let c = if !on {
                    pl.track
                } else if (k as f32 + 1.0) / n as f32 > 0.85 {
                    pl.rec
                } else if (k as f32 + 1.0) / n as f32 > warn_at {
                    pl.warn
                } else {
                    pl.meter
                };
                let sr = Rect::from_min_size(
                    Pos2::new(x0 + k as f32 * segw, y - 3.0),
                    egui::vec2(segw - 1.0, 6.0),
                );
                p.rect_filled(sr, 1.0, c);
            }
        };
        let cap_l = lab_w(&mp, "BUF").max(lab_w(&mp, "DSP"));
        let small = |p: &egui::Painter, y: f32, s: String, col: Color32| {
            p.text(
                Pos2::new(r.right() - 9.0, row(y)),
                egui::Align2::RIGHT_CENTER,
                s,
                theme::num_font(CAP),
                col,
            );
        };
        if narrow || mid {
            let pct = match st.load {
                Some(_) => format!("DSP {dsp:.0}%"),
                None => "DSP --".into(),
            };
            p.text(
                Pos2::new(x, row(y1)),
                egui::Align2::LEFT_CENTER,
                pct,
                theme::label_font(CAP),
                pl.label,
            );
            let bt = match st.load {
                // in-between: the ms; at its largest just MAX (the tip has it)
                Some(_) if maxed && mid => "BUF MAX".into(),
                Some(_) if maxed => format!("BUF {buf:.0} MAX"),
                Some(_) if mid => format!("BUF {buf:.0} ms"),
                Some(_) => format!("BUF {buf:.0}"),
                None => "NO AUDIO".into(),
            };
            p.text(
                Pos2::new(x, row(y2)),
                egui::Align2::LEFT_CENTER,
                bt,
                theme::label_font(CAP),
                if xr > 0 { pl.warn } else { pl.label },
            );
        } else {
            p.text(
                Pos2::new(x, row(y1)),
                egui::Align2::LEFT_CENTER,
                "DSP",
                theme::label_font(CAP),
                pl.label,
            );
            meter(&p, x + cap_l + 5.0, row(y1), dsp / 100.0, 0.6);
            small(
                &p,
                y1,
                if st.load.is_some() {
                    format!("{dsp:.0}%")
                } else {
                    "--".into()
                },
                pl.lit,
            );
            p.text(
                Pos2::new(x, row(y2)),
                egui::Align2::LEFT_CENTER,
                if st.load.is_some() { "BUF" } else { "NO AUDIO" },
                theme::label_font(CAP),
                pl.label,
            );
            if st.load.is_some() {
                // the buffer against 100 ms; an underrun turns it amber
                meter(&p, x + cap_l + 5.0, row(y2), (buf / 100.0).min(1.0), 0.7);
                small(
                    &p,
                    y2,
                    if maxed {
                        format!("{buf:.0} MAX")
                    } else {
                        format!("{buf:.0} ms")
                    },
                    if xr > 0 { pl.warn } else { pl.lit },
                );
            }
        }
        // the underrun light: amber after an underrun (by BUF's meter)
        if xr > 0 && !narrow && !mid {
            let xc = Pos2::new(x + cap_l + 41.0, row(y2));
            p.circle_filled(xc, 2.6, pl.warn);
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
                // the swatch: the tinted glass in its frame, a digit-dark dot
                let sp = pal(hex(c));
                ui.painter().circle_filled(r.center(), 9.5, sp.edge);
                ui.painter().circle_filled(r.center(), 7.5, sp.bg_top);
                ui.painter().circle_filled(r.center(), 2.5, sp.lit);
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
            .font(theme::num_font(crate::theme::ts::LARGE))
            .text_color(pl.hot)
            .background_color(pl.bg_top)
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
        let w = master_w(self.lcd_narrow(ui.ctx()));
        let (r, resp) = ui.allocate_exact_size(egui::vec2(w, lcd_h()), Sense::click());
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
                theme::label_font(CAP),
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
                theme::label_font(CAP),
                dim,
            );
            if vol > 0.0001 {
                // (11 px segments are too small to read: a mono readout)
                p.text(
                    Pos2::new(r.right() - 40.0, r.top() + 3.0),
                    egui::Align2::RIGHT_TOP,
                    &db,
                    theme::num_font(crate::theme::ts::COMPACT),
                    pl.lit,
                );
            } else {
                p.text(
                    Pos2::new(r.right() - 40.0, r.top() + 4.0),
                    egui::Align2::RIGHT_TOP,
                    "-inf",
                    theme::label_font(CAP),
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
            for (k, (name, y)) in [("L", r.bottom() - 21.0), ("R", r.bottom() - 12.0)]
                .iter()
                .enumerate()
            {
                p.text(
                    Pos2::new(r.left() + 9.0, *y + 3.0),
                    egui::Align2::LEFT_CENTER,
                    *name,
                    theme::label_font(CAP),
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
                        pl.meter
                    };
                    let on = s < lit || (hk > 0 && s + 1 == hk);
                    p.rect_filled(sr, 0.5, if on { zone } else { pl.track });
                }
            }
            // 0 dB tick
            let x0 = bar_l + (48.0 / 51.0) * (bar_r - bar_l);
            p.vline(
                x0,
                (r.bottom() - 23.5)..=(r.bottom() - 3.0),
                Stroke::new(1.0, pl.div),
            );
            // limiter reduction: a short bar growing down
            let gx = r.right() - 27.0;
            let gr_r = Rect::from_min_max(
                Pos2::new(gx, r.bottom() - 22.0),
                Pos2::new(gx + 5.0, r.bottom() - 6.0),
            );
            p.rect_filled(gr_r, 1, pl.track);
            let f = (-loud.gain_red / 12.0).clamp(0.0, 1.0);
            if f > 0.0 && lim.enabled {
                p.rect_filled(
                    Rect::from_min_size(gr_r.min, egui::vec2(5.0, gr_r.height() * f)),
                    1,
                    pl.warn,
                );
            }
            // clip light: a true peak over 0 dBTP since Play (the GR bar and
            // the light have no captions: at a readable size they do not
            // fit the LCD; the tooltip names them)
            let clip = loud.true_peak.is_finite() && loud.true_peak > 0.0;
            let cc = Pos2::new(r.right() - 11.0, r.bottom() - 15.0);
            if clip {
                p.circle_filled(cc, 6.0, pl.rec.gamma_multiply(0.25));
                p.circle_filled(cc, 3.2, pl.rec);
            } else {
                p.circle_stroke(cc, 3.0, Stroke::new(1.0, pl.off_bd));
            }
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
