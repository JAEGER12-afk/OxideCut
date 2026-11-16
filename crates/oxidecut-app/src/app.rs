//! Keadaan aplikasi dan semua aksi pengguna.
//!
//! Panel-panel (berkas lain) hanya menggambar dan memanggil aksi di sini.
//! Aksi di sini memanggil `oxidecut-core` lalu menampilkan pesan singkat.

use std::path::PathBuf;

use eframe::egui;
use oxidecut_core::{AudioKind, Edge, Editor, Effect, Id, Ms, Timeline, Transition, AUDIO_LANES};

use crate::labels;
use crate::media::{self, MediaItem, MediaKind};
use crate::video::VideoPool;
use crate::{inspector, media_panel, preview, timeline_view, top_bar};

/// Apa yang sedang dipilih pengguna. Panel kanan mengikuti ini.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Selection {
    None,
    Clip(Id),
    Text(Id),
    Audio(Id),
    /// Sambungan antara klip ini dan klip sesudahnya (tempat transisi).
    Junction(Id),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Ratio {
    Portrait,
    Square,
    Landscape,
}

impl Ratio {
    pub const ALL: [Ratio; 3] = [Ratio::Portrait, Ratio::Square, Ratio::Landscape];

    pub fn label(self) -> &'static str {
        match self {
            Ratio::Portrait => "9:16",
            Ratio::Square => "1:1",
            Ratio::Landscape => "16:9",
        }
    }

    pub fn width_over_height(self) -> f32 {
        match self {
            Ratio::Portrait => 9.0 / 16.0,
            Ratio::Square => 1.0,
            Ratio::Landscape => 16.0 / 9.0,
        }
    }
}

/// Cara sebuah item diseret.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DragKind {
    /// Badan item diseret: item berpindah.
    Move,
    /// Tepi kiri diseret: item dipangkas dari depan.
    TrimStart,
    /// Tepi kanan diseret: item dipangkas dari belakang.
    TrimEnd,
}

/// Item timeline yang sedang diseret.
///
/// Nilai "calon" di bawah hanya untuk digambar selama seret berlangsung.
/// Timeline baru benar-benar diubah saat tetikus dilepas (`App::finish_drag`).
pub struct Drag {
    /// Item yang diseret: `Selection::Clip`, `Text`, atau `Audio`.
    pub item: Selection,
    pub kind: DragKind,
    /// Posisi mendatar kursor saat tombol tetikus ditekan.
    pub press_x: f32,
    /// Waktu mulai dan panjang item saat mulai diseret.
    pub origin_start: Ms,
    pub origin_len: Ms,
    /// Panjang terbesar yang diizinkan saat memangkas.
    pub max_len: Ms,
    /// Calon waktu mulai dan panjang.
    pub start: Ms,
    pub len: Ms,
    /// Calon lajur (hanya untuk audio).
    pub lane: u8,
    /// Calon urutan (hanya untuk klip video yang digeser).
    pub insert_index: usize,
    /// Waktu tempat tepi item sedang menempel, kalau ada.
    pub snapped_to: Option<Ms>,
}

pub struct App {
    pub editor: Editor,
    pub library: Vec<MediaItem>,
    pub selection: Selection,
    /// Posisi garis oranye, dalam milidetik.
    pub playhead: f64,
    pub playing: bool,
    pub ratio: Ratio,
    pub drag: Option<Drag>,
    /// Pemutar video asli untuk pratinjau.
    pub video: VideoPool,
    pub export_open: bool,
    pub export_height: u32,
    /// Minta kotak isian teks di panel kanan mengambil fokus pada frame berikutnya.
    pub focus_text_box: bool,
    /// Pesan singkat di bawah pratinjau, dan sampai kapan ia tampil.
    pub toast: String,
    pub toast_until: f64,
    /// Waktu sekarang menurut egui, diperbarui tiap frame.
    pub now: f64,
    next_tint: u8,
}

impl App {
    pub fn new() -> Self {
        let mut app = Self {
            editor: Editor::new(sample_project()),
            library: media::sample_library(),
            selection: Selection::None,
            playhead: 3_000.0,
            playing: false,
            ratio: Ratio::Portrait,
            drag: None,
            video: VideoPool::new(),
            export_open: false,
            export_height: 1080,
            focus_text_box: false,
            toast: String::new(),
            toast_until: 0.0,
            now: 0.0,
            next_tint: 4,
        };

        // Berkas yang diberikan saat aplikasi dibuka (misalnya lewat "Open with")
        // langsung masuk ke panel Media.
        let files: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
        if !files.is_empty() {
            app.import_paths(&files);
        }
        app
    }

    pub fn timeline(&self) -> &Timeline {
        self.editor.timeline()
    }

    pub fn playhead_ms(&self) -> Ms {
        self.playhead.max(0.0) as Ms
    }

    pub fn set_playhead(&mut self, ms: f64) {
        let end = self.timeline().duration() as f64;
        self.playhead = ms.clamp(0.0, end);
    }

    /// Menampilkan pesan singkat selama dua detik.
    pub fn say(&mut self, message: impl Into<String>) {
        self.toast = message.into();
        self.toast_until = self.now + 2.0;
    }

    // ---- putar ----

    pub fn toggle_play(&mut self) {
        if !self.playing && self.playhead >= self.timeline().duration() as f64 {
            self.playhead = 0.0;
        }
        self.playing = !self.playing;
    }

    // ---- urungkan ----

    pub fn undo(&mut self) {
        if self.editor.undo() {
            self.say("Diurungkan");
        }
    }

    pub fn redo(&mut self) {
        if self.editor.redo() {
            self.say("Diulang");
        }
    }

    // ---- aksi utama: potong, hapus, teks ----

    /// Memotong di garis oranye. Kalau audio sedang dipilih, audionya yang dipotong;
    /// selain itu klip video di bawah garis oranye.
    pub fn cut(&mut self) {
        let t = self.playhead_ms();

        if let Selection::Audio(id) = self.selection {
            match self.editor.edit(|tl| tl.split_audio(id, t)) {
                Ok(right) => {
                    self.selection = Selection::Audio(right);
                    self.say(format!("Audio dipotong di {}", labels::clock(t)));
                }
                Err(_) => self.say("Geser garis oranye ke tengah audio dulu"),
            }
            return;
        }

        match self.editor.edit(|tl| tl.split_clip(t)) {
            Ok(right) => {
                self.selection = Selection::Clip(right);
                self.say(format!("Klip dipotong di {}", labels::clock(t)));
            }
            Err(_) => self.say("Geser garis oranye ke tengah klip dulu"),
        }
    }

    pub fn can_delete(&self) -> bool {
        matches!(
            self.selection,
            Selection::Clip(_) | Selection::Text(_) | Selection::Audio(_)
        )
    }

    pub fn delete_selected(&mut self) {
        match self.selection {
            Selection::Clip(id) => {
                if self.editor.edit(|tl| tl.delete_clip(id)).is_ok() {
                    self.say("Klip dihapus");
                }
            }
            Selection::Text(id) => {
                if self.editor.edit(|tl| tl.delete_text(id)).is_ok() {
                    self.say("Teks dihapus");
                }
            }
            Selection::Audio(id) => {
                if let Ok(removed) = self.editor.edit(|tl| tl.delete_audio(id)) {
                    match removed.kind {
                        AudioKind::Music => self.say("Musik dihapus"),
                        AudioKind::Detached => self.say("Audio dihapus, videonya tetap"),
                    }
                }
            }
            Selection::Junction(_) | Selection::None => return,
        }
        self.selection = Selection::None;
        self.set_playhead(self.playhead);
    }

    /// Menambah teks di garis oranye, lalu langsung siap diketik.
    pub fn add_text(&mut self) {
        let video_len = self.timeline().video_len();
        let start = self.playhead_ms().min(video_len.saturating_sub(1_000));
        let len = (video_len.saturating_sub(start)).clamp(1_000, 3_000);

        let result = self
            .editor
            .edit(|tl| Ok(tl.add_text(start, len, "Teks baru")));
        if let Ok(id) = result {
            self.selection = Selection::Text(id);
            self.focus_text_box = true;
            self.say("Teks ditambahkan di garis oranye");
        }
    }

    // ---- media ----

    /// Menaruh media dari panel kiri ke timeline.
    pub fn add_media_to_timeline(&mut self, index: usize) {
        let Some(item) = self.library.get(index).cloned() else {
            return;
        };
        match item.kind {
            MediaKind::Video => {
                let result = self.editor.edit(|tl| {
                    let id = tl.push_clip(&item.name, item.len, item.tint, item.path.clone());
                    if !item.has_audio {
                        tl.set_no_audio(id)?;
                    }
                    Ok(id)
                });
                if let Ok(id) = result {
                    self.selection = Selection::Clip(id);
                    self.say(format!("{} ditambahkan di akhir", item.name));
                }
            }
            MediaKind::Audio => {
                let start = self.playhead_ms();
                let result = self
                    .editor
                    .edit(|tl| tl.add_music(&item.name, start, item.len, item.path.clone()));
                match result {
                    Ok(id) => {
                        self.selection = Selection::Audio(id);
                        self.say(format!("{} ditambahkan di garis oranye", item.name));
                    }
                    Err(e) => self.say(labels::error(e)),
                }
            }
        }
    }

    /// Membuka pemilih berkas Windows dan memasukkan berkas pilihan ke panel Media.
    pub fn import_files(&mut self) {
        let mut extensions: Vec<&str> = Vec::new();
        extensions.extend(media::VIDEO_EXTENSIONS);
        extensions.extend(media::AUDIO_EXTENSIONS);

        let picked = rfd::FileDialog::new()
            .set_title("Impor media")
            .add_filter("Video dan audio", &extensions)
            .pick_files();
        let Some(paths) = picked else {
            return;
        };

        self.import_paths(&paths);
    }

    /// Memasukkan berkas-berkas ke panel Media.
    pub fn import_paths(&mut self, paths: &[PathBuf]) {
        let mut unknown_length = 0;
        for path in paths {
            let item = media::from_file(path, self.next_tint);
            self.next_tint = self.next_tint.wrapping_add(1);
            if !item.len_known {
                unknown_length += 1;
            }
            self.library.push(item);
        }

        if unknown_length > 0 {
            self.say(format!(
                "{} media diimpor. {} tidak terbaca durasinya, dianggap 5 detik",
                paths.len(),
                unknown_length
            ));
        } else {
            self.say(format!("{} media diimpor", paths.len()));
        }
    }

    // ---- efek dan transisi ----

    pub fn set_effect(&mut self, clip: Id, effect: Effect) {
        if self.editor.edit(|tl| tl.set_effect(clip, effect)).is_ok() {
            self.say(format!("Efek {} dipasang", labels::effect(effect)));
        }
    }

    pub fn set_transition(&mut self, clip: Id, transition: Option<Transition>) {
        match self.editor.edit(|tl| tl.set_transition(clip, transition)) {
            Ok(()) => match transition {
                Some(kind) => {
                    // Pindahkan garis oranye ke awal transisi supaya hasilnya langsung bisa diputar.
                    if let Some(index) = self.timeline().clip_index(clip) {
                        let edge = self.timeline().clip_start(index + 1);
                        self.set_playhead(edge.saturating_sub(preview::TRANSITION_HALF_MS) as f64);
                    }
                    self.say(format!("Transisi {} dipasang", labels::transition(kind)));
                }
                None => self.say("Transisi dilepas"),
            },
            Err(e) => self.say(labels::error(e)),
        }
    }

    // ---- audio ----

    pub fn detach_audio(&mut self, clip: Id) {
        match self.editor.edit(|tl| tl.detach_audio(clip)) {
            Ok(audio) => {
                self.selection = Selection::Audio(audio);
                self.say("Audio dipisah. Seret potongannya di jalur Audio");
            }
            Err(e) => self.say(labels::error(e)),
        }
    }

    pub fn reattach_audio(&mut self, audio: Id) {
        match self.editor.edit(|tl| tl.reattach_audio(audio)) {
            Ok(clip) => {
                self.selection = Selection::Clip(clip);
                self.say("Audio digabung lagi ke video");
            }
            Err(e) => self.say(labels::error(e)),
        }
    }

    pub fn restore_clip_audio(&mut self, clip: Id) {
        if self.editor.edit(|tl| tl.restore_clip_audio(clip)).is_ok() {
            self.say("Suara klip dipulihkan");
        }
    }

    /// Menyimpan posisi baru audio setelah diseret.
    /// Kalau lajur tujuan terisi, coba lajur lain pada waktu yang sama.
    pub fn move_audio(&mut self, audio: Id, start: Ms, lane: u8) {
        let Some(item) = self.timeline().audio(audio) else {
            return;
        };
        if item.start == start && item.lane == lane {
            return;
        }
        let name = labels::audio_name(item);
        let len = item.len;

        let mut target_lane = lane;
        if !self.timeline().lane_free(lane, start, len, Some(audio)) {
            let other = (0..AUDIO_LANES)
                .find(|l| *l != lane && self.timeline().lane_free(*l, start, len, Some(audio)));
            match other {
                Some(free) => target_lane = free,
                None => {
                    self.say("Tempat itu sudah terisi audio lain");
                    return;
                }
            }
        }

        match self
            .editor
            .edit(|tl| tl.move_audio(audio, start, target_lane))
        {
            Ok(()) => self.say(format!(
                "{} dipindah ke detik {}",
                name,
                labels::seconds_number(start)
            )),
            Err(e) => self.say(labels::error(e)),
        }
    }

    // ---- geser dan pangkas dengan seret ----

    /// Dipanggil saat tetikus dilepas: menyimpan hasil seret ke timeline.
    pub fn finish_drag(&mut self) {
        let Some(drag) = self.drag.take() else {
            return;
        };
        let edge = match drag.kind {
            DragKind::TrimStart => Edge::Start,
            DragKind::TrimEnd | DragKind::Move => Edge::End,
        };
        let moving = drag.kind == DragKind::Move;

        match drag.item {
            Selection::Clip(id) if moving => self.move_clip(id, drag.insert_index),
            Selection::Clip(id) => self.trim_clip(id, edge, drag.len),
            Selection::Text(id) if moving => self.move_text(id, drag.start),
            Selection::Text(id) => self.trim_text(id, edge, drag.len),
            Selection::Audio(id) if moving => self.move_audio(id, drag.start, drag.lane),
            Selection::Audio(id) => self.trim_audio(id, edge, drag.len),
            Selection::Junction(_) | Selection::None => {}
        }

        // Timeline bisa memendek; pastikan garis oranye tidak tertinggal di luar.
        self.set_playhead(self.playhead);
    }

    fn move_clip(&mut self, clip: Id, new_index: usize) {
        let old_index = self.timeline().clip_index(clip);
        let last_index = self.timeline().clips().len().saturating_sub(1);
        if old_index == Some(new_index.min(last_index)) {
            return;
        }
        match self.editor.edit(|tl| tl.move_clip(clip, new_index)) {
            Ok(()) => self.say("Klip dipindah"),
            Err(e) => self.say(labels::error(e)),
        }
    }

    fn trim_clip(&mut self, clip: Id, edge: Edge, new_len: Ms) {
        let unchanged = match self.timeline().clip(clip) {
            Some(item) => item.len == new_len,
            None => true,
        };
        if unchanged {
            return;
        }
        match self.editor.edit(|tl| tl.trim_clip(clip, edge, new_len)) {
            Ok(()) => self.say(format!("Panjang klip: {}", labels::seconds(new_len))),
            Err(e) => self.say(labels::error(e)),
        }
    }

    fn move_text(&mut self, text: Id, start: Ms) {
        let unchanged = match self.timeline().text(text) {
            Some(item) => item.start == start,
            None => true,
        };
        if unchanged {
            return;
        }
        match self.editor.edit(|tl| tl.move_text(text, start)) {
            Ok(()) => self.say(format!(
                "Teks dipindah ke detik {}",
                labels::seconds_number(start)
            )),
            Err(e) => self.say(labels::error(e)),
        }
    }

    fn trim_text(&mut self, text: Id, edge: Edge, new_len: Ms) {
        let unchanged = match self.timeline().text(text) {
            Some(item) => item.len == new_len,
            None => true,
        };
        if unchanged {
            return;
        }
        match self.editor.edit(|tl| tl.trim_text(text, edge, new_len)) {
            Ok(()) => self.say(format!("Panjang teks: {}", labels::seconds(new_len))),
            Err(e) => self.say(labels::error(e)),
        }
    }

    fn trim_audio(&mut self, audio: Id, edge: Edge, new_len: Ms) {
        let unchanged = match self.timeline().audio(audio) {
            Some(item) => item.len == new_len,
            None => true,
        };
        if unchanged {
            return;
        }
        match self.editor.edit(|tl| tl.trim_audio(audio, edge, new_len)) {
            Ok(()) => self.say(format!("Panjang audio: {}", labels::seconds(new_len))),
            Err(e) => self.say(labels::error(e)),
        }
    }

    /// Menggeser audio terpilih sedikit dengan tombol panah.
    fn nudge_audio(&mut self, audio: Id, delta_ms: i64) {
        let Some(item) = self.timeline().audio(audio) else {
            return;
        };
        let lane = item.lane;
        let start = (item.start as i64 + delta_ms).max(0) as Ms;
        let result = self
            .editor
            .edit_merged(merge_tag(audio, 9), |tl| tl.move_audio(audio, start, lane));
        if let Err(e) = result {
            self.say(labels::error(e));
        }
    }

    // ---- tiap frame ----

    /// Dijalankan di awal tiap frame, sebelum panel digambar.
    fn begin_frame(&mut self, ui: &egui::Ui) {
        let (now, dt) = ui.input(|i| (i.time, i.stable_dt));
        self.now = now;

        if self.playing {
            let end = self.timeline().duration() as f64;
            self.playhead += dt as f64 * 1000.0;
            if self.playhead >= end {
                self.playhead = end;
                self.playing = false;
            }
            ui.ctx().request_repaint();
        }

        if self.now < self.toast_until {
            ui.ctx().request_repaint_after_secs(0.2);
        }

        // Berkas yang diseret dari Explorer lalu dilepas di jendela aplikasi.
        let dropped: Vec<PathBuf> = ui.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|file| file.path().to_owned())
                .collect()
        });
        if !dropped.is_empty() {
            self.import_paths(&dropped);
        }

        self.drop_stale_selection();
        self.handle_keys(ui);
    }

    /// Setelah Undo atau Hapus, item yang dipilih bisa sudah tidak ada.
    fn drop_stale_selection(&mut self) {
        let tl = self.timeline();
        let still_exists = match self.selection {
            Selection::None => true,
            Selection::Clip(id) => tl.clip(id).is_some(),
            Selection::Text(id) => tl.text(id).is_some(),
            Selection::Audio(id) => tl.audio(id).is_some(),
            Selection::Junction(id) => match tl.clip_index(id) {
                Some(index) => index + 1 < tl.clips().len(),
                None => false,
            },
        };
        if !still_exists {
            self.selection = Selection::None;
        }
    }

    /// Pintasan papan ketik. Tidak aktif saat pengguna sedang mengetik teks.
    fn handle_keys(&mut self, ui: &egui::Ui) {
        if ui.ctx().text_edit_focused() {
            return;
        }
        use egui::{Key, Modifiers};

        let (undo, redo, space, delete, left, right, shift) = ui.input_mut(|i| {
            let redo = i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z)
                || i.consume_key(Modifiers::COMMAND, Key::Y);
            let undo = i.consume_key(Modifiers::COMMAND, Key::Z);
            (
                undo,
                redo,
                i.key_pressed(Key::Space),
                i.key_pressed(Key::Delete),
                i.key_pressed(Key::ArrowLeft),
                i.key_pressed(Key::ArrowRight),
                i.modifiers.shift,
            )
        });

        if undo {
            self.undo();
        }
        if redo {
            self.redo();
        }
        if space {
            self.toggle_play();
        }
        if delete && self.can_delete() {
            self.delete_selected();
        }
        if left || right {
            let step: i64 = if shift { 1_000 } else { 100 };
            let delta = if left { -step } else { step };
            match self.selection {
                Selection::Audio(id) => self.nudge_audio(id, delta),
                _ => {
                    self.playing = false;
                    self.set_playhead(self.playhead + delta as f64);
                }
            }
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.begin_frame(ui);

        // Urutan penting: panel tepi dulu, pratinjau terakhir mengisi sisa ruang.
        top_bar::show(self, ui);
        timeline_view::show(self, ui);
        media_panel::show(self, ui);
        inspector::show(self, ui);
        preview::show(self, ui);

        top_bar::show_export_window(self, ui.ctx());
    }
}

/// Tanda untuk menggabung perubahan beruntun (mengetik, menggeser slider)
/// menjadi satu langkah Undo. `field` membedakan isian yang berbeda pada item yang sama.
pub fn merge_tag(id: Id, field: u64) -> u64 {
    id * 16 + field
}

/// Proyek contoh yang tampil saat aplikasi dibuka.
fn sample_project() -> Timeline {
    let mut tl = Timeline::new();
    tl.push_clip("Klip A", 4_000, 0, None);
    tl.push_clip("Klip B", 5_000, 1, None);
    tl.push_clip("Klip C", 3_000, 2, None);
    tl.add_text(5_500, 3_000, "Halo!");
    // Lajur 0 kosong di proyek baru, jadi ini tidak bisa gagal.
    let _ = tl.add_music("Musik latar", 0, 8_000, None);
    tl
}
