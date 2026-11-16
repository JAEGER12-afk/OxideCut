//! Daftar media (panel kiri).

use std::path::{Path, PathBuf};

use oxidecut_core::Ms;

use crate::ffmpeg;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MediaKind {
    Video,
    Audio,
}

#[derive(Clone)]
pub struct MediaItem {
    pub name: String,
    pub kind: MediaKind,
    pub len: Ms,
    /// `false` kalau durasi berkas tidak terbaca dan `len` hanya perkiraan.
    pub len_known: bool,
    pub tint: u8,
    /// `false` kalau berkas video ini tidak berisi suara.
    pub has_audio: bool,
    /// `None` untuk media contoh bawaan.
    pub path: Option<PathBuf>,
}

/// Durasi yang dipakai kalau durasi berkas tidak terbaca.
pub const FALLBACK_LEN: Ms = 5_000;

pub const VIDEO_EXTENSIONS: [&str; 6] = ["mp4", "mov", "m4v", "mkv", "webm", "avi"];
pub const AUDIO_EXTENSIONS: [&str; 6] = ["mp3", "wav", "m4a", "aac", "flac", "ogg"];

/// Media contoh supaya aplikasi bisa langsung dicoba tanpa berkas.
pub fn sample_library() -> Vec<MediaItem> {
    let video = |name: &str, len: Ms, tint: u8| MediaItem {
        name: name.to_owned(),
        kind: MediaKind::Video,
        len,
        len_known: true,
        tint,
        has_audio: true,
        path: None,
    };
    vec![
        video("Klip A", 4_000, 0),
        video("Klip B", 5_000, 1),
        video("Klip C", 3_000, 2),
        video("Klip D", 4_000, 3),
        MediaItem {
            name: "Musik contoh".to_owned(),
            kind: MediaKind::Audio,
            len: 8_000,
            len_known: true,
            tint: 0,
            has_audio: true,
            path: None,
        },
    ]
}

/// Membuat entri media dari berkas di komputer pengguna.
/// Durasi dan jenis isinya dibaca lewat FFmpeg.
pub fn from_file(path: &Path, tint: u8) -> MediaItem {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Media")
        .to_owned();

    let info = ffmpeg::probe(path);

    // Audio kalau ekstensinya audio (MP3 bersampul tetap audio),
    // atau kalau berkasnya memang tidak berisi gambar.
    let audio_extension = AUDIO_EXTENSIONS.contains(&extension.as_str());
    let no_picture = match &info {
        Some(info) => !info.has_video,
        None => false,
    };
    let kind = if audio_extension || no_picture {
        MediaKind::Audio
    } else {
        MediaKind::Video
    };

    let (len, len_known) = match &info {
        Some(info) => (info.duration, true),
        None => (FALLBACK_LEN, false),
    };
    // Kalau berkasnya tidak terbaca, anggap saja ada suaranya.
    let has_audio = match &info {
        Some(info) => info.has_audio,
        None => true,
    };

    MediaItem {
        name,
        kind,
        len,
        len_known,
        tint,
        has_audio,
        path: Some(path.to_owned()),
    }
}
