//! "Workbench Gradient" (A2) theme with the muted palette and rounded corners
//! (docs/DESIGN_LANGUAGE.md).
use eframe::egui::{self, Color32, CornerRadius, Pos2, Rect, Stroke};

pub mod instskin;
pub use instskin::{active as active_skin, instskin, is_builtin, skin_or, SkinScope};

pub const fn hex(c: u32) -> Color32 {
    Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8)
}

#[derive(Clone, Copy, PartialEq)]
pub struct Theme {
    pub name: &'static str,
    /// true: light Workbench panels (Amiga theme); false: dark
    pub light: bool,
    /// outer border colour of panels
    pub border: Color32,
    pub bg_top: Color32,
    pub bg_bot: Color32,
    pub cart_top: Color32,
    pub cart_bot: Color32,
    pub pill: Color32,
    pub hdr_top: Color32,
    pub hdr_bot: Color32,
    pub crt_top: Color32,
    pub crt_bot: Color32,
    pub crt_grid: Color32,
    pub ink: Color32,
    pub ink2: Color32,
    pub value: Color32,
    pub slate: Color32,
    pub brick: Color32,
    pub sage: Color32,
    pub ochre: Color32,
    pub note: Color32,
    pub instr: Color32,
    pub vol: Color32,
    pub fx: Color32,
    pub empty: Color32,
    pub play: Color32,
    /// selection fill / accent (focus, selection, active)
    pub sel: Color32,
    pub accent: Color32,
    /// keyboard-focus ring of a panel (must contrast with clips and the accent)
    pub focus_ring: Color32,
    /// outline of selected clips / items
    pub selection_outline: Color32,
    /// the Ember skin (Slate): pic-1 knobs with LED arcs, pic-2 bevelled
    /// panels, knurled knobs, LED dot meters (docs/STYLE_EMBER.md)
    pub ember: bool,
    /// LED arc start (red-orange), lit tips / readouts, unlit LED segments
    pub acc_deep: Color32,
    pub acc_hot: Color32,
    pub led_off: Color32,
    /// unlit meter dots and LEDs
    pub dot_off: Color32,
    /// outer panel edge (bevel shadow) and outlined-button outline
    pub edge: Color32,
    pub line2: Color32,
}

/// Default (0.2.4.x): "Slate", the Ember skin on slate grey (pic 2 panels,
/// pic 1 knobs and readouts, orange accent); see docs/STYLE_EMBER.md.
pub const SLATE: Theme = Theme {
    name: "Slate (default)",
    light: false,
    border: hex(0x121315),
    bg_top: hex(0x202326),
    bg_bot: hex(0x1B1D20),
    cart_top: hex(0x2F3338),
    cart_bot: hex(0x25282C),
    pill: hex(0x3A3E44),
    hdr_top: hex(0x2C3035),
    hdr_bot: hex(0x25282C),
    crt_top: hex(0x1A1C20),
    crt_bot: hex(0x141619),
    crt_grid: hex(0x262A2F),
    ink: hex(0xE9EBED),
    ink2: hex(0xAAB0B6),
    value: hex(0xFFB45A),
    slate: hex(0x454A51),
    brick: hex(0xC4623F),
    sage: hex(0x9BC46A),
    ochre: hex(0xFF8A2A),
    note: hex(0xE9EBED),
    instr: hex(0xFFB45A),
    vol: hex(0x7FC8D6),
    fx: hex(0xE0C46C),
    empty: hex(0x50555C),
    play: hex(0xFF8A2A),
    sel: hex(0x4A3C30),
    accent: hex(0xFF8A2A),
    focus_ring: hex(0xE9EBED),
    selection_outline: hex(0xFFB45A),
    ember: true,
    acc_deep: hex(0xE2481F),
    acc_hot: hex(0xFFB45A),
    led_off: hex(0x3A2B28),
    dot_off: hex(0x454A50),
    edge: hex(0x121315),
    line2: hex(0x6A7078),
};

/// The 0.2.3-0.2.4.1 default: modern dark grey with thin muted-green borders.
pub const DARK: Theme = Theme {
    name: "Dark Circuit",
    light: false,
    border: hex(0x4CAF6A),
    bg_top: hex(0x24272B),
    bg_bot: hex(0x17191C),
    cart_top: hex(0x33373C),
    cart_bot: hex(0x26292D),
    pill: hex(0x34383D),
    hdr_top: hex(0x2A2D31),
    hdr_bot: hex(0x232629),
    crt_top: hex(0x17191C),
    crt_bot: hex(0x131517),
    crt_grid: hex(0x262A2F),
    ink: hex(0xD4D7DB),
    // at least 6:1 on the cards and pills (was 9AA0A8, 4.5:1)
    ink2: hex(0xB4BAC2),
    value: hex(0x8FC7A0),
    slate: hex(0x3E4A44),
    brick: hex(0xB06A64),
    sage: hex(0x4CAF6A),
    ochre: hex(0x4CAF6A),
    note: hex(0xE6E8EB),
    instr: hex(0x8FD9A3),
    vol: hex(0x7FC8D6),
    fx: hex(0xE0C46C),
    empty: hex(0x4A4F56),
    play: hex(0xE08A3C),
    sel: hex(0x2E4A37),
    accent: hex(0x5CC07A),
    focus_ring: hex(0xE6E9ED),
    selection_outline: hex(0xFFFFFF),
    ember: false,
    acc_deep: hex(0xE2481F),
    acc_hot: hex(0xFFB45A),
    led_off: hex(0x3A2B28),
    dot_off: hex(0x454A50),
    edge: hex(0x121315),
    line2: hex(0x6A7078),
};

/// Optional "Amiga" theme: Workbench Gradient (A2) with the muted palette.
pub const AMIGA: Theme = Theme {
    name: "Amiga (Workbench Gradient)",
    light: true,
    border: hex(0x3E4652),
    bg_top: hex(0x9AA5B4),
    bg_bot: hex(0x6F7A8A),
    cart_top: hex(0xDFE4EA),
    cart_bot: hex(0xAEB6C1),
    pill: hex(0xEEF1F4),
    // white header text reads at 5.9:1 or more (3F7FE6 gave 3.9:1)
    hdr_top: hex(0x2A5FC4),
    hdr_bot: hex(0x1747A8),
    crt_top: hex(0x173A72),
    crt_bot: hex(0x0A1F45),
    crt_grid: hex(0x24477F),
    ink: hex(0x1B2330),
    ink2: hex(0x3E4652),
    // 6.8:1 on the cards (1F5FD1 was 4.5:1, 2.8:1 on their lower edge)
    value: hex(0x17469E),
    slate: hex(0x516D96),
    brick: hex(0x9C4F4B),
    sage: hex(0x4E7D5B),
    ochre: hex(0x946B3E),
    note: Color32::WHITE,
    instr: hex(0x7CFF9B),
    vol: hex(0x22E3FF),
    fx: hex(0xFFD400),
    empty: hex(0x4F6FA0),
    play: hex(0xFF7A00),
    sel: hex(0xC9D9F5),
    accent: hex(0x1F5FD1),
    focus_ring: hex(0xFF9F1C),
    selection_outline: hex(0xFFE08A),
    ember: false,
    acc_deep: hex(0xE2481F),
    acc_hot: hex(0xFFB45A),
    led_off: hex(0x3A2B28),
    dot_off: hex(0x454A50),
    edge: hex(0x121315),
    line2: hex(0x6A7078),
};

/// "The Matrix": Slate's studio skin (the same panels, knobs and LED dots)
/// on near-black green with a code-rain green accent (Del's pick B of the
/// three mockups, 29 Oct 2026). Built-in instruments get a teal skin
/// instead of the violet one (`instskin`), so they still stand apart.
pub const MATRIX: Theme = Theme {
    name: "The Matrix",
    light: false,
    border: hex(0x050A07),
    bg_top: hex(0x0F1612),
    bg_bot: hex(0x0A100C),
    cart_top: hex(0x18231C),
    cart_bot: hex(0x121B15),
    pill: hex(0x22322A),
    hdr_top: hex(0x16211A),
    hdr_bot: hex(0x121B15),
    crt_top: hex(0x07100B),
    crt_bot: hex(0x040A06),
    crt_grid: hex(0x14241A),
    ink: hex(0xE2F5E8),
    ink2: hex(0x9CC2A8),
    value: hex(0x5CFF8E),
    slate: hex(0x2C4034),
    brick: hex(0xC4623F),
    sage: hex(0x5CFF8E),
    ochre: hex(0x28FF6E),
    note: hex(0xE2F5E8),
    instr: hex(0x5CFF8E),
    vol: hex(0x7FC8D6),
    fx: hex(0xC8E06C),
    empty: hex(0x34483C),
    play: hex(0x28FF6E),
    sel: hex(0x1A3A26),
    accent: hex(0x28FF6E),
    focus_ring: hex(0xE2F5E8),
    selection_outline: hex(0x8CFFAE),
    ember: true,
    acc_deep: hex(0x0B9A3A),
    acc_hot: hex(0x8CFFAE),
    led_off: hex(0x14301E),
    dot_off: hex(0x2A3E31),
    edge: hex(0x050A07),
    line2: hex(0x4E6E58),
};

pub const THEMES: [Theme; 4] = [SLATE, DARK, AMIGA, MATRIX];

static CURRENT: std::sync::RwLock<Theme> = std::sync::RwLock::new(SLATE);
/// Interface style: Polished (default: soft depth, gradients, shadows) or
/// Flat (the look before 0.2.4.x's polish pass). Settings › Look.
static POLISHED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
pub fn polished() -> bool {
    POLISHED.load(std::sync::atomic::Ordering::Relaxed)
}
pub fn set_polished(on: bool) {
    POLISHED.store(on, std::sync::atomic::Ordering::Relaxed);
}
pub fn th() -> Theme {
    *CURRENT.read().unwrap()
}
pub fn set(t: Theme) {
    *CURRENT.write().unwrap() = t;
}

/// WCAG relative luminance of a colour (alpha ignored)
pub fn luminance(c: Color32) -> f32 {
    let f = |v: u8| {
        let v = v as f32 / 255.0;
        if v <= 0.040_45 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * f(c.r()) + 0.7152 * f(c.g()) + 0.0722 * f(c.b())
}

/// WCAG contrast ratio between two colours (1 … 21)
pub fn contrast(a: Color32, b: Color32) -> f32 {
    let (x, y) = (luminance(a), luminance(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

/// light and dark ink for surfaces the theme's own ink does not suit
const LIGHT_INK: Color32 = Color32::from_rgb(0xEE, 0xF1, 0xF4);
const LIGHT_INK2: Color32 = Color32::from_rgb(0xB4, 0xBC, 0xC8);
const DARK_INK: Color32 = Color32::from_rgb(0x14, 0x18, 0x1E);
const DARK_INK2: Color32 = Color32::from_rgb(0x3A, 0x42, 0x4E);

/// the text colour for a surface: the theme's ink where it reads (at
/// least 4.5:1), else light or dark ink, whichever reads better. Painted
/// views use it on wells, pads, the CRT and headers so text follows the
/// surface (the Amiga theme's dark-on-dark fix).
pub fn on(bg: Color32) -> Color32 {
    pick(th().ink, LIGHT_INK, DARK_INK, bg)
}
/// secondary text for a surface (as [`on`])
pub fn on2(bg: Color32) -> Color32 {
    pick(th().ink2, LIGHT_INK2, DARK_INK2, bg)
}
fn pick(own: Color32, light: Color32, dark: Color32, bg: Color32) -> Color32 {
    if contrast(own, bg) >= 4.5 {
        own
    } else if contrast(light, bg) >= contrast(dark, bg) {
        light
    } else {
        dark
    }
}

pub fn apply(ctx: &egui::Context, t: Theme) {
    set(t);
    let mut v = if t.light {
        egui::Visuals::light()
    } else {
        egui::Visuals::dark()
    };
    v.window_corner_radius = CornerRadius::same(10);
    v.menu_corner_radius = CornerRadius::same(8);
    v.panel_fill = Color32::TRANSPARENT;
    v.window_fill = t.cart_top;
    v.extreme_bg_color = t.crt_bot;
    v.override_text_color = Some(t.ink);
    // selected rows keep the theme ink readable (Amiga: the slate blue
    // gave 3:1, so it takes its pale selection blue)
    v.selection.bg_fill = if contrast(t.ink, t.slate) >= 4.5 {
        t.slate
    } else {
        t.sel
    };
    // text fields: typed text is the theme ink, so their fill must suit it
    // (Amiga's dark CRT blue hid it)
    if contrast(t.ink, t.crt_bot) < 4.5 {
        v.text_edit_bg_color = Some(t.pill);
    }
    v.selection.stroke = Stroke::new(1.0, sel_text(v.selection.bg_fill));
    v.window_stroke = Stroke::new(1.0, t.border);
    for w in [
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
        &mut v.widgets.noninteractive,
    ] {
        w.corner_radius = CornerRadius::same(6);
    }
    v.widgets.inactive.bg_fill = t.pill;
    v.widgets.inactive.weak_bg_fill = t.pill;
    v.widgets.inactive.bg_stroke = Stroke::new(
        1.0,
        if t.light {
            hex(0x7D8794)
        } else {
            hex(0x4A4F56)
        },
    );
    v.widgets.hovered.weak_bg_fill = if t.light {
        hex(0xF7F9FB)
    } else {
        hex(0x3C4147)
    };
    v.widgets.hovered.bg_fill = v.widgets.hovered.weak_bg_fill;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, t.border);
    v.widgets.active.weak_bg_fill = if t.light {
        hex(0xC6CCD4)
    } else {
        hex(0x454A51)
    };
    v.widgets.noninteractive.bg_stroke = Stroke::new(
        1.0,
        if t.light {
            hex(0x7D8794)
        } else {
            hex(0x3A3F45)
        },
    );
    for w in [
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
        &mut v.widgets.noninteractive,
    ] {
        w.fg_stroke.color = t.ink;
    }
    if polished() {
        polish(&mut v, &t);
    }
    if t.ember {
        ember_visuals(&mut v, &t);
    }
    fonts(ctx);
    ctx.set_visuals(v);
    ctx.global_style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(6.0, 5.0);
        s.spacing.button_padding = egui::vec2(9.0, 3.0);
        s.text_styles = text_styles();
    });
}

/// The Polished style on top of the base visuals: soft shadows under
/// windows and menus, gentle hover / pressed states and a clearer selected
/// state (active tabs, selectable labels). Sizes and spacing are unchanged.
fn polish(v: &mut egui::Visuals, t: &Theme) {
    let (win_a, pop_a) = if t.light { (70, 50) } else { (120, 90) };
    v.window_shadow = egui::Shadow {
        offset: [0, 6],
        blur: 22,
        spread: 0,
        color: Color32::from_black_alpha(win_a),
    };
    v.popup_shadow = egui::Shadow {
        offset: [0, 3],
        blur: 12,
        spread: 0,
        color: Color32::from_black_alpha(pop_a),
    };
    // hover lifts a touch, pressed sinks (darker fill, no lift)
    v.widgets.hovered.expansion = 0.5;
    v.widgets.active.expansion = 0.0;
    v.widgets.active.weak_bg_fill = mix(v.widgets.inactive.weak_bg_fill, Color32::BLACK, 0.18);
    v.widgets.active.bg_fill = v.widgets.active.weak_bg_fill;
    v.widgets.active.bg_stroke = Stroke::new(1.0, mix(t.border, Color32::BLACK, 0.2));
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, mix(t.border, t.ink, 0.25));
    // selected tabs / toggles: the accent shows through, text stays light
    let selc = mix(t.slate, t.accent, if t.light { 0.45 } else { 0.4 });
    // the text on it is the theme ink: dark ink (Amiga) needs the pale
    // selection blue to stay readable
    v.selection.bg_fill = if contrast(t.ink, selc) >= 4.5 {
        selc
    } else {
        t.sel
    };
    v.selection.stroke = Stroke::new(1.0, sel_text(v.selection.bg_fill));
}

/// the text on a selected widget: white where it reads (4.5:1), else the
/// ink that suits the fill (Amiga's pale selection blue)
fn sel_text(fill: Color32) -> Color32 {
    if contrast(Color32::WHITE, fill) >= 4.5 {
        Color32::WHITE
    } else {
        on(fill)
    }
}

/// The Ember skin (Slate) on top: pic-2 outlined grey buttons, recessed
/// dark inputs, selected = orange text on a dark fill (never an orange
/// block, never a glow), 4 px widget corners.
fn ember_visuals(v: &mut egui::Visuals, t: &Theme) {
    v.window_corner_radius = CornerRadius::same(7);
    v.menu_corner_radius = CornerRadius::same(5);
    v.window_fill = t.cart_top;
    v.window_stroke = Stroke::new(1.0, t.edge);
    v.extreme_bg_color = hex(0x17191C);
    v.faint_bg_color = t.cart_top;
    v.code_bg_color = hex(0x17191C);
    v.hyperlink_color = t.acc_hot;
    v.text_cursor.stroke = Stroke::new(2.0, t.accent);
    for w in [
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
        &mut v.widgets.noninteractive,
    ] {
        w.corner_radius = CornerRadius::same(4);
    }
    v.widgets.noninteractive.bg_fill = t.cart_top;
    v.widgets.noninteractive.weak_bg_fill = t.cart_top;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, hex(0x3B3F45));
    v.widgets.inactive.bg_fill = hex(0x363A40);
    v.widgets.inactive.weak_bg_fill = hex(0x363A40);
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, hex(0x5A6067));
    v.widgets.hovered.bg_fill = hex(0x41464C);
    v.widgets.hovered.weak_bg_fill = hex(0x41464C);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, hex(0x8A8F96));
    v.widgets.active.bg_fill = hex(0x2B2E33);
    v.widgets.active.weak_bg_fill = hex(0x2B2E33);
    v.widgets.active.bg_stroke = Stroke::new(1.0, hex(0x8A8F96));
    v.widgets.open.bg_fill = hex(0x2B2E33);
    v.widgets.open.weak_bg_fill = hex(0x2B2E33);
    v.widgets.open.bg_stroke = Stroke::new(1.0, hex(0x8A8F96));
    // selected toggles / tabs: orange text on a dark warm fill
    v.selection.bg_fill = t.sel;
    v.selection.stroke = Stroke::new(1.0, t.acc_hot);
    v.widgets.hovered.expansion = 0.0;
}

/// The central type scale (docs/UI_SCALING.md). Code asks for a step by
/// name, never a raw number; every size goes through [`ts::px`], which
/// applies the minimum readable size and Settings › Look › Text size.
pub mod ts {
    use std::sync::atomic::{AtomicU32, Ordering};
    /// the smallest text anywhere: LCD captions, knob scale marks, badges
    pub const TINY: f32 = 10.0;
    /// secondary text, hints, `.small()`
    pub const SMALL: f32 = 11.0;
    /// uppercase captions and knob names (label family)
    pub const LABEL: f32 = 11.5;
    /// dense lists and table rows
    pub const COMPACT: f32 = 12.5;
    /// tracker cells and mono text
    pub const MONO: f32 = 12.5;
    /// normal UI text and buttons
    pub const BODY: f32 = 13.0;
    /// numeric readouts (num family)
    pub const LCD: f32 = 13.0;
    /// emphasised names (instrument titles, big pads)
    pub const LARGE: f32 = 15.0;
    pub const HEADING: f32 = 16.0;
    pub const TITLE: f32 = 19.0;
    /// the big LCD digits
    pub const DISPLAY: f32 = 26.0;
    /// nothing renders smaller than this (at 100 % Text size)
    pub const MIN: f32 = TINY;
    /// Text size range (Settings › Look)
    pub const TEXT_MIN: f32 = 0.9;
    pub const TEXT_MAX: f32 = 1.5;

    static SCALE: AtomicU32 = AtomicU32::new(0x3F80_0000); // 1.0
    pub fn text_scale() -> f32 {
        f32::from_bits(SCALE.load(Ordering::Relaxed))
    }
    /// set the Text size (1.0 = 100 %); call `theme::apply` afterwards
    pub fn set_text_scale(k: f32) {
        let k = if k.is_finite() {
            k.clamp(TEXT_MIN, TEXT_MAX)
        } else {
            1.0
        };
        SCALE.store(k.to_bits(), Ordering::Relaxed);
    }
    /// a type-scale size → the points to render: times the Text size, and
    /// never below [`MIN`] (a 90 % Text size keeps the smallest role at 10 pt)
    pub fn px(size: f32) -> f32 {
        px_at(size, text_scale())
    }
    /// [`px`] at a given Text size
    pub fn px_at(size: f32, text: f32) -> f32 {
        (size * text).max(MIN)
    }
}

/// proportional (body) text at a type-scale size
pub fn prop(size: f32) -> egui::FontId {
    egui::FontId::proportional(ts::px(size))
}
/// monospace text at a type-scale size
pub fn mono(size: f32) -> egui::FontId {
    egui::FontId::monospace(ts::px(size))
}

/// the height of one row of text in `font` (follows the type scale and
/// the Text size, so painted layouts can size their rows from it)
pub fn row_h(p: &egui::Painter, font: &egui::FontId) -> f32 {
    p.fonts_mut(|f| f.row_height(font))
}

/// the width of `text` in `font`
pub fn text_w(p: &egui::Painter, text: &str, font: egui::FontId) -> f32 {
    p.layout_no_wrap(text.to_owned(), font, egui::Color32::WHITE)
        .size()
        .x
}

/// `font`, a half point smaller at a time (never under the type scale's
/// minimum) until `text` fits `max_w`: tight painted cells try this before
/// cutting with "…"
pub fn shrink_to_fit(
    p: &egui::Painter,
    text: &str,
    font: egui::FontId,
    max_w: f32,
) -> egui::FontId {
    let min = ts::px(ts::MIN);
    let mut f = font;
    while f.size > min && text_w(p, text, f.clone()) > max_w {
        f.size = (f.size - 0.5).max(min);
    }
    f
}

/// paint one line of text that never runs past `max_w`: it is cut with
/// "…" where it would. Returns the painted rect and whether it was cut
/// (so the caller can put the full text in a tooltip).
pub fn text_fit(
    p: &egui::Painter,
    pos: egui::Pos2,
    align: egui::Align2,
    text: &str,
    font: egui::FontId,
    color: egui::Color32,
    max_w: f32,
) -> (egui::Rect, bool) {
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_owned(), font, color);
    job.wrap = egui::text::TextWrapping::truncate_at_width(max_w.max(1.0));
    let g = p.layout_job(job);
    let r = align.anchor_size(pos, g.size());
    let cut = g.elided;
    p.galley(r.min, g, color);
    (r, cut)
}

/// egui's text styles from the type scale (plus the named Tiny, Label and
/// Lcd styles), so `.small()`, buttons and headings follow it too
pub fn text_styles() -> std::collections::BTreeMap<egui::TextStyle, egui::FontId> {
    use egui::{FontFamily as F, FontId, TextStyle as T};
    let lab = || F::Name("label".into());
    [
        (T::Small, prop(ts::SMALL)),
        (T::Body, prop(ts::BODY)),
        (T::Button, prop(ts::BODY)),
        (T::Heading, FontId::new(ts::px(ts::HEADING), lab())),
        (T::Monospace, mono(ts::MONO)),
        (T::Name("tiny".into()), prop(ts::TINY)),
        (
            T::Name("label".into()),
            FontId::new(ts::px(ts::LABEL), lab()),
        ),
        (T::Name("lcd".into()), num_font(ts::LCD)),
    ]
    .into()
}

/// The label family (Atkinson Hyperlegible Next SemiBold: knob names,
/// captions, headings, buttons) and the numeric readout family (Atkinson
/// Hyperlegible Mono SemiBold: LCD, values). Registered in every theme.
pub fn label_font(size: f32) -> egui::FontId {
    egui::FontId::new(ts::px(size), egui::FontFamily::Name("label".into()))
}
pub fn num_font(size: f32) -> egui::FontId {
    egui::FontId::new(ts::px(size), egui::FontFamily::Name("num".into()))
}

/// Embedded fonts (SIL OFL 1.1, LICENSES/*-OFL.txt, credited in About and
/// THIRD_PARTY_LICENSES.txt), the same in every theme (docs/UI_SCALING.md):
/// - text: Atkinson Hyperlegible Next Regular, labels: its SemiBold;
/// - monospace (tracker, mono text): Atkinson Hyperlegible Mono Medium,
///   numbers / LCD: its SemiBold.
///
/// Fallbacks for glyphs the Atkinson fonts lack: Inter (arrows, ▶ ■ ● ✓ ♥,
/// Greek and Cyrillic in song / instrument names), then egui's Hack (▾) and
/// emoji fonts. egui's Ubuntu-Light is not used.
fn fonts(ctx: &egui::Context) {
    let id = egui::Id::NULL.with("chipwave_fonts_atkinson");
    if ctx.data(|d| d.get_temp::<bool>(id)).is_some() {
        return;
    }
    ctx.data_mut(|d| d.insert_temp(id, true));
    ctx.set_fonts(font_definitions());
}

/// the font set [`fonts`] installs (also used by tests)
pub fn font_definitions() -> egui::FontDefinitions {
    use egui::{FontData, FontFamily};
    use std::sync::Arc;
    let mut f = egui::FontDefinitions::default();
    // the emoji / icon fallbacks egui ships (everything but its text fonts)
    let emoji: Vec<String> = f
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|k| k != "Ubuntu-Light" && k != "Hack")
        .collect();
    f.font_data.remove("Ubuntu-Light");
    let mut add = |k: &str, b: &'static [u8]| {
        f.font_data
            .insert(k.into(), Arc::new(FontData::from_static(b)));
    };
    add(
        "atkinson",
        include_bytes!("../assets/fonts/AtkinsonHyperlegibleNext-Regular.ttf"),
    );
    add(
        "atkinson-sb",
        include_bytes!("../assets/fonts/AtkinsonHyperlegibleNext-SemiBold.ttf"),
    );
    add(
        "atkinson-mono",
        include_bytes!("../assets/fonts/AtkinsonHyperlegibleMono-Medium.ttf"),
    );
    add(
        "atkinson-mono-sb",
        include_bytes!("../assets/fonts/AtkinsonHyperlegibleMono-SemiBold.ttf"),
    );
    add("inter", include_bytes!("../assets/fonts/Inter-Regular.ttf"));
    let chain = |first: &str| {
        let mut v = vec![first.to_string(), "inter".into(), "Hack".into()];
        v.extend(emoji.iter().cloned());
        v
    };
    f.families
        .insert(FontFamily::Proportional, chain("atkinson"));
    f.families
        .insert(FontFamily::Name("label".into()), chain("atkinson-sb"));
    f.families
        .insert(FontFamily::Monospace, chain("atkinson-mono"));
    f.families
        .insert(FontFamily::Name("num".into()), chain("atkinson-mono-sb"));
    f
}

/// linear mix of two colours (opaque result), `k` = 0 → a, 1 → b
pub fn mix(a: Color32, b: Color32, k: f32) -> Color32 {
    let k = k.clamp(0.0, 1.0);
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * k).round() as u8;
    Color32::from_rgb(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()))
}

/// a colour a little lighter (`k` > 0) or darker (`k` < 0), keeping the hue
pub fn tone(c: Color32, k: f32) -> Color32 {
    if k >= 0.0 {
        mix(c, Color32::WHITE, k)
    } else {
        mix(c, Color32::BLACK, -k)
    }
}

/// A rounded rect with a vertical gradient, added to `mesh` (no extra
/// shapes, so thousands of clips stay one draw call). `rad` = (nw, ne, se,
/// sw) corner radii, clamped to the rect.
pub fn round_mesh(mesh: &mut egui::Mesh, r: Rect, rad: [f32; 4], top: Color32, bot: Color32) {
    if r.width() <= 0.0 || r.height() <= 0.0 {
        return;
    }
    let lim = (r.width().min(r.height()) * 0.5).max(0.0);
    let rad = rad.map(|x| x.clamp(0.0, lim));
    let col = |y: f32| {
        let k = ((y - r.top()) / r.height()).clamp(0.0, 1.0);
        let m = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * k).round() as u8;
        Color32::from_rgba_premultiplied(
            m(top.r(), bot.r()),
            m(top.g(), bot.g()),
            m(top.b(), bot.b()),
            m(top.a(), bot.a()),
        )
    };
    // outline: 4 corner arcs (3 segments each; a single point when square)
    let corners = [
        (
            Pos2::new(r.left() + rad[0], r.top() + rad[0]),
            rad[0],
            180.0f32,
        ),
        (
            Pos2::new(r.right() - rad[1], r.top() + rad[1]),
            rad[1],
            270.0,
        ),
        (
            Pos2::new(r.right() - rad[2], r.bottom() - rad[2]),
            rad[2],
            0.0,
        ),
        (
            Pos2::new(r.left() + rad[3], r.bottom() - rad[3]),
            rad[3],
            90.0,
        ),
    ];
    let base = mesh.vertices.len() as u32;
    let c = r.center();
    mesh.colored_vertex(c, col(c.y));
    let mut n = 0u32;
    for (o, rr, a0) in corners {
        let steps = if rr > 0.5 { 3 } else { 0 };
        for i in 0..=steps {
            let a = (a0 + 90.0 * i as f32 / steps.max(1) as f32).to_radians();
            let p = o + egui::vec2(a.cos(), a.sin()) * rr;
            mesh.colored_vertex(p, col(p.y));
            n += 1;
            if steps == 0 {
                break;
            }
        }
    }
    for i in 0..n {
        mesh.add_triangle(base, base + 1 + i, base + 1 + (i + 1) % n);
    }
}

/// Polished: a slight raised look on a button-like rect (highlight under the
/// top edge, shade over the bottom edge); `pressed` sinks it instead.
pub fn raise(p: &egui::Painter, r: Rect, rad: f32, pressed: bool) {
    if !polished() || r.width() < 2.0 * rad + 2.0 {
        return;
    }
    if th().ember && !pressed {
        crate::ember::bevel(p, r, rad);
        return;
    }
    let x = (r.left() + rad)..=(r.right() - rad);
    if pressed {
        p.hline(
            x,
            r.top() + 1.5,
            Stroke::new(1.0, Color32::from_black_alpha(60)),
        );
    } else {
        p.hline(
            x.clone(),
            r.top() + 1.5,
            Stroke::new(
                1.0,
                Color32::from_white_alpha(if th().light { 110 } else { 26 }),
            ),
        );
        p.hline(
            x,
            r.bottom() - 1.5,
            Stroke::new(1.0, Color32::from_black_alpha(45)),
        );
    }
}

/// a soft glow around `r` (selected items): a few fading outlines
pub fn glow(p: &egui::Painter, r: Rect, rad: u8, c: Color32) {
    for (i, a) in [0.45f32, 0.22, 0.1].iter().enumerate() {
        let e = 1.0 + i as f32;
        p.rect_stroke(
            r.expand(e),
            rad.saturating_add(i as u8 + 1),
            Stroke::new(1.0, c.gamma_multiply(*a)),
            egui::StrokeKind::Outside,
        );
    }
}

/// An arranger block / clip body in the Polished style: ~3 px corners, a
/// faint top highlight fading to a soft bottom shade, a 1 px darker border.
pub fn block(p: &egui::Painter, r: Rect, col: Color32) {
    let mut m = egui::Mesh::default();
    let rad = [3.0; 4];
    round_mesh(&mut m, r, rad, tone(col, 0.10), tone(col, -0.12));
    p.add(egui::Shape::mesh(m));
    // a 1 px highlight just under the top edge
    p.hline(
        (r.left() + 3.0)..=(r.right() - 3.0),
        r.top() + 1.0,
        Stroke::new(1.0, Color32::from_white_alpha(28)),
    );
    p.rect_stroke(
        r,
        3,
        Stroke::new(1.0, tone(col, -0.38)),
        egui::StrokeKind::Inside,
    );
}

/// a level meter's lit part: green → amber → red over the dB scale
/// (`frac` = lit height 0..1 of `r`, scale -60..+6 dB), plus the track
pub fn meter(p: &egui::Painter, r: Rect, frac: f32) {
    let t = th();
    if t.ember {
        // Slate: the LED dot column (vertical when taller than wide)
        let vertical = r.height() >= r.width();
        crate::ember::dot_meter(p, r, frac, None, vertical, true);
        return;
    }
    p.rect_filled(r, 3, tone(t.crt_bot, -0.15));
    p.rect_stroke(
        r,
        3,
        Stroke::new(1.0, Color32::from_black_alpha(70)),
        egui::StrokeKind::Inside,
    );
    if frac <= 0.0 {
        return;
    }
    let y = |db: f32| r.bottom() - ((db + 60.0) / 66.0).clamp(0.0, 1.0) * r.height();
    let top = r.bottom() - frac.clamp(0.0, 1.0) * r.height();
    let stops = [
        (-60.0, hex(0x3E8E57)),
        (-18.0, hex(0x5CC07A)),
        (-6.0, hex(0xA9C95E)),
        (-3.0, hex(0xE8C35A)),
        (0.0, hex(0xE39A55)),
        (6.0, hex(0xD9605A)),
    ];
    let mut m = egui::Mesh::default();
    let ins = r.shrink(1.0);
    for w in stops.windows(2) {
        let (y0, y1) = (y(w[0].0), y(w[1].0));
        let (lo, hi) = (y0.max(y1), y0.min(y1)); // lo = lower edge (bigger y)
        if lo <= top {
            continue;
        }
        let cut = hi.max(top);
        let k = |yy: f32| ((lo - yy) / (lo - hi).max(0.5)).clamp(0.0, 1.0);
        let seg = Rect::from_min_max(Pos2::new(ins.left(), cut), Pos2::new(ins.right(), lo));
        let i = m.vertices.len() as u32;
        let ct = mix(w[0].1, w[1].1, k(cut));
        m.colored_vertex(seg.left_top(), ct);
        m.colored_vertex(seg.right_top(), ct);
        m.colored_vertex(seg.right_bottom(), w[0].1);
        m.colored_vertex(seg.left_bottom(), w[0].1);
        m.add_triangle(i, i + 1, i + 2);
        m.add_triangle(i, i + 2, i + 3);
    }
    p.add(egui::Shape::mesh(m));
    // a soft sheen along the left edge
    p.vline(
        ins.left() + 1.0,
        top..=ins.bottom(),
        Stroke::new(1.0, Color32::from_white_alpha(40)),
    );
}

/// Vertical gradient rect (egui has no gradient fill: mesh with per-vertex colours).
pub fn gradient(painter: &egui::Painter, r: Rect, top: Color32, bot: Color32) {
    let mut m = egui::Mesh::default();
    m.colored_vertex(r.left_top(), top);
    m.colored_vertex(r.right_top(), top);
    m.colored_vertex(r.right_bottom(), bot);
    m.colored_vertex(Pos2::new(r.left(), r.bottom()), bot);
    m.add_triangle(0, 1, 2);
    m.add_triangle(0, 2, 3);
    painter.add(egui::Shape::mesh(m));
}

/// Rounded gradient "cartridge" with a muted spine; approximated with a
/// gradient mesh clipped by painting rounded corners in the background colour is
/// overkill for an MVP, so we draw a flat rounded base + inner gradient band.
pub fn cartridge(painter: &egui::Painter, r: Rect, spine: Color32) {
    let t = th();
    if t.ember {
        crate::ember::panel(painter, r, 7);
        return;
    }
    if polished() {
        painter.add(
            egui::Shadow {
                offset: [0, 2],
                blur: 10,
                spread: 0,
                color: Color32::from_black_alpha(if t.light { 45 } else { 90 }),
            }
            .as_shape(r, CornerRadius::same(8)),
        );
    }
    painter.rect_filled(r, CornerRadius::same(8), t.cart_bot);
    let inner = r.shrink2(egui::vec2(0.0, 6.0));
    gradient(
        painter,
        Rect::from_min_max(Pos2::new(r.left() + 6.0, inner.top()), inner.right_bottom()),
        t.cart_top,
        t.cart_bot,
    );
    painter.rect_filled(
        Rect::from_min_max(r.left_top(), Pos2::new(r.right(), r.top() + 8.0)),
        CornerRadius {
            nw: 8,
            ne: 8,
            sw: 0,
            se: 0,
        },
        t.cart_top,
    );
    painter.rect_filled(
        Rect::from_min_max(r.left_top(), Pos2::new(r.left() + 6.0, r.bottom())),
        CornerRadius {
            nw: 8,
            ne: 0,
            sw: 8,
            se: 0,
        },
        spine,
    );
    if polished() {
        // depth under the toolbar: a soft shade just inside the top edge,
        // and a faint highlight on the top border
        gradient(
            painter,
            Rect::from_min_max(
                Pos2::new(r.left() + 7.0, r.top() + 1.0),
                Pos2::new(r.right() - 1.0, r.top() + 7.0),
            ),
            Color32::from_black_alpha(if t.light { 22 } else { 40 }),
            Color32::TRANSPARENT,
        );
        painter.hline(
            (r.left() + 8.0)..=(r.right() - 8.0),
            r.top() + 0.5,
            Stroke::new(
                1.0,
                Color32::from_white_alpha(if t.light { 60 } else { 18 }),
            ),
        );
    }
    painter.rect_stroke(
        r,
        CornerRadius::same(8),
        Stroke::new(1.0, t.border),
        egui::StrokeKind::Inside,
    );
}

/// relative luminance (Rec. 709 weights on the 0..1 sRGB values)
pub fn luma(c: Color32) -> f32 {
    (0.2126 * c.r() as f32 + 0.7152 * c.g() as f32 + 0.0722 * c.b() as f32) / 255.0
}

/// An overlay that moves the pattern background's luminance by `amount`
/// (0.05 = 5 %): the theme's own light ink on dark backgrounds, its dark ink
/// on light ones (and on the Workbench blue, which reads as "light theme
/// darker"). Theme-derived, so a new theme gets a matching stripe for free.
pub fn shade(t: &Theme, amount: f32) -> Color32 {
    let bg = mid(t.crt_top, t.crt_bot);
    let lighten = !t.light;
    let ink = if lighten {
        if luma(t.note) > luma(t.ink) {
            t.note
        } else {
            t.ink
        }
    } else {
        // the theme's dark ink taken well below the background, so a light
        // wash of it darkens without shifting the hue
        let k = |c: u8| (c as f32 * 0.3) as u8;
        Color32::from_rgb(k(t.ink.r()), k(t.ink.g()), k(t.ink.b()))
    };
    let gap = (luma(ink) - luma(bg)).abs().max(0.1);
    let a = (amount / gap).clamp(0.0, 1.0);
    Color32::from_rgba_unmultiplied(ink.r(), ink.g(), ink.b(), (a * 255.0).round() as u8)
}

fn mid(a: Color32, b: Color32) -> Color32 {
    let m = |x: u8, y: u8| ((x as u16 + y as u16) / 2) as u8;
    Color32::from_rgb(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()))
}

pub fn crt(painter: &egui::Painter, r: Rect) {
    let t = th();
    if t.ember {
        crate::ember::well(painter, r, 5);
        return;
    }
    painter.rect_filled(r, CornerRadius::same(6), t.crt_bot);
    gradient(
        painter,
        r.shrink2(egui::vec2(0.0, 6.0)),
        t.crt_top,
        t.crt_bot,
    );
    painter.rect_filled(
        Rect::from_min_max(r.left_top(), Pos2::new(r.right(), r.top() + 7.0)),
        CornerRadius {
            nw: 6,
            ne: 6,
            sw: 0,
            se: 0,
        },
        t.crt_top,
    );
    painter.rect_stroke(
        r,
        CornerRadius::same(6),
        Stroke::new(1.0, t.ink2),
        egui::StrokeKind::Inside,
    );
}

/// accent colour choices (settings > look); "" = theme default
pub const ACCENTS: [(&str, u32); 5] = [
    ("Green", 0x5CC07A),
    ("Cyan", 0x4FC3D9),
    ("Amber", 0xE8B04A),
    ("Magenta", 0xD07ACB),
    ("Blue", 0x6E9BF0),
];

/// the theme with an optional accent override
pub fn with_accent(mut t: Theme, accent: &str) -> Theme {
    if let Some((_, c)) = ACCENTS.iter().find(|a| a.0 == accent) {
        t.accent = hex(*c);
    }
    t
}

#[cfg(test)]
mod shade_tests {
    use super::*;

    fn over(bg: Color32, o: Color32) -> Color32 {
        let a = o.a() as f32 / 255.0;
        // Color32 is premultiplied: out = bg * (1 - a) + c
        let f = |b: u8, c: u8| (b as f32 * (1.0 - a) + c as f32).round() as u8;
        Color32::from_rgb(f(bg.r(), o.r()), f(bg.g(), o.g()), f(bg.b(), o.b()))
    }

    #[test]
    fn column_stripe_is_4_to_6_percent_lighter_on_dark_and_darker_on_workbench() {
        for t in THEMES {
            let bg = mid(t.crt_top, t.crt_bot);
            let d = luma(over(bg, shade(&t, 0.05))) - luma(bg);
            assert!((0.04..=0.06).contains(&d.abs()), "{}: {d}", t.name);
            assert!(
                shade(&t, 0.05).a() < 128,
                "{}: a light wash, not a fill",
                t.name
            );
            assert_eq!(
                d > 0.0,
                !t.light,
                "{}: lighter on dark, darker on light",
                t.name
            );
        }
    }
}

// ------------------------------------------------------------ knob painter

/// What a knob shows; shared by every rotary control (instrument and device
/// knobs, generator macros, the mixer's send / pan knobs) so the themes stay
/// in one place. Slate paints the studio knob (`ember::knob_studio`), the
/// classic themes the shaded (Polished) or flat cap with a value arc.
#[derive(Clone, Copy)]
pub struct KnobFace {
    /// position 0..1
    pub frac: f32,
    /// where the value arc starts (0.5 = bipolar, lit from 12 o'clock)
    pub from: f32,
    /// opacity of the value parts (disabled controls dim)
    pub dim: f32,
    /// a locked value draws its arc in ink instead of the accent
    pub locked: bool,
    /// discrete steps over the range (dots around the knob for 1..=16)
    pub steps: f32,
}

impl KnobFace {
    pub fn new(frac: f32) -> Self {
        Self {
            frac,
            from: 0.0,
            dim: 1.0,
            locked: false,
            steps: 0.0,
        }
    }
}

/// Paint a knob of radius `rad` (the value arc's radius) centred on `c`.
pub fn knob_face(p: &egui::Painter, c: Pos2, rad: f32, k: KnobFace) {
    let t = th();
    let v = k.frac.clamp(0.0, 1.0);
    if t.ember {
        // the studio knob in the old footprint; stepped knobs get a dot per step
        let steps = k.steps.round() as usize;
        crate::ember::knob_studio(
            p,
            c,
            crate::ember::studio_rad(rad + 2.5),
            &crate::ember::StudioKnob {
                frac: v,
                bipolar: (k.from - 0.5).abs() < 0.01,
                dots: if (1..=16).contains(&steps) {
                    steps + 1
                } else {
                    0
                },
                dim: k.dim.max(0.5),
                locked: k.locked,
                ..Default::default()
            },
        );
        return;
    }
    let dim = k.dim;
    let a0 = std::f32::consts::PI * 0.75;
    let span = std::f32::consts::PI * 1.5;
    let pt = |ang: f32, r: f32| c + egui::vec2(ang.cos(), ang.sin()) * r;
    let arc = |from: f32, to: f32, col: Color32, w: f32| {
        let n = 28;
        let pts: Vec<Pos2> = (0..=n)
            .map(|i| pt(a0 + span * (from + (to - from) * i as f32 / n as f32), rad))
            .collect();
        p.add(egui::Shape::line(pts, Stroke::new(w, col)));
    };
    if polished() {
        // a soft shadow under the cap, the cap lit from above, a recessed track
        p.circle_filled(
            c + egui::vec2(0.0, 1.5),
            rad - 2.5,
            Color32::from_black_alpha(if t.light { 40 } else { 70 }),
        );
        let cap = t.pill.gamma_multiply(dim.max(0.7));
        p.circle_filled(c, rad - 3.0, tone(cap, -0.06));
        p.circle_filled(
            c - egui::vec2(0.0, 1.0),
            rad - 4.5,
            tone(cap, if t.light { 0.25 } else { 0.07 }),
        );
        p.circle_stroke(c, rad - 3.0, Stroke::new(1.0, t.ink2.gamma_multiply(0.35)));
        arc(
            0.0,
            1.0,
            Color32::from_black_alpha(if t.light { 45 } else { 90 }),
            4.5,
        );
    } else {
        p.circle_filled(c, rad - 3.0, t.pill.gamma_multiply(dim.max(0.7)));
        p.circle_stroke(c, rad - 3.0, Stroke::new(1.0, t.ink2.gamma_multiply(0.35)));
    }
    // stepped knobs (octave, voices...): a dot per step
    if k.steps >= 1.0 && k.steps <= 16.0 {
        let n = k.steps.round() as i32;
        for s in 0..=n {
            let f = s as f32 / n as f32;
            p.circle_filled(
                pt(a0 + span * f, rad + 3.5),
                1.0,
                t.ink2.gamma_multiply(0.6 * dim),
            );
        }
    }
    arc(0.0, 1.0, t.ink2.gamma_multiply(0.35), 3.0);
    let col = if k.locked { t.ink2 } else { skin_or(t.accent) };
    let (lo, hi) = if v < k.from { (v, k.from) } else { (k.from, v) };
    if hi > lo {
        arc(lo, hi, col.gamma_multiply(dim), 3.0);
    }
    p.line_segment(
        [pt(a0 + span * v, 3.0), pt(a0 + span * v, rad - 4.0)],
        Stroke::new(2.0, t.ink.gamma_multiply(dim)),
    );
}

/// A big knob with its title and value (master knobs, instrument macros).
/// Slate paints the studio knob (`ember::knob_studio`, `rad` = the knob's
/// radius, dots and labels outside it); the classic themes the arc knob with
/// the title above and the value under it.
pub fn knob_hero(p: &egui::Painter, c: Pos2, rad: f32, k: &crate::ember::StudioKnob) {
    if th().ember {
        crate::ember::knob_studio(p, c, rad, k);
        return;
    }
    let mut f = KnobFace::new(k.frac);
    f.from = if k.bipolar { 0.5 } else { 0.0 };
    f.dim = k.dim;
    f.locked = k.locked;
    knob_face(p, c, rad * 0.8, f);
    if !k.title.is_empty() {
        p.text(
            c - egui::vec2(0.0, rad * 0.8 + 8.0),
            egui::Align2::CENTER_CENTER,
            k.title.to_uppercase(),
            prop(ts::SMALL),
            th().ink.gamma_multiply(k.dim),
        );
    }
    p.text(
        c + egui::vec2(0.0, rad * 0.8 + 8.0),
        egui::Align2::CENTER_CENTER,
        k.value,
        mono(ts::TINY),
        th().value.gamma_multiply(k.dim),
    );
}

/// A level meter with an optional peak-hold mark (`peak` 0..1). Slate draws
/// the LED dot column (vertical when taller than wide); the classic themes
/// the gradient bar (`meter`) with a peak line.
pub fn meter_peak(p: &egui::Painter, r: Rect, frac: f32, peak: Option<f32>) {
    if th().ember {
        let vertical = r.height() >= r.width();
        crate::ember::dot_meter(p, r, frac, peak, vertical, true);
        return;
    }
    if polished() {
        meter(p, r, frac);
    } else {
        // Flat: one solid colour by level (green, amber over -6 dB, red over 0)
        p.rect_filled(r, 3, th().crt_bot);
        if frac > 0.0 {
            let db = frac.clamp(0.0, 1.0) * 66.0 - 60.0;
            let col = if db > 0.0 {
                hex(0xD9605A)
            } else if db > -6.0 {
                hex(0xE8C35A)
            } else {
                hex(0x5CC07A)
            };
            let top = r.bottom() - frac.clamp(0.0, 1.0) * r.height();
            p.rect_filled(Rect::from_min_max(Pos2::new(r.left(), top), r.max), 3, col);
        }
    }
    if let Some(pk) = peak.filter(|k| *k > 0.0) {
        let y = r.bottom() - pk.clamp(0.0, 1.0) * r.height();
        p.hline(r.x_range(), y, Stroke::new(1.5, th().ink));
    }
}

#[cfg(test)]
mod type_tests {
    use super::*;

    #[test]
    fn every_type_scale_role_is_at_least_10_pt() {
        for (name, s) in [
            ("TINY", ts::TINY),
            ("SMALL", ts::SMALL),
            ("LABEL", ts::LABEL),
            ("COMPACT", ts::COMPACT),
            ("MONO", ts::MONO),
            ("BODY", ts::BODY),
            ("LCD", ts::LCD),
            ("LARGE", ts::LARGE),
            ("HEADING", ts::HEADING),
            ("TITLE", ts::TITLE),
            ("DISPLAY", ts::DISPLAY),
        ] {
            assert!(s >= 10.0, "{name} is {s} pt");
        }
        // a stray literal below the floor still renders at the floor
        assert!(ts::px(7.0) >= 10.0);
        let small = ts::px_at(ts::TINY, ts::TEXT_MIN);
        assert!(small >= 10.0, "TINY at the smallest Text size is {small}");
    }

    #[test]
    fn theme_text_reads_on_its_surfaces() {
        // the panels, cards and pills text sits on (the window backdrop
        // `bg_*` only shows between panels)
        for t in THEMES {
            for (what, bg) in [
                ("cart_top", t.cart_top),
                ("cart_bot", t.cart_bot),
                ("pill", t.pill),
            ] {
                let c = contrast(t.ink, bg);
                assert!(c >= 4.5, "{}: ink on {what} {c:.2}", t.name);
                let c = contrast(t.ink2, bg);
                assert!(c >= 4.5, "{}: ink2 on {what} {c:.2}", t.name);
            }
        }
    }

    #[test]
    fn ink_on_a_fill_follows_the_fill() {
        // the Ember faces are dark in every theme; Amiga's pale blues light
        for bg in [
            crate::ember::BG2,
            crate::ember::BG3,
            crate::ember::BG4,
            crate::ember::DISP0,
            crate::ember::DISP1,
            AMIGA.sel,
            AMIGA.pill,
            AMIGA.cart_top,
            hex(0xFFFFFF),
            hex(0x000000),
        ] {
            assert!(contrast(on(bg), bg) >= 4.5, "on({bg:?})");
            assert!(contrast(on2(bg), bg) >= 4.5, "on2({bg:?})");
        }
    }

    /// sizes come from the type scale: no raw FontId outside theme.rs (and
    /// the tests), so Text size and the 10 pt floor reach every label
    #[test]
    fn no_raw_font_ids_outside_the_theme() {
        let mut bad = Vec::new();
        let mut dirs = vec![std::path::PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src"
        ))];
        while let Some(d) = dirs.pop() {
            for e in std::fs::read_dir(&d).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    dirs.push(p);
                    continue;
                }
                let name = p.file_name().unwrap().to_string_lossy().to_string();
                if !name.ends_with(".rs") || name == "theme.rs" || name.ends_with("_tests.rs") {
                    continue;
                }
                let src = std::fs::read_to_string(&p).unwrap();
                for (i, l) in src.lines().enumerate() {
                    for pat in [
                        "FontId::proportional(",
                        "FontId::monospace(",
                        "FontId::new(",
                    ] {
                        if l.contains(pat) {
                            bad.push(format!("{}:{}: {}", p.display(), i + 1, l.trim()));
                        }
                    }
                }
            }
        }
        assert!(
            bad.is_empty(),
            "raw FontId (use theme::prop / mono / label_font / num_font):\n{}",
            bad.join("\n")
        );
    }
}
