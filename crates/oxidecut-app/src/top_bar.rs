//! Bilah atas: nama aplikasi, nama proyek, tombol Ekspor, dan jendela Ekspor.

use eframe::egui::{self, Align, Align2, Frame, Layout, Margin, RichText, Ui};

use crate::app::App;
use crate::icons;
use crate::theme;
use crate::widgets;

pub fn show(app: &mut App, ui: &mut Ui) {
    let frame = Frame::new()
        .fill(theme::PANEL)
        .inner_margin(Margin::symmetric(12, 0));

    egui::Panel::top("top_bar")
        .exact_size(46.0)
        .frame(frame)
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                ui.label(RichText::new("OxideCut").size(15.0).strong());
                ui.label(
                    RichText::new("Proyek tanpa judul")
                        .size(12.0)
                        .color(theme::MUTED),
                );

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let button = widgets::primary_button(icons::ARROW_DOWNLOAD, "Ekspor");
                    if ui.add(button).clicked() {
                        app.export_open = !app.export_open;
                    }
                });
            });
        });
}

/// Jendela kecil di bawah tombol Ekspor.
///
/// CATATAN: ekspor video sungguhan belum dibuat (butuh FFmpeg).
/// Jendela ini sudah menunjukkan alurnya: pilih resolusi, lalu satu tombol.
pub fn show_export_window(app: &mut App, ctx: &egui::Context) {
    if !app.export_open {
        return;
    }

    egui::Window::new("Ekspor video")
        .title_bar(false)
        .resizable(false)
        .collapsible(false)
        .anchor(Align2::RIGHT_TOP, [-12.0, 52.0])
        .show(ctx, |ui| {
            ui.set_width(250.0);

            ui.horizontal(|ui| {
                widgets::section_title(ui, "Ekspor video");
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let close = egui::Button::new(icons::icon(icons::DISMISS, 14.0)).frame(false);
                    if ui.add(close).on_hover_text("Tutup").clicked() {
                        app.export_open = false;
                    }
                });
            });

            ui.horizontal(|ui| {
                for (height, label) in [(720, "720p"), (1080, "1080p"), (2160, "4K")] {
                    ui.selectable_value(&mut app.export_height, height, label);
                }
            });

            let start = widgets::primary_button(icons::ARROW_DOWNLOAD, "Mulai ekspor");
            ui.add_enabled_ui(false, |ui| widgets::wide_button(ui, start));

            widgets::hint(
                ui,
                "Ekspor belum tersedia di versi awal ini. Nanti hasilnya MP4 tanpa watermark.",
            );
        });
}
