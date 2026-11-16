//! Timeline di bagian bawah: bilah alat, penggaris waktu, jalur Teks, Video, dan Audio.
//!
//! Seluruh timeline selalu muat selebar layar (belum ada zoom).
//! Semua posisi dihitung lewat `Tracks`, jadi kalau nanti zoom ditambahkan,
//! cukup `Tracks` yang berubah.

use eframe::egui::{
    self, Align, Align2, Color32, CursorIcon, FontId, Frame, Layout, Margin, Painter, Pos2, Rect,
    Sense, Stroke, StrokeKind, Ui,
};
use oxidecut_core::{AudioItem, AudioKind, ClipAudio, Effect, Ms, Timeline};

use crate::app::{App, AudioDrag, Selection};
use crate::icons;
use crate::labels;
use crate::theme;
use crate::widgets;

const LABEL_WIDTH: f32 = 48.0;
const RULER_HEIGHT: f32 = 22.0;
const TEXT_ROW_HEIGHT: f32 = 26.0;
const VIDEO_ROW_HEIGHT: f32 = 50.0;
const LANE_HEIGHT: f32 = 22.0;
const ROW_GAP: f32 = 4.0;

/// Audio yang diseret menempel ke tepi terdekat dalam jarak ini (piksel).
const SNAP_PIXELS: f32 = 8.0;

pub fn show(app: &mut App, ui: &mut Ui) {
    let frame = Frame::new()
        .fill(theme::PANEL)
        .inner_margin(Margin::symmetric(12, 8));

    egui::Panel::bottom("timeline")
        .exact_size(214.0)
        .resizable(false)
        .frame(frame)
        .show(ui, |ui| {
            toolbar(app, ui);
            tracks(app, ui);
        });
}

fn toolbar(app: &mut App, ui: &mut Ui) {
    ui.horizontal(|ui| {
        let undo = widgets::icon_only_button(icons::ARROW_UNDO);
        if ui
            .add_enabled(app.editor.can_undo(), undo)
            .on_hover_text("Urungkan (Ctrl+Z)")
            .clicked()
        {
            app.undo();
        }

        let redo = widgets::icon_only_button(icons::ARROW_REDO);
        if ui
            .add_enabled(app.editor.can_redo(), redo)
            .on_hover_text("Ulangi (Ctrl+Y)")
            .clicked()
        {
            app.redo();
        }

        ui.add_space(8.0);

        let cut = widgets::icon_button(icons::CUT, "Potong");
        if ui
            .add(cut)
            .on_hover_text("Potong di garis oranye")
            .clicked()
        {
            app.cut();
        }

        let delete = widgets::icon_button(icons::DELETE, "Hapus");
        if ui
            .add_enabled(app.can_delete(), delete)
            .on_hover_text("Hapus yang dipilih (Delete)")
            .clicked()
        {
            app.delete_selected();
        }

        let text = widgets::icon_button(icons::TEXT, "Teks");
        if ui
            .add(text)
            .on_hover_text("Tambah teks di garis oranye")
            .clicked()
        {
            app.add_text();
        }

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            widgets::hint(ui, "Klik penggaris waktu untuk memindah garis oranye");
        });
    });
}

/// Posisi dan ukuran semua jalur pada frame ini.
struct Tracks {
    /// Tepi kiri area waktu (setelah kolom nama jalur).
    left: f32,
    /// Lebar area waktu dalam piksel.
    width: f32,
    /// Panjang waktu yang ditampilkan selebar `width`.
    duration: Ms,
    ruler: Rect,
    text_row: Rect,
    video_row: Rect,
    lanes: [Rect; 2],
}

impl Tracks {
    fn new(area: Rect, duration: Ms) -> Self {
        let left = area.left() + LABEL_WIDTH;
        let right = area.right() - 12.0;
        let width = (right - left).max(50.0);

        let row = |top: f32, height: f32| {
            Rect::from_min_size(Pos2::new(left, top), egui::vec2(width, height))
        };
        let ruler = row(area.top(), RULER_HEIGHT);
        let text_row = row(ruler.bottom() + ROW_GAP, TEXT_ROW_HEIGHT);
        let video_row = row(text_row.bottom() + ROW_GAP, VIDEO_ROW_HEIGHT);
        let lane_0 = row(video_row.bottom() + ROW_GAP, LANE_HEIGHT);
        let lane_1 = row(lane_0.bottom() + 3.0, LANE_HEIGHT);

        Self {
            left,
            width,
            // Timeline kosong tetap ditampilkan sepanjang 1 detik agar tidak dibagi nol.
            duration: duration.max(1_000),
            ruler,
            text_row,
            video_row,
            lanes: [lane_0, lane_1],
        }
    }

    fn pixels_per_ms(&self) -> f32 {
        self.width / self.duration as f32
    }

    /// Posisi layar untuk waktu `t`.
    fn x(&self, t: f64) -> f32 {
        self.left + t as f32 * self.pixels_per_ms()
    }

    /// Waktu (milidetik) untuk posisi layar `x`.
    fn time_at(&self, x: f32) -> f64 {
        ((x - self.left) / self.pixels_per_ms()) as f64
    }

    /// Persegi sebuah item di dalam jalur `row`.
    fn span(&self, row: Rect, start: Ms, len: Ms) -> Rect {
        let x0 = self.x(start as f64);
        let x1 = (self.x((start + len) as f64) - 2.0).max(x0 + 2.0);
        Rect::from_min_max(Pos2::new(x0, row.top()), Pos2::new(x1, row.bottom()))
    }
}

fn tracks(app: &mut App, ui: &mut Ui) {
    let (area, background) = ui.allocate_exact_size(ui.available_size(), Sense::click());
    if background.clicked() {
        app.selection = Selection::None;
    }

    // Salinan isi timeline untuk digambar. Isinya kecil (hanya data, bukan video),
    // dan salinan ini membuat kita bebas memanggil aksi `app` sambil menggambar.
    let tl = app.timeline().clone();
    let tracks = Tracks::new(area, tl.duration());
    let painter = ui.painter_at(area);

    paint_row_backgrounds(&painter, &tracks, area);
    ruler(app, ui, &painter, &tracks);

    if tl.clips().is_empty() {
        painter.text(
            tracks.video_row.center(),
            Align2::CENTER_CENTER,
            "Timeline kosong. Klik media di kiri untuk mulai.",
            FontId::proportional(12.0),
            theme::MUTED,
        );
    }

    text_items(app, ui, &painter, &tracks, &tl);
    video_clips(app, ui, &painter, &tracks, &tl);
    junction_buttons(app, ui, &painter, &tracks, &tl);
    audio_items(app, ui, &painter, &tracks, &tl);
    paint_playhead(&painter, &tracks, app.playhead);
}

fn paint_row_backgrounds(painter: &Painter, tracks: &Tracks, area: Rect) {
    let faint = Color32::from_white_alpha(5);
    let rows = [
        (tracks.text_row, "Teks"),
        (tracks.video_row, "Video"),
        (tracks.lanes[0].union(tracks.lanes[1]), "Audio"),
    ];
    for (row, name) in rows {
        painter.rect_filled(row, 4.0, faint);
        painter.text(
            Pos2::new(area.left(), row.center().y),
            Align2::LEFT_CENTER,
            name,
            FontId::proportional(11.0),
            theme::MUTED,
        );
    }
}

/// Penggaris waktu. Klik atau seret di sini memindah garis oranye.
fn ruler(app: &mut App, ui: &mut Ui, painter: &Painter, tracks: &Tracks) {
    let response = ui
        .interact(
            tracks.ruler,
            ui.scope_id().with("ruler"),
            Sense::click_and_drag(),
        )
        .on_hover_cursor(CursorIcon::ResizeHorizontal);
    if response.is_pointer_button_down_on() {
        if let Some(pointer) = response.interact_pointer_pos() {
            app.playing = false;
            app.set_playhead(tracks.time_at(pointer.x));
        }
    }

    painter.hline(
        tracks.ruler.x_range(),
        tracks.ruler.bottom(),
        Stroke::new(1.0, theme::LINE),
    );

    // Pilih jarak antar angka supaya tidak berdempetan.
    let pixels_per_second = tracks.pixels_per_ms() * 1000.0;
    let mut step_seconds = 600;
    for candidate in [1, 2, 5, 10, 15, 30, 60, 120, 300] {
        if candidate as f32 * pixels_per_second >= 48.0 {
            step_seconds = candidate;
            break;
        }
    }

    let total_seconds = tracks.duration / 1000;
    let mut second = 0;
    while second <= total_seconds {
        let x = tracks.x(second as f64 * 1000.0);
        painter.vline(
            x,
            (tracks.ruler.bottom() - 8.0)..=tracks.ruler.bottom(),
            Stroke::new(1.0, theme::LINE),
        );
        let label = if total_seconds >= 60 {
            labels::clock(second * 1000)
        } else {
            format!("{second}s")
        };
        // Angka paling kanan dilewati kalau tidak muat.
        if x + 28.0 < tracks.ruler.right() {
            painter.text(
                Pos2::new(x + 4.0, tracks.ruler.top() + 2.0),
                Align2::LEFT_TOP,
                label,
                FontId::monospace(10.0),
                theme::MUTED,
            );
        }
        second += step_seconds;
    }
}

fn text_items(app: &mut App, ui: &mut Ui, painter: &Painter, tracks: &Tracks, tl: &Timeline) {
    for text in tl.texts() {
        let rect = tracks.span(tracks.text_row, text.start, text.len);
        let response = ui.interact(rect, ui.scope_id().with(("text", text.id)), Sense::click());
        if response.clicked() {
            app.selection = Selection::Text(text.id);
        }
        let selected = app.selection == Selection::Text(text.id);
        let label = if text.text.is_empty() {
            "Teks"
        } else {
            text.text.as_str()
        };
        paint_item(painter, rect, theme::TEXT_ITEM, selected, label, 8.0);
    }
}

fn video_clips(app: &mut App, ui: &mut Ui, painter: &Painter, tracks: &Tracks, tl: &Timeline) {
    for (start, clip) in tl.clip_spans() {
        let rect = tracks.span(tracks.video_row, start, clip.len);
        let response = ui.interact(rect, ui.scope_id().with(("clip", clip.id)), Sense::click());
        if response.clicked() {
            app.selection = Selection::Clip(clip.id);
        }
        let selected = app.selection == Selection::Clip(clip.id);

        // Nama klip, lalu keterangan singkat keadaannya.
        let mut label = clip.name.clone();
        if clip.effect != Effect::None {
            label.push_str(&format!("  ·  {}", labels::effect(clip.effect)));
        }
        match clip.audio {
            ClipAudio::Attached | ClipAudio::NoAudio => {}
            ClipAudio::Detached { .. } => label.push_str("  ·  Audio dipisah"),
            ClipAudio::Removed => label.push_str("  ·  Tanpa suara"),
        }
        // Nama digeser ke kanan supaya tidak tertutup tombol sambungan.
        paint_item(
            painter,
            rect,
            theme::tint_flat(clip.tint),
            selected,
            &label,
            18.0,
        );
    }
}

/// Tombol bundar di tiap sambungan antar klip, tempat memasang transisi.
fn junction_buttons(app: &mut App, ui: &mut Ui, painter: &Painter, tracks: &Tracks, tl: &Timeline) {
    let last_index = tl.clips().len().saturating_sub(1);

    for (index, (start, clip)) in tl.clip_spans().enumerate() {
        if index == last_index {
            break;
        }
        let edge_x = tracks.x((start + clip.len) as f64) - 1.0;
        let center = Pos2::new(edge_x, tracks.video_row.center().y);
        let rect = Rect::from_center_size(center, egui::vec2(22.0, 22.0));

        let hover_text = match clip.transition_out {
            Some(kind) => format!("Transisi: {}", labels::transition(kind)),
            None => "Tambah transisi".to_owned(),
        };
        let response = ui
            .interact(
                rect,
                ui.scope_id().with(("junction", clip.id)),
                Sense::click(),
            )
            .on_hover_text(hover_text)
            .on_hover_cursor(CursorIcon::PointingHand);
        if response.clicked() {
            app.selection = Selection::Junction(clip.id);
        }

        let selected = app.selection == Selection::Junction(clip.id);
        let has_transition = clip.transition_out.is_some();
        let filled = has_transition || selected || response.hovered();

        let (fill, ink) = if filled {
            (theme::ACCENT, theme::ACCENT_INK)
        } else {
            (theme::PANEL, theme::ACCENT)
        };
        painter.circle(center, 10.0, fill, Stroke::new(1.0, theme::ACCENT));
        if selected {
            painter.circle_stroke(center, 12.5, Stroke::new(1.5, theme::TEXT));
        }
        let icon = if has_transition {
            icons::CHECKMARK
        } else {
            icons::ADD
        };
        painter.text(center, Align2::CENTER_CENTER, icon, icons::font(12.0), ink);
    }
}

fn audio_items(app: &mut App, ui: &mut Ui, painter: &Painter, tracks: &Tracks, tl: &Timeline) {
    for audio in tl.audios() {
        let lane = tracks.lanes[audio.lane as usize % tracks.lanes.len()];
        let rect = tracks.span(lane, audio.start, audio.len);
        let response = ui
            .interact(
                rect,
                ui.scope_id().with(("audio", audio.id)),
                Sense::click_and_drag(),
            )
            .on_hover_cursor(CursorIcon::Grab);

        if response.clicked() {
            app.selection = Selection::Audio(audio.id);
        }

        if response.drag_started() {
            // Titik tempat tombol tetikus pertama ditekan.
            let pressed_at = ui
                .input(|i| i.pointer.press_origin())
                .unwrap_or(rect.left_center());
            app.playing = false;
            app.selection = Selection::Audio(audio.id);
            app.audio_drag = Some(AudioDrag {
                id: audio.id,
                grab_offset_x: pressed_at.x - rect.left(),
                start: audio.start,
                lane: audio.lane,
                snapped_to: None,
            });
        }

        if response.dragged() {
            if let Some(pointer) = response.interact_pointer_pos() {
                update_audio_drag(app, tracks, tl, audio, pointer);
            }
        }

        if response.drag_stopped() {
            if let Some(drag) = app.audio_drag.take() {
                let moved = drag.start != audio.start || drag.lane != audio.lane;
                if drag.id == audio.id && moved {
                    app.move_audio(drag.id, drag.start, drag.lane);
                }
            }
        }

        let being_dragged = match &app.audio_drag {
            Some(drag) => drag.id == audio.id,
            None => false,
        };
        if !being_dragged {
            let selected = app.selection == Selection::Audio(audio.id);
            paint_item(
                painter,
                rect,
                audio_color(audio),
                selected,
                &labels::audio_name(audio),
                8.0,
            );
        }
    }

    // Potongan yang sedang diseret digambar paling akhir supaya berada di atas.
    if let Some(drag) = &app.audio_drag {
        if let Some(audio) = tl.audio(drag.id) {
            let lane = tracks.lanes[drag.lane as usize % tracks.lanes.len()];
            let rect = tracks.span(lane, drag.start, audio.len);
            let label = format!(
                "{}  ·  detik {}",
                labels::audio_name(audio),
                labels::seconds_number(drag.start)
            );
            paint_item(painter, rect, audio_color(audio), true, &label, 8.0);

            if let Some(point) = drag.snapped_to {
                let x = tracks.x(point as f64);
                let top = tracks.ruler.top();
                let bottom = tracks.lanes[1].bottom();
                painter.vline(x, top..=bottom, Stroke::new(1.0, theme::TEXT));
            }
        }
    }
}

fn audio_color(audio: &AudioItem) -> Color32 {
    match audio.kind {
        AudioKind::Music => theme::MUSIC_ITEM,
        AudioKind::Detached => theme::DETACHED_ITEM,
    }
}

/// Menghitung posisi calon audio yang sedang diseret: waktu, lajur, dan tempelan.
fn update_audio_drag(
    app: &mut App,
    tracks: &Tracks,
    tl: &Timeline,
    audio: &AudioItem,
    pointer: Pos2,
) {
    let playhead = app.playhead_ms();
    let Some(drag) = app.audio_drag.as_mut() else {
        return;
    };

    let len = audio.len as f64;
    let max_start = tl.duration().saturating_sub(audio.len) as f64;
    let raw_start = tracks
        .time_at(pointer.x - drag.grab_offset_x)
        .clamp(0.0, max_start);

    // Titik waktu tempat tepi audio boleh menempel.
    let mut snap_points: Vec<Ms> = vec![0, tl.video_len(), playhead];
    for (clip_start, _) in tl.clip_spans() {
        snap_points.push(clip_start);
    }
    for other in tl.audios() {
        if other.id != audio.id {
            snap_points.push(other.start);
            snap_points.push(other.start + other.len);
        }
    }

    // Cari titik terdekat, baik untuk tepi kiri maupun tepi kanan audio.
    let mut start = raw_start;
    let mut snapped_to = None;
    let mut nearest = (SNAP_PIXELS / tracks.pixels_per_ms()) as f64;
    for point in snap_points {
        let left_edge_here = point as f64;
        let right_edge_here = point as f64 - len;
        for candidate in [left_edge_here, right_edge_here] {
            let distance = (candidate - raw_start).abs();
            if distance < nearest && candidate >= 0.0 && candidate <= max_start {
                nearest = distance;
                start = candidate;
                snapped_to = Some(point);
            }
        }
    }

    // Tanpa tempelan, bulatkan ke 10 ms supaya angkanya rapi.
    if snapped_to.is_none() {
        start = ((start / 10.0).round() * 10.0).min(max_start);
    }

    let lane_boundary = (tracks.lanes[0].bottom() + tracks.lanes[1].top()) / 2.0;
    drag.start = start.round() as Ms;
    drag.lane = if pointer.y < lane_boundary { 0 } else { 1 };
    drag.snapped_to = snapped_to;
}

/// Menggambar satu item timeline: kotak berwarna dengan nama di dalamnya.
/// `label_inset` adalah jarak nama dari tepi kiri kotak.
fn paint_item(
    painter: &Painter,
    rect: Rect,
    fill: Color32,
    selected: bool,
    label: &str,
    label_inset: f32,
) {
    let stroke = if selected {
        Stroke::new(2.0, theme::ACCENT)
    } else {
        Stroke::new(1.0, Color32::from_white_alpha(40))
    };
    painter.rect(rect, 5.0, fill, stroke, StrokeKind::Inside);

    // Nama dipotong di tepi kotak supaya tidak meluber ke item sebelah.
    let text_area = rect.shrink2(egui::vec2(6.0, 0.0));
    painter.with_clip_rect(text_area).text(
        Pos2::new(rect.left() + label_inset, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(12.0),
        theme::TEXT,
    );
}

/// Garis oranye penanda waktu sekarang.
fn paint_playhead(painter: &Painter, tracks: &Tracks, playhead: f64) {
    let x = tracks.x(playhead);
    let top = tracks.ruler.top();
    let bottom = tracks.lanes[1].bottom();
    painter.vline(x, top..=bottom, Stroke::new(2.0, theme::ACCENT));

    let head = vec![
        Pos2::new(x - 6.0, top),
        Pos2::new(x + 6.0, top),
        Pos2::new(x, top + 9.0),
    ];
    painter.add(egui::Shape::convex_polygon(
        head,
        theme::ACCENT,
        Stroke::NONE,
    ));
}
