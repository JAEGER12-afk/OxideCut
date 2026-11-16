use crate::timeline::{EditError, Timeline};

const HISTORY_LIMIT: usize = 200;

/// `Timeline` dengan riwayat undo/redo.
///
/// Riwayat disimpan sebagai salinan utuh `Timeline` sebelum tiap perubahan.
/// Cara ini sengaja dipilih karena sederhana dan sulit salah; isi proyek kecil
/// (hanya metadata, bukan video), jadi salinannya murah.
#[derive(Debug, Clone, Default)]
pub struct Editor {
    timeline: Timeline,
    undo: Vec<Timeline>,
    redo: Vec<Timeline>,
    last_tag: Option<u64>,
}

impl Editor {
    pub fn new(timeline: Timeline) -> Self {
        Self {
            timeline,
            ..Self::default()
        }
    }

    pub fn timeline(&self) -> &Timeline {
        &self.timeline
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Menjalankan satu perubahan sebagai satu langkah undo.
    ///
    /// Kalau `f` gagal, timeline dikembalikan persis seperti semula.
    /// Kalau `f` berhasil tetapi tidak mengubah apa pun, riwayat tidak bertambah.
    pub fn edit<T>(
        &mut self,
        f: impl FnOnce(&mut Timeline) -> Result<T, EditError>,
    ) -> Result<T, EditError> {
        self.run(None, f)
    }

    /// Seperti [`Editor::edit`], tetapi perubahan beruntun dengan `tag` yang sama
    /// digabung menjadi satu langkah undo. Dipakai untuk mengetik teks atau
    /// menggeser slider, supaya satu Undo membatalkan seluruh gerakan.
    pub fn edit_merged<T>(
        &mut self,
        tag: u64,
        f: impl FnOnce(&mut Timeline) -> Result<T, EditError>,
    ) -> Result<T, EditError> {
        self.run(Some(tag), f)
    }

    fn run<T>(
        &mut self,
        tag: Option<u64>,
        f: impl FnOnce(&mut Timeline) -> Result<T, EditError>,
    ) -> Result<T, EditError> {
        let before = self.timeline.clone();
        match f(&mut self.timeline) {
            Ok(value) => {
                if self.timeline != before {
                    let merge = tag.is_some() && tag == self.last_tag && !self.undo.is_empty();
                    if !merge {
                        self.undo.push(before);
                        if self.undo.len() > HISTORY_LIMIT {
                            self.undo.remove(0);
                        }
                    }
                    self.redo.clear();
                    self.last_tag = tag;
                }
                Ok(value)
            }
            Err(e) => {
                self.timeline = before;
                Err(e)
            }
        }
    }

    /// Menutup rangkaian `edit_merged`, sehingga perubahan berikutnya menjadi langkah baru.
    pub fn end_merge(&mut self) {
        self.last_tag = None;
    }

    pub fn undo(&mut self) -> bool {
        self.last_tag = None;
        match self.undo.pop() {
            Some(prev) => {
                self.redo.push(std::mem::replace(&mut self.timeline, prev));
                true
            }
            None => false,
        }
    }

    pub fn redo(&mut self) -> bool {
        self.last_tag = None;
        match self.redo.pop() {
            Some(next) => {
                self.undo.push(std::mem::replace(&mut self.timeline, next));
                true
            }
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::Effect;

    fn editor() -> Editor {
        let mut t = Timeline::new();
        t.push_clip("A", 4000, 0, None);
        t.push_clip("B", 5000, 1, None);
        Editor::new(t)
    }

    #[test]
    fn undo_and_redo_restore_exact_state() {
        let mut e = editor();
        let start = e.timeline().clone();
        e.edit(|t| t.split_clip(2000)).unwrap();
        let after = e.timeline().clone();
        assert!(e.undo());
        assert_eq!(e.timeline(), &start);
        assert!(e.redo());
        assert_eq!(e.timeline(), &after);
        assert!(!e.redo());
    }

    #[test]
    fn failed_edit_changes_nothing_and_adds_no_history() {
        let mut e = editor();
        let start = e.timeline().clone();
        let r = e.edit(|t| {
            t.split_clip(2000)?;
            t.split_clip(2000)
        });
        assert_eq!(r, Err(EditError::TooCloseToEdge));
        assert_eq!(e.timeline(), &start);
        assert!(!e.can_undo());
    }

    #[test]
    fn no_op_edit_adds_no_history() {
        let mut e = editor();
        let id = e.timeline().clips()[0].id;
        e.edit(|t| t.set_effect(id, Effect::None)).unwrap();
        assert!(!e.can_undo());
    }

    #[test]
    fn new_edit_clears_redo() {
        let mut e = editor();
        let id = e.timeline().clips()[0].id;
        e.edit(|t| t.split_clip(2000)).unwrap();
        e.undo();
        e.edit(|t| t.set_effect(id, Effect::Blur)).unwrap();
        assert!(!e.can_redo());
    }

    #[test]
    fn merged_edits_undo_in_one_step() {
        let mut e = editor();
        let id = e.edit(|t| Ok(t.add_text(0, 3000, ""))).unwrap();
        for s in ["H", "Ha", "Hal", "Halo"] {
            e.edit_merged(id, |t| {
                t.text_mut(id)?.text = s.into();
                Ok(())
            })
            .unwrap();
        }
        assert_eq!(e.timeline().text(id).unwrap().text, "Halo");
        e.undo();
        assert_eq!(e.timeline().text(id).unwrap().text, "");
        e.undo();
        assert!(e.timeline().texts().is_empty());
    }

    #[test]
    fn end_merge_starts_a_new_step() {
        let mut e = editor();
        let id = e.timeline().clips()[0].id;
        e.edit_merged(7, |t| t.set_clip_volume(id, 50)).unwrap();
        e.end_merge();
        e.edit_merged(7, |t| t.set_clip_volume(id, 20)).unwrap();
        e.undo();
        assert_eq!(e.timeline().clips()[0].volume, 50);
    }

    #[test]
    fn full_story_split_audio_move_and_undo_everything() {
        let mut e = editor();
        let start = e.timeline().clone();
        let b = e.timeline().clips()[1].id;
        let audio = e.edit(|t| t.detach_audio(b)).unwrap();
        e.edit(|t| t.move_audio(audio, 0, 0)).unwrap();
        let piece = e.edit(|t| t.split_audio(audio, 2500)).unwrap();
        e.edit(|t| t.delete_audio(piece).map(|_| ())).unwrap();
        assert_eq!(e.timeline().audios().len(), 1);
        while e.undo() {}
        assert_eq!(e.timeline(), &start);
    }
}
