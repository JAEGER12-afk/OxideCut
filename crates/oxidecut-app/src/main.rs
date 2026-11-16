//! Aplikasi desktop OxideCut.
//!
//! Peta berkas (satu berkas = satu tanggung jawab):
//! - `main.rs`       membuka jendela.
//! - `app.rs`        keadaan aplikasi dan semua aksi (potong, hapus, teks, ...).
//! - `theme.rs`      warna dan gaya.
//! - `icons.rs`      ikon Fluent dari Microsoft.
//! - `labels.rs`     teks bahasa Indonesia dan format waktu.
//! - `media.rs`      daftar media dan pembacaan durasi berkas.
//! - `widgets.rs`    tombol dan potongan tampilan kecil yang dipakai bersama.
//! - `top_bar.rs`    bilah atas dan jendela Ekspor.
//! - `media_panel.rs` panel kiri.
//! - `preview.rs`    pratinjau di tengah.
//! - `inspector.rs`  panel kanan yang berubah sesuai pilihan.
//! - `timeline_view.rs` timeline di bawah.
//!
//! Aturan edit (apa yang boleh dan tidak) TIDAK ada di sini.
//! Semuanya ada di crate `oxidecut-core` dan diuji di sana.

// Di Windows, jangan buka jendela konsol hitam saat aplikasi rilis dijalankan.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod icons;
mod inspector;
mod labels;
mod media;
mod media_panel;
mod preview;
mod theme;
mod timeline_view;
mod top_bar;
mod widgets;

use eframe::egui;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("OxideCut")
            .with_inner_size([1180.0, 760.0])
            .with_min_inner_size([980.0, 640.0]),
        ..Default::default()
    };

    eframe::run_native(
        "OxideCut",
        options,
        Box::new(|creation| {
            icons::install(&creation.egui_ctx);
            theme::install(&creation.egui_ctx);
            Ok(Box::new(app::App::new()))
        }),
    )
}
