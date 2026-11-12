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

/// Le plus petit remplacement qui mène de `ancien` au texte donné par ses MORCEAUX (les `chunks` du
/// `Rope` du fork), et le texte inséré. Même résultat que `Changement::entre(ancien, &nouveau)`.
///
/// ★ Sans matérialiser le nouveau texte (`value()` le recopie en entier, deux fois) ni comparer
///   octet par octet : un morceau égal se compare d'un coup (`memcmp`). Sur 10 000 lignes, ≈ 1,2 ms
///   de moins par frappe en release (fr-t3.4).
#[must_use]
pub fn changement_par_morceaux(ancien: &str, morceaux: &[&str]) -> (Changement, String) {
    let a = ancien.as_bytes();
    let total: usize = morceaux.iter().map(|m| m.len()).sum();
    // Préfixe commun.
    let mut de = 0;
    for m in morceaux {
        let m = m.as_bytes();
        let fin = (de + m.len()).min(a.len());
        if a[de..fin] == *m {
            de += m.len();
        } else {
            de += a[de..fin].iter().zip(m).take_while(|(x, y)| x == y).count();
            break;
        }
    }
    let octet = |i: usize| -> Option<u8> {
        let mut base = 0;
        for m in morceaux {
            if i < base + m.len() {
                return m.as_bytes().get(i - base).copied();
            }
            base += m.len();
        }
        None
    };
    let frontiere_neuve =
        |i: usize| i >= total || octet(i).is_some_and(|b| !(0x80..0xC0).contains(&b));
    while !ancien.is_char_boundary(de) || !frontiere_neuve(de) {
        de -= 1;
    }
    // Suffixe commun, sans chevaucher le préfixe.
    let max_suffixe = a.len().min(total) - de;
    let mut s = 0;
    for m in morceaux.iter().rev() {
        let m = m.as_bytes();
        let reste = max_suffixe - s;
        if m.len() <= reste && a[a.len() - s - m.len()..a.len() - s] == *m {
            s += m.len();
        } else {
            s += a[..a.len() - s]
                .iter()
                .rev()
                .zip(m.iter().rev())
                .take(reste)
                .take_while(|(x, y)| x == y)
                .count();
            break;
        }
    }
    while !ancien.is_char_boundary(a.len() - s) || !frontiere_neuve(total - s) {
        s -= 1;
    }
    let c = Changement {
        de,
        fin_ancien: a.len() - s,
        fin_nouveau: total - s,
    };
    // Le texte inséré : les morceaux qui couvrent `[de, fin_nouveau)`.
    let mut insere = String::with_capacity(c.fin_nouveau - de);
    let mut base = 0;
    for m in morceaux {
        let (d, f) = (base, base + m.len());
        base = f;
        if f <= de || d >= c.fin_nouveau {
            continue;
        }
        let (i, j) = (de.max(d) - d, c.fin_nouveau.min(f) - d);
        // Les morceaux d'un `Rope` sont des `&str` entiers : les bornes y sont des frontières de
        // caractères. Sinon (un appelant qui couperait un caractère), le repli.
        let Some(partie) = m.get(i..j) else {
            return changement_lent(ancien, morceaux);
        };
        insere.push_str(partie);
    }
    (c, insere)
}

/// Le repli : le nouveau texte matérialisé.
fn changement_lent(ancien: &str, morceaux: &[&str]) -> (Changement, String) {
    let nouveau = morceaux.concat();
    let c = Changement::entre(ancien, &nouveau);
    (c, nouveau[c.de..c.fin_nouveau].to_owned())
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

    /// Le résultat est celui de `Changement::entre` sur le texte recollé, quel que soit le découpage
    /// en morceaux (caractères de 1 à 4 octets, coupés ou non).
    #[test]
    fn par_morceaux_comme_entre() {
        let alphabet = [
            "a", "b", "é", "😀", "
", "€",
        ];
        let mut graine = 11u64;
        let mut alea = |n: usize| {
            graine = graine
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            usize::try_from(graine >> 33).unwrap_or(0) % n.max(1)
        };
        for _ in 0..2000 {
            let texte = |alea: &mut dyn FnMut(usize) -> usize| {
                (0..alea(12))
                    .map(|_| alphabet[alea(alphabet.len())])
                    .collect::<String>()
            };
            let (pre, mil, suf, mil2) = (
                texte(&mut alea),
                texte(&mut alea),
                texte(&mut alea),
                texte(&mut alea),
            );
            let ancien = format!("{pre}{mil}{suf}");
            let nouveau = format!("{pre}{mil2}{suf}");
            // Découpage en morceaux, sur des frontières de caractères (comme ropey).
            let mut morceaux = Vec::new();
            let mut reste = nouveau.as_str();
            while !reste.is_empty() {
                let mut k = (1 + alea(5)).min(reste.len());
                while !reste.is_char_boundary(k) {
                    k += 1;
                }
                morceaux.push(&reste[..k]);
                reste = &reste[k..];
            }
            let (c, insere) = changement_par_morceaux(&ancien, &morceaux);
            assert_eq!(
                c,
                Changement::entre(&ancien, &nouveau),
                "{ancien:?} → {nouveau:?}"
            );
            assert_eq!(insere, &nouveau[c.de..c.fin_nouveau]);
        }
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
