//! Menemukan dan menjalankan FFmpeg.
//!
//! PENTING soal lisensi: FFmpeg TIDAK digabung ke dalam OxideCut.
//! FFmpeg adalah program terpisah (`ffmpeg.exe` dan `ffprobe.exe`) yang kita
//! jalankan seperti menjalankan aplikasi lain, lalu kita baca hasilnya.
//! Karena itu kode OxideCut tetap MIT. Jangan menambahkan crate yang
//! menautkan pustaka FFmpeg ke dalam aplikasi (misalnya `ffmpeg-next`).
//!
//! Di Windows, FFmpeg versi LGPL disertakan di folder `ffmpeg` di samping
//! `OxideCut.exe` (lihat `.github/workflows/build.yml`).

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

use oxidecut_core::{Effect, Ms};

pub struct Tools {
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
}

/// Lokasi FFmpeg di komputer ini, atau `None` kalau tidak ada.
/// Dicari sekali saja, hasilnya diingat.
pub fn tools() -> Option<&'static Tools> {
    static TOOLS: OnceLock<Option<Tools>> = OnceLock::new();
    TOOLS.get_or_init(find_tools).as_ref()
}

fn find_tools() -> Option<Tools> {
    Some(Tools {
        ffmpeg: find_program("ffmpeg")?,
        ffprobe: find_program("ffprobe")?,
    })
}

fn find_program(name: &str) -> Option<PathBuf> {
    let file_name = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    };

    // 1. Yang disertakan bersama OxideCut: folder "ffmpeg" di samping aplikasi,
    //    atau folder yang sama dengan aplikasi.
    if let Ok(app_path) = std::env::current_exe() {
        if let Some(app_folder) = app_path.parent() {
            let candidates = [
                app_folder.join("ffmpeg").join(&file_name),
                app_folder.join(&file_name),
            ];
            for candidate in candidates {
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }

    // 2. FFmpeg yang sudah terpasang di komputer.
    let installed = command(Path::new(&file_name))
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false);
    if installed {
        Some(PathBuf::from(file_name))
    } else {
        None
    }
}

/// Perintah untuk menjalankan `program` tanpa memunculkan jendela konsol hitam di Windows.
pub fn command(program: &Path) -> Command {
    let mut command = Command::new(program);
    command.stdin(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

/// Keterangan dasar sebuah berkas media.
#[derive(Debug, PartialEq)]
pub struct MediaInfo {
    pub duration: Ms,
    pub has_video: bool,
    pub has_audio: bool,
}

/// Membaca durasi dan jenis isi berkas lewat `ffprobe`.
pub fn probe(path: &Path) -> Option<MediaInfo> {
    let tools = tools()?;
    let output = command(&tools.ffprobe)
        .args(["-v", "error"])
        .args(["-show_entries", "format=duration:stream=codec_type"])
        .args(["-of", "default=noprint_wrappers=1"])
        .arg(path)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_probe_output(&String::from_utf8_lossy(&output.stdout))
}

/// Keluaran `ffprobe` berupa baris-baris `nama=nilai`, misalnya:
/// ```text
/// codec_type=video
/// codec_type=audio
/// duration=7.500000
/// ```
fn parse_probe_output(text: &str) -> Option<MediaInfo> {
    let mut duration = None;
    let mut has_video = false;
    let mut has_audio = false;

    for line in text.lines() {
        match line.trim().split_once('=') {
            Some(("codec_type", "video")) => has_video = true,
            Some(("codec_type", "audio")) => has_audio = true,
            Some(("duration", value)) => {
                if let Ok(seconds) = value.parse::<f64>() {
                    if seconds > 0.0 {
                        duration = Some((seconds * 1000.0).round() as Ms);
                    }
                }
            }
            _ => {}
        }
    }

    Some(MediaInfo {
        duration: duration?,
        has_video,
        has_audio,
    })
}

/// Rangkaian filter FFmpeg untuk satu klip: efek, lalu pas-kan ke bingkai
/// `width` x `height` (sisanya hitam), lalu samakan jumlah frame per detik.
///
/// Fungsi ini sengaja ada di satu tempat: pratinjau memakainya sekarang,
/// dan ekspor nanti harus memakai fungsi yang sama supaya hasilnya identik.
pub fn video_filter(effect: Effect, width: usize, height: usize, fps: u64) -> String {
    // Zoom memotong bagian tengah gambar sebelum dipaskan ke bingkai.
    let before_fit = match effect {
        Effect::Zoom => "crop=iw/1.35:ih/1.35,",
        _ => "",
    };

    let fit = format!(
        "scale={width}:{height}:force_original_aspect_ratio=decrease,\
         pad={width}:{height}:(ow-iw)/2:(oh-ih)/2:black,setsar=1"
    );

    let after_fit = match effect {
        Effect::BlackWhite => ",hue=s=0".to_owned(),
        // Kekuatan blur mengikuti tinggi bingkai supaya tampak sama di ukuran apa pun.
        Effect::Blur => format!(",gblur=sigma={}", (height / 90).max(1)),
        Effect::Warm => ",colorbalance=rm=0.18:bm=-0.18:rh=0.10:bh=-0.10".to_owned(),
        Effect::None | Effect::Zoom => String::new(),
    };

    format!("{before_fit}{fit}{after_fit},fps={fps}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_output_with_video_and_audio() {
        let text = "codec_type=video\ncodec_type=audio\nduration=7.500000\n";
        assert_eq!(
            parse_probe_output(text),
            Some(MediaInfo {
                duration: 7_500,
                has_video: true,
                has_audio: true
            })
        );
    }

    #[test]
    fn probe_output_audio_only_with_windows_line_endings() {
        let text = "codec_type=audio\r\nduration=183.027000\r\n";
        assert_eq!(
            parse_probe_output(text),
            Some(MediaInfo {
                duration: 183_027,
                has_video: false,
                has_audio: true
            })
        );
    }

    #[test]
    fn probe_output_without_usable_duration_is_rejected() {
        assert_eq!(parse_probe_output("codec_type=video\nduration=N/A\n"), None);
        assert_eq!(parse_probe_output(""), None);
    }

    #[test]
    fn filter_always_fits_frame_and_sets_fps() {
        let plain = video_filter(Effect::None, 304, 540, 30);
        assert!(
            plain.starts_with("scale=304:540:force_original_aspect_ratio=decrease,pad=304:540:")
        );
        assert!(plain.ends_with(",fps=30"));

        assert!(video_filter(Effect::Zoom, 304, 540, 30).starts_with("crop=iw/1.35:ih/1.35,scale="));
        assert!(video_filter(Effect::BlackWhite, 304, 540, 30).contains(",hue=s=0,"));
        assert!(video_filter(Effect::Blur, 304, 540, 30).contains(",gblur=sigma=6,"));
    }
}
