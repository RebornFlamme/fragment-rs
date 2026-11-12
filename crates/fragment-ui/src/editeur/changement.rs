// Port de `CmEditor.toChange` (app/src/ui/views/markdown/CmEditor.ts) : le remappage `mapPos` d'un
// changement, en unités UTF-16, avec le drapeau de suppression de `MapMode.TrackDel`.
//! Un changement de texte de l'éditeur, vu comme le contrat `EditorChange` de `fragment-text`.
//!
//! L'éditeur du fork n'expose pas ses transactions : l'hôte compare le texte d'avant et celui
//! d'après (`Changement::entre`, préfixe et suffixe communs) et en tire UN remplacement. La règle de
//! remappage est celle de CodeMirror pour un remplacement `[de, fin)` → `insere` :
//! - avant `de` : inchangé ; après `fin` : décalé de la différence de longueur ;
//! - à `de` : reste à `de`, SAUF une insertion pure suivie d'une position collée après
//!   (`Assoc::After`), qui passe derrière le texte inséré ;
//! - strictement dans `(de, fin)` : SUPPRIMÉE ; revient au début (`Before`) ou à la fin du texte
//!   inséré (`After`) ;
//! - à `fin` (d'une plage non vide) : derrière le texte inséré, non supprimée.

use fragment_markdown::syntaxe::Changement;
use fragment_text::editor::{Assoc, EditorChange, MappedPos};
use fragment_text::utf16::byte_to_utf16;

/// Un remplacement, en unités UTF-16 : `[de, fin_ancien)` de l'ancien texte est devenu
/// `[de, fin_nouveau)` du nouveau.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Remplacement {
    pub de: usize,
    pub fin_ancien: usize,
    pub fin_nouveau: usize,
}

impl Remplacement {
    /// Le remplacement en UTF-16 qui correspond à un `Changement` en octets.
    #[must_use]
    pub fn depuis(ancien: &str, nouveau: &str, c: Changement) -> Remplacement {
        Remplacement {
            de: byte_to_utf16(ancien, c.de),
            fin_ancien: byte_to_utf16(ancien, c.fin_ancien),
            fin_nouveau: byte_to_utf16(nouveau, c.fin_nouveau),
        }
    }

    /// `mapPos(pos, assoc)` et `TrackDel`.
    #[must_use]
    pub fn remapper(self, pos: usize, assoc: Assoc) -> MappedPos {
        let Remplacement {
            de,
            fin_ancien,
            fin_nouveau,
        } = self;
        let insertion_pure = de == fin_ancien;
        if pos < de || (pos == de && !(insertion_pure && assoc == Assoc::After)) {
            return MappedPos {
                pos,
                deleted: false,
            };
        }
        if pos < fin_ancien {
            return MappedPos {
                pos: if assoc == Assoc::Before {
                    de
                } else {
                    fin_nouveau
                },
                deleted: pos > de,
            };
        }
        MappedPos {
            pos: pos - fin_ancien + fin_nouveau,
            deleted: false,
        }
    }

    /// Le `EditorChange` d'un changement de document local.
    #[must_use]
    pub fn en_changement(self) -> EditorChange {
        EditorChange::doc(move |p, a| self.remapper(p, a))
    }
}

/// Un changement de sélection seule (document inchangé, `map_pos` identité).
#[must_use]
pub fn changement_de_selection() -> EditorChange {
    EditorChange {
        doc_changed: false,
        selection_changed: true,
        viewport_changed: false,
        map_pos: Box::new(|pos, _| MappedPos {
            pos,
            deleted: false,
        }),
        origin: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(de: usize, fin_ancien: usize, fin_nouveau: usize) -> Remplacement {
        Remplacement {
            de,
            fin_ancien,
            fin_nouveau,
        }
    }

    fn m(pos: usize, deleted: bool) -> MappedPos {
        MappedPos { pos, deleted }
    }

    #[test]
    fn insertion_pure_le_cote_decide() {
        // « ab » → « aXYb » : insertion de 2 en 1.
        let c = r(1, 1, 3);
        assert_eq!(c.remapper(0, Assoc::After), m(0, false));
        assert_eq!(c.remapper(1, Assoc::Before), m(1, false));
        assert_eq!(c.remapper(1, Assoc::After), m(3, false));
        assert_eq!(c.remapper(2, Assoc::Before), m(4, false));
    }

    #[test]
    fn remplacement_supprime_seulement_l_interieur() {
        // « abcdef » : [1, 4) remplacé par « Z » → « aZef ».
        let c = r(1, 4, 2);
        assert_eq!(c.remapper(1, Assoc::After), m(1, false));
        assert_eq!(c.remapper(2, Assoc::Before), m(1, true));
        assert_eq!(c.remapper(3, Assoc::After), m(2, true));
        assert_eq!(c.remapper(4, Assoc::Before), m(2, false));
        assert_eq!(c.remapper(6, Assoc::After), m(4, false));
    }

    #[test]
    fn depuis_un_changement_en_octets() {
        let (a, n) = ("é😀b", "é😀Xb");
        let c = Changement::entre(a, n);
        let rp = Remplacement::depuis(a, n, c);
        assert_eq!(rp, r(3, 3, 4));
        let ch = rp.en_changement();
        assert!(ch.doc_changed);
        assert_eq!(ch.map_pos(4, Assoc::After), m(5, false));
    }
}
