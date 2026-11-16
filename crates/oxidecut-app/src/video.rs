//! Mengambil gambar (frame) video asli untuk pratinjau.
//!
//! Cara kerjanya, untuk tiap klip yang sedang tampil:
//! 1. Jalankan FFmpeg mulai dari waktu yang diminta. FFmpeg mengirim frame demi
//!    frame berupa piksel mentah lewat pipa.
//! 2. Satu thread kecil membaca pipa itu dan menaruh frame di antrean pendek.
//!    Kalau antrean penuh, FFmpeg otomatis menunggu, jadi tidak boros memori.
//! 3. Tiap kali layar digambar, kita ambil frame dari antrean sampai waktunya
//!    menyusul garis oranye, lalu menampilkannya sebagai tekstur.
//!
//! Saat diputar, frame mengalir terus. Saat garis oranye digeser jauh atau
//! mundur, FFmpeg dihentikan dan dijalankan lagi dari waktu yang baru.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::mpsc::{sync_channel, Receiver, TryRecvError};
use std::time::{Duration, Instant};

use eframe::egui;
use oxidecut_core::{Effect, Id, Ms};

use crate::app::Ratio;
use crate::ffmpeg;

/// Jumlah frame per detik di pratinjau.
pub const FPS: u64 = 30;

/// Lama satu frame dalam milidetik (dibulatkan).
const FRAME_MS: Ms = 1000 / FPS;

/// Banyaknya frame yang boleh menunggu di antrean.
const QUEUE_LEN: usize = 3;

/// Ukuran gambar pratinjau (lebar, tinggi) untuk tiap rasio. Harus genap.
pub fn frame_size(ratio: Ratio) -> (usize, usize) {
    match ratio {
        Ratio::Portrait => (304, 540),
        Ratio::Square => (540, 540),
        Ratio::Landscape => (960, 540),
    }
}

/// Permintaan: "gambar klip ini pada waktu sekian".
pub struct FrameRequest<'a> {
    pub clip_id: Id,
    pub path: &'a Path,
    /// Waktu di dalam berkas sumber.
    pub source_ms: Ms,
    pub effect: Effect,
    pub size: (usize, usize),
}

/// Kumpulan pemutar, satu per klip yang sedang tampil (biasanya 1, saat transisi 2).
pub struct VideoPool {
    players: Vec<Player>,
    /// Nomor urut penggambaran layar, untuk tahu pemutar mana yang masih dipakai.
    paint_number: u64,
}

impl VideoPool {
    pub fn new() -> Self {
        Self {
            players: Vec::new(),
            paint_number: 0,
        }
    }

    /// Panggil sebelum meminta frame untuk satu kali penggambaran layar.
    pub fn begin_paint(&mut self) {
        self.paint_number += 1;
    }

    /// Panggil setelah selesai. Pemutar yang tidak diminta lagi dihentikan.
    pub fn end_paint(&mut self) {
        let current = self.paint_number;
        self.players.retain(|player| player.last_used == current);
    }

    /// Tekstur berisi gambar yang diminta, atau `None` kalau belum ada frame sama sekali.
    pub fn frame(
        &mut self,
        ctx: &egui::Context,
        request: FrameRequest<'_>,
    ) -> Option<egui::TextureId> {
        let index = match self
            .players
            .iter()
            .position(|p| p.clip_id == request.clip_id)
        {
            Some(index) => index,
            None => {
                self.players.push(Player::new(request.clip_id));
                self.players.len() - 1
            }
        };
        let player = &mut self.players[index];
        player.last_used = self.paint_number;
        player.update(ctx, &request);
        player.texture.as_ref().map(|texture| texture.id())
    }
}

/// Pemutar untuk satu klip.
struct Player {
    clip_id: Id,
    stream: Option<Stream>,
    /// Gambar terakhir yang berhasil diambil. Tetap ditampilkan selagi menunggu yang baru.
    texture: Option<egui::TextureHandle>,
    last_used: u64,
    /// Kapan terakhir mencoba menjalankan FFmpeg, supaya kegagalan tidak diulang tiap frame.
    last_start: Option<Instant>,
}

impl Player {
    fn new(clip_id: Id) -> Self {
        Self {
            clip_id,
            stream: None,
            texture: None,
            last_used: 0,
            last_start: None,
        }
    }

    fn update(&mut self, ctx: &egui::Context, request: &FrameRequest<'_>) {
        if self.needs_restart(request) {
            self.last_start = Some(Instant::now());
            // Menimpa aliran lama otomatis menghentikan FFmpeg yang lama.
            self.stream = Stream::start(ctx, request);
        }

        let Some(stream) = self.stream.as_mut() else {
            return;
        };

        // Ambil frame dari antrean sampai menyusul waktu yang diminta.
        // Frame pertama selalu diambil supaya layar segera terisi.
        let mut newest: Option<Vec<u8>> = None;
        loop {
            let caught_up = stream.taken > 0 && stream.next_frame_ms() > request.source_ms;
            if caught_up {
                break;
            }
            match stream.frames.try_recv() {
                Ok(frame) => {
                    newest = Some(frame);
                    stream.taken += 1;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    stream.ended = true;
                    break;
                }
            }
        }

        if let Some(pixels) = newest {
            let (width, height) = stream.size;
            let image = egui::ColorImage::from_rgba_unmultiplied([width, height], &pixels);
            match self.texture.as_mut() {
                Some(texture) => texture.set(image, egui::TextureOptions::LINEAR),
                None => {
                    let name = format!("video-{}", self.clip_id);
                    self.texture =
                        Some(ctx.load_texture(name, image, egui::TextureOptions::LINEAR));
                }
            }
        }
    }

    /// Perlu menjalankan FFmpeg dari awal lagi?
    fn needs_restart(&self, request: &FrameRequest<'_>) -> bool {
        let Some(stream) = &self.stream else {
            // Belum pernah jalan, atau gagal jalan. Coba lagi paling cepat sedetik sekali.
            return match self.last_start {
                Some(at) => at.elapsed() > Duration::from_secs(1),
                None => true,
            };
        };

        // Berkas, efek, atau ukuran berubah: gambarnya pasti berbeda.
        let changed = stream.path != request.path
            || stream.effect != request.effect
            || stream.size != request.size;
        if changed {
            return true;
        }

        // Beri FFmpeg kesempatan mengirim frame pertamanya dulu. Tanpa ini,
        // menggeser garis oranye dengan cepat akan terus-menerus memulai ulang.
        let settled = stream.taken > 0
            || stream.ended
            || stream.started.elapsed() > Duration::from_millis(400);
        if !settled {
            return false;
        }

        // Mundur: frame yang sudah lewat tidak bisa diminta lagi dari aliran ini.
        let went_backward = request.source_ms + FRAME_MS < stream.shown_ms();
        // Maju jauh: lebih cepat mulai ulang daripada membuang ratusan frame.
        let far_ahead = !stream.ended && request.source_ms > stream.next_frame_ms() + 1_000;
        went_backward || far_ahead
    }
}

/// Satu proses FFmpeg yang sedang mengirim frame.
struct Stream {
    path: PathBuf,
    effect: Effect,
    size: (usize, usize),
    /// Waktu sumber untuk frame pertama aliran ini.
    start_ms: Ms,
    /// Jumlah frame yang sudah diambil dari antrean.
    taken: u64,
    ended: bool,
    started: Instant,
    frames: Receiver<Vec<u8>>,
    process: Child,
}

impl Stream {
    fn start(ctx: &egui::Context, request: &FrameRequest<'_>) -> Option<Stream> {
        let tools = ffmpeg::tools()?;
        let (width, height) = request.size;
        let seconds = format!("{:.3}", request.source_ms as f64 / 1000.0);
        let filter = ffmpeg::video_filter(request.effect, width, height, FPS);

        let mut process = ffmpeg::command(&tools.ffmpeg)
            .args(["-v", "error", "-nostdin"])
            .args(["-ss", &seconds])
            .arg("-i")
            .arg(request.path)
            .args(["-an", "-sn"])
            .args(["-vf", &filter])
            .args(["-f", "rawvideo", "-pix_fmt", "rgba", "-"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;

        let mut pipe = process.stdout.take()?;
        let (sender, receiver) = sync_channel::<Vec<u8>>(QUEUE_LEN);
        let frame_len = width * height * 4;
        let ctx = ctx.clone();

        // Thread pembaca. Berhenti sendiri saat FFmpeg selesai atau saat
        // `Stream` ini dibuang (pengiriman ke antrean gagal).
        std::thread::spawn(move || loop {
            let mut frame = vec![0u8; frame_len];
            if pipe.read_exact(&mut frame).is_err() {
                break;
            }
            if sender.send(frame).is_err() {
                break;
            }
            // Ada frame baru: minta layar digambar ulang.
            ctx.request_repaint();
        });

        Some(Stream {
            path: request.path.to_owned(),
            effect: request.effect,
            size: request.size,
            start_ms: request.source_ms,
            taken: 0,
            ended: false,
            started: Instant::now(),
            frames: receiver,
            process,
        })
    }

    /// Waktu sumber untuk frame berikutnya yang akan diambil.
    fn next_frame_ms(&self) -> Ms {
        self.start_ms + self.taken * 1000 / FPS
    }

    /// Waktu sumber untuk frame yang sedang tampil.
    fn shown_ms(&self) -> Ms {
        self.start_ms + self.taken.saturating_sub(1) * 1000 / FPS
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        // Hentikan FFmpeg dan tunggu sampai benar-benar selesai.
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}
