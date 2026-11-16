//! Panel kiri: daftar media. Klik media untuk menaruhnya di timeline.

use eframe::egui::{self, Align2, FontId, Frame, Margin, Sense, Stroke, StrokeKind, Ui};

use crate::app::App;
use crate::icons;
use crate::labels;
use crate::media::MediaKind;
use crate::theme;
use crate::widgets;

const TILE_HEIGHT: f32 = 56.0;

pub fn show(app: &mut App, ui: &mut Ui) {
    let frame = Frame::new()
        .fill(theme::PANEL)
        .inner_margin(Margin::same(12));

    egui::Panel::left("media_panel")
        .exact_size(196.0)
        .resizable(false)
        .frame(frame)
        .show(ui, |ui| {
            widgets::section_title(ui, "Media");

            egui::ScrollArea::vertical().show(ui, |ui| {
                let clicked = media_grid(app, ui);
                if let Some(index) = clicked {
                    app.add_media_to_timeline(index);
                }

                let import = widgets::icon_button(icons::ARROW_IMPORT, "Impor");
                if widgets::wide_button(ui, import).clicked() {
                    app.import_files();
                }

                widgets::hint(ui, "Klik media untuk menaruhnya di timeline.");
            });
        });
}

/// Menggambar ubin media dua kolom. Mengembalikan indeks ubin yang diklik.
fn media_grid(app: &App, ui: &mut Ui) -> Option<usize> {
    let gap = 8.0;
    let tile_width = (ui.available_width() - gap) / 2.0;
    let mut clicked = None;

    for (row_index, row) in app.library.chunks(2).enumerate() {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for (column, item) in row.iter().enumerate() {
                let size = egui::vec2(tile_width, TILE_HEIGHT);
                let (rect, response) = ui.allocate_exact_size(size, Sense::click());

                let fill = match item.kind {
                    MediaKind::Video => theme::tint_flat(item.tint),
                    MediaKind::Audio => theme::MUSIC_ITEM,
                };
                let stroke = if response.hovered() {
                    Stroke::new(1.5, theme::ACCENT)
                } else {
                    Stroke::new(1.0, theme::LINE)
                };
                let painter = ui.painter_at(rect);
                painter.rect(rect, theme::RADIUS, fill, stroke, StrokeKind::Inside);

                let icon = match item.kind {
                    MediaKind::Video => icons::VIDEO_CLIP,
                    MediaKind::Audio => icons::MUSIC_NOTE,
                };
                painter.text(
                    rect.left_top() + egui::vec2(6.0, 5.0),
                    Align2::LEFT_TOP,
                    icon,
                    icons::font(14.0),
                    theme::TEXT,
                );
                painter.text(
                    rect.left_bottom() + egui::vec2(6.0, -17.0),
                    Align2::LEFT_BOTTOM,
                    &item.name,
                    FontId::proportional(11.0),
                    theme::TEXT,
                );
                let length = if item.len_known {
                    labels::seconds(item.len)
                } else {
                    format!("± {}", labels::seconds(item.len))
                };
                painter.text(
                    rect.left_bottom() + egui::vec2(6.0, -4.0),
                    Align2::LEFT_BOTTOM,
                    length,
                    FontId::proportional(10.0),
                    theme::TEXT.gamma_multiply(0.75),
                );

                let response = response
                    .on_hover_text(format!("Tambah {} ke timeline", item.name))
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                if response.clicked() {
                    clicked = Some(row_index * 2 + column);
                }
            }
        });
    }
    clicked
}
