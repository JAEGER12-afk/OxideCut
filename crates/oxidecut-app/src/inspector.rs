//! Panel kanan. Isinya berubah mengikuti apa yang sedang dipilih:
//! klip → efek dan audio, sambungan → transisi, teks → isi/ukuran/warna, audio → volume.

use eframe::egui::{
    self, Align2, Color32, FontId, Frame, Margin, Painter, Pos2, Rect, RichText, Sense, Slider,
    Stroke, StrokeKind, Ui,
};
use oxidecut_core::{AudioKind, Clip, ClipAudio, Effect, Id, Transition};

use crate::app::{merge_tag, App, Selection};
use crate::icons;
use crate::labels;
use crate::preview;
use crate::theme;
use crate::widgets;

const TILE_HEIGHT: f32 = 62.0;
const THUMB_HEIGHT: f32 = 36.0;

/// Pilihan warna teks.
const TEXT_COLORS: [[u8; 3]; 5] = [
    [255, 255, 255],
    [255, 216, 77],
    [255, 107, 107],
    [107, 214, 255],
    [20, 16, 14],
];

pub fn show(app: &mut App, ui: &mut Ui) {
    let frame = Frame::new()
        .fill(theme::PANEL)
        .inner_margin(Margin::same(12));

    egui::Panel::right("inspector")
        .exact_size(250.0)
        .resizable(false)
        .frame(frame)
        .show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| match app.selection {
                Selection::None => nothing_selected(ui),
                Selection::Clip(id) => clip_panel(app, ui, id),
                Selection::Junction(id) => junction_panel(app, ui, id),
                Selection::Text(id) => text_panel(app, ui, id),
                Selection::Audio(id) => audio_panel(app, ui, id),
            });
        });
}

fn nothing_selected(ui: &mut Ui) {
    ui.label(RichText::new("Belum ada yang dipilih").strong());
    widgets::hint(
        ui,
        "Klik klip, teks, potongan audio, atau tombol + di sambungan klip. Pengaturannya muncul di sini.",
    );
}

// ---------------------------------------------------------------- klip

fn clip_panel(app: &mut App, ui: &mut Ui, id: Id) {
    let Some(clip) = app.timeline().clip(id).cloned() else {
        return;
    };

    ui.label(RichText::new(&clip.name).size(15.0).strong());
    widgets::hint(ui, &labels::seconds(clip.len));
    ui.add_space(4.0);

    widgets::section_title(ui, "Efek");
    if let Some(effect) = effect_grid(ui, &clip) {
        app.set_effect(id, effect);
    }
    ui.add_space(4.0);

    widgets::section_title(ui, "Audio (opsional)");
    match clip.audio {
        ClipAudio::Attached => {
            let (changed, released) = volume_slider(ui, clip.volume);
            if let Some(volume) = changed {
                let _ = app
                    .editor
                    .edit_merged(merge_tag(id, 1), |tl| tl.set_clip_volume(id, volume));
            }
            if released {
                app.editor.end_merge();
            }

            let button = widgets::icon_button(icons::SPLIT, "Pisahkan audio");
            if widgets::wide_button(ui, button).clicked() {
                app.detach_audio(id);
            }
        }
        ClipAudio::Detached { group } => {
            widgets::hint(ui, "Suara klip ini sudah dipisah ke jalur Audio.");
            let piece = app
                .timeline()
                .audios()
                .iter()
                .find(|a| a.group == group)
                .map(|a| a.id);
            if let Some(audio) = piece {
                let button = widgets::icon_button(icons::MUSIC_NOTE, "Pilih audionya");
                if widgets::wide_button(ui, button).clicked() {
                    app.selection = Selection::Audio(audio);
                }
            }
        }
        ClipAudio::NoAudio => {
            widgets::hint(ui, "Video ini tidak punya suara.");
        }
        ClipAudio::Removed => {
            widgets::hint(ui, "Suara klip ini sudah dihapus.");
            let button = widgets::icon_button(icons::SPEAKER, "Pulihkan suara");
            if widgets::wide_button(ui, button).clicked() {
                app.restore_clip_audio(id);
            }
        }
    }
}

/// Ubin efek dua kolom. Mengembalikan efek yang diklik.
fn effect_grid(ui: &mut Ui, clip: &Clip) -> Option<Effect> {
    let mut picked = None;
    for row in Effect::ALL.chunks(2) {
        ui.horizontal(|ui| {
            for effect in row {
                let selected = clip.effect == *effect;
                let (response, thumb) = tile(ui, labels::effect(*effect), selected);

                // Contoh kecil: gambar klip ini dengan efek tersebut.
                let mut sample = clip.clone();
                sample.effect = *effect;
                preview::paint_scene(ui.painter(), thumb, thumb, &sample, 1.0);

                if response.clicked() {
                    picked = Some(*effect);
                }
            }
        });
    }
    picked
}

// ---------------------------------------------------------------- transisi

fn junction_panel(app: &mut App, ui: &mut Ui, id: Id) {
    let tl = app.timeline();
    let Some(index) = tl.clip_index(id) else {
        return;
    };
    let Some(next) = tl.clips().get(index + 1).cloned() else {
        return;
    };
    let clip = tl.clips()[index].clone();
    let current = clip.transition_out;

    ui.label(RichText::new("Transisi").size(15.0).strong());
    widgets::hint(ui, &format!("{} ke {}", clip.name, next.name));
    ui.add_space(4.0);

    let from_color = theme::tint_flat(clip.tint);
    let to_color = theme::tint_flat(next.tint);
    let mut picked = None;

    for row in Transition::ALL.chunks(2) {
        ui.horizontal(|ui| {
            for kind in row {
                let selected = current == Some(*kind);
                let (response, thumb) = tile(ui, labels::transition(*kind), selected);

                // Diam: tampil setengah jalan. Disentuh kursor: bergerak berulang.
                let progress = if response.hovered() {
                    ui.ctx().request_repaint();
                    ((app.now % 1.3) / 1.0).min(1.0) as f32
                } else {
                    0.5
                };
                paint_transition_thumb(ui.painter(), thumb, *kind, progress, from_color, to_color);

                if response.clicked() {
                    picked = Some(*kind);
                }
            }
        });
    }
    if let Some(kind) = picked {
        app.set_transition(id, Some(kind));
    }

    let remove = widgets::icon_button(icons::DISMISS, "Lepas transisi");
    let response = ui.add_enabled_ui(current.is_some(), |ui| widgets::wide_button(ui, remove));
    if response.inner.clicked() {
        app.set_transition(id, None);
    }
}

/// Contoh kecil sebuah transisi dengan dua warna polos.
fn paint_transition_thumb(
    painter: &Painter,
    rect: Rect,
    kind: Transition,
    progress: f32,
    from: Color32,
    to: Color32,
) {
    let painter = painter.with_clip_rect(rect);
    painter.rect_filled(rect, 0.0, from);
    match kind {
        Transition::Fade => {
            painter.rect_filled(rect, 0.0, to.gamma_multiply(progress));
        }
        Transition::Slide => {
            let shift = rect.width() * (1.0 - progress);
            painter.rect_filled(rect.translate(egui::vec2(shift, 0.0)), 0.0, to);
        }
        Transition::Zoom => {
            let small = Rect::from_center_size(rect.center(), rect.size() * progress);
            painter.rect_filled(small, 0.0, to);
        }
        Transition::Wipe => {
            let right = rect.left() + rect.width() * progress;
            let revealed = Rect::from_min_max(rect.min, Pos2::new(right, rect.bottom()));
            painter.rect_filled(revealed, 0.0, to);
        }
    }
}

// ---------------------------------------------------------------- teks

fn text_panel(app: &mut App, ui: &mut Ui, id: Id) {
    let Some(text) = app.timeline().text(id).cloned() else {
        return;
    };

    ui.label(RichText::new("Teks").size(15.0).strong());
    ui.add_space(4.0);

    widgets::section_title(ui, "Isi");
    let mut content = text.text.clone();
    let response = ui.add(egui::TextEdit::singleline(&mut content).desired_width(f32::INFINITY));
    if app.focus_text_box {
        // Teks baru: langsung fokus dan pilih semua isinya, supaya ketikan
        // pertama pengguna langsung mengganti tulisan contoh.
        response.request_focus();
        let all = egui::text::CCursorRange::two(
            egui::text::CCursor::new(0usize),
            egui::text::CCursor::new(content.chars().count()),
        );
        let mut state = egui::TextEdit::load_state(ui.ctx(), response.id).unwrap_or_default();
        state.cursor.set_char_range(Some(all));
        state.store(ui.ctx(), response.id);
        app.focus_text_box = false;
    }
    if response.changed() {
        let _ = app.editor.edit_merged(merge_tag(id, 2), |tl| {
            tl.text_mut(id)?.text = content.clone();
            Ok(())
        });
    }
    if response.lost_focus() {
        app.editor.end_merge();
    }

    widgets::section_title(ui, "Ukuran");
    let mut size = text.size as f32;
    let response = ui.add(Slider::new(&mut size, 24.0..=160.0).fixed_decimals(0));
    if response.changed() {
        let _ = app.editor.edit_merged(merge_tag(id, 3), |tl| {
            tl.text_mut(id)?.size = size as u16;
            Ok(())
        });
    }
    if response.drag_stopped() {
        app.editor.end_merge();
    }

    widgets::section_title(ui, "Warna");
    ui.horizontal(|ui| {
        for color in TEXT_COLORS {
            let (rect, response) = ui.allocate_exact_size(egui::vec2(26.0, 26.0), Sense::click());
            let fill = Color32::from_rgb(color[0], color[1], color[2]);
            let ring = if text.color == color {
                theme::ACCENT
            } else {
                theme::LINE
            };
            ui.painter()
                .circle(rect.center(), 11.0, fill, Stroke::new(2.0, ring));
            if response.clicked() {
                let _ = app.editor.edit(|tl| {
                    tl.text_mut(id)?.color = color;
                    Ok(())
                });
            }
        }
    });
}

// ---------------------------------------------------------------- audio

fn audio_panel(app: &mut App, ui: &mut Ui, id: Id) {
    let Some(audio) = app.timeline().audio(id).cloned() else {
        return;
    };

    let title = match audio.kind {
        AudioKind::Music => "Musik",
        AudioKind::Detached => "Audio terpisah",
    };
    ui.label(RichText::new(title).size(15.0).strong());
    ui.label(labels::audio_name(&audio));
    widgets::hint(
        ui,
        &format!(
            "Mulai di detik {}, panjang {}",
            labels::seconds_number(audio.start),
            labels::seconds(audio.len)
        ),
    );
    ui.add_space(4.0);

    widgets::section_title(ui, "Volume");
    let (changed, released) = volume_slider(ui, audio.volume);
    if let Some(volume) = changed {
        let _ = app
            .editor
            .edit_merged(merge_tag(id, 4), |tl| tl.set_audio_volume(id, volume));
    }
    if released {
        app.editor.end_merge();
    }

    widgets::hint(
        ui,
        "Seret potongan ini di timeline untuk memindahnya. Seret ke atas atau bawah untuk ganti lajur. Tombol panah kiri dan kanan menggesernya sedikit.",
    );

    let linked = audio.kind == AudioKind::Detached && app.timeline().group_has_clip(audio.group);
    if linked {
        let button = widgets::icon_button(icons::LINK, "Gabungkan lagi ke video");
        if widgets::wide_button(ui, button).clicked() {
            app.reattach_audio(id);
        }
    }
}

// ---------------------------------------------------------------- bersama

/// Slider volume 0..100. Mengembalikan (nilai baru kalau berubah, apakah slider baru dilepas).
fn volume_slider(ui: &mut Ui, volume: u8) -> (Option<u8>, bool) {
    let mut value = volume as f32;
    let response = ui.add(
        Slider::new(&mut value, 0.0..=100.0)
            .fixed_decimals(0)
            .suffix("%"),
    );
    let changed = if response.changed() {
        Some(value as u8)
    } else {
        None
    };
    (changed, response.drag_stopped())
}

/// Satu ubin pilihan: kotak contoh di atas, nama di bawah.
/// Mengembalikan respons klik dan persegi tempat contoh digambar oleh pemanggil.
fn tile(ui: &mut Ui, label: &str, selected: bool) -> (egui::Response, Rect) {
    let gap = ui.spacing().item_spacing.x;
    let width = (ui.max_rect().width() - gap) / 2.0;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, TILE_HEIGHT), Sense::click());
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);

    let stroke = if selected {
        Stroke::new(2.0, theme::ACCENT)
    } else if response.hovered() {
        Stroke::new(1.0, theme::MUTED)
    } else {
        Stroke::new(1.0, theme::LINE)
    };
    ui.painter().rect(
        rect,
        theme::RADIUS,
        theme::PANEL_2,
        stroke,
        StrokeKind::Inside,
    );

    let thumb = Rect::from_min_size(
        rect.min + egui::vec2(5.0, 5.0),
        egui::vec2(rect.width() - 10.0, THUMB_HEIGHT),
    );
    ui.painter().text(
        Pos2::new(rect.left() + 6.0, rect.bottom() - 5.0),
        Align2::LEFT_BOTTOM,
        label,
        FontId::proportional(12.0),
        theme::TEXT,
    );
    (response, thumb)
}
