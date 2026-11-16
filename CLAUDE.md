# OxideCut: panduan untuk pengembang dan asisten AI

Editor video Windows yang gratis, open-source (MIT), ditulis dengan Rust.
Pemilik proyek adalah desainer (bukan programmer) dan menguji lewat `.exe` hasil GitHub Actions.

## Aturan utama

1. **KISS.** Tulis kode yang lurus dan gamblang. Kode lebih panjang tidak apa-apa asal mudah dibaca.
   Hindari trait, generic, macro, dan abstraksi baru kecuali benar-benar perlu.
   Satu berkas = satu tanggung jawab. Fungsi biasa lebih baik daripada pola pintar.
2. **Straight forward untuk pengguna.** Aksi utama (tambah teks, potong, transisi, efek, ekspor)
   maksimal dua klik, tanpa dialog yang menghalangi, hasilnya langsung terlihat di pratinjau.
   Panel kanan hanya menampilkan yang relevan dengan pilihan saat ini. Jangan menambah tab atau menu.
3. **Aturan edit hanya di `oxidecut-core`.** UI tidak boleh memutuskan apa yang sah.
   Tiap aturan baru di core wajib punya tes.
4. **Bahasa.** Teks antarmuka dan komentar: bahasa Indonesia. Nama fungsi dan variabel: bahasa Inggris.
5. **Ikon.** Hanya Fluent System Icons dari Microsoft. Cara menambah ikon ada di `crates/oxidecut-app/src/icons.rs`.
6. **Desain acuan.** `docs/prototipe-ui.html` adalah prototipe yang sudah disetujui pemilik. Buka di browser.

## Peta proyek

```
crates/oxidecut-core/   Inti timeline. Murni Rust, tanpa UI, tanpa FFmpeg.
  src/timeline.rs       Isi proyek (klip, teks, audio) dan semua operasi edit + tes.
  src/editor.rs         Undo/redo (menyimpan salinan Timeline sebelum tiap perubahan).
crates/oxidecut-app/    Aplikasi desktop (egui/eframe 0.37, renderer wgpu).
  src/main.rs           Membuka jendela. Di atasnya ada peta berkas aplikasi.
  src/app.rs            Keadaan aplikasi dan semua aksi pengguna.
  src/*_panel.rs, preview.rs, inspector.rs, timeline_view.rs   Satu berkas per bagian layar.
assets/fonts/           Font ikon Fluent (MIT, Microsoft).
.github/workflows/      Tes + build Windows otomatis.
```

Alur data: panel memanggil aksi di `app.rs` → aksi memanggil `Editor::edit(...)` di core →
core mengubah `Timeline` atau menolak dengan `EditError` → `app.rs` menampilkan pesan singkat.

## Model timeline (ringkas)

- Waktu dalam milidetik (`Ms = u64`).
- Jalur video magnetik: klip selalu rapat berurutan, tanpa celah.
- Transisi disimpan di klip kiri sebuah sambungan (`Clip::transition_out`).
- Suara klip: `Attached` (bawaan), `Detached { group }` (dipisah ke jalur Audio), atau `Removed`.
- Audio terpisah bebas dipindah di dua lajur dan tidak boleh bertumpuk dalam satu lajur.

## Perintah

```
cargo test --workspace          # wajib lulus sebelum commit
cargo clippy --workspace --all-targets
cargo fmt --all
cargo run -p oxidecut-app       # menjalankan aplikasi
```

Di Linux tanpa layar, tampilan bisa diperiksa dengan `Xvfb` lalu `import -window root shot.png`.

## Status

Sudah jalan (dengan klip contoh berwarna, belum video asli):
timeline, potong, hapus, teks, efek, transisi beserta animasinya di pratinjau,
pisahkan audio, seret audio dengan tempelan otomatis, undo/redo, impor berkas ke panel Media
(durasi terbaca untuk keluarga MP4).

Belum ada, urut prioritas:
1. Memutar video asli di pratinjau (FFmpeg, tautan dinamis, build LGPL: jangan pakai `--enable-gpl`
   atau `--enable-nonfree`, hindari libx264). Yang diganti hanya `preview::paint_scene`.
2. Ekspor MP4 (jendela Ekspor sudah ada, tombolnya masih nonaktif).
3. Memutar suara.
4. Simpan dan buka proyek.
5. Memindah dan mengubah panjang teks serta klip dengan seret; zoom timeline.
6. Teks yang waktunya bertumpuk masih digambar bertumpuk di satu baris.
7. Distribusi: ZIP portabel lewat winget, lalu tanda tangan gratis SignPath Foundation.
