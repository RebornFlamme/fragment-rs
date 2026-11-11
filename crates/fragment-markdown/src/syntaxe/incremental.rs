// Port du principe de réutilisation incrémentale de @lezer/markdown 1.7.2 et @lezer/common
// (`TreeFragment.applyChanges`, reprise des blocs intacts ; MIT, © Marijn Haverbeke et contributeurs).
// Traduction du principe, pas du code, voir syntaxe/mod.rs.
//! Réanalyse INCRÉMENTALE : après une modification, seuls les blocs de tête (enfants de `Document`)
//! touchés sont réanalysés ; ceux d'avant sont gardés tels quels, ceux d'après décalés.
//!
//! Le principe est celui de Lezer (des fragments de l'ancien arbre, réutilisés là où l'analyse
//! retombe dans le même état), réduit à ce que permet l'analyse de blocs :
//!
//! - Un **repère** est un pas de l'analyse où seule la racine est ouverte, au début d'une ligne. De
//!   là, la suite ne dépend que du texte qui suit (aucune lecture en arrière, aucun état hérité).
//! - **Avant** la modification : on reprend au dernier repère dont tout ce qui avait été LU (`lu`,
//!   lecture d'avance comprise : la ligne qui clôt un paragraphe, la ligne de délimiteurs d'un
//!   tableau…) précède le premier octet modifié. Tout ce qui précède est identique par construction.
//! - **Après** : dès que la nouvelle analyse atteint un repère situé au-delà de la modification et
//!   qu'un repère de l'ancien arbre tombait sur la même ligne (décalée), la suite est celle de
//!   l'ancien arbre, décalée en octets, en UTF-16 et en index.
//!
//! ★ Exact, pas heuristique : l'arbre rendu est IDENTIQUE à une analyse complète (repères compris),
//!   ce que vérifie l'oracle de `tests/syntaxe.rs`. Le prix : une modification à l'intérieur d'un
//!   très grand bloc de tête (une liste de mille éléments, une clôture de code ouverte qui avale la
//!   suite) réanalyse ce bloc entier, comme Lezer réanalyse ce qu'il ne peut réutiliser.

use super::analyse_blocs::{self, Fin};
use super::arbre::{Arbre, Convertisseur, Noeud, Repere, aplatir_bloc};
use super::genre::Genre;
use super::modele_rangees::{Etendue, Plage};

/// Une modification du texte, en OCTETS : `ancien[de..fin_ancien]` est devenu
/// `nouveau[de..fin_nouveau]` ; le reste est inchangé.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Changement {
    pub de: usize,
    pub fin_ancien: usize,
    pub fin_nouveau: usize,
}

impl Changement {
    /// Le remplacement de `[de, a)` (octets de l'ancien texte) par `insere`.
    #[must_use]
    pub fn remplacement(de: usize, a: usize, insere: &str) -> Changement {
        Changement {
            de,
            fin_ancien: a,
            fin_nouveau: de + insere.len(),
        }
    }

    /// La plus petite modification qui mène de `ancien` à `nouveau` (préfixe et suffixe communs,
    /// sur des frontières de caractères). Pour regrouper plusieurs modifications d'une transaction.
    #[must_use]
    pub fn entre(ancien: &str, nouveau: &str) -> Changement {
        let (a, n) = (ancien.as_bytes(), nouveau.as_bytes());
        let mut de = a.iter().zip(n).take_while(|(x, y)| x == y).count();
        while !ancien.is_char_boundary(de) || !nouveau.is_char_boundary(de) {
            de -= 1;
        }
        let max_suffixe = a.len().min(n.len()) - de;
        let mut s = a
            .iter()
            .rev()
            .zip(n.iter().rev())
            .take(max_suffixe)
            .take_while(|(x, y)| x == y)
            .count();
        while !ancien.is_char_boundary(a.len() - s) || !nouveau.is_char_boundary(n.len() - s) {
            s -= 1;
        }
        Changement {
            de,
            fin_ancien: a.len() - s,
            fin_nouveau: n.len() - s,
        }
    }
}

impl Arbre {
    /// L'arbre de `texte`, obtenu de `self` (l'arbre de l'ancien texte) après `changement`, en ne
    /// réanalysant que les blocs touchés. Identique à `Arbre::analyser(texte)`.
    ///
    /// Un changement incohérent avec les longueurs connues (ou hors du texte) retombe sur une
    /// analyse complète.
    #[must_use]
    pub fn reanalyser(self, texte: &str, c: Changement) -> Arbre {
        let coherent = c.de <= c.fin_ancien
            && c.de <= c.fin_nouveau
            && c.fin_ancien <= self.longueur
            && c.fin_nouveau <= texte.len()
            && self.longueur - c.fin_ancien == texte.len() - c.fin_nouveau
            && !self.noeuds.is_empty();
        if !coherent {
            return Arbre::analyser(texte);
        }
        let ancien_doc = self.noeuds[0].plage;
        let (mut brut, anciens) = self.en_brut();

        // Reprise : le dernier repère qui n'a rien lu au-delà de `de` (`lu` est croissant).
        let k = anciens.partition_point(|r| r.lu <= c.de);
        let reprise = match k {
            0 => Repere {
                octet: 0,
                utf16: 0,
                lu: 0,
                noeud: 1,
            },
            _ => anciens[k - 1],
        };

        // Arrêt : le premier repère de l'ancien arbre à partir duquel le texte est inchangé.
        let premier_apres = anciens.partition_point(|r| r.octet < c.fin_ancien);
        let mut rejoint: Option<usize> = None;
        let arret = |ligne: usize| {
            if ligne < c.fin_nouveau {
                return false;
            }
            let ancienne = ligne - c.fin_nouveau + c.fin_ancien;
            let suite = &anciens[premier_apres..];
            match suite.binary_search_by_key(&ancienne, |r| r.octet) {
                Ok(j) => {
                    rejoint = Some(premier_apres + j);
                    true
                }
                Err(_) => false,
            }
        };
        let r = analyse_blocs::analyser_depuis(texte, reprise.octet, reprise.lu, arret);

        // L'ancienne suite (à décaler), puis le préfixe gardé.
        let suite = match rejoint {
            Some(j) => brut.split_off(anciens[j].noeud.min(brut.len())),
            None => Vec::new(),
        };
        brut.truncate(reprise.noeud);

        // Les blocs réanalysés.
        let borne = match (&r.fin, rejoint) {
            (Fin::Arret, Some(j)) => (anciens[j].octet + c.fin_nouveau - c.fin_ancien)
                .max(r.enfants.iter().map(|e| e.to).max().unwrap_or(0))
                .min(texte.len()),
            _ => texte.len(),
        };
        let conv = Convertisseur::sur(texte, reprise.octet, borne, reprise.utf16);
        let mut reperes = anciens[..k.saturating_sub(1)].to_vec();
        reperes.extend(aplatir_bloc(&r.enfants, &r.reperes, &conv, &mut brut));

        // La suite de l'ancien arbre, décalée.
        let fin_doc = match (r.fin, rejoint) {
            (Fin::Arret, Some(j)) => {
                let ancien = anciens[j];
                let nouveau = reperes.last().copied().unwrap_or(reprise);
                let (d_octets, d_u16, d_noeud) = (
                    nouveau.octet as isize - ancien.octet as isize,
                    nouveau.utf16 as isize - ancien.utf16 as isize,
                    brut.len() as isize - ancien.noeud as isize,
                );
                let dec = |v: usize, d: isize| v.saturating_add_signed(d);
                brut.extend(suite.into_iter().map(|n| Noeud {
                    plage: Plage {
                        utf16: Etendue::new(
                            dec(n.plage.utf16.from, d_u16),
                            dec(n.plage.utf16.to, d_u16),
                        ),
                        octets: Etendue::new(
                            dec(n.plage.octets.from, d_octets),
                            dec(n.plage.octets.to, d_octets),
                        ),
                    },
                    parent: n.parent.map(|p| if p == 0 { 0 } else { dec(p, d_noeud) }),
                    fin_sous_arbre: dec(n.fin_sous_arbre, d_noeud),
                    ..n
                }));
                reperes.extend(anciens[j + 1..].iter().map(|a| Repere {
                    octet: dec(a.octet, d_octets),
                    utf16: dec(a.utf16, d_u16),
                    lu: dec(a.lu, d_octets),
                    noeud: dec(a.noeud, d_noeud),
                }));
                Plage {
                    utf16: Etendue::new(0, dec(ancien_doc.utf16.to, d_u16)),
                    octets: Etendue::new(0, dec(ancien_doc.octets.to, d_octets)),
                }
            }
            (fin, _) => {
                let fin = match fin {
                    Fin::Document(fin) => fin,
                    Fin::Arret => texte.len(),
                };
                // ★ Comme `Composite::en_elt` : la racine couvre au moins son dernier enfant, qui
                //   peut appartenir au préfixe gardé si rien n'a été produit depuis la reprise.
                let mut i = 1;
                let mut dernier = 0;
                while i < brut.len() {
                    dernier = brut[i].plage.octets.to;
                    i = brut[i].fin_sous_arbre.max(i + 1);
                }
                let fin = fin.max(dernier);
                Plage {
                    utf16: Etendue::new(0, conv.utf16(fin)),
                    octets: Etendue::new(0, fin),
                }
            }
        };
        brut[0] = Noeud {
            genre: Genre::Document,
            plage: fin_doc,
            parent: None,
            fin_sous_arbre: brut.len(),
            profondeur: 0,
        };
        Arbre::assembler(texte, brut, reperes)
    }
}
