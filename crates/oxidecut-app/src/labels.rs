//! Teks bahasa Indonesia untuk antarmuka dan format waktu.

use oxidecut_core::{AudioItem, AudioKind, EditError, Effect, Ms, Transition};

pub fn effect(effect: Effect) -> &'static str {
    match effect {
        Effect::None => "Tanpa",
        Effect::BlackWhite => "Hitam putih",
        Effect::Blur => "Blur",
        Effect::Warm => "Hangat",
        Effect::Zoom => "Zoom",
    }
}

pub fn transition(transition: Transition) -> &'static str {
    match transition {
        Transition::Fade => "Pudar",
        Transition::Slide => "Geser",
        Transition::Zoom => "Zoom",
        Transition::Wipe => "Sapu",
    }
}

/// Pesan untuk pengguna saat sebuah aksi ditolak oleh inti timeline.
pub fn error(error: EditError) -> &'static str {
    match error {
        EditError::NotFound => "Itemnya sudah tidak ada",
        EditError::TooCloseToEdge => "Geser garis oranye ke tengah dulu",
        EditError::Overlap => "Tempat itu sudah terisi audio lain",
        EditError::NoFreeLane => "Jalur audio penuh di posisi itu. Pindahkan audio lain dulu",
        EditError::BadLane => "Lajur audio tidak ada",
        EditError::AudioNotAttached => "Suara klip ini sudah dipisah atau dihapus",
        EditError::NotLinked => "Audio ini tidak punya klip video asal",
        EditError::NoJunction => "Klip terakhir tidak punya sambungan",
    }
}

/// 75_000 ms menjadi "01:15".
pub fn clock(ms: Ms) -> String {
    let seconds = ms / 1000;
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}

/// 7_500 ms menjadi "7,5".
pub fn seconds_number(ms: Ms) -> String {
    let tenths = (ms + 50) / 100;
    if tenths.is_multiple_of(10) {
        format!("{}", tenths / 10)
    } else {
        format!("{},{}", tenths / 10, tenths % 10)
    }
}

/// 7_500 ms menjadi "7,5 detik".
pub fn seconds(ms: Ms) -> String {
    format!("{} detik", seconds_number(ms))
}

/// Nama potongan audio seperti yang tampil di layar.
pub fn audio_name(audio: &AudioItem) -> String {
    match audio.kind {
        AudioKind::Music => audio.name.clone(),
        AudioKind::Detached => format!("Audio {}", audio.name),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_formats_minutes_and_seconds() {
        assert_eq!(clock(0), "00:00");
        assert_eq!(clock(3_999), "00:03");
        assert_eq!(clock(75_000), "01:15");
    }

    #[test]
    fn seconds_uses_indonesian_decimal_comma() {
        assert_eq!(seconds(7_500), "7,5 detik");
        assert_eq!(seconds(4_000), "4 detik");
        assert_eq!(seconds(4_960), "5 detik");
    }
}
