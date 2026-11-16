//! Inti timeline OxideCut.
//!
//! Crate ini murni Rust: tidak tahu apa-apa soal UI, GPU, atau FFmpeg.
//! Semua aturan edit (potong, hapus, pisahkan audio, pindah audio, undo/redo)
//! ada di sini supaya bisa diuji otomatis tanpa membuka aplikasi.
//!
//! - [`Timeline`] menyimpan isi proyek dan menyediakan operasi edit.
//! - [`Editor`] membungkus `Timeline` dengan riwayat undo/redo.
//!
//! Waktu selalu dalam milidetik ([`Ms`]) supaya tidak ada galat pembulatan.

mod editor;
mod timeline;

pub use editor::Editor;
pub use timeline::{
    Audible, AudioItem, AudioKind, Clip, ClipAudio, EditError, Effect, Id, Ms, TextItem, Timeline,
    Transition, AUDIO_LANES, MIN_PIECE_MS,
};
