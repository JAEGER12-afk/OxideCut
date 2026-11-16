//! Daftar media (panel kiri) dan pembacaan durasi berkas.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use oxidecut_core::Ms;

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
            path: None,
        },
    ]
}

/// Membuat entri media dari berkas di komputer pengguna.
pub fn from_file(path: &Path, tint: u8) -> MediaItem {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let kind = if AUDIO_EXTENSIONS.contains(&extension.as_str()) {
        MediaKind::Audio
    } else {
        MediaKind::Video
    };
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Media")
        .to_owned();
    let duration = read_duration(path);
    MediaItem {
        name,
        kind,
        len: duration.unwrap_or(FALLBACK_LEN),
        len_known: duration.is_some(),
        tint,
        path: Some(path.to_owned()),
    }
}

/// Membaca durasi berkas. Saat ini hanya keluarga MP4 (mp4, mov, m4v, m4a).
/// Format lain akan terbaca setelah FFmpeg dipasang.
fn read_duration(path: &Path) -> Option<Ms> {
    let mut file = File::open(path).ok()?;
    mp4_duration(&mut file)
}

/// Durasi berkas MP4 dari kotak `moov` > `mvhd`.
///
/// Berkas MP4 tersusun dari "kotak": 4 byte ukuran, 4 byte nama, lalu isi.
/// Durasi keseluruhan ada di kotak `mvhd`, yang berada di dalam kotak `moov`.
pub fn mp4_duration<R: Read + Seek>(reader: &mut R) -> Option<Ms> {
    let file_len = reader.seek(SeekFrom::End(0)).ok()?;
    let (moov_start, moov_end) = find_box(reader, 0, file_len, b"moov")?;
    let (mvhd_start, _) = find_box(reader, moov_start, moov_end, b"mvhd")?;

    reader.seek(SeekFrom::Start(mvhd_start)).ok()?;
    let version = read_bytes::<1, R>(reader)?[0];
    read_bytes::<3, R>(reader)?; // flags

    let (timescale, duration) = if version == 1 {
        read_bytes::<16, R>(reader)?; // waktu dibuat dan diubah (64-bit)
        let timescale = u32::from_be_bytes(read_bytes(reader)?) as u64;
        let duration = u64::from_be_bytes(read_bytes(reader)?);
        (timescale, duration)
    } else {
        read_bytes::<8, R>(reader)?; // waktu dibuat dan diubah (32-bit)
        let timescale = u32::from_be_bytes(read_bytes(reader)?) as u64;
        let duration = u32::from_be_bytes(read_bytes(reader)?) as u64;
        (timescale, duration)
    };

    if timescale == 0 || duration == 0 {
        return None;
    }
    Some(duration.saturating_mul(1000) / timescale)
}

/// Mencari kotak bernama `name` di antara `start` dan `end`.
/// Mengembalikan posisi awal dan akhir ISI kotak itu.
fn find_box<R: Read + Seek>(
    reader: &mut R,
    start: u64,
    end: u64,
    name: &[u8; 4],
) -> Option<(u64, u64)> {
    let mut position = start;
    while position + 8 <= end {
        reader.seek(SeekFrom::Start(position)).ok()?;
        let size32 = u32::from_be_bytes(read_bytes(reader)?) as u64;
        let box_name: [u8; 4] = read_bytes(reader)?;

        let mut header_len = 8;
        let box_len = if size32 == 1 {
            // Ukuran 64-bit menyusul setelah nama.
            header_len = 16;
            u64::from_be_bytes(read_bytes(reader)?)
        } else if size32 == 0 {
            // Kotak ini memanjang sampai akhir.
            end - position
        } else {
            size32
        };
        if box_len < header_len || position + box_len > end {
            return None;
        }
        if &box_name == name {
            return Some((position + header_len, position + box_len));
        }
        position += box_len;
    }
    None
}

fn read_bytes<const N: usize, R: Read>(reader: &mut R) -> Option<[u8; N]> {
    let mut buffer = [0u8; N];
    reader.read_exact(&mut buffer).ok()?;
    Some(buffer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn boxed(name: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut out = ((body.len() + 8) as u32).to_be_bytes().to_vec();
        out.extend_from_slice(name);
        out.extend_from_slice(body);
        out
    }

    fn mvhd_v0(timescale: u32, duration: u32) -> Vec<u8> {
        let mut body = vec![0u8; 4 + 8];
        body.extend_from_slice(&timescale.to_be_bytes());
        body.extend_from_slice(&duration.to_be_bytes());
        body.extend_from_slice(&[0u8; 80]);
        boxed(b"mvhd", &body)
    }

    #[test]
    fn reads_duration_when_moov_comes_after_media_data() {
        let mut file = boxed(b"ftyp", b"isom0000");
        file.extend(boxed(b"mdat", &[7u8; 300]));
        file.extend(boxed(b"moov", &mvhd_v0(600, 4500)));
        assert_eq!(mp4_duration(&mut Cursor::new(file)), Some(7_500));
    }

    #[test]
    fn reads_64_bit_version() {
        let mut body = vec![1u8, 0, 0, 0];
        body.extend_from_slice(&[0u8; 16]);
        body.extend_from_slice(&1000u32.to_be_bytes());
        body.extend_from_slice(&12_345u64.to_be_bytes());
        let file = boxed(b"moov", &boxed(b"mvhd", &body));
        assert_eq!(mp4_duration(&mut Cursor::new(file)), Some(12_345));
    }

    #[test]
    fn returns_none_for_files_that_are_not_mp4() {
        assert_eq!(
            mp4_duration(&mut Cursor::new(b"RIFF....WAVEfmt ".to_vec())),
            None
        );
        assert_eq!(mp4_duration(&mut Cursor::new(Vec::new())), None);
        assert_eq!(
            mp4_duration(&mut Cursor::new(boxed(b"moov", &[0u8; 4]))),
            None
        );
    }
}
