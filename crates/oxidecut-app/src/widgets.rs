//! Potongan tampilan kecil yang dipakai di banyak panel.

use eframe::egui::{self, Button, Color32, RichText, Ui};

use crate::icons;
use crate::theme;

/// Judul bagian kecil berhuruf besar, misalnya "MEDIA" atau "EFEK".
pub fn section_title(ui: &mut Ui, text: &str) {
    ui.label(
        RichText::new(text.to_uppercase())
            .size(11.0)
            .strong()
            .color(theme::MUTED),
    );
}

/// Teks keterangan berwarna redup.
pub fn hint(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).size(12.0).color(theme::MUTED));
}

/// Tombol dengan ikon Fluent di kiri dan tulisan di kanan.
pub fn icon_button<'a>(icon: char, label: &'a str) -> Button<'a> {
    Button::new((icons::icon(icon, 16.0), label)).min_size(egui::vec2(0.0, 30.0))
}

/// Tombol yang hanya berisi ikon.
pub fn icon_only_button(icon: char) -> Button<'static> {
    Button::new(icons::icon(icon, 16.0)).min_size(egui::vec2(30.0, 30.0))
}

/// Tombol utama berwarna aksen (dipakai untuk Ekspor).
pub fn primary_button<'a>(icon: char, label: &'a str) -> Button<'a> {
    Button::new((
        icons::icon(icon, 16.0).color(theme::ACCENT_INK),
        RichText::new(label).strong().color(theme::ACCENT_INK),
    ))
    .fill(theme::ACCENT)
    .min_size(egui::vec2(0.0, 30.0))
}

/// Tombol selebar panel.
pub fn wide_button(ui: &mut Ui, button: Button<'_>) -> egui::Response {
    ui.add_sized([ui.available_width(), 32.0], button)
}

/// Kotak kecil berisi tulisan di atas gambar, misalnya nama klip di pratinjau.
pub fn paint_chip(
    painter: &egui::Painter,
    pos: egui::Pos2,
    anchor: egui::Align2,
    text: &str,
    fill: Color32,
    ink: Color32,
) {
    let galley = painter.layout_no_wrap(text.to_owned(), egui::FontId::proportional(11.0), ink);
    let size = galley.size() + egui::vec2(12.0, 6.0);
    let rect = anchor.anchor_size(pos, size);
    painter.rect_filled(rect, 4.0, fill);
    painter.galley(rect.min + egui::vec2(6.0, 3.0), galley, ink);
}
