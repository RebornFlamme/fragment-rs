// Port de app/src/ui/views/markdown/decorationPlugin.ts (markdownFoldService, atxHeadingLevel,
// leadingIndent, toggleFoldAt, blockFoldProvider) et de properties/frontmatterField.ts (repli,
// curseurHorsDuBloc).
//! Le repli : ce qui est repliable, ce qui est replié, et où le curseur n'a plus le droit d'aller.
//!
//! Une plage repliable commence à la FIN de la première ligne (la ligne « parent » reste visible) et
//! s'arrête à la dernière ligne non vide du corps. Trois sources, dans l'ordre du TS :
//! 1. un titre ATX replie jusqu'au prochain titre de niveau inférieur ou égal : replier un `##`
//!    avale ses `###` ;
//! 2. sinon, une ligne non vide replie les lignes suivantes PLUS indentées qu'elle (les lignes vides
//!    sont tolérées au milieu) : c'est ainsi que se replient les listes imbriquées ;
//! 3. le bloc Properties se replie à part (`blockFoldProvider`) : pas de texte caché, son widget change
//!    de rendu.
//!
//! ★ Un titre sans corps n'est PAS repliable et ne retombe pas sur l'indentation : le TS rend `null`
//!   dans la branche des titres.
//! ★ Les titres sont lus dans l'arbre, jamais par regex : un `#` dans un bloc de code n'en est pas un ;
//!   un titre dans une citation (`> # T`) n'est pas repliable (le nœud ne commence pas au début de la
//!   ligne), comme chez Fragment.

use std::collections::HashSet;

use super::calcul::plage_proprietes;
use crate::syntaxe::modele_rangees::{Etendue, Plage, lignes_de};
use crate::syntaxe::{Arbre, Genre};

/// D'où vient une plage repliable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NatureRepli {
    /// Un titre ATX de niveau 1 à 6.
    Titre(u8),
    /// Un élément de liste (la ligne commence par une marque de liste) et ses sous-éléments.
    Liste,
    /// Une ligne quelconque suivie de lignes plus indentées.
    Indentation,
    /// Le bloc Properties : se replie sans cacher de texte.
    Proprietes,
}

/// Une plage repliable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlageRepli {
    /// La ligne qui porte le chevron (à partir de 0).
    pub ligne: usize,
    /// La dernière ligne repliée (non vide), ou la dernière du bloc pour `Proprietes`.
    pub derniere_ligne: usize,
    pub nature: NatureRepli,
    /// Ce qui est replié : de la fin de `ligne` à la fin de `derniere_ligne` (saut de ligne compris).
    /// Pour `Proprietes`, tout le bloc.
    pub plage: Plage,
}

/// Les lignes du document, lues une fois : niveau de titre, indentation, ligne vide, marque de liste.
/// Rend chaque plage en O(longueur du corps replié) sans reparcourir l'arbre.
#[derive(Debug, Clone)]
pub struct AnalyseRepli {
    lignes: Vec<Plage>,
    titre: Vec<Option<u8>>,
    indentation: Vec<usize>,
    vide: Vec<bool>,
    liste: Vec<bool>,
    proprietes: Option<(usize, Plage)>,
}

impl AnalyseRepli {
    #[must_use]
    pub fn new(texte: &str, arbre: &Arbre) -> AnalyseRepli {
        let lignes = lignes_de(texte);
        let n = lignes.len();
        let de_octet = |b: usize| {
            lignes
                .partition_point(|l| l.octets.from <= b)
                .saturating_sub(1)
        };
        let mut titre = vec![None; n];
        let mut liste = vec![false; n];
        let mut vus = HashSet::new();
        for nd in arbre.noeuds() {
            let niveau = match nd.genre {
                Genre::ATXHeading1 => Some(1),
                Genre::ATXHeading2 => Some(2),
                Genre::ATXHeading3 => Some(3),
                Genre::ATXHeading4 => Some(4),
                Genre::ATXHeading5 => Some(5),
                Genre::ATXHeading6 => Some(6),
                _ => None,
            };
            let l = de_octet(nd.plage.octets.from);
            if let Some(k) = niveau {
                // `resolveInner(line.from, 1)` : le titre doit COMMENCER la ligne.
                if nd.plage.octets.from == lignes[l].octets.from {
                    titre[l] = Some(k);
                }
            } else if nd.genre == Genre::ListMark && vus.insert(l) {
                liste[l] = true;
            }
        }
        let mut indentation = Vec::with_capacity(n);
        let mut vide = Vec::with_capacity(n);
        for p in &lignes {
            let t = p.texte(texte);
            indentation.push(
                t.chars()
                    .take_while(|c| c.is_whitespace())
                    .map(char::len_utf16)
                    .sum(),
            );
            vide.push(t.trim().is_empty());
        }
        let proprietes = plage_proprietes(texte).map(|p| {
            let derniere = de_octet(p.octets.to);
            (derniere, p)
        });
        AnalyseRepli {
            lignes,
            titre,
            indentation,
            vide,
            liste,
            proprietes,
        }
    }

    /// Le nombre de lignes logiques.
    #[must_use]
    pub fn nombre_lignes(&self) -> usize {
        self.lignes.len()
    }

    /// La plage que replie la ligne `ligne`, si elle est repliable (`foldable`).
    #[must_use]
    pub fn plage(&self, ligne: usize) -> Option<PlageRepli> {
        if ligne >= self.lignes.len() {
            return None;
        }
        // Le bloc Properties : repliable sur sa première ligne, par son propre mécanisme.
        if let Some((derniere, p)) = self.proprietes
            && ligne == 0
        {
            return Some(PlageRepli {
                ligne,
                derniere_ligne: derniere,
                nature: NatureRepli::Proprietes,
                plage: p,
            });
        }
        if let Some(niveau) = self.titre[ligne] {
            let mut derniere = ligne;
            for n in ligne + 1..self.lignes.len() {
                if self.titre[n].is_some_and(|k| k <= niveau) {
                    break;
                }
                if !self.vide[n] {
                    derniere = n;
                }
            }
            return self.corps(ligne, derniere, NatureRepli::Titre(niveau));
        }
        if self.vide[ligne] {
            return None;
        }
        let base = self.indentation[ligne];
        let mut derniere = ligne;
        for n in ligne + 1..self.lignes.len() {
            if self.vide[n] {
                continue;
            }
            if self.indentation[n] > base {
                derniere = n;
            } else {
                break;
            }
        }
        let nature = if self.liste[ligne] {
            NatureRepli::Liste
        } else {
            NatureRepli::Indentation
        };
        self.corps(ligne, derniere, nature)
    }

    fn corps(&self, ligne: usize, derniere: usize, nature: NatureRepli) -> Option<PlageRepli> {
        if derniere == ligne {
            return None;
        }
        let (a, b) = (self.lignes[ligne], self.lignes[derniere]);
        Some(PlageRepli {
            ligne,
            derniere_ligne: derniere,
            nature,
            plage: Plage {
                utf16: Etendue::new(a.utf16.to, b.utf16.to),
                octets: Etendue::new(a.octets.to, b.octets.to),
            },
        })
    }

    /// Toutes les plages repliables, par ligne croissante.
    #[must_use]
    pub fn toutes(&self) -> Vec<PlageRepli> {
        (0..self.lignes.len())
            .filter_map(|l| self.plage(l))
            .collect()
    }

    /// La ligne qui contient la position UTF-16 `pos`.
    fn ligne_utf16(&self, pos: usize) -> usize {
        self.lignes
            .partition_point(|l| l.utf16.from <= pos)
            .saturating_sub(1)
    }
}

/// Une modification du texte, en UTF-16 : `[de, fin_ancien)` est devenu `[de, fin_nouveau)`. Une suite
/// d'éditions se lit dans l'ordre, chacune relative au texte rendu par la précédente.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Edition {
    pub de: usize,
    pub fin_ancien: usize,
    pub fin_nouveau: usize,
}

impl Edition {
    #[must_use]
    pub fn remplacement(de: usize, fin_ancien: usize, longueur_inseree: usize) -> Edition {
        Edition {
            de,
            fin_ancien,
            fin_nouveau: de + longueur_inseree,
        }
    }

    /// `ChangeSet.mapPos(pos, assoc, MapMode.TrackDel)` : `None` si la position est dans du texte
    /// supprimé. `avance` = `assoc > 0` : une insertion exactement ici passe AVANT la position.
    fn mapper(self, pos: usize, avance: bool) -> Option<usize> {
        let (supprime, insere) = (self.fin_ancien - self.de, self.fin_nouveau - self.de);
        if pos < self.de {
            return Some(pos);
        }
        if pos > self.fin_ancien {
            return Some(pos - supprime + insere);
        }
        if supprime == 0 {
            // Insertion pure en `pos`.
            return Some(if avance { self.de + insere } else { self.de });
        }
        if pos == self.de {
            Some(self.de)
        } else if pos == self.fin_ancien {
            Some(self.fin_nouveau)
        } else {
            None
        }
    }
}

/// L'état de repli d'un éditeur : les plages repliées (UTF-16) et le repli des Properties.
///
/// Ce que garde CodeMirror dans `foldState` ; une plage repliée est une décoration de remplacement
/// non inclusive, d'où le remappage de [`EtatRepli::remapper`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EtatRepli {
    plis: Vec<Etendue>,
    /// Le bloc Properties est replié.
    pub proprietes_repliees: bool,
}

impl EtatRepli {
    /// Les plages repliées, triées.
    #[must_use]
    pub fn plis(&self) -> &[Etendue] {
        &self.plis
    }

    /// La plage repliée qui touche `pos` (la FIN d'une ligne repliable : `foldedRanges.between`).
    #[must_use]
    pub fn replie_en(&self, pos: usize) -> Option<Etendue> {
        self.plis
            .iter()
            .copied()
            .find(|p| p.from <= pos && pos <= p.to)
    }

    /// `foldEffect` : replie `plage` (sans doublon).
    pub fn replier(&mut self, plage: Etendue) {
        if !self.plis.contains(&plage) {
            self.plis.push(plage);
            self.plis.sort();
        }
    }

    /// `unfoldEffect`.
    pub fn deplier(&mut self, plage: Etendue) {
        self.plis.retain(|p| *p != plage);
    }

    /// Le clic sur le chevron de la ligne `ligne` (`toggleFoldAt`) : Properties → bascule son repli ;
    /// sinon dé-plie la plage repliée à la fin de la ligne, ou replie la plage repliable. Rend `true`
    /// si l'état a changé.
    pub fn basculer(&mut self, analyse: &AnalyseRepli, ligne: usize) -> bool {
        let Some(plage) = analyse.plage(ligne) else {
            // Plus repliable : on laisse quand même déplier ce qui a été replié ici.
            return self.deplier_en(analyse, ligne);
        };
        if plage.nature == NatureRepli::Proprietes {
            self.proprietes_repliees = !self.proprietes_repliees;
            return true;
        }
        if self.deplier_en(analyse, ligne) {
            return true;
        }
        self.replier(plage.plage.utf16);
        true
    }

    fn deplier_en(&mut self, analyse: &AnalyseRepli, ligne: usize) -> bool {
        let Some(l) = analyse.lignes.get(ligne) else {
            return false;
        };
        match self.replie_en(l.utf16.to) {
            Some(p) => {
                self.deplier(p);
                true
            }
            None => false,
        }
    }

    /// La ligne `ligne` est-elle repliée (son chevron est fermé) ?
    #[must_use]
    pub fn ligne_repliee(&self, analyse: &AnalyseRepli, ligne: usize) -> bool {
        if ligne == 0 && analyse.proprietes.is_some() && self.proprietes_repliees {
            return true;
        }
        analyse
            .lignes
            .get(ligne)
            .is_some_and(|l| self.replie_en(l.utf16.to).is_some())
    }

    /// Fait suivre les plages repliées par une suite d'éditions (`folded.map(tr.changes)`) : le début
    /// d'une plage reste en place si on insère juste à sa position, la fin aussi (une frappe au bout de
    /// la ligne « parent » ou après la dernière ligne repliée reste dehors) ; une plage dont une borne
    /// tombe dans du texte supprimé, ou devenue vide, disparaît.
    pub fn remapper(&mut self, editions: &[Edition]) {
        for e in editions {
            self.plis = self
                .plis
                .iter()
                .filter_map(|p| {
                    let from = e.mapper(p.from, true)?;
                    let to = e.mapper(p.to, false)?;
                    (from < to).then(|| Etendue::new(from, to))
                })
                .collect();
        }
        self.plis.sort();
        self.plis.dedup();
    }

    /// Les lignes cachées par le repli : des intervalles `[premiere, derniere]` (bornes comprises)
    /// triés et disjoints. La ligne « parent » reste visible.
    #[must_use]
    pub fn lignes_cachees(&self, analyse: &AnalyseRepli) -> Vec<(usize, usize)> {
        let mut v: Vec<(usize, usize)> = self
            .plis
            .iter()
            .filter_map(|p| {
                let (a, b) = (analyse.ligne_utf16(p.from) + 1, analyse.ligne_utf16(p.to));
                (a <= b && b < analyse.lignes.len()).then_some((a, b))
            })
            .collect();
        v.sort_unstable();
        let mut fusion: Vec<(usize, usize)> = Vec::with_capacity(v.len());
        for (a, b) in v {
            match fusion.last_mut() {
                Some(d) if a <= d.1 + 1 => d.1 = d.1.max(b),
                _ => fusion.push((a, b)),
            }
        }
        fusion
    }
}

/// Le filtre du curseur : où une sélection VIDE en `pos` doit-elle atterrir ? (`transactionFilter`)
///
/// - dans le bloc Properties (`proprietes` = sa plage UTF-16, valide) : après le bloc, sur la ligne
///   d'après (ou à la fin du document s'il le termine) : une frappe dans la source cachée casserait le
///   bloc ;
/// - dans une plage repliée `(from, to]` : au bout de la ligne « parent » (`from`).
///
/// `longueur` = longueur du document en UTF-16. À n'appliquer qu'à une sélection unique et vide, et
/// pas aux transactions annotées `propertiesSync` (celles du widget lui-même).
#[must_use]
pub fn chasser_curseur(
    pos: usize,
    longueur: usize,
    proprietes: Option<Etendue>,
    repli: &EtatRepli,
) -> usize {
    let mut pos = pos;
    if let Some(r) = proprietes
        && pos <= r.to
    {
        pos = (r.to + 1).min(longueur);
    }
    // Un pli peut en contenir un autre : on remonte jusqu'à la stabilité.
    loop {
        let Some(p) = repli.plis.iter().find(|p| p.from < pos && pos <= p.to) else {
            return pos;
        };
        pos = p.from;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mapper_une_insertion_pure() {
        let e = Edition::remplacement(5, 5, 3);
        assert_eq!(e.mapper(5, true), Some(8));
        assert_eq!(e.mapper(5, false), Some(5));
        assert_eq!(e.mapper(9, false), Some(12));
    }

    #[test]
    fn mapper_une_suppression() {
        let e = Edition::remplacement(2, 6, 0);
        assert_eq!(e.mapper(2, true), Some(2));
        assert_eq!(e.mapper(4, true), None);
        assert_eq!(e.mapper(6, false), Some(2));
        assert_eq!(e.mapper(10, false), Some(6));
    }
}
