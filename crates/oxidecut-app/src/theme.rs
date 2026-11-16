//! Warna dan gaya tampilan. Semua warna aplikasi didefinisikan di sini.

use eframe::egui::{self, Color32, CornerRadius, Stroke};

pub const BG_DEEP: Color32 = Color32::from_rgb(0x0f, 0x0c, 0x0a);
pub const PANEL: Color32 = Color32::from_rgb(0x1d, 0x18, 0x15);
pub const PANEL_2: Color32 = Color32::from_rgb(0x27, 0x20, 0x19);
pub const PANEL_3: Color32 = Color32::from_rgb(0x33, 0x2a, 0x22);
pub const LINE: Color32 = Color32::from_rgb(0x3a, 0x31, 0x2a);
pub const TEXT: Color32 = Color32::from_rgb(0xef, 0xe7, 0xdf);
pub const MUTED: Color32 = Color32::from_rgb(0xa0, 0x93, 0x86);
pub const ACCENT: Color32 = Color32::from_rgb(0xe0, 0x70, 0x3a);
pub const ACCENT_INK: Color32 = Color32::from_rgb(0x1a, 0x0e, 0x07);

pub const TEXT_ITEM: Color32 = Color32::from_rgb(0x5c, 0x37, 0x80);
pub const MUSIC_ITEM: Color32 = Color32::from_rgb(0x2e, 0x60, 0x47);
pub const DETACHED_ITEM: Color32 = Color32::from_rgb(0x2a, 0x66, 0x6f);

pub const RADIUS: u8 = 6;

/// Warna klip sementara, dipakai sampai pratinjau video asli tersedia.
const TINT_HUES: [f32; 6] = [205.0, 335.0, 150.0, 45.0, 270.0, 12.0];

pub fn tint_hue(tint: u8) -> f32 {
    TINT_HUES[tint as usize % TINT_HUES.len()]
}

/// Warna datar untuk klip di timeline dan ubin media.
pub fn tint_flat(tint: u8) -> Color32 {
    hsl(tint_hue(tint), 0.50, 0.32)
}

/// Warna atas dan bawah untuk gambar contoh di pratinjau.
pub fn tint_gradient(tint: u8) -> (Color32, Color32) {
    let hue = tint_hue(tint);
    (hsl(hue, 0.55, 0.42), hsl(hue + 40.0, 0.50, 0.18))
}

/// Mengubah warna HSL (hue 0..360, saturasi 0..1, terang 0..1) menjadi warna layar.
pub fn hsl(hue: f32, saturation: f32, lightness: f32) -> Color32 {
    let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let sector = hue.rem_euclid(360.0) / 60.0;
    let x = chroma * (1.0 - (sector % 2.0 - 1.0).abs());
    let (r, g, b) = match sector as u32 {
        0 => (chroma, x, 0.0),
        1 => (x, chroma, 0.0),
        2 => (0.0, chroma, x),
        3 => (0.0, x, chroma),
        4 => (x, 0.0, chroma),
        _ => (chroma, 0.0, x),
    };
    let m = lightness - chroma / 2.0;
    let to_byte = |v: f32| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgb(to_byte(r), to_byte(g), to_byte(b))
}

/// Memasang tema gelap OxideCut. Dipanggil sekali saat aplikasi dibuka.
pub fn install(ctx: &egui::Context) {
    // Selalu gelap, apa pun tema Windows pengguna.
    ctx.set_theme(egui::Theme::Dark);

    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = PANEL;
    visuals.window_fill = PANEL_2;
    visuals.window_stroke = Stroke::new(1.0, LINE);
    visuals.extreme_bg_color = BG_DEEP;
    visuals.faint_bg_color = PANEL_2;
    visuals.selection.bg_fill = ACCENT.gamma_multiply(0.45);
    visuals.selection.stroke = Stroke::new(1.0, ACCENT);
    visuals.window_corner_radius = CornerRadius::same(8);
    visuals.slider_trailing_fill = true;

    let radius = CornerRadius::same(RADIUS);

    let w = &mut visuals.widgets;
    w.noninteractive.bg_fill = PANEL;
    w.noninteractive.bg_stroke = Stroke::new(1.0, LINE);
    w.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
    w.noninteractive.corner_radius = radius;

    w.inactive.bg_fill = PANEL_2;
    w.inactive.weak_bg_fill = PANEL_2;
    w.inactive.bg_stroke = Stroke::new(1.0, LINE);
    w.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    w.inactive.corner_radius = radius;

    w.hovered.bg_fill = PANEL_3;
    w.hovered.weak_bg_fill = PANEL_3;
    w.hovered.bg_stroke = Stroke::new(1.0, MUTED);
    w.hovered.fg_stroke = Stroke::new(1.0, TEXT);
    w.hovered.corner_radius = radius;
    w.hovered.expansion = 0.0;

    w.active.bg_fill = PANEL_3;
    w.active.weak_bg_fill = PANEL_3;
    w.active.bg_stroke = Stroke::new(1.0, ACCENT);
    w.active.fg_stroke = Stroke::new(1.0, TEXT);
    w.active.corner_radius = radius;
    w.active.expansion = 0.0;

    w.open = w.active;

    ctx.set_visuals_of(egui::Theme::Dark, visuals);

    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        style.spacing.button_padding = egui::vec2(10.0, 6.0);
        style.spacing.slider_width = 150.0;
        for (text_style, font) in style.text_styles.iter_mut() {
            font.size = match text_style {
                egui::TextStyle::Small => 11.0,
                egui::TextStyle::Heading => 16.0,
                _ => 13.5,
            };
        }
    });
}
