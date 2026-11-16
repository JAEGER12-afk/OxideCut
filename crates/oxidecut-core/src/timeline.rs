use std::fmt;
use std::path::PathBuf;

/// Pengenal unik untuk klip, teks, dan potongan audio dalam satu proyek.
pub type Id = u64;

/// Waktu dalam milidetik.
pub type Ms = u64;

/// Jumlah lajur di jalur audio (musik dan audio yang dipisah dari video).
pub const AUDIO_LANES: u8 = 2;

/// Potongan hasil "Potong" tidak boleh lebih pendek dari ini.
pub const MIN_PIECE_MS: Ms = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Effect {
    #[default]
    None,
    BlackWhite,
    Blur,
    Warm,
    Zoom,
}

impl Effect {
    pub const ALL: [Effect; 5] = [
        Effect::None,
        Effect::BlackWhite,
        Effect::Blur,
        Effect::Warm,
        Effect::Zoom,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transition {
    Fade,
    Slide,
    Zoom,
    Wipe,
}

impl Transition {
    pub const ALL: [Transition; 4] = [
        Transition::Fade,
        Transition::Slide,
        Transition::Zoom,
        Transition::Wipe,
    ];
}

/// Keadaan suara bawaan sebuah klip video.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClipAudio {
    /// Suara menempel di video (bawaan).
    #[default]
    Attached,
    /// Suara sudah dipisah menjadi potongan audio dengan `group` ini.
    Detached { group: Id },
    /// Suara sudah dihapus; videonya bisu.
    Removed,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Clip {
    pub id: Id,
    pub name: String,
    /// Berkas sumber. `None` untuk klip contoh.
    pub source: Option<PathBuf>,
    /// Titik mulai di dalam berkas sumber.
    pub source_in: Ms,
    pub len: Ms,
    /// Indeks warna untuk tampilan sementara sebelum ada pratinjau video asli.
    pub tint: u8,
    pub effect: Effect,
    pub audio: ClipAudio,
    /// 0..=100
    pub volume: u8,
    /// Transisi dari klip ini ke klip berikutnya.
    pub transition_out: Option<Transition>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextItem {
    pub id: Id,
    pub start: Ms,
    pub len: Ms,
    pub text: String,
    /// Ukuran huruf relatif terhadap tinggi video 1080.
    pub size: u16,
    pub color: [u8; 3],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioKind {
    Music,
    /// Audio yang dipisah dari sebuah klip video.
    Detached,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AudioItem {
    pub id: Id,
    /// Potongan hasil memotong satu audio memakai `group` yang sama.
    pub group: Id,
    pub name: String,
    pub source: Option<PathBuf>,
    pub source_in: Ms,
    pub start: Ms,
    pub len: Ms,
    pub lane: u8,
    pub kind: AudioKind,
    /// 0..=100
    pub volume: u8,
}

/// Sumber suara yang terdengar pada satu titik waktu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Audible {
    ClipSound(Id),
    Audio(Id),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditError {
    NotFound,
    /// Titik potong terlalu dekat dengan tepi.
    TooCloseToEdge,
    /// Tempat tujuan sudah terisi audio lain.
    Overlap,
    /// Tidak ada lajur audio kosong pada rentang waktu itu.
    NoFreeLane,
    BadLane,
    /// Suara klip tidak sedang menempel di video.
    AudioNotAttached,
    /// Audio ini tidak punya klip video asal untuk digabung lagi.
    NotLinked,
    /// Klip terakhir tidak punya sambungan untuk transisi.
    NoJunction,
}

impl fmt::Display for EditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            EditError::NotFound => "item not found",
            EditError::TooCloseToEdge => "cut point is too close to an edge",
            EditError::Overlap => "target position overlaps another audio item",
            EditError::NoFreeLane => "no free audio lane in that time range",
            EditError::BadLane => "audio lane out of range",
            EditError::AudioNotAttached => "clip audio is not attached",
            EditError::NotLinked => "audio has no source clip to rejoin",
            EditError::NoJunction => "last clip has no junction for a transition",
        };
        f.write_str(s)
    }
}

impl std::error::Error for EditError {}

/// Isi satu proyek.
///
/// Jalur video bersifat "magnetik": klip selalu rapat berurutan tanpa celah,
/// seperti di CapCut. Teks dan audio bebas diletakkan di waktu mana pun.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Timeline {
    clips: Vec<Clip>,
    texts: Vec<TextItem>,
    audios: Vec<AudioItem>,
    next_id: Id,
}

impl Timeline {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            ..Self::default()
        }
    }

    fn fresh_id(&mut self) -> Id {
        let id = self.next_id.max(1);
        self.next_id = id + 1;
        id
    }

    // ---- baca ----

    pub fn clips(&self) -> &[Clip] {
        &self.clips
    }

    pub fn texts(&self) -> &[TextItem] {
        &self.texts
    }

    pub fn audios(&self) -> &[AudioItem] {
        &self.audios
    }

    /// Panjang jalur video.
    pub fn video_len(&self) -> Ms {
        self.clips.iter().map(|c| c.len).sum()
    }

    /// Panjang proyek: ujung paling akhir dari video, teks, atau audio.
    pub fn duration(&self) -> Ms {
        let t = self
            .texts
            .iter()
            .map(|x| x.start + x.len)
            .max()
            .unwrap_or(0);
        let a = self
            .audios
            .iter()
            .map(|x| x.start + x.len)
            .max()
            .unwrap_or(0);
        self.video_len().max(t).max(a)
    }

    pub fn clip_index(&self, id: Id) -> Option<usize> {
        self.clips.iter().position(|c| c.id == id)
    }

    pub fn clip(&self, id: Id) -> Option<&Clip> {
        self.clips.iter().find(|c| c.id == id)
    }

    pub fn text(&self, id: Id) -> Option<&TextItem> {
        self.texts.iter().find(|x| x.id == id)
    }

    pub fn audio(&self, id: Id) -> Option<&AudioItem> {
        self.audios.iter().find(|a| a.id == id)
    }

    /// Waktu mulai klip pada indeks `index`.
    pub fn clip_start(&self, index: usize) -> Ms {
        self.clips.iter().take(index).map(|c| c.len).sum()
    }

    /// Klip beserta waktu mulainya, berurutan.
    pub fn clip_spans(&self) -> impl Iterator<Item = (Ms, &Clip)> {
        let mut at = 0;
        self.clips.iter().map(move |c| {
            let start = at;
            at += c.len;
            (start, c)
        })
    }

    /// Klip yang tampil pada waktu `t`, beserta waktu mulainya.
    pub fn clip_at(&self, t: Ms) -> Option<(Ms, &Clip)> {
        self.clip_spans().find(|(s, c)| t >= *s && t < *s + c.len)
    }

    /// Apakah masih ada potongan audio yang berasal dari `group`.
    pub fn group_has_audio(&self, group: Id) -> bool {
        self.audios.iter().any(|a| a.group == group)
    }

    /// Apakah masih ada klip video yang suaranya dipisah ke `group`.
    pub fn group_has_clip(&self, group: Id) -> bool {
        self.clips
            .iter()
            .any(|c| c.audio == ClipAudio::Detached { group })
    }

    /// Apakah rentang `[start, start+len)` di `lane` kosong (mengabaikan `except`).
    pub fn lane_free(&self, lane: u8, start: Ms, len: Ms, except: Option<Id>) -> bool {
        self.audios.iter().all(|a| {
            Some(a.id) == except
                || a.lane != lane
                || start >= a.start + a.len
                || start + len <= a.start
        })
    }

    /// Semua suara yang terdengar pada waktu `t`.
    pub fn audible_at(&self, t: Ms) -> Vec<Audible> {
        let mut out = Vec::new();
        if let Some((_, c)) = self.clip_at(t) {
            if c.audio == ClipAudio::Attached && c.volume > 0 {
                out.push(Audible::ClipSound(c.id));
            }
        }
        for a in &self.audios {
            if t >= a.start && t < a.start + a.len && a.volume > 0 {
                out.push(Audible::Audio(a.id));
            }
        }
        out
    }

    /// Teks yang tampil pada waktu `t`.
    pub fn texts_at(&self, t: Ms) -> impl Iterator<Item = &TextItem> {
        self.texts
            .iter()
            .filter(move |x| t >= x.start && t < x.start + x.len)
    }

    /// Transisi yang sedang berlangsung pada waktu `t`.
    ///
    /// Sebuah transisi berlangsung dari `half` milidetik sebelum sambungan
    /// sampai `half` milidetik sesudahnya. Hasilnya: jenis transisi, waktu
    /// sambungan, dan indeks klip di sebelah kiri sambungan.
    pub fn transition_at(&self, t: Ms, half: Ms) -> Option<(Transition, Ms, usize)> {
        let mut edge = 0;
        for (index, clip) in self.clips.iter().enumerate() {
            edge += clip.len;
            if let Some(transition) = clip.transition_out {
                if t + half >= edge && t <= edge + half {
                    return Some((transition, edge, index));
                }
            }
        }
        None
    }

    // ---- klip video ----

    /// Menaruh klip baru di akhir jalur video.
    pub fn push_clip(
        &mut self,
        name: impl Into<String>,
        len: Ms,
        tint: u8,
        source: Option<PathBuf>,
    ) -> Id {
        let id = self.fresh_id();
        self.clips.push(Clip {
            id,
            name: name.into(),
            source,
            source_in: 0,
            len: len.max(MIN_PIECE_MS),
            tint,
            effect: Effect::None,
            audio: ClipAudio::Attached,
            volume: 100,
            transition_out: None,
        });
        id
    }

    /// Memotong klip video pada waktu `t`. Mengembalikan id potongan kanan.
    pub fn split_clip(&mut self, t: Ms) -> Result<Id, EditError> {
        let (start, id) = self
            .clip_at(t)
            .map(|(s, c)| (s, c.id))
            .ok_or(EditError::NotFound)?;
        let index = self.clip_index(id).ok_or(EditError::NotFound)?;
        let off = t - start;
        let len = self.clips[index].len;
        if off < MIN_PIECE_MS || len - off < MIN_PIECE_MS {
            return Err(EditError::TooCloseToEdge);
        }
        let new_id = self.fresh_id();
        let left = &mut self.clips[index];
        let mut right = left.clone();
        right.id = new_id;
        right.len = len - off;
        right.source_in = left.source_in + off;
        left.len = off;
        // Transisi keluar ikut ke potongan kanan, karena dialah yang kini
        // bersebelahan dengan klip berikutnya.
        left.transition_out = None;
        self.clips.insert(index + 1, right);
        Ok(new_id)
    }

    pub fn delete_clip(&mut self, id: Id) -> Result<Clip, EditError> {
        let index = self.clip_index(id).ok_or(EditError::NotFound)?;
        let removed = self.clips.remove(index);
        self.normalize();
        Ok(removed)
    }

    pub fn set_effect(&mut self, id: Id, effect: Effect) -> Result<(), EditError> {
        self.clip_mut(id)?.effect = effect;
        Ok(())
    }

    pub fn set_clip_volume(&mut self, id: Id, volume: u8) -> Result<(), EditError> {
        self.clip_mut(id)?.volume = volume.min(100);
        Ok(())
    }

    /// Memasang atau melepas (`None`) transisi antara klip `id` dan klip sesudahnya.
    pub fn set_transition(
        &mut self,
        id: Id,
        transition: Option<Transition>,
    ) -> Result<(), EditError> {
        let index = self.clip_index(id).ok_or(EditError::NotFound)?;
        if index + 1 >= self.clips.len() {
            return Err(EditError::NoJunction);
        }
        self.clips[index].transition_out = transition;
        Ok(())
    }

    fn clip_mut(&mut self, id: Id) -> Result<&mut Clip, EditError> {
        self.clips
            .iter_mut()
            .find(|c| c.id == id)
            .ok_or(EditError::NotFound)
    }

    fn normalize(&mut self) {
        if let Some(last) = self.clips.last_mut() {
            last.transition_out = None;
        }
    }

    // ---- teks ----

    pub fn add_text(&mut self, start: Ms, len: Ms, text: impl Into<String>) -> Id {
        let id = self.fresh_id();
        self.texts.push(TextItem {
            id,
            start,
            len: len.max(MIN_PIECE_MS),
            text: text.into(),
            size: 64,
            color: [255, 255, 255],
        });
        id
    }

    pub fn text_mut(&mut self, id: Id) -> Result<&mut TextItem, EditError> {
        self.texts
            .iter_mut()
            .find(|x| x.id == id)
            .ok_or(EditError::NotFound)
    }

    pub fn delete_text(&mut self, id: Id) -> Result<TextItem, EditError> {
        let index = self
            .texts
            .iter()
            .position(|x| x.id == id)
            .ok_or(EditError::NotFound)?;
        Ok(self.texts.remove(index))
    }

    // ---- audio ----

    fn free_lane(&self, start: Ms, len: Ms, prefer: u8) -> Option<u8> {
        std::iter::once(prefer)
            .chain((0..AUDIO_LANES).filter(|l| *l != prefer))
            .find(|l| self.lane_free(*l, start, len, None))
    }

    /// Menaruh musik di lajur kosong mulai waktu `start`.
    pub fn add_music(
        &mut self,
        name: impl Into<String>,
        start: Ms,
        len: Ms,
        source: Option<PathBuf>,
    ) -> Result<Id, EditError> {
        let len = len.max(MIN_PIECE_MS);
        let lane = self.free_lane(start, len, 0).ok_or(EditError::NoFreeLane)?;
        let id = self.fresh_id();
        self.audios.push(AudioItem {
            id,
            group: id,
            name: name.into(),
            source,
            source_in: 0,
            start,
            len,
            lane,
            kind: AudioKind::Music,
            volume: 100,
        });
        Ok(id)
    }

    /// Memisahkan suara klip video menjadi potongan audio sendiri.
    /// Mengembalikan id potongan audio itu.
    pub fn detach_audio(&mut self, clip_id: Id) -> Result<Id, EditError> {
        let index = self.clip_index(clip_id).ok_or(EditError::NotFound)?;
        if self.clips[index].audio != ClipAudio::Attached {
            return Err(EditError::AudioNotAttached);
        }
        let start = self.clip_start(index);
        let len = self.clips[index].len;
        let lane = self
            .free_lane(start, len, AUDIO_LANES - 1)
            .ok_or(EditError::NoFreeLane)?;
        let id = self.fresh_id();
        let clip = &mut self.clips[index];
        clip.audio = ClipAudio::Detached { group: id };
        let item = AudioItem {
            id,
            group: id,
            name: clip.name.clone(),
            source: clip.source.clone(),
            source_in: clip.source_in,
            start,
            len,
            lane,
            kind: AudioKind::Detached,
            volume: clip.volume,
        };
        self.audios.push(item);
        Ok(id)
    }

    /// Menggabungkan lagi audio yang dipisah ke klip video asalnya.
    /// Semua potongan audio dari grup itu dibuang. Mengembalikan id klip pertama yang pulih.
    pub fn reattach_audio(&mut self, audio_id: Id) -> Result<Id, EditError> {
        let group = self.audio(audio_id).ok_or(EditError::NotFound)?.group;
        let mut first = None;
        for c in &mut self.clips {
            if c.audio == (ClipAudio::Detached { group }) {
                c.audio = ClipAudio::Attached;
                first.get_or_insert(c.id);
            }
        }
        let first = first.ok_or(EditError::NotLinked)?;
        self.audios.retain(|a| a.group != group);
        Ok(first)
    }

    /// Memulihkan suara klip yang sudah dihapus.
    pub fn restore_clip_audio(&mut self, clip_id: Id) -> Result<(), EditError> {
        let clip = self.clip_mut(clip_id)?;
        if clip.audio == ClipAudio::Removed {
            clip.audio = ClipAudio::Attached;
        }
        Ok(())
    }

    /// Memindah potongan audio ke waktu dan lajur baru.
    pub fn move_audio(&mut self, id: Id, start: Ms, lane: u8) -> Result<(), EditError> {
        if lane >= AUDIO_LANES {
            return Err(EditError::BadLane);
        }
        let len = self.audio(id).ok_or(EditError::NotFound)?.len;
        if !self.lane_free(lane, start, len, Some(id)) {
            return Err(EditError::Overlap);
        }
        let a = self.audio_mut(id)?;
        a.start = start;
        a.lane = lane;
        Ok(())
    }

    /// Memotong potongan audio pada waktu `t`. Mengembalikan id potongan kanan.
    pub fn split_audio(&mut self, id: Id, t: Ms) -> Result<Id, EditError> {
        let a = self.audio(id).ok_or(EditError::NotFound)?;
        if t < a.start + MIN_PIECE_MS || t + MIN_PIECE_MS > a.start + a.len {
            return Err(EditError::TooCloseToEdge);
        }
        let off = t - a.start;
        let mut right = a.clone();
        let new_id = self.fresh_id();
        right.id = new_id;
        right.start = t;
        right.len -= off;
        right.source_in += off;
        self.audio_mut(id)?.len = off;
        self.audios.push(right);
        Ok(new_id)
    }

    pub fn set_audio_volume(&mut self, id: Id, volume: u8) -> Result<(), EditError> {
        self.audio_mut(id)?.volume = volume.min(100);
        Ok(())
    }

    /// Menghapus potongan audio. Kalau itu potongan terakhir dari suara sebuah
    /// klip video, klipnya menjadi bisu (videonya tetap).
    pub fn delete_audio(&mut self, id: Id) -> Result<AudioItem, EditError> {
        let index = self
            .audios
            .iter()
            .position(|a| a.id == id)
            .ok_or(EditError::NotFound)?;
        let removed = self.audios.remove(index);
        if !self.group_has_audio(removed.group) {
            for c in &mut self.clips {
                if c.audio
                    == (ClipAudio::Detached {
                        group: removed.group,
                    })
                {
                    c.audio = ClipAudio::Removed;
                }
            }
        }
        Ok(removed)
    }

    fn audio_mut(&mut self, id: Id) -> Result<&mut AudioItem, EditError> {
        self.audios
            .iter_mut()
            .find(|a| a.id == id)
            .ok_or(EditError::NotFound)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn three_clips() -> (Timeline, [Id; 3]) {
        let mut t = Timeline::new();
        let a = t.push_clip("A", 4000, 0, None);
        let b = t.push_clip("B", 5000, 1, None);
        let c = t.push_clip("C", 3000, 2, None);
        (t, [a, b, c])
    }

    #[test]
    fn clips_are_packed_without_gaps() {
        let (t, [_, b, c]) = three_clips();
        assert_eq!(t.video_len(), 12_000);
        assert_eq!(t.clip_at(0).unwrap().1.name, "A");
        assert_eq!(t.clip_at(3999).unwrap().1.name, "A");
        assert_eq!(t.clip_at(4000).unwrap().1.id, b);
        assert_eq!(t.clip_at(9000), Some((9000, t.clip(c).unwrap())));
        assert!(t.clip_at(12_000).is_none());
    }

    #[test]
    fn split_keeps_total_length_and_source_offset() {
        let (mut t, [_, b, _]) = three_clips();
        let right = t.split_clip(6000).unwrap();
        assert_eq!(t.clips().len(), 4);
        assert_eq!(t.video_len(), 12_000);
        assert_eq!(t.clip(b).unwrap().len, 2000);
        let r = t.clip(right).unwrap();
        assert_eq!((r.len, r.source_in), (3000, 2000));
        assert_eq!(t.clip_index(right), Some(2));
    }

    #[test]
    fn split_rejects_points_near_edges() {
        let (mut t, _) = three_clips();
        assert_eq!(t.split_clip(4000), Err(EditError::TooCloseToEdge));
        assert_eq!(t.split_clip(4050), Err(EditError::TooCloseToEdge));
        assert_eq!(t.split_clip(8950), Err(EditError::TooCloseToEdge));
        assert_eq!(t.split_clip(12_000), Err(EditError::NotFound));
        assert_eq!(t.clips().len(), 3);
    }

    #[test]
    fn split_moves_transition_to_right_piece() {
        let (mut t, [a, _, _]) = three_clips();
        t.set_transition(a, Some(Transition::Fade)).unwrap();
        let right = t.split_clip(2000).unwrap();
        assert_eq!(t.clip(a).unwrap().transition_out, None);
        assert_eq!(
            t.clip(right).unwrap().transition_out,
            Some(Transition::Fade)
        );
    }

    #[test]
    fn last_clip_cannot_have_transition() {
        let (mut t, [_, b, c]) = three_clips();
        assert_eq!(
            t.set_transition(c, Some(Transition::Wipe)),
            Err(EditError::NoJunction)
        );
        t.set_transition(b, Some(Transition::Wipe)).unwrap();
        t.delete_clip(c).unwrap();
        assert_eq!(t.clip(b).unwrap().transition_out, None);
    }

    #[test]
    fn transition_is_found_near_its_junction() {
        let (mut t, [a, _, _]) = three_clips();
        t.set_transition(a, Some(Transition::Slide)).unwrap();
        assert_eq!(
            t.transition_at(3500, 600),
            Some((Transition::Slide, 4000, 0))
        );
        assert_eq!(
            t.transition_at(4600, 600),
            Some((Transition::Slide, 4000, 0))
        );
        assert_eq!(t.transition_at(3000, 600), None);
        assert_eq!(t.transition_at(8900, 600), None);
    }

    #[test]
    fn detach_puts_audio_under_the_clip() {
        let (mut t, [_, b, _]) = three_clips();
        let audio = t.detach_audio(b).unwrap();
        let a = t.audio(audio).unwrap();
        assert_eq!((a.start, a.len, a.kind), (4000, 5000, AudioKind::Detached));
        assert_eq!(
            t.clip(b).unwrap().audio,
            ClipAudio::Detached { group: audio }
        );
        assert_eq!(t.detach_audio(b), Err(EditError::AudioNotAttached));
    }

    #[test]
    fn detach_avoids_occupied_lane() {
        let (mut t, [a, b, _]) = three_clips();
        let music = t.add_music("Musik", 0, 8000, None).unwrap();
        assert_eq!(t.audio(music).unwrap().lane, 0);
        let first = t.detach_audio(a).unwrap();
        assert_eq!(t.audio(first).unwrap().lane, 1);
        t.move_audio(first, 4000, 1).unwrap();
        // Rentang klip B (4000..9000) kini terisi di kedua lajur.
        assert_eq!(t.detach_audio(b), Err(EditError::NoFreeLane));
        assert_eq!(t.clip(b).unwrap().audio, ClipAudio::Attached);
    }

    #[test]
    fn detached_audio_moves_freely_but_never_overlaps() {
        let (mut t, [a, b, _]) = three_clips();
        let x = t.detach_audio(a).unwrap();
        let y = t.detach_audio(b).unwrap();
        t.move_audio(x, 9000, 1).unwrap();
        assert_eq!(t.audio(x).unwrap().start, 9000);
        assert_eq!(t.move_audio(y, 7000, 1), Err(EditError::Overlap));
        assert_eq!(t.audio(y).unwrap().start, 4000);
        t.move_audio(y, 7000, 0).unwrap();
        assert_eq!(t.move_audio(y, 0, 5), Err(EditError::BadLane));
        // Menyentuh ujung tidak dianggap bertumpuk.
        t.move_audio(y, 4000, 1).unwrap();
        assert_eq!(t.audio(y).unwrap().start + t.audio(y).unwrap().len, 9000);
    }

    #[test]
    fn moved_audio_changes_what_is_audible() {
        let (mut t, [a, _, _]) = three_clips();
        let x = t.detach_audio(a).unwrap();
        assert_eq!(t.audible_at(1000), vec![Audible::Audio(x)]);
        t.move_audio(x, 8000, 1).unwrap();
        assert_eq!(t.audible_at(1000), vec![]);
        let b = t.clips()[1].id;
        assert_eq!(
            t.audible_at(8500),
            vec![Audible::ClipSound(b), Audible::Audio(x)]
        );
        assert_eq!(t.duration(), 12_000);
        t.move_audio(x, 10_000, 1).unwrap();
        assert_eq!(t.duration(), 14_000);
    }

    #[test]
    fn deleting_detached_audio_mutes_clip_only_when_last_piece_goes() {
        let (mut t, [a, _, _]) = three_clips();
        let x = t.detach_audio(a).unwrap();
        let y = t.split_audio(x, 1500).unwrap();
        assert_eq!(t.audio(x).unwrap().len, 1500);
        assert_eq!(
            (
                t.audio(y).unwrap().start,
                t.audio(y).unwrap().len,
                t.audio(y).unwrap().source_in
            ),
            (1500, 2500, 1500)
        );
        t.delete_audio(x).unwrap();
        assert_eq!(t.clip(a).unwrap().audio, ClipAudio::Detached { group: x });
        t.delete_audio(y).unwrap();
        assert_eq!(t.clip(a).unwrap().audio, ClipAudio::Removed);
        assert_eq!(t.video_len(), 12_000);
        t.restore_clip_audio(a).unwrap();
        assert_eq!(t.clip(a).unwrap().audio, ClipAudio::Attached);
    }

    #[test]
    fn reattach_removes_all_pieces_and_restores_clip() {
        let (mut t, [a, _, _]) = three_clips();
        let x = t.detach_audio(a).unwrap();
        let y = t.split_audio(x, 2000).unwrap();
        t.move_audio(y, 9000, 0).unwrap();
        assert_eq!(t.reattach_audio(y), Ok(a));
        assert!(t.audios().is_empty());
        assert_eq!(t.clip(a).unwrap().audio, ClipAudio::Attached);
    }

    #[test]
    fn reattach_fails_for_music_and_orphans() {
        let (mut t, [a, _, _]) = three_clips();
        let music = t.add_music("Musik", 0, 8000, None).unwrap();
        assert_eq!(t.reattach_audio(music), Err(EditError::NotLinked));
        let x = t.detach_audio(a).unwrap();
        t.delete_clip(a).unwrap();
        assert_eq!(t.reattach_audio(x), Err(EditError::NotLinked));
        assert_eq!(t.audios().len(), 2);
    }

    #[test]
    fn split_clip_with_detached_audio_keeps_both_halves_linked() {
        let (mut t, [a, _, _]) = three_clips();
        let x = t.detach_audio(a).unwrap();
        let right = t.split_clip(2000).unwrap();
        assert_eq!(
            t.clip(right).unwrap().audio,
            ClipAudio::Detached { group: x }
        );
        t.reattach_audio(x).unwrap();
        assert_eq!(t.clip(a).unwrap().audio, ClipAudio::Attached);
        assert_eq!(t.clip(right).unwrap().audio, ClipAudio::Attached);
    }

    #[test]
    fn texts_show_only_inside_their_range() {
        let (mut t, _) = three_clips();
        let id = t.add_text(5500, 3000, "Halo!");
        assert_eq!(t.texts_at(5499).count(), 0);
        assert_eq!(t.texts_at(5500).next().unwrap().id, id);
        assert_eq!(t.texts_at(8500).count(), 0);
        t.text_mut(id).unwrap().text = "Hai".into();
        assert_eq!(t.text(id).unwrap().text, "Hai");
        t.delete_text(id).unwrap();
        assert_eq!(t.delete_text(id), Err(EditError::NotFound));
    }

    #[test]
    fn muted_sources_are_not_audible() {
        let (mut t, [a, _, _]) = three_clips();
        t.set_clip_volume(a, 0).unwrap();
        assert!(t.audible_at(100).is_empty());
        t.set_clip_volume(a, 250).unwrap();
        assert_eq!(t.clip(a).unwrap().volume, 100);
    }

    #[test]
    fn ids_are_never_reused() {
        let (mut t, [a, b, c]) = three_clips();
        t.delete_clip(c).unwrap();
        let d = t.push_clip("D", 1000, 0, None);
        assert!(d > c && c > b && b > a);
    }
}
