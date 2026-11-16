//! Ikon Fluent System Icons dari Microsoft (lisensi MIT).
//!
//! Ikon disimpan sebagai font di `assets/fonts/FluentSystemIcons-Regular.ttf`.
//! Tiap ikon adalah satu karakter. Untuk menambah ikon baru:
//! 1. Cari namanya di https://github.com/microsoft/fluentui-system-icons
//! 2. Lihat kodenya di `fonts/FluentSystemIcons-Regular.json` pada repo itu
//!    (pakai varian `_20_regular`), ubah angkanya ke heksadesimal.
//! 3. Tambahkan satu baris `pub const` di bawah.

use eframe::egui::{self, FontFamily, RichText};

pub const ADD: char = '\u{f109}';
pub const ARROW_DOWNLOAD: char = '\u{f150}';
pub const ARROW_IMPORT: char = '\u{f159}';
pub const ARROW_REDO: char = '\u{f16e}';
pub const ARROW_UNDO: char = '\u{f199}';
pub const CHECKMARK: char = '\u{f294}';
pub const CUT: char = '\u{f33a}';
pub const DELETE: char = '\u{f34c}';
pub const DISMISS: char = '\u{f369}';
pub const LINK: char = '\u{f4e4}';
pub const MUSIC_NOTE: char = '\u{e854}';
pub const PAUSE: char = '\u{f5a1}';
pub const PLAY: char = '\u{f605}';
pub const PREVIOUS: char = '\u{f628}';
pub const SPEAKER: char = '\u{eb42}';
pub const SPEAKER_MUTE: char = '\u{eb4a}';
pub const SPLIT: char = '\u{eb60}';
pub const TEXT: char = '\u{ed63}';
pub const VIDEO_CLIP: char = '\u{edff}';

const FAMILY_NAME: &str = "fluent-icons";

pub fn family() -> FontFamily {
    FontFamily::Name(FAMILY_NAME.into())
}

/// Ikon sebagai teks, siap dipakai di tombol atau label.
pub fn icon(icon: char, size: f32) -> RichText {
    RichText::new(icon.to_string()).family(family()).size(size)
}

/// Font untuk menggambar ikon langsung dengan `Painter`.
pub fn font(size: f32) -> egui::FontId {
    egui::FontId::new(size, family())
}

/// Mendaftarkan font ikon. Dipanggil sekali saat aplikasi dibuka.
pub fn install(ctx: &egui::Context) {
    use egui::epaint::text::{FontInsert, FontPriority, InsertFontFamily};

    let bytes = include_bytes!("../../../assets/fonts/FluentSystemIcons-Regular.ttf");
    ctx.add_font(FontInsert::new(
        FAMILY_NAME,
        egui::FontData::from_static(bytes),
        vec![InsertFontFamily {
            family: family(),
            priority: FontPriority::Highest,
        }],
    ));
}
