//! "Workbench Gradient" (A2) theme with the muted palette and rounded corners
//! (docs/DESIGN_LANGUAGE.md).
use eframe::egui::{self, Color32, CornerRadius, Pos2, Rect, Stroke};

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
}

/// Default: modern dark grey with thin muted-green borders.
pub const DARK: Theme = Theme {
    name: "Dark Circuit (default)",
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
    ink2: hex(0x9AA0A8),
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
    hdr_top: hex(0x3F7FE6),
    hdr_bot: hex(0x1747A8),
    crt_top: hex(0x173A72),
    crt_bot: hex(0x0A1F45),
    crt_grid: hex(0x24477F),
    ink: hex(0x1B2330),
    ink2: hex(0x3E4652),
    value: hex(0x1F5FD1),
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
};

pub const THEMES: [Theme; 2] = [DARK, AMIGA];

static CURRENT: std::sync::RwLock<Theme> = std::sync::RwLock::new(DARK);
pub fn th() -> Theme {
    *CURRENT.read().unwrap()
}
pub fn set(t: Theme) {
    *CURRENT.write().unwrap() = t;
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
    v.selection.bg_fill = t.slate;
    v.selection.stroke = Stroke::new(1.0, Color32::WHITE);
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
    ctx.set_visuals(v);
    ctx.global_style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(6.0, 5.0);
        s.spacing.button_padding = egui::vec2(9.0, 3.0);
    });
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
    painter.rect_stroke(
        r,
        CornerRadius::same(8),
        Stroke::new(1.0, t.border),
        egui::StrokeKind::Inside,
    );
}

pub fn crt(painter: &egui::Painter, r: Rect) {
    let t = th();
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
