//! Pratinjau di tengah layar, tombol putar, dan keterangan suara.
//!
//! Klip dari berkas video ditampilkan sebagai video asli (lihat `video.rs`).
//! Klip contoh bawaan, yang tidak punya berkas, digambar sebagai pemandangan
//! sederhana berwarna oleh `paint_scene`.

use eframe::egui::{
    self, Align, Align2, Color32, FontId, Frame, Layout, Margin, Painter, Pos2, Rect, RichText,
    Sense, Stroke, StrokeKind, Ui,
};
use oxidecut_core::{Audible, Clip, Effect, Ms, Timeline, Transition};

use crate::app::{App, Ratio};
use crate::ffmpeg;
use crate::icons;
use crate::labels;
use crate::theme;
use crate::video::{self, FrameRequest};
use crate::widgets;

/// Transisi berlangsung dari 300 ms sebelum sambungan sampai 300 ms sesudahnya.
pub const TRANSITION_HALF_MS: Ms = 300;

pub fn show(app: &mut App, ui: &mut Ui) {
    let frame = Frame::new()
        .fill(theme::BG_DEEP)
        .inner_margin(Margin::same(10));

    egui::CentralPanel::default().frame(frame).show(ui, |ui| {
        ratio_row(app, ui);

        // Sisakan ruang di bawah gambar untuk tombol putar dan keterangan suara.
        let below_picture = 74.0;
        let size = egui::vec2(
            ui.available_width(),
            (ui.available_height() - below_picture).max(80.0),
        );
        let (area, _) = ui.allocate_exact_size(size, Sense::hover());
        let picture = fit_rect(area, app.ratio.width_over_height());
        paint_picture(app, ui, picture);

        transport_row(app, ui);
        audible_row(app, ui);
        paint_toast(app, ui, area);
    });
}

/// Baris atas: keterangan di kiri, pilihan rasio di kanan.
fn ratio_row(app: &mut App, ui: &mut Ui) {
    ui.horizontal(|ui| {
        // Tanpa FFmpeg, video asli tidak bisa ditampilkan. Beri tahu pengguna.
        if ffmpeg::tools().is_none() {
            widgets::hint(
                ui,
                "FFmpeg tidak ditemukan. Video tampil sebagai gambar contoh.",
            );
        }

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            // Tata letak kanan-ke-kiri, jadi urutannya dibalik.
            for ratio in Ratio::ALL.iter().rev() {
                let text = RichText::new(ratio.label()).monospace().size(12.0);
                ui.selectable_value(&mut app.ratio, *ratio, text);
            }
        });
    });
}

/// Persegi terbesar dengan rasio `width_over_height` yang muat di tengah `area`.
fn fit_rect(area: Rect, width_over_height: f32) -> Rect {
    let mut height = area.height();
    let mut width = height * width_over_height;
    if width > area.width() {
        width = area.width();
        height = width / width_over_height;
    }
    Rect::from_center_size(area.center(), egui::vec2(width, height))
}

fn paint_picture(app: &mut App, ui: &Ui, picture: Rect) {
    let painter = ui.painter_at(picture);
    painter.rect_filled(picture, 0.0, Color32::BLACK);

    // Salinan isi timeline (kecil), supaya bebas memakai `app.video` sambil menggambar.
    let tl = app.timeline().clone();

    app.video.begin_paint();
    let (shown_name, shown_transition) = paint_video(app, ui, &painter, picture, &tl);
    app.video.end_paint();

    // Teks di atas video, disusun dari bawah ke atas.
    let mut bottom = picture.bottom() - picture.height() * 0.12;
    for text in tl.texts_at(app.playhead_ms()) {
        let size = (text.size as f32 * picture.height() / 1080.0).max(6.0);
        let color = Color32::from_rgb(text.color[0], text.color[1], text.color[2]);
        let galley = painter.layout(
            text.text.clone(),
            FontId::proportional(size),
            color,
            picture.width() * 0.9,
        );
        let pos = Pos2::new(
            picture.center().x - galley.size().x / 2.0,
            bottom - galley.size().y,
        );
        let shadow = Color32::from_black_alpha(150);
        painter.galley_with_override_text_color(pos + egui::vec2(1.0, 1.5), galley.clone(), shadow);
        painter.galley(pos, galley, color);
        bottom = pos.y - 4.0;
    }

    // Keterangan kecil di pojok.
    if !shown_name.is_empty() {
        widgets::paint_chip(
            &painter,
            picture.left_top() + egui::vec2(8.0, 8.0),
            Align2::LEFT_TOP,
            &shown_name,
            Color32::from_black_alpha(130),
            theme::TEXT,
        );
    }
    if let Some(kind) = shown_transition {
        widgets::paint_chip(
            &painter,
            picture.right_top() + egui::vec2(-8.0, 8.0),
            Align2::RIGHT_TOP,
            &format!("Transisi: {}", labels::transition(kind)),
            theme::ACCENT,
            theme::ACCENT_INK,
        );
    }

    painter.rect_stroke(
        picture,
        0.0,
        Stroke::new(1.0, theme::LINE),
        StrokeKind::Inside,
    );
}

/// Menggambar video pada posisi garis oranye: satu klip, atau dua klip saat transisi.
/// Mengembalikan nama klip yang tampil dan transisi yang sedang berlangsung.
fn paint_video(
    app: &mut App,
    ui: &Ui,
    painter: &Painter,
    picture: Rect,
    tl: &Timeline,
) -> (String, Option<Transition>) {
    let video_len = tl.video_len();
    if video_len == 0 {
        painter.text(
            picture.center(),
            Align2::CENTER_CENTER,
            "Timeline kosong",
            FontId::proportional(13.0),
            theme::MUTED,
        );
        return (String::new(), None);
    }

    // Di ujung video, tetap tampilkan gambar terakhir.
    let t = app.playhead_ms().min(video_len - 1);

    // Sedang di dalam transisi: gambar dua klip.
    if let Some((kind, edge, left_index)) = tl.transition_at(t, TRANSITION_HALF_MS) {
        if let Some(to_clip) = tl.clips().get(left_index + 1) {
            let from_clip = &tl.clips()[left_index];
            let from = Shot {
                clip: from_clip,
                start: edge - from_clip.len,
            };
            let to = Shot {
                clip: to_clip,
                start: edge,
            };

            let begin = edge.saturating_sub(TRANSITION_HALF_MS) as f32;
            let progress = ((t as f32 - begin) / (2 * TRANSITION_HALF_MS) as f32).clamp(0.0, 1.0);
            paint_transition(app, ui, painter, picture, &from, &to, t, kind, progress);

            let name = if progress < 0.5 {
                &from_clip.name
            } else {
                &to_clip.name
            };
            return (name.clone(), Some(kind));
        }
    }

    // Keadaan biasa: gambar satu klip.
    let Some((start, clip)) = tl.clip_at(t) else {
        return (String::new(), None);
    };
    let shot = Shot { clip, start };
    paint_clip(app, ui, painter, picture, picture, &shot, t, 1.0);

    // Saat diputar, siapkan klip berikutnya sedikit sebelum gilirannya,
    // supaya pergantian klip tidak tersendat.
    let remaining = start + clip.len - t;
    if app.playing && remaining < 1_500 {
        if let Some(index) = tl.clip_index(clip.id) {
            if let Some(next_clip) = tl.clips().get(index + 1) {
                let next = Shot {
                    clip: next_clip,
                    start: start + clip.len,
                };
                prepare_clip(app, ui, &next, t);
            }
        }
    }

    (clip.name.clone(), None)
}

/// Sebuah klip beserta waktu mulainya di timeline.
struct Shot<'a> {
    clip: &'a Clip,
    start: Ms,
}

impl Shot<'_> {
    /// Waktu di dalam berkas sumber untuk waktu timeline `t`.
    /// Sebelum klip mulai, hasilnya frame pertama; setelah klip habis, frame terakhir.
    fn source_ms(&self, t: Ms) -> Ms {
        let inside = t.saturating_sub(self.start);
        let last = self.clip.len.saturating_sub(1);
        self.clip.source_in + inside.min(last)
    }
}

/// Apakah klip ini bisa ditampilkan sebagai video asli?
/// Klip contoh (tanpa berkas) dan komputer tanpa FFmpeg memakai gambar contoh.
fn has_real_video(clip: &Clip) -> bool {
    clip.source.is_some() && ffmpeg::tools().is_some()
}

/// Permintaan frame untuk `shot` pada waktu timeline `t`.
fn frame_request<'a>(app: &App, shot: &'a Shot<'a>, t: Ms) -> Option<FrameRequest<'a>> {
    if !has_real_video(shot.clip) {
        return None;
    }
    let path = shot.clip.source.as_ref()?;
    Some(FrameRequest {
        clip_id: shot.clip.id,
        path,
        source_ms: shot.source_ms(t),
        effect: shot.clip.effect,
        size: video::frame_size(app.ratio),
    })
}

/// Menggambar satu klip pada waktu timeline `t`.
///
/// - `visible`: bagian layar yang boleh digambari.
/// - `frame`: posisi dan ukuran gambar utuh (boleh bergeser atau mengecil saat transisi).
/// - `opacity`: 1.0 penuh, 0.0 tembus pandang.
#[allow(clippy::too_many_arguments)]
fn paint_clip(
    app: &mut App,
    ui: &Ui,
    painter: &Painter,
    visible: Rect,
    frame: Rect,
    shot: &Shot<'_>,
    t: Ms,
    opacity: f32,
) {
    let Some(request) = frame_request(app, shot, t) else {
        paint_scene(painter, visible, frame, shot.clip, opacity);
        return;
    };

    // Selama frame pertama belum datang, layar dibiarkan hitam.
    if let Some(texture) = app.video.frame(ui.ctx(), request) {
        let whole_image = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0));
        let tint = Color32::WHITE.gamma_multiply(opacity);
        painter
            .with_clip_rect(visible)
            .image(texture, frame, whole_image, tint);
    }
}

/// Menyalakan pemutar untuk klip yang sebentar lagi tampil, tanpa menggambarnya.
fn prepare_clip(app: &mut App, ui: &Ui, shot: &Shot<'_>, t: Ms) {
    if let Some(request) = frame_request(app, shot, t) {
        app.video.frame(ui.ctx(), request);
    }
}

/// Menggambar pergantian dari klip `from` ke klip `to`.
/// `progress` berjalan dari 0.0 (masih `from`) ke 1.0 (sudah `to`).
#[allow(clippy::too_many_arguments)]
fn paint_transition(
    app: &mut App,
    ui: &Ui,
    painter: &Painter,
    picture: Rect,
    from: &Shot<'_>,
    to: &Shot<'_>,
    t: Ms,
    kind: Transition,
    progress: f32,
) {
    match kind {
        Transition::Fade => {
            paint_clip(app, ui, painter, picture, picture, from, t, 1.0);
            paint_clip(app, ui, painter, picture, picture, to, t, progress);
        }
        Transition::Slide => {
            let shift = picture.width() * progress;
            let from_frame = picture.translate(egui::vec2(-shift, 0.0));
            let to_frame = picture.translate(egui::vec2(picture.width() - shift, 0.0));
            paint_clip(app, ui, painter, picture, from_frame, from, t, 1.0);
            paint_clip(app, ui, painter, picture, to_frame, to, t, 1.0);
        }
        Transition::Zoom => {
            paint_clip(app, ui, painter, picture, picture, from, t, 1.0);
            let small = Rect::from_center_size(picture.center(), picture.size() * progress);
            paint_clip(app, ui, painter, small, small, to, t, 1.0);
        }
        Transition::Wipe => {
            paint_clip(app, ui, painter, picture, picture, from, t, 1.0);
            let right = picture.left() + picture.width() * progress;
            let revealed = Rect::from_min_max(picture.min, Pos2::new(right, picture.bottom()));
            paint_clip(app, ui, painter, revealed, picture, to, t, 1.0);
        }
    }
}

/// Menggambar klip contoh sebagai pemandangan sederhana.
///
/// - `visible`: bagian layar yang boleh digambari.
/// - `frame`: posisi dan ukuran gambar utuh (boleh lebih besar atau bergeser dari `visible`).
/// - `opacity`: 1.0 penuh, 0.0 tembus pandang.
pub fn paint_scene(painter: &Painter, visible: Rect, frame: Rect, clip: &Clip, opacity: f32) {
    let mut painter = painter.with_clip_rect(visible);
    painter.set_opacity(opacity);

    // Efek Zoom memperbesar gambar dari tengah.
    let frame = if clip.effect == Effect::Zoom {
        Rect::from_center_size(frame.center(), frame.size() * 1.35)
    } else {
        frame
    };

    let (sky_top, sky_bottom) = theme::tint_gradient(clip.tint);
    let sky_top = effect_color(sky_top, clip.effect);
    let sky_bottom = effect_color(sky_bottom, clip.effect);
    let sun = effect_color(Color32::from_rgb(255, 214, 140), clip.effect);
    let ground = effect_color(Color32::from_rgb(24, 20, 30), clip.effect);

    // Langit: gradasi dari atas ke bawah.
    let mut sky = egui::Mesh::default();
    sky.colored_vertex(frame.left_top(), sky_top);
    sky.colored_vertex(frame.right_top(), sky_top);
    sky.colored_vertex(frame.right_bottom(), sky_bottom);
    sky.colored_vertex(frame.left_bottom(), sky_bottom);
    sky.add_triangle(0, 1, 2);
    sky.add_triangle(0, 2, 3);
    painter.add(egui::Shape::mesh(sky));

    // Matahari.
    let unit = frame.width().min(frame.height());
    let sun_center = frame.left_top() + egui::vec2(frame.width() * 0.68, frame.height() * 0.30);
    let sun_radius = unit * 0.12;
    if clip.effect == Effect::Blur {
        // Blur ditiru dengan beberapa lingkaran tembus pandang yang makin besar.
        for ring in 0..6 {
            let radius = sun_radius * (0.7 + ring as f32 * 0.18);
            painter.circle_filled(sun_center, radius, sun.gamma_multiply(0.22));
        }
    } else {
        painter.circle_filled(sun_center, sun_radius, sun);
    }

    // Tanah.
    let ground_top = frame.top() + frame.height() * 0.78;
    let ground_rect = Rect::from_min_max(Pos2::new(frame.left(), ground_top), frame.right_bottom());
    painter.rect_filled(ground_rect, 0.0, ground);
    if clip.effect == Effect::Blur {
        let haze = Rect::from_min_max(
            Pos2::new(frame.left(), ground_top - unit * 0.04),
            Pos2::new(frame.right(), ground_top + unit * 0.04),
        );
        painter.rect_filled(haze, 0.0, ground.gamma_multiply(0.45));
    }
}

/// Mengubah warna sesuai efek. Efek yang tidak mengubah warna mengembalikan warna asli.
fn effect_color(color: Color32, effect: Effect) -> Color32 {
    let (r, g, b) = (color.r() as f32, color.g() as f32, color.b() as f32);
    let to_byte = |v: f32| v.round().clamp(0.0, 255.0) as u8;
    match effect {
        Effect::BlackWhite => {
            let gray = to_byte(0.299 * r + 0.587 * g + 0.114 * b);
            Color32::from_rgb(gray, gray, gray)
        }
        Effect::Warm => Color32::from_rgb(
            to_byte(r * 1.10 + 24.0),
            to_byte(g * 0.95 + 8.0),
            to_byte(b * 0.65),
        ),
        Effect::None | Effect::Blur | Effect::Zoom => color,
    }
}

/// Tombol ke awal, tombol putar, dan waktu.
fn transport_row(app: &mut App, ui: &mut Ui) {
    let size = egui::vec2(ui.available_width(), 36.0);
    let (row, _) = ui.allocate_exact_size(size, Sense::hover());
    let center = row.center();

    let back_rect = Rect::from_center_size(center + egui::vec2(-84.0, 0.0), egui::vec2(30.0, 30.0));
    let back = widgets::icon_only_button(icons::PREVIOUS);
    if ui.put(back_rect, back).on_hover_text("Ke awal").clicked() {
        app.playing = false;
        app.set_playhead(0.0);
    }

    let play_rect = Rect::from_center_size(center + egui::vec2(-46.0, 0.0), egui::vec2(34.0, 34.0));
    let play_icon = if app.playing {
        icons::PAUSE
    } else {
        icons::PLAY
    };
    let play = widgets::icon_only_button(play_icon);
    if ui
        .put(play_rect, play)
        .on_hover_text("Putar atau jeda (Spasi)")
        .clicked()
    {
        app.toggle_play();
    }

    let time = format!(
        "{} / {}",
        labels::clock(app.playhead_ms()),
        labels::clock(app.timeline().duration())
    );
    ui.painter().text(
        center + egui::vec2(-18.0, 0.0),
        Align2::LEFT_CENTER,
        time,
        FontId::monospace(13.0),
        theme::MUTED,
    );
}

/// Satu baris yang menyebut suara apa saja yang terdengar di garis oranye.
fn audible_row(app: &App, ui: &mut Ui) {
    let tl = app.timeline();
    let mut names: Vec<String> = Vec::new();
    for source in tl.audible_at(app.playhead_ms()) {
        match source {
            Audible::ClipSound(id) => {
                if let Some(clip) = tl.clip(id) {
                    names.push(format!("suara {}", clip.name));
                }
            }
            Audible::Audio(id) => {
                if let Some(audio) = tl.audio(id) {
                    names.push(labels::audio_name(audio));
                }
            }
        }
    }

    let (icon, text) = if names.is_empty() {
        (
            icons::SPEAKER_MUTE,
            "Tidak ada suara di titik ini".to_owned(),
        )
    } else {
        (icons::SPEAKER, format!("Terdengar: {}", names.join(", ")))
    };

    let size = egui::vec2(ui.available_width(), 20.0);
    let (row, _) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter_at(row);
    let galley = painter.layout_no_wrap(text, FontId::proportional(12.0), theme::MUTED);
    let icon_width = 20.0;
    let left = row.center().x - (icon_width + galley.size().x) / 2.0;

    painter.text(
        Pos2::new(left, row.center().y),
        Align2::LEFT_CENTER,
        icon,
        icons::font(14.0),
        theme::MUTED,
    );
    painter.galley(
        Pos2::new(left + icon_width, row.center().y - galley.size().y / 2.0),
        galley,
        theme::MUTED,
    );
}

/// Pesan singkat yang muncul sebentar di bagian bawah pratinjau.
fn paint_toast(app: &App, ui: &Ui, area: Rect) {
    if app.now >= app.toast_until || app.toast.is_empty() {
        return;
    }
    let layer = egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("toast"));
    let painter = ui.ctx().layer_painter(layer);
    let galley = painter.layout_no_wrap(app.toast.clone(), FontId::proportional(12.5), theme::TEXT);
    let size = galley.size() + egui::vec2(24.0, 12.0);
    let rect =
        Align2::CENTER_BOTTOM.anchor_size(area.center_bottom() - egui::vec2(0.0, 12.0), size);
    painter.rect(
        rect,
        theme::RADIUS,
        theme::PANEL_2,
        Stroke::new(1.0, theme::LINE),
        StrokeKind::Inside,
    );
    painter.galley(rect.min + egui::vec2(12.0, 6.0), galley, theme::TEXT);
}
