//! The Ember (Slate) mixer strips: insert slots, send and pan knobs, a
//! fader with a dB scale (0 in acc-hot), LED dot meters with peak hold, an
//! LCD dB readout and outlined M / S buttons; plus the Master section's
//! knobs and dot rows (docs: STYLE_EMBER.md, mockup c-mixer-eq). Paint and
//! interaction only: the mixer data and what it does are unchanged.
use super::*;
use crate::ember;
use crate::theme::{label_font, num_font};

/// width of every strip (channel, return, master)
pub(super) const STRIP_W: f32 = 110.0;
/// the fader's left edge from the strip's left (the UI tests rely on it)
const FADER_X: f32 = 17.0;
const FADER_W: f32 = 40.0;
/// strip rows above the fader (header, inserts, sends, pan/duck)
const TOP_H: f32 = 178.0;
/// strip rows below the fader (readout, buttons)
const FOOT_H: f32 = 64.0;

/// fader position 0..1 <-> dB: a console taper (gain itself is unchanged,
/// 0..2 linear, 0 dB = 1.0 at 72 % of the travel)
const TAPER: [(f32, f32); 7] = [
    (0.0, -90.0),
    (0.06, -48.0),
    (0.24, -24.0),
    (0.42, -12.0),
    (0.57, -6.0),
    (0.72, 0.0),
    (1.0, 6.0206),
];
/// the scale printed beside the fader
const SCALE: [(&str, f32); 7] = [
    ("+6", 1.0),
    ("0", 0.72),
    ("-6", 0.57),
    ("-12", 0.42),
    ("-24", 0.24),
    ("-48", 0.06),
    ("-∞", 0.0),
];

pub(super) fn gain_to_pos(g: f32) -> f32 {
    if g <= 1e-5 {
        return 0.0;
    }
    let db = 20.0 * g.log10();
    for w in TAPER.windows(2) {
        if db <= w[1].1 {
            let k = ((db - w[0].1) / (w[1].1 - w[0].1)).clamp(0.0, 1.0);
            return w[0].0 + k * (w[1].0 - w[0].0);
        }
    }
    1.0
}

pub(super) fn pos_to_gain(p: f32) -> f32 {
    if p <= 0.004 {
        return 0.0;
    }
    let p = p.min(1.0);
    for w in TAPER.windows(2) {
        if p <= w[1].0 {
            let k = (p - w[0].0) / (w[1].0 - w[0].0);
            let db = w[0].1 + k * (w[1].1 - w[0].1);
            return 10f32.powf(db / 20.0).clamp(0.0, 2.0);
        }
    }
    2.0
}

/// the strip's height for the room the mixer has
pub(super) fn strip_h(avail: f32) -> f32 {
    avail.clamp(TOP_H + FOOT_H + 120.0, 640.0)
}

/// a fader: drag the cap (Ctrl/Shift fine), double-click = 0 dB, arrows
/// nudge it when focused
pub(super) fn fader(ui: &mut egui::Ui, r: Rect, id: egui::Id, vol: &mut f32) -> egui::Response {
    let slot = Rect::from_min_max(
        Pos2::new(r.left() + 25.0, r.top() + 6.0),
        Pos2::new(r.left() + 31.0, r.bottom() - 6.0),
    );
    let resp = ui.interact(r, id, Sense::click_and_drag());
    let fine = if ui.input(|i| i.modifiers.ctrl || i.modifiers.shift || i.modifiers.command) {
        0.1
    } else {
        1.0
    };
    if resp.dragged() {
        let nid = id.with("pos");
        let p0 = if resp.drag_started() {
            gain_to_pos(*vol)
        } else {
            ui.data(|m| m.get_temp::<f32>(nid))
                .unwrap_or(gain_to_pos(*vol))
        };
        let p = (p0 - resp.drag_delta().y / slot.height() * fine).clamp(0.0, 1.0);
        ui.data_mut(|m| m.insert_temp(nid, p));
        *vol = pos_to_gain(p);
    }
    if resp.double_clicked() {
        *vol = 1.0;
    }
    if resp.has_focus() {
        let (up, down) = ui.input(|i| {
            (
                i.key_pressed(egui::Key::ArrowUp),
                i.key_pressed(egui::Key::ArrowDown),
            )
        });
        if up || down {
            let p = gain_to_pos(*vol) + if up { 0.01 } else { -0.01 };
            *vol = pos_to_gain(p.clamp(0.0, 1.0));
        }
    }
    let v = *vol;
    resp.widget_info(|| {
        let mut w = egui::WidgetInfo::labeled(egui::Role::Slider, true, "Volume");
        w.value = Some(v as f64);
        w.current_text_value = Some(db_text(v));
        w
    });
    let hovered = resp.hovered() || resp.dragged();
    ember::fader(ui.painter(), slot, gain_to_pos(*vol), &SCALE, hovered);
    resp
}

/// a meter column with a peak-hold dot (held 1.5 s); `level` = peak gain
fn meter(ui: &egui::Ui, r: Rect, id: egui::Id, level: f32) {
    let db = if level > 1e-5 {
        20.0 * level.log10()
    } else {
        -90.0
    };
    let f = ember::db_frac(db);
    let now = ui.input(|i| i.time);
    let hid = id.with("hold");
    let (mut pk, mut at) = ui
        .data(|d| d.get_temp::<(f32, f64)>(hid))
        .unwrap_or((0.0, now));
    if f >= pk || now - at > 1.5 {
        pk = f;
        at = now;
    }
    ui.data_mut(|d| d.insert_temp(hid, (pk, at)));
    crate::theme::meter_peak(ui.painter(), r, f, (pk > 0.0).then_some(pk));
}

/// a pic-2 knob with its name above it; `v` 0..1
#[allow(clippy::too_many_arguments)]
pub(super) fn mini_knob(
    ui: &mut egui::Ui,
    c: Pos2,
    id: egui::Id,
    v: &mut f32,
    def: f32,
    name: &str,
    value: &str,
    enabled: bool,
) -> egui::Response {
    let rad = 11.0;
    let kr = Rect::from_center_size(c, egui::vec2(rad * 2.0 + 6.0, rad * 2.0 + 6.0));
    let resp = ui.interact(
        kr,
        id,
        if enabled {
            Sense::click_and_drag()
        } else {
            Sense::hover()
        },
    );
    if enabled {
        if resp.dragged() {
            let fine = if ui.input(|i| i.modifiers.ctrl || i.modifiers.shift) {
                0.1
            } else {
                1.0
            };
            let d = resp.drag_delta();
            *v = (*v + (d.x - d.y) * 0.005 * fine).clamp(0.0, 1.0);
        }
        if resp.double_clicked() {
            *v = def;
        }
    }
    let p = ui.painter();
    let mut k = crate::theme::KnobFace::new(*v);
    k.from = if (def - 0.5).abs() < 0.01 { 0.5 } else { 0.0 };
    k.dim = if enabled { 1.0 } else { 0.45 };
    crate::theme::knob_face(p, c, rad + 0.5, k);
    ember::knob_label(p, Pos2::new(c.x, c.y - rad - 9.0), name, 11.0);
    let nm = name.to_string();
    let val = value.to_string();
    let x = *v;
    let nm = ui
        .data(|d| d.get_temp::<String>(id.with("a11y")))
        .unwrap_or(nm);
    resp.widget_info(|| {
        let mut w = egui::WidgetInfo::labeled(egui::Role::Slider, enabled, &nm);
        w.value = Some(x as f64);
        w.current_text_value = Some(val.clone());
        w
    });
    resp.on_hover_text(format!("{name}: {value} · drag · double-click resets"))
}

/// the coloured title chip at the top of a strip (a real label, so screen
/// readers and the UI tests find it)
fn header(ui: &mut egui::Ui, r: Rect, title: &str, col: Color32) {
    let t = th();
    let h = Rect::from_min_size(
        r.min + egui::vec2(5.0, 5.0),
        egui::vec2(r.width() - 10.0, 21.0),
    );
    let mut m = egui::Mesh::default();
    theme::round_mesh(
        &mut m,
        h,
        [3.0; 4],
        theme::tone(col, 0.06),
        theme::tone(col, -0.12),
    );
    ui.painter().add(egui::Shape::mesh(m));
    ui.painter().hline(
        (h.left() + 2.0)..=(h.right() - 2.0),
        h.top() + 1.0,
        Stroke::new(1.0, Color32::from_white_alpha(40)),
    );
    let ink = if theme::luma(col) > 0.55 {
        hex(0x16181B)
    } else {
        t.ink
    };
    let txt = egui::RichText::new(title).font(label_font(13.0)).color(ink);
    ui.put(h, egui::Label::new(txt).truncate().selectable(false));
}

/// the strip's panel; returns its rect
fn frame(ui: &mut egui::Ui, h: f32) -> Rect {
    let (r, _) = ui.allocate_exact_size(egui::vec2(STRIP_W, h), Sense::hover());
    ember::panel(ui.painter(), r, 6);
    r
}

fn fader_rect(r: Rect) -> Rect {
    Rect::from_min_max(
        Pos2::new(r.left() + FADER_X, r.top() + TOP_H),
        Pos2::new(r.left() + FADER_X + FADER_W, r.bottom() - FOOT_H),
    )
}

/// the meter columns right of the fader: one per level (mono channels
/// show one column, Master shows L and R)
fn meters(ui: &mut egui::Ui, r: Rect, id: egui::Id, levels: &[f32]) {
    let fr = fader_rect(r);
    let col_y = (fr.top() + 6.0)..=(fr.bottom() - 6.0);
    let x0 = fr.right() + 10.0;
    for (k, l) in levels.iter().enumerate() {
        let mr = Rect::from_x_y_ranges(
            (x0 + k as f32 * 11.0)..=(x0 + k as f32 * 11.0 + 6.0),
            col_y.clone(),
        );
        meter(ui, mr, id.with(k), *l);
    }
    if levels.len() == 2 {
        for (k, s) in ["L", "R"].iter().enumerate() {
            ui.painter().text(
                Pos2::new(x0 + 3.0 + k as f32 * 11.0, fr.bottom() + 1.0),
                egui::Align2::CENTER_TOP,
                *s,
                label_font(10.0),
                ember::INK3,
            );
        }
    }
}

/// the LCD dB readout under the fader
fn readout(ui: &egui::Ui, r: Rect, vol: f32) {
    let lr = Rect::from_min_size(
        Pos2::new(r.left() + 16.0, r.bottom() - FOOT_H + 12.0),
        egui::vec2(r.width() - 32.0, 19.0),
    );
    let txt = if vol <= 1e-4 {
        "-inf".to_string()
    } else {
        format!("{:+.1}", 20.0 * vol.log10())
    };
    ember::lcd(ui.painter(), lr, "", &txt, 12.5);
}

/// pan as text: C, L40, R25
fn pan_text(pan: f32) -> String {
    if pan.abs() < 0.01 {
        "C".to_string()
    } else if pan < 0.0 {
        format!("L{:.0}", -pan * 100.0)
    } else {
        format!("R{:.0}", pan * 100.0)
    }
}

/// what a channel strip edits
pub(super) struct Chan<'a> {
    pub ch: usize,
    pub title: String,
    pub colour: Color32,
    pub vol: &'a mut f32,
    pub pan: &'a mut f32,
    pub mute: &'a mut bool,
    pub solo: &'a mut bool,
    pub send: &'a mut [f32; 2],
    pub duck: &'a mut f32,
    pub is_key: bool,
    pub level: f32,
    /// the track instrument's effects: (name, enabled)
    pub inserts: &'a [(String, bool)],
}

pub(super) struct ChanOut {
    pub rect: Rect,
    pub fader: egui::Response,
    pub pan: egui::Response,
    pub sends: [egui::Response; 2],
    /// an insert slot (or "+ Insert") was clicked
    pub open_inserts: bool,
}

/// one channel strip (mockup c: inserts, sends, pan, fader, meter, M S)
pub(super) fn channel(ui: &mut egui::Ui, h: f32, c: Chan) -> ChanOut {
    let r = frame(ui, h);
    let id = ui.make_persistent_id(("ember_strip", c.ch));
    header(ui, r, &c.title, c.colour);
    // insert slots: the track instrument's effect chain (click: Devices)
    let mut open_inserts = false;
    for k in 0..3 {
        let sr = Rect::from_min_size(
            Pos2::new(r.left() + 6.0, r.top() + 31.0 + k as f32 * 17.0),
            egui::vec2(r.width() - 12.0, 15.0),
        );
        let resp = ui.interact(sr, id.with(("ins", k)), Sense::click());
        let p = ui.painter();
        p.rect_filled(sr, 2, hex(0x17191C));
        p.rect_stroke(
            sr,
            2,
            Stroke::new(
                1.0,
                if resp.hovered() {
                    ember::LINE2
                } else {
                    ember::LINE
                },
            ),
            egui::StrokeKind::Inside,
        );
        let more = c.inserts.len() > 3 && k == 2;
        match c.inserts.get(k) {
            Some((name, on)) => {
                ember::led(p, Pos2::new(sr.left() + 7.0, sr.center().y), 2.5, *on);
                let label = if more {
                    format!("{name} +{}", c.inserts.len() - 3)
                } else {
                    name.clone()
                };
                let label = super::pattern::fit(
                    p,
                    &label,
                    &egui::FontId::proportional(10.5),
                    sr.width() - 18.0,
                );
                p.text(
                    Pos2::new(sr.left() + 14.0, sr.center().y),
                    egui::Align2::LEFT_CENTER,
                    label,
                    egui::FontId::proportional(10.5),
                    if *on { th().ink } else { ember::INK3 },
                );
            }
            None if k == c.inserts.len() => {
                p.text(
                    sr.center(),
                    egui::Align2::CENTER_CENTER,
                    "+ Insert",
                    egui::FontId::proportional(10.5),
                    ember::INK3,
                );
            }
            None => {}
        }
        if resp
            .on_hover_text("this track's effects: click to edit them in Nested Device Chains")
            .clicked()
        {
            open_inserts = true;
        }
    }
    // sends A (delay) and B (reverb)
    let ky = r.top() + 106.0;
    let mut sends = Vec::with_capacity(2);
    for (b, (nm, what)) in [("Dly", "delay"), ("Rev", "reverb")].iter().enumerate() {
        let x = r.left() + if b == 0 { 32.0 } else { 78.0 };
        let mut v = c.send[b];
        let vt = format!("{:.0} %", v * 100.0);
        // screen readers (and the UI tests) hear what the classic slider said
        let a11y = format!("send {} ({what}): {:.0} %", ["A", "B"][b], v * 100.0);
        ui.data_mut(|d| d.insert_temp(id.with(("send", b)).with("a11y"), a11y));
        let resp = mini_knob(
            ui,
            Pos2::new(x, ky),
            id.with(("send", b)),
            &mut v,
            0.0,
            &format!("{} {}", ["A", "B"][b], nm),
            &vt,
            true,
        )
        .on_hover_text(format!("send {} ({what})", ["A", "B"][b]));
        if v != c.send[b] {
            c.send[b] = v;
        }
        sends.push(resp);
    }
    // pan + duck
    let py = r.top() + 152.0;
    let pn0 = (*c.pan + 1.0) * 0.5;
    let mut pn = pn0;
    let pan = mini_knob(
        ui,
        Pos2::new(r.left() + 32.0, py),
        id.with("pan"),
        &mut pn,
        0.5,
        "Pan",
        &pan_text(*c.pan),
        true,
    );
    // only an edit writes (no float round trip on every frame)
    if pn != pn0 {
        *c.pan = (pn * 2.0 - 1.0).clamp(-1.0, 1.0);
    }
    if pan.double_clicked() {
        *c.pan = 0.0;
    }
    ember::knob_label(
        ui.painter(),
        Pos2::new(r.left() + 80.0, py - 20.0),
        "Duck",
        11.0,
    );
    let mut pct = *c.duck * 100.0;
    let dr = Rect::from_center_size(Pos2::new(r.left() + 80.0, py + 1.0), egui::vec2(46.0, 18.0));
    let is_key = c.is_key;
    let d = ui
        .put(dr, |ui: &mut egui::Ui| {
            ui.add_enabled(
                !is_key,
                egui::DragValue::new(&mut pct)
                    .range(0.0..=100.0)
                    .speed(0.5)
                    .suffix(" %"),
            )
        })
        .on_disabled_hover_text("this is the sidechain key channel")
        .on_hover_text(
            "sidechain ducking: how far this channel dips on each note of the key \
             channel (the 'pump'); double-click = 0",
        );
    if c.is_key {
        ui.painter().text(
            Pos2::new(r.left() + 80.0, py + 14.0),
            egui::Align2::CENTER_TOP,
            "KEY",
            label_font(10.0),
            th().acc_hot,
        );
    }
    if d.changed() && !c.is_key {
        *c.duck = pct / 100.0;
    }
    if d.double_clicked() {
        *c.duck = 0.0;
    }
    // fader, meter, readout
    let fr = fader_rect(r);
    let fader = fader(ui, fr, id.with("fader"), c.vol);
    meters(ui, r, id.with("meter"), &[c.level]);
    readout(ui, r, *c.vol);
    // M / S
    let by = r.bottom() - 27.0;
    let mut row = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(Rect::from_min_size(
                Pos2::new(r.left() + 16.0, by),
                egui::vec2(r.width() - 32.0, 22.0),
            ))
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    row.spacing_mut().item_spacing.x = 6.0;
    if ember::button_sized(&mut row, " M ", *c.mute, 36.0)
        .on_hover_text("mute (F9)")
        .clicked()
    {
        *c.mute = !*c.mute;
    }
    if ember::button_sized(&mut row, " S ", *c.solo, 36.0)
        .on_hover_text("solo (F10)")
        .clicked()
    {
        *c.solo = !*c.solo;
    }
    let [a, b]: [egui::Response; 2] = sends.try_into().unwrap_or_else(|_| unreachable!());
    ChanOut {
        rect: r,
        fader,
        pan,
        sends: [a, b],
        open_inserts,
    }
}

/// a return strip (Ret A delay, Ret B reverb): its key parameters as
/// knobs, its level fader and the On button
pub(super) fn ret(
    ui: &mut egui::Ui,
    h: f32,
    b: usize,
    title: &str,
    lvl: &mut f32,
    dev: &mut crate::fx::Device,
) -> Rect {
    let r = frame(ui, h);
    let id = ui.make_persistent_id(("ember_ret", b));
    let col = fxview::strip_fill(dev.kind, true);
    header(ui, r, title, col);
    let keys: &[&str] = if b == 0 {
        &["sync", "feedback", "lp"]
    } else {
        &["size", "damping", "predelay", "lowcut", "highcut"]
    };
    let mut n = 0;
    for key in keys {
        let Some(k) = crate::fx::Device::param_index(dev.kind, key) else {
            continue;
        };
        let info = &crate::fx::params(dev.kind)[k];
        let (col, row) = (n % 2, n / 2);
        let c = Pos2::new(
            r.left() + if col == 0 { 32.0 } else { 78.0 },
            r.top() + 60.0 + row as f32 * 44.0,
        );
        let mut v = dev.params[k];
        let v0 = v;
        let short: String = info.name.chars().take(7).collect();
        mini_knob(
            ui,
            c,
            id.with(k),
            &mut v,
            v0,
            &short,
            &info.format(dev.params[k]),
            dev.enabled,
        )
        .on_hover_text(format!("{} {}", info.name, info.format(dev.params[k])));
        if v != v0 {
            dev.params[k] = v;
        }
        n += 1;
    }
    let fr = fader_rect(r);
    let fresp = fader(ui, fr, id.with("fader"), lvl);
    let _ = fresp;
    readout(ui, r, *lvl);
    let br = Rect::from_center_size(
        Pos2::new(r.center().x, r.bottom() - 16.0),
        egui::vec2(46.0, 22.0),
    );
    let mut row = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(br)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    if ember::button_sized(&mut row, "On", dev.enabled, 46.0)
        .on_hover_text("this return bus on / off")
        .clicked()
    {
        dev.enabled = !dev.enabled;
    }
    r
}

/// the Master strip: fader, L/R meters, readout and the limiter note
pub(super) fn master(
    ui: &mut egui::Ui,
    h: f32,
    vol: &mut f32,
    lr: &[f32],
    note: &str,
) -> (Rect, egui::Response) {
    let r = frame(ui, h);
    let id = ui.make_persistent_id("ember_master");
    header(ui, r, "Master", th().accent);
    let fr = fader_rect(r);
    let f = fader(ui, fr, id.with("fader"), vol);
    meters(ui, r, id.with("meter"), lr);
    readout(ui, r, *vol);
    ui.painter().text(
        Pos2::new(r.center().x, r.bottom() - 16.0),
        egui::Align2::CENTER_CENTER,
        note,
        egui::FontId::proportional(10.5),
        th().ink2,
    );
    (r, f)
}

/// a pic-1 knob for the Master section: name above, readout inside, an
/// editable value under it; `norm` 0..1
#[allow(clippy::too_many_arguments)]
pub(super) fn big(
    ui: &mut egui::Ui,
    c: Pos2,
    id: egui::Id,
    norm: &mut f32,
    def: f32,
    name: &str,
    readout: &str,
    enabled: bool,
) -> egui::Response {
    let rad = 27.0;
    let kr = Rect::from_center_size(c, egui::vec2(rad * 2.0, rad * 2.0));
    let resp = ui.interact(
        kr,
        id,
        if enabled {
            Sense::click_and_drag()
        } else {
            Sense::hover()
        },
    );
    if enabled {
        if resp.dragged() {
            let fine = if ui.input(|i| i.modifiers.ctrl || i.modifiers.shift) {
                0.1
            } else {
                1.0
            };
            let d = resp.drag_delta();
            *norm = (*norm + (d.x - d.y) * 0.004 * fine).clamp(0.0, 1.0);
        }
        if resp.double_clicked() {
            *norm = def;
        }
    }
    let p = ui.painter();
    let dim = if enabled { 1.0 } else { 0.5 };
    crate::theme::knob_hero(p, c, rad, *norm, false, readout, enabled, dim);
    ember::knob_label(p, Pos2::new(c.x, c.y - rad - 9.0), name, 12.0);
    let nm = name.to_string();
    let x = *norm;
    resp.widget_info(|| {
        let mut w = egui::WidgetInfo::labeled(egui::Role::Slider, enabled, &nm);
        w.value = Some(x as f64);
        w
    });
    resp
}

/// a dot row with a name on the left and a number on the right
pub(super) fn dot_row(ui: &mut egui::Ui, name: &str, frac: f32, warn: bool, num: &str) {
    let (r, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 14.0), Sense::hover());
    let p = ui.painter();
    p.text(
        r.left_center(),
        egui::Align2::LEFT_CENTER,
        name.to_uppercase(),
        label_font(11.5),
        th().ink2,
    );
    let mr = Rect::from_min_max(
        Pos2::new(r.left() + 78.0, r.top() + 3.5),
        Pos2::new(r.right() - 44.0, r.bottom() - 3.5),
    );
    ember::dot_meter(p, mr, frac, None, false, warn);
    p.text(
        r.right_center(),
        egui::Align2::RIGHT_CENTER,
        num,
        num_font(11.0),
        th().ink,
    );
}

/// the Master section (mockup c): limiter on/off, Gain / Ceiling / Release
/// knobs, LEVEL L / R and REDUCTION dot rows, the loudness readout and the
/// target. Same settings as the classic panel.
pub(super) fn master_panel(
    ui: &mut egui::Ui,
    h: f32,
    lim: &mut crate::master::LimiterSettings,
    loud: &crate::master::Readout,
    lr: &[f32],
) {
    let t = th();
    let fmt = |v: f32, unit: &str| {
        if v.is_finite() && v > -70.0 {
            format!("{v:.1} {unit}")
        } else {
            format!("–– {unit}")
        }
    };
    let w = 300.0;
    let (r, _) = ui.allocate_exact_size(egui::vec2(w, h), Sense::hover());
    ember::card(ui.painter(), r);
    let inner = r.shrink2(egui::vec2(16.0, 10.0));
    let mut ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(inner)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    let ui = &mut ui;
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Limiter & loudness")
                .font(label_font(14.0))
                .color(t.ink),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let v = if loud.integrated.is_finite() && loud.integrated > -70.0 {
                format!("{:.1}", loud.integrated)
            } else {
                "––".into()
            };
            let (lr, _) = ui.allocate_exact_size(egui::vec2(52.0, 18.0), Sense::hover());
            ember::glow_text(
                ui.painter(),
                lr.right_center(),
                egui::Align2::RIGHT_CENTER,
                &v,
                num_font(13.0),
                t.acc_hot,
                true,
            );
            ui.label(
                egui::RichText::new("LUFS")
                    .font(label_font(11.5))
                    .color(t.ink2),
            );
        });
    });
    ui.add_space(2.0);
    let on = lim.enabled;
    if ember::led_item(ui, "True-peak limiter", on)
        .on_hover_text(
            "lookahead brickwall limiter on the master and in every export: the output \
             never goes over the ceiling, even between samples (4x oversampled true \
             peak). Off = the classic soft clipper.",
        )
        .clicked()
    {
        lim.enabled = !lim.enabled;
    }
    ui.add_space(26.0);
    // three pic-1 knobs with the exact value under each
    let row = ui.available_rect_before_wrap();
    let cy = row.top() + 30.0;
    let xs = [row.left() + 40.0, row.center().x, row.right() - 40.0];
    let id = ui.make_persistent_id("ember_master_knobs");
    let en = lim.enabled;
    // (a knob writes only when it is turned: no float round trips)
    let g0 = lim.gain_db / 24.0;
    let mut g = g0;
    big(
        ui,
        Pos2::new(xs[0], cy),
        id.with(0),
        &mut g,
        0.0,
        "Gain",
        &format!("{:.1}", lim.gain_db),
        en,
    )
    .on_hover_text("drives the mix into the limiter: louder, more limiting");
    if g != g0 {
        lim.gain_db = (g * 24.0 * 10.0).round() / 10.0;
    }
    let c0 = (lim.ceiling_db + 12.0) / 12.0;
    let mut c = c0;
    big(
        ui,
        Pos2::new(xs[1], cy),
        id.with(1),
        &mut c,
        11.0 / 12.0,
        "Ceiling",
        &format!("{:.1}", lim.ceiling_db),
        en,
    )
    .on_hover_text("maximum true peak; −1 dBTP is safe for MP3 and streaming");
    if c != c0 {
        lim.ceiling_db = ((c * 12.0 - 12.0).clamp(-12.0, 0.0) * 10.0).round() / 10.0;
    }
    let rl0 = (lim.release_ms.max(10.0) / 10.0).log10() / 2.0;
    let mut rl = rl0;
    big(
        ui,
        Pos2::new(xs[2], cy),
        id.with(2),
        &mut rl,
        0.45,
        "Release",
        &format!("{:.0}", lim.release_ms),
        en,
    )
    .on_hover_text("how fast the gain comes back after a peak");
    if rl != rl0 {
        lim.release_ms = (10.0 * 100f32.powf(rl)).round().clamp(10.0, 1000.0);
    }
    // exact values (type or drag)
    let vy = cy + 36.0;
    let vals = [(xs[0], 0usize), (xs[1], 1), (xs[2], 2)];
    ui.add_enabled_ui(en, |ui| {
        for (x, k) in vals {
            let vr = Rect::from_center_size(Pos2::new(x, vy), egui::vec2(78.0, 18.0));
            match k {
                0 => ui.put(
                    vr,
                    egui::DragValue::new(&mut lim.gain_db)
                        .range(0.0..=24.0)
                        .speed(0.05)
                        .suffix(" dB")
                        .fixed_decimals(1),
                ),
                1 => ui.put(
                    vr,
                    egui::DragValue::new(&mut lim.ceiling_db)
                        .range(-12.0..=0.0)
                        .speed(0.05)
                        .suffix(" dBTP")
                        .fixed_decimals(1),
                ),
                _ => ui.put(
                    vr,
                    egui::DragValue::new(&mut lim.release_ms)
                        .range(10.0..=1000.0)
                        .speed(2.0)
                        .suffix(" ms")
                        .fixed_decimals(0),
                ),
            };
        }
    });
    // (put() moved the cursor under the values)
    let below = vy + 14.0 - ui.cursor().top();
    if below > 0.0 {
        ui.add_space(below);
    }
    ui.add_space(10.0);
    // levels and reduction as dot rows
    let db = |g: f32| if g > 1e-5 { 20.0 * g.log10() } else { -90.0 };
    let num = |d: f32| {
        if d > -70.0 {
            format!("{d:.1}")
        } else {
            "-inf".into()
        }
    };
    let l = lr.first().copied().unwrap_or(0.0);
    let rr = lr.get(1).copied().unwrap_or(0.0);
    dot_row(ui, "Level L", ember::db_frac(db(l)), true, &num(db(l)));
    dot_row(ui, "Level R", ember::db_frac(db(rr)), true, &num(db(rr)));
    let gr = loud.gain_red;
    dot_row(
        ui,
        "Reduction",
        (-gr / 12.0).clamp(0.0, 1.0),
        false,
        &format!("{gr:.1}"),
    );
    ui.add_space(8.0);
    egui::Grid::new("loud_grid").num_columns(2).show(ui, |ui| {
        for (name, v, unit, tip) in [
            (
                "Momentary",
                loud.momentary,
                "LUFS",
                "loudness of the last 0.4 s (ITU-R BS.1770)",
            ),
            (
                "Short-term",
                loud.short_term,
                "LUFS",
                "loudness of the last 3 s",
            ),
            (
                "Integrated",
                loud.integrated,
                "LUFS",
                "gated loudness since Play was pressed",
            ),
            (
                "True peak",
                loud.true_peak,
                "dBTP",
                "highest true peak since Play was pressed",
            ),
        ] {
            ui.label(
                egui::RichText::new(name)
                    .font(label_font(12.0))
                    .color(t.ink2),
            )
            .on_hover_text(tip);
            ui.label(
                egui::RichText::new(fmt(v, unit))
                    .font(num_font(12.0))
                    .color(t.acc_hot),
            );
            ui.end_row();
        }
    });
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("TARGET")
                .font(label_font(11.5))
                .color(t.ink2),
        );
        let cur = crate::master::TARGETS
            .iter()
            .find(|x| x.0 == lim.target_lufs)
            .map(|x| x.1)
            .unwrap_or("custom");
        egui::ComboBox::from_id_salt("lufs_target")
            .width(170.0)
            .selected_text(cur)
            .show_ui(ui, |ui| {
                for (v, n) in crate::master::TARGETS {
                    ui.selectable_value(&mut lim.target_lufs, v, n);
                }
            });
    });
    let gtxt = crate::master::guidance(loud.integrated, lim.target_lufs);
    if !gtxt.is_empty() {
        ui.label(egui::RichText::new(gtxt).small().color(t.ink2));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn taper_round_trips_and_keeps_unity_at_0_db() {
        assert_eq!(pos_to_gain(0.0), 0.0);
        assert_eq!(gain_to_pos(0.0), 0.0);
        assert!((pos_to_gain(0.72) - 1.0).abs() < 1e-5);
        assert!((pos_to_gain(1.0) - 2.0).abs() < 1e-3);
        for g in [0.01f32, 0.1, 0.5, 1.0, 1.5, 2.0] {
            let back = pos_to_gain(gain_to_pos(g));
            assert!((back - g).abs() / g < 1e-3, "{g} -> {back}");
        }
    }
}
