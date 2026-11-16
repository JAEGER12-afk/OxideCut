//! Timeline di bagian bawah: bilah alat, penggaris waktu, jalur Teks, Video, dan Audio.
//!
//! Seluruh timeline selalu muat selebar layar (belum ada zoom).
//! Semua posisi dihitung lewat `Tracks`, jadi kalau nanti zoom ditambahkan,
//! cukup `Tracks` yang berubah.

use eframe::egui::{
    self, Align, Align2, Color32, CursorIcon, FontId, Frame, Layout, Margin, Painter, Pos2, Rect,
    Sense, Stroke, StrokeKind, Ui,
};
use oxidecut_core::{AudioItem, AudioKind, ClipAudio, Effect, Id, Ms, Timeline, MIN_PIECE_MS};

use crate::app::{App, Drag, DragKind, Selection};
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

/// Lebar pegangan di tepi item untuk memangkas (piksel).
const HANDLE_WIDTH: f32 = 7.0;

/// Item yang diseret menempel ke tepi terdekat dalam jarak ini (piksel).
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
            widgets::hint(
                ui,
                "Seret item untuk menggeser, seret tepinya untuk memangkas",
            );
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
    paint_drag(app, &painter, &tracks, &tl);
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

/// Keterangan sebuah item yang dibutuhkan untuk memulai seret.
struct ItemInfo {
    item: Selection,
    start: Ms,
    len: Ms,
    /// Lajur audio; 0 untuk klip dan teks.
    lane: u8,
    /// Panjang terbesar kalau tepi kiri yang diseret.
    max_len_from_start: Ms,
    /// Panjang terbesar kalau tepi kanan yang diseret.
    max_len_from_end: Ms,
}

fn text_items(app: &mut App, ui: &mut Ui, painter: &Painter, tracks: &Tracks, tl: &Timeline) {
    for text in tl.texts() {
        let item = Selection::Text(text.id);
        let rect = tracks.span(tracks.text_row, text.start, text.len);
        let end = text.start + text.len;
        let info = ItemInfo {
            item,
            start: text.start,
            len: text.len,
            lane: 0,
            // Ke kiri sampai waktu nol; ke kanan sampai ujung timeline.
            max_len_from_start: end,
            max_len_from_end: tl.duration().max(end) - text.start,
        };
        item_interaction(app, ui, tracks, tl, rect, &info);

        if !is_dragged(app, item) {
            let selected = app.selection == item;
            let label = if text.text.is_empty() {
                "Teks"
            } else {
                text.text.as_str()
            };
            paint_item(painter, rect, theme::TEXT_ITEM, selected, label, 8.0);
        }
    }
}

fn video_clips(app: &mut App, ui: &mut Ui, painter: &Painter, tracks: &Tracks, tl: &Timeline) {
    for (start, clip) in tl.clip_spans() {
        let item = Selection::Clip(clip.id);
        let rect = tracks.span(tracks.video_row, start, clip.len);
        let info = ItemInfo {
            item,
            start,
            len: clip.len,
            lane: 0,
            // Klip hanya bisa dipanjangkan selama berkas sumbernya masih ada isinya.
            max_len_from_start: clip.source_in + clip.len,
            max_len_from_end: clip.source_len.saturating_sub(clip.source_in),
        };
        item_interaction(app, ui, tracks, tl, rect, &info);

        if !is_dragged(app, item) {
            let selected = app.selection == item;

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
        let item = Selection::Audio(audio.id);
        let lane = tracks.lanes[audio.lane as usize % tracks.lanes.len()];
        let rect = tracks.span(lane, audio.start, audio.len);
        let info = ItemInfo {
            item,
            start: audio.start,
            len: audio.len,
            lane: audio.lane,
            // Ke kiri butuh ruang di timeline dan di berkas sumber.
            max_len_from_start: audio.start.min(audio.source_in) + audio.len,
            max_len_from_end: audio.source_len.saturating_sub(audio.source_in),
        };
        item_interaction(app, ui, tracks, tl, rect, &info);

        if !is_dragged(app, item) {
            let selected = app.selection == item;
            let name = labels::audio_name(audio);
            paint_item(painter, rect, audio_color(audio), selected, &name, 8.0);
        }
    }
}

fn audio_color(audio: &AudioItem) -> Color32 {
    match audio.kind {
        AudioKind::Music => theme::MUSIC_ITEM,
        AudioKind::Detached => theme::DETACHED_ITEM,
    }
}

fn is_dragged(app: &App, item: Selection) -> bool {
    match &app.drag {
        Some(drag) => drag.item == item,
        None => false,
    }
}

// ------------------------------------------------------------------ seret
//
// Semua item (klip, teks, audio) memakai mekanisme seret yang sama:
// - seret badan  = geser
// - seret tepi   = pangkas (trim)
// Selama diseret, hanya "calon" di `app.drag` yang berubah dan digambar.
// Timeline baru diubah saat tetikus dilepas, lewat `App::finish_drag`.

/// Menangani klik, seret badan, dan seret tepi untuk satu item.
fn item_interaction(
    app: &mut App,
    ui: &mut Ui,
    tracks: &Tracks,
    tl: &Timeline,
    rect: Rect,
    info: &ItemInfo,
) {
    let salt = match info.item {
        Selection::Clip(id) => ("clip", id),
        Selection::Text(id) => ("text", id),
        Selection::Audio(id) => ("audio", id),
        Selection::Junction(id) => ("junction-item", id),
        Selection::None => ("none", 0),
    };
    let id = ui.scope_id().with(salt);

    // Tepi kiri dan kanan adalah pegangan untuk memangkas.
    // Pada item yang sempit, pegangan mengecil supaya badannya tetap bisa dipegang.
    let handle = HANDLE_WIDTH.min(rect.width() / 3.0);
    let left_zone = Rect::from_min_max(rect.min, Pos2::new(rect.left() + handle, rect.bottom()));
    let right_zone = Rect::from_min_max(Pos2::new(rect.right() - handle, rect.top()), rect.max);

    let body = ui
        .interact(rect, id.with("body"), Sense::click_and_drag())
        .on_hover_cursor(CursorIcon::Grab);
    let left = ui
        .interact(left_zone, id.with("left"), Sense::click_and_drag())
        .on_hover_cursor(CursorIcon::ResizeHorizontal);
    let right = ui
        .interact(right_zone, id.with("right"), Sense::click_and_drag())
        .on_hover_cursor(CursorIcon::ResizeHorizontal);

    if body.clicked() || left.clicked() || right.clicked() {
        app.selection = info.item;
    }

    let parts = [
        (&body, DragKind::Move),
        (&left, DragKind::TrimStart),
        (&right, DragKind::TrimEnd),
    ];
    for (response, kind) in parts {
        if response.drag_started() {
            let press_x = match ui.input(|i| i.pointer.press_origin()) {
                Some(pos) => pos.x,
                None => rect.center().x,
            };
            start_drag(app, info, kind, press_x);
        }
        if response.dragged() {
            if let Some(pointer) = response.interact_pointer_pos() {
                update_drag(app, tracks, tl, pointer);
            }
        }
        if response.drag_stopped() {
            app.finish_drag();
        }
    }
}

fn start_drag(app: &mut App, info: &ItemInfo, kind: DragKind, press_x: f32) {
    let max_len = match kind {
        DragKind::Move => info.len,
        DragKind::TrimStart => info.max_len_from_start,
        DragKind::TrimEnd => info.max_len_from_end,
    };
    app.playing = false;
    app.selection = info.item;
    app.drag = Some(Drag {
        item: info.item,
        kind,
        press_x,
        origin_start: info.start,
        origin_len: info.len,
        max_len: max_len.max(info.len),
        start: info.start,
        len: info.len,
        lane: info.lane,
        insert_index: 0,
        snapped_to: None,
    });
}

/// Menghitung calon posisi dan panjang item yang sedang diseret.
fn update_drag(app: &mut App, tracks: &Tracks, tl: &Timeline, pointer: Pos2) {
    let playhead = app.playhead_ms();
    let Some(drag) = app.drag.as_mut() else {
        return;
    };

    // Seberapa jauh kursor sudah bergerak, dalam milidetik.
    let moved = tracks.time_at(pointer.x) - tracks.time_at(drag.press_x);
    // Jarak tempel, dalam milidetik.
    let reach = (SNAP_PIXELS / tracks.pixels_per_ms()) as f64;
    let points = snap_points(tl, drag.item, playhead);

    let origin_start = drag.origin_start as f64;
    let origin_len = drag.origin_len as f64;
    let origin_end = origin_start + origin_len;
    let shortest = MIN_PIECE_MS as f64;
    let longest = drag.max_len as f64;
    drag.snapped_to = None;

    match drag.kind {
        DragKind::Move => {
            // Klip video: ikuti kursor, lalu hitung di urutan ke berapa ia akan mendarat.
            if let Selection::Clip(id) = drag.item {
                drag.start = (origin_start + moved).max(0.0) as Ms;
                drag.insert_index = insert_index_at(tracks, tl, id, pointer.x);
                return;
            }

            // Teks dan audio: bebas di sepanjang timeline, tepinya bisa menempel.
            let last_start = (tl.duration() as f64 - origin_len).max(0.0);
            let raw = (origin_start + moved).clamp(0.0, last_start);
            let mut start = round_to_10(raw).min(last_start);
            let mut nearest = reach;
            for point in &points {
                let left_edge_here = *point as f64;
                let right_edge_here = *point as f64 - origin_len;
                for candidate in [left_edge_here, right_edge_here] {
                    let distance = (candidate - raw).abs();
                    if distance < nearest && candidate >= 0.0 && candidate <= last_start {
                        nearest = distance;
                        start = candidate;
                        drag.snapped_to = Some(*point);
                    }
                }
            }
            drag.start = start.round() as Ms;

            if let Selection::Audio(_) = drag.item {
                let lane_boundary = (tracks.lanes[0].bottom() + tracks.lanes[1].top()) / 2.0;
                drag.lane = if pointer.y < lane_boundary { 0 } else { 1 };
            }
        }

        DragKind::TrimStart => {
            // Tepi kiri bergerak, tepi kanan diam.
            let mut len = round_to_10(origin_len - moved).clamp(shortest, longest);
            if let Some(point) = nearest_point(origin_end - len, &points, reach) {
                let snapped_len = origin_end - point as f64;
                if snapped_len >= shortest && snapped_len <= longest {
                    len = snapped_len;
                    drag.snapped_to = Some(point);
                }
            }
            drag.len = len.round() as Ms;
            drag.start = (drag.origin_start + drag.origin_len).saturating_sub(drag.len);
        }

        DragKind::TrimEnd => {
            // Tepi kanan bergerak, tepi kiri diam.
            let mut len = round_to_10(origin_len + moved).clamp(shortest, longest);
            if let Some(point) = nearest_point(origin_start + len, &points, reach) {
                let snapped_len = point as f64 - origin_start;
                if snapped_len >= shortest && snapped_len <= longest {
                    len = snapped_len;
                    drag.snapped_to = Some(point);
                }
            }
            drag.len = len.round() as Ms;
            drag.start = drag.origin_start;
        }
    }
}

/// Membulatkan ke 10 milidetik supaya angkanya rapi.
fn round_to_10(ms: f64) -> f64 {
    (ms / 10.0).round() * 10.0
}

/// Titik waktu tempat tepi item boleh menempel: awal timeline, akhir video,
/// garis oranye, dan tepi semua item lain.
fn snap_points(tl: &Timeline, dragged: Selection, playhead: Ms) -> Vec<Ms> {
    let mut points = vec![0, tl.video_len(), playhead];
    for (start, clip) in tl.clip_spans() {
        if Selection::Clip(clip.id) != dragged {
            points.push(start);
            points.push(start + clip.len);
        }
    }
    for text in tl.texts() {
        if Selection::Text(text.id) != dragged {
            points.push(text.start);
            points.push(text.start + text.len);
        }
    }
    for audio in tl.audios() {
        if Selection::Audio(audio.id) != dragged {
            points.push(audio.start);
            points.push(audio.start + audio.len);
        }
    }
    points
}

/// Titik tempel yang paling dekat dengan `time`, kalau ada yang berjarak kurang dari `reach`.
fn nearest_point(time: f64, points: &[Ms], reach: f64) -> Option<Ms> {
    let mut best = None;
    let mut nearest = reach;
    for point in points {
        let distance = (*point as f64 - time).abs();
        if distance < nearest {
            nearest = distance;
            best = Some(*point);
        }
    }
    best
}

/// Urutan tempat klip yang diseret akan mendarat: jumlah klip lain yang
/// titik tengahnya berada di kiri kursor.
fn insert_index_at(tracks: &Tracks, tl: &Timeline, dragged: Id, pointer_x: f32) -> usize {
    let mut index = 0;
    for (start, clip) in tl.clip_spans() {
        if clip.id == dragged {
            continue;
        }
        let center_x = tracks.x((start + clip.len / 2) as f64);
        if center_x < pointer_x {
            index += 1;
        }
    }
    index
}

/// Waktu di timeline tempat penanda sisip digambar untuk urutan `index`.
fn insert_marker_time(tl: &Timeline, dragged: Id, index: usize) -> Ms {
    // Tidak berpindah: penanda di awal klip itu sendiri.
    if tl.clip_index(dragged) == Some(index) {
        return tl.clip_start(index);
    }
    let mut position = 0;
    let mut end_of_last = 0;
    for (start, clip) in tl.clip_spans() {
        if clip.id == dragged {
            continue;
        }
        if position == index {
            return start;
        }
        position += 1;
        end_of_last = start + clip.len;
    }
    end_of_last
}

/// Menggambar item yang sedang diseret pada posisi calonnya, paling atas.
fn paint_drag(app: &App, painter: &Painter, tracks: &Tracks, tl: &Timeline) {
    let Some(drag) = &app.drag else {
        return;
    };

    let (row, fill, name) = match drag.item {
        Selection::Clip(id) => match tl.clip(id) {
            Some(clip) => (
                tracks.video_row,
                theme::tint_flat(clip.tint),
                clip.name.clone(),
            ),
            None => return,
        },
        Selection::Text(id) => match tl.text(id) {
            Some(text) => (tracks.text_row, theme::TEXT_ITEM, text.text.clone()),
            None => return,
        },
        Selection::Audio(id) => match tl.audio(id) {
            Some(audio) => {
                let lane = tracks.lanes[drag.lane as usize % tracks.lanes.len()];
                (lane, audio_color(audio), labels::audio_name(audio))
            }
            None => return,
        },
        Selection::Junction(_) | Selection::None => return,
    };

    let moving_clip = drag.kind == DragKind::Move && matches!(drag.item, Selection::Clip(_));

    // Keterangan: saat digeser tampilkan waktunya, saat dipangkas tampilkan panjangnya.
    let label = if moving_clip {
        name
    } else if drag.kind == DragKind::Move {
        format!("{name}  ·  detik {}", labels::seconds_number(drag.start))
    } else {
        format!("{name}  ·  {}", labels::seconds(drag.len))
    };

    let rect = tracks.span(row, drag.start, drag.len);
    let fill = if moving_clip {
        fill.gamma_multiply(0.85)
    } else {
        fill
    };
    paint_item(painter, rect, fill, true, &label, 8.0);

    // Klip yang digeser: garis tebal menunjukkan di mana ia akan mendarat.
    if let (Selection::Clip(id), true) = (drag.item, moving_clip) {
        let x = tracks.x(insert_marker_time(tl, id, drag.insert_index) as f64);
        painter.vline(x, row.y_range(), Stroke::new(3.0, theme::TEXT));
    }

    // Garis tipis menunjukkan tempat tepi item menempel.
    if let Some(point) = drag.snapped_to {
        let x = tracks.x(point as f64);
        let top = tracks.ruler.top();
        let bottom = tracks.lanes[1].bottom();
        painter.vline(x, top..=bottom, Stroke::new(1.0, theme::TEXT));
    }
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

    // Item terpilih menampilkan dua pegangan kecil di tepinya untuk memangkas.
    if selected && rect.width() > 24.0 {
        let bar = egui::vec2(3.0, rect.height() * 0.5);
        let left = Rect::from_center_size(rect.left_center() + egui::vec2(4.0, 0.0), bar);
        let right = Rect::from_center_size(rect.right_center() - egui::vec2(4.0, 0.0), bar);
        painter.rect_filled(left, 1.5, theme::TEXT);
        painter.rect_filled(right, 1.5, theme::TEXT);
    }

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
