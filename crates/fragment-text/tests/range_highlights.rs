// Port de app/src/ui/core/editor/rangeHighlights.test.ts

use std::cell::Cell;

use fragment_text::{
    Assoc, DocumentSurface, EditorChange, EditorPosition, EditorRange, HighlightBox,
    HighlightRange, MappedPos, RangeHighlights, Rect, TextSurface,
};

/// Une fausse surface de texte : un caractère = 10 px, une ligne. Le poke est
/// compté ; `changer` simule une insertion, `supprimer` une suppression.
#[derive(Default)]
struct Surface {
    pokes: Cell<usize>,
}

impl DocumentSurface for Surface {
    fn coords_at_pos(&self, _offset: usize) -> Option<Rect> {
        None
    }
    fn pos_at_coords(&self, _x: f64, _y: f64) -> Option<usize> {
        None
    }
    fn viewport_range(&self) -> EditorRange {
        EditorRange { from: 0, to: 1000 }
    }
    fn rendered_ranges(&self) -> Vec<EditorRange> {
        vec![EditorRange { from: 0, to: 1000 }]
    }
    fn request_update(&self) {
        self.pokes.set(self.pokes.get() + 1);
    }
    fn as_text(&self) -> Option<&dyn TextSurface> {
        Some(self)
    }
}

impl TextSurface for Surface {
    fn get_line(&self, _n: usize) -> String {
        String::new()
    }
    fn line_count(&self) -> usize {
        1
    }
    fn offset_to_pos(&self, offset: usize) -> EditorPosition {
        EditorPosition {
            line: 0,
            ch: offset,
        }
    }
    fn pos_to_offset(&self, pos: EditorPosition) -> usize {
        pos.ch
    }
    fn get_selection(&self) -> EditorRange {
        EditorRange::default()
    }
    fn coords_for_range(&self, from: usize, to: usize) -> Vec<Rect> {
        vec![Rect {
            left: from as f64 * 10.0,
            right: to as f64 * 10.0,
            top: 0.0,
            bottom: 20.0,
        }]
    }
}

/// Insère `n` caractères à `at`.
fn changer(at: usize, n: usize) -> EditorChange {
    EditorChange::doc(move |pos, assoc| MappedPos {
        pos: if pos > at || (pos == at && assoc == Assoc::After) {
            pos + n
        } else {
            pos
        },
        deleted: false,
    })
}

/// Supprime [from, to).
fn supprimer(from: usize, to: usize) -> EditorChange {
    EditorChange::doc(move |pos, _| MappedPos {
        pos: if pos <= from {
            pos
        } else if pos >= to {
            pos - (to - from)
        } else {
            from
        },
        deleted: pos > from && pos < to,
    })
}

/// La surface livre le poke en attente (TS : la microtâche) et le compte.
fn livrer(h: &mut RangeHighlights, s: &Surface) {
    if h.take_redraw() {
        s.request_update();
    }
}

fn boite(cls: &str, left: f64, width: f64) -> HighlightBox {
    HighlightBox {
        cls: cls.to_owned(),
        left,
        top: 0.0,
        width,
        height: 20.0,
    }
}

fn r(from: usize, to: usize) -> HighlightRange {
    HighlightRange::new(from, to)
}

// describe('rangeHighlights')

#[test]
fn une_table_et_un_calque_par_surface_sous_le_texte_partages_par_toutes_les_clefs() {
    let s = Surface::default();
    let mut h = RangeHighlights::new();
    h.set("a", vec![r(2, 5)]);
    h.set("b", vec![HighlightRange::with_cls(8, 9, "mine")]);
    assert!(h.is_armed());
    // Le calque est dessiné SOUS le texte (TS : `calques[0].above === false`).
    assert!(!h.layer_above());
    assert_eq!(
        h.boxes(&s),
        vec![
            boite("range-highlight", 20.0, 30.0),
            boite("range-highlight mine", 80.0, 10.0),
        ]
    );
}

#[test]
fn les_plages_suivent_le_texte_une_frappe_dedans_les_allonge_au_dessus_les_decale() {
    let mut h = RangeHighlights::new();
    h.set("k", vec![r(10, 16)]);
    h.remap(&changer(13, 3)); // au milieu
    assert_eq!(h.get("k"), &[r(10, 19)]);
    h.remap(&changer(0, 5)); // au-dessus
    assert_eq!(h.get("k"), &[r(15, 24)]);
    h.remap(&changer(24, 2)); // juste APRÈS la fin : pas dedans
    assert_eq!(h.get("k"), &[r(15, 24)]);
}

#[test]
fn une_plage_videe_par_une_suppression_disparait_le_retrait_rendu_ne_retire_que_sa_pose() {
    let mut h = RangeHighlights::new();
    h.set("k", vec![r(10, 12)]);
    h.remap(&supprimer(5, 20));
    assert_eq!(h.get("k"), &[]);
    let retirer = h.set("x", vec![r(1, 2)]);
    h.set("x", vec![r(3, 4)]); // reposée : l'ancien retrait n'y touche plus
    h.retire(&retirer);
    assert_eq!(h.get("x"), &[r(3, 4)]);
    h.delete("x");
    assert_eq!(h.get("x"), &[]);
}

#[test]
fn le_poke_est_regroupe_en_microtache_jamais_pendant_une_mise_a_jour_de_l_editeur() {
    let s = Surface::default();
    let mut h = RangeHighlights::new();
    h.set("a", vec![r(1, 2)]);
    h.set("b", vec![r(3, 4)]);
    assert_eq!(s.pokes.get(), 0);
    livrer(&mut h, &s);
    livrer(&mut h, &s);
    assert_eq!(s.pokes.get(), 1);
}
