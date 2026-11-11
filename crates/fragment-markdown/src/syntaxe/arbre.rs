//! L'arbre syntaxique d'un document : nœuds à plat, en ordre préfixe, plages UTF-16 ET octets.
//!
//! C'est le `syntaxTree(state)` que Fragment lit dans CodeMirror, réduit à ce que lisent ses
//! consommateurs : le genre (`node.name`), la plage (`from`/`to`), la parenté, et un parcours LIMITÉ
//! à une plage (le viewport : `tree.iterate({ from, to, enter })`).
//!
//! ★ À plat plutôt qu'en arbre de boîtes : un parcours est une boucle sur un tableau, et « sauter un
//!   sous-arbre » est un saut à `fin_sous_arbre` (aucune récursion, aucune allocation).

use super::analyse_blocs::{self, Fin};
use super::en_ligne::Elt;
use super::genre::Genre;
use super::modele_rangees::{Etendue, Plage};

/// Un nœud de l'arbre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Noeud {
    pub genre: Genre,
    pub plage: Plage,
    /// Index du parent (`None` pour la racine `Document`).
    pub parent: Option<usize>,
    /// Index qui suit le dernier descendant : les descendants sont `index + 1 .. fin_sous_arbre`.
    pub fin_sous_arbre: usize,
    /// 0 pour la racine.
    pub profondeur: usize,
}

impl Noeud {
    /// Début en UTF-16 (`node.from`).
    #[must_use]
    pub fn from(&self) -> usize {
        self.plage.utf16.from
    }
    /// Fin en UTF-16 (`node.to`).
    #[must_use]
    pub fn to(&self) -> usize {
        self.plage.utf16.to
    }
}

/// Que faire des enfants d'un nœud visité.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Parcours {
    Entrer,
    /// Ne pas visiter ses descendants (le `return false` de `enter` en TS).
    Sauter,
}

/// De quel côté d'une position chercher un nœud (`resolveInner(pos, side)` de Lezer).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cote {
    /// Entre aussi dans les nœuds qui FINISSENT à la position.
    Avant,
    /// Seulement les nœuds qui couvrent la position des deux côtés.
    Dedans,
    /// Entre aussi dans les nœuds qui COMMENCENT à la position.
    Apres,
}

/// Un point de reprise de l'analyse, gardé dans l'arbre pour la réanalyse incrémentale
/// (`incremental.rs`) : le [`analyse_blocs::Repere`], placé dans l'arbre « brut » (sans le nœud
/// `FrontMatter`, qui n'est pas de Lezer).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Repere {
    /// Début de la ligne, en octets.
    pub octet: usize,
    /// Le même, en UTF-16.
    pub utf16: usize,
    /// Tout ce que l'analyse avait lu en arrivant là (octets, borne exclue).
    pub lu: usize,
    /// Index, dans l'arbre brut, du premier nœud produit à partir d'ici.
    pub noeud: usize,
}

/// L'arbre syntaxique Markdown d'un document (Lezer et les extensions de Fragment).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arbre {
    pub(super) noeuds: Vec<Noeud>,
    pub(super) front_matter: Option<Plage>,
    /// Longueur du texte analysé, en octets.
    pub(super) longueur: usize,
    /// Les points de reprise, par ligne croissante (et `lu` croissant).
    pub(super) reperes: Vec<Repere>,
}

impl Arbre {
    /// Analyse `texte` en entier. Après une modification, [`Arbre::reanalyser`] rend le même arbre
    /// en ne réanalysant que les blocs touchés.
    #[must_use]
    pub fn analyser(texte: &str) -> Arbre {
        let r = analyse_blocs::analyser_depuis(texte, 0, 0, |_| false);
        let fin = match r.fin {
            Fin::Document(fin) => fin,
            Fin::Arret => texte.len(),
        };
        let conv = Convertisseur::sur(texte, 0, texte.len(), 0);
        let mut noeuds = vec![Noeud {
            genre: Genre::Document,
            plage: Plage {
                utf16: conv.etendue(Etendue::new(0, fin)),
                octets: Etendue::new(0, fin),
            },
            parent: None,
            fin_sous_arbre: 0,
            profondeur: 0,
        }];
        let reperes = aplatir_bloc(&r.enfants, &r.reperes, &conv, &mut noeuds);
        noeuds[0].fin_sous_arbre = noeuds.len();
        Arbre::assembler(texte, noeuds, reperes)
    }

    /// L'arbre définitif à partir de l'arbre brut : le nœud `FrontMatter` enveloppe les blocs de tête.
    pub(super) fn assembler(texte: &str, mut noeuds: Vec<Noeud>, reperes: Vec<Repere>) -> Arbre {
        let front_matter = plage_front_matter(texte).map(|fin| {
            let octets = Etendue::new(0, fin);
            let u16 = fragment_text::utf16::utf16_len(texte.get(..fin).unwrap_or(""));
            envelopper_front_matter(fin, u16, &mut noeuds);
            Plage {
                utf16: Etendue::new(0, u16),
                octets,
            }
        });
        Arbre {
            noeuds,
            front_matter,
            longueur: texte.len(),
            reperes,
        }
    }

    /// L'inverse d'[`Arbre::assembler`] : l'arbre brut (nœud `FrontMatter` retiré) et ses repères.
    pub(super) fn en_brut(self) -> (Vec<Noeud>, Vec<Repere>) {
        let mut noeuds = self.noeuds;
        if noeuds.get(1).is_some_and(|n| n.genre == Genre::FrontMatter) {
            let fin_fm = noeuds[1].fin_sous_arbre;
            noeuds.remove(1);
            for (k, n) in noeuds.iter_mut().enumerate().skip(1) {
                n.fin_sous_arbre -= 1;
                n.parent = n.parent.map(|p| p.saturating_sub(1));
                if k < fin_fm - 1 {
                    n.profondeur -= 1;
                }
            }
            noeuds[0].fin_sous_arbre = noeuds.len();
        }
        (noeuds, self.reperes)
    }

    /// Tous les nœuds, en ordre préfixe (la racine `Document` en 0).
    #[must_use]
    pub fn noeuds(&self) -> &[Noeud] {
        &self.noeuds
    }

    #[must_use]
    pub fn noeud(&self, i: usize) -> &Noeud {
        &self.noeuds[i]
    }

    /// Le frontmatter de tête (`---` … `---`), même si le YAML est invalide. Le nœud `FrontMatter`
    /// existe aussi dans l'arbre, sauf si un bloc de Lezer déborde de cette plage (alors seule cette
    /// valeur le signale).
    #[must_use]
    pub fn front_matter(&self) -> Option<Plage> {
        self.front_matter
    }

    /// Les enfants directs de `i`, dans l'ordre.
    pub fn enfants(&self, i: usize) -> impl Iterator<Item = usize> + '_ {
        let fin = self.noeuds[i].fin_sous_arbre;
        let mut k = i + 1;
        std::iter::from_fn(move || {
            if k >= fin {
                return None;
            }
            let courant = k;
            k = self.noeuds[k].fin_sous_arbre;
            Some(courant)
        })
    }

    /// Les ancêtres de `i`, du parent à la racine.
    pub fn ancetres(&self, i: usize) -> impl Iterator<Item = usize> + '_ {
        let mut k = self.noeuds[i].parent;
        std::iter::from_fn(move || {
            let courant = k?;
            k = self.noeuds[courant].parent;
            Some(courant)
        })
    }

    /// Visite, en ordre préfixe, les nœuds qui TOUCHENT `[de, a]` (UTF-16, bornes comprises, comme
    /// `tree.iterate({ from, to })`). `f` rend `Parcours::Sauter` pour ne pas descendre.
    pub fn parcourir(&self, de: usize, a: usize, mut f: impl FnMut(usize, &Noeud) -> Parcours) {
        let mut i = 0;
        while i < self.noeuds.len() {
            let n = &self.noeuds[i];
            if n.to() < de || n.from() > a {
                i = n.fin_sous_arbre;
                continue;
            }
            match f(i, n) {
                Parcours::Entrer => i += 1,
                Parcours::Sauter => i = n.fin_sous_arbre,
            }
        }
    }

    /// Les index des nœuds qui touchent `[de, a]` (UTF-16), en ordre préfixe.
    #[must_use]
    pub fn noeuds_dans(&self, de: usize, a: usize) -> Vec<usize> {
        let mut v = Vec::new();
        self.parcourir(de, a, |i, _| {
            v.push(i);
            Parcours::Entrer
        });
        v
    }

    /// Le nœud le plus profond à la position `pos` (UTF-16), selon `cote` (`resolveInner`).
    #[must_use]
    pub fn noeud_a(&self, pos: usize, cote: Cote) -> usize {
        let mut courant = 0;
        'descente: loop {
            for e in self.enfants(courant) {
                let n = &self.noeuds[e];
                let dedans = match cote {
                    Cote::Avant => n.from() < pos && pos <= n.to(),
                    Cote::Dedans => n.from() < pos && pos < n.to(),
                    Cote::Apres => n.from() <= pos && pos < n.to(),
                };
                if dedans {
                    courant = e;
                    continue 'descente;
                }
                if n.from() > pos {
                    break;
                }
            }
            return courant;
        }
    }

    /// Le premier ancêtre de `i` (ou `i` lui-même) du genre donné.
    #[must_use]
    pub fn ancetre_de_genre(&self, i: usize, genre: Genre) -> Option<usize> {
        std::iter::once(i)
            .chain(self.ancetres(i))
            .find(|&k| self.noeuds[k].genre == genre)
    }
}

fn aplatir(
    e: &Elt,
    parent: Option<usize>,
    profondeur: usize,
    conv: &Convertisseur,
    out: &mut Vec<Noeud>,
) {
    let i = out.len();
    let octets = Etendue::new(e.from, e.to);
    out.push(Noeud {
        genre: e.genre,
        plage: Plage {
            utf16: conv.etendue(octets),
            octets,
        },
        parent,
        fin_sous_arbre: 0,
        profondeur,
    });
    for enfant in &e.enfants {
        aplatir(enfant, Some(i), profondeur + 1, conv, out);
    }
    out[i].fin_sous_arbre = out.len();
}

/// Aplatit les enfants `blocs` de la racine (profondeur 1) à la suite de `out`, et place dans l'arbre
/// les repères relevés pendant leur analyse (`enfants` compté depuis le premier de `blocs`).
pub(super) fn aplatir_bloc(
    blocs: &[Elt],
    reperes: &[analyse_blocs::Repere],
    conv: &Convertisseur,
    out: &mut Vec<Noeud>,
) -> Vec<Repere> {
    let mut debuts = Vec::with_capacity(blocs.len() + 1);
    for e in blocs {
        debuts.push(out.len());
        aplatir(e, Some(0), 1, conv, out);
    }
    debuts.push(out.len());
    reperes
        .iter()
        .map(|r| Repere {
            octet: r.ligne,
            utf16: conv.utf16(r.ligne),
            lu: r.lu,
            noeud: debuts.get(r.enfants).copied().unwrap_or(out.len()),
        })
        .collect()
}

/// Ligne 1 = `---` (espaces permis derrière) et une ligne `---` plus loin : la règle de
/// `frontmatterModel`. Rend la fin (octets) de la ligne `---` fermante.
fn plage_front_matter(texte: &str) -> Option<usize> {
    let est_delim = |l: &str| {
        l.strip_prefix("---")
            .is_some_and(|r| r.chars().all(char::is_whitespace))
    };
    let mut lignes = texte.split('\n');
    if !lignes.next().is_some_and(est_delim) {
        return None;
    }
    let mut pos = texte.find('\n')? + 1;
    for l in lignes {
        if est_delim(l) {
            return Some(pos + l.len());
        }
        pos += l.len() + 1;
    }
    None
}

/// Enveloppe dans un nœud `FrontMatter` (index 1) les blocs de tête de l'arbre brut qui tiennent
/// dans `[0, fin)`, sauf si le bloc suivant déborde de cette plage.
fn envelopper_front_matter(fin: usize, u16_fin: usize, noeuds: &mut Vec<Noeud>) {
    let mut k = 1;
    while k < noeuds.len() && noeuds[k].plage.octets.to <= fin {
        k = noeuds[k].fin_sous_arbre;
    }
    let deborde = noeuds.get(k).is_some_and(|n| n.plage.octets.from < fin);
    if deborde || k == 1 {
        return;
    }
    for (i, n) in noeuds.iter_mut().enumerate().skip(1) {
        n.fin_sous_arbre += 1;
        n.parent = n.parent.map(|p| if p == 0 && i >= k { 0 } else { p + 1 });
        if i < k {
            n.profondeur += 1;
        }
    }
    noeuds.insert(
        1,
        Noeud {
            genre: Genre::FrontMatter,
            plage: Plage {
                utf16: Etendue::new(0, u16_fin),
                octets: Etendue::new(0, fin),
            },
            parent: Some(0),
            fin_sous_arbre: k + 1,
            profondeur: 1,
        },
    );
    noeuds[0].fin_sous_arbre = noeuds.len();
}

/// Conversion octets → UTF-16, par ligne (identité sur une ligne ASCII).
pub(super) struct Convertisseur<'a> {
    texte: &'a str,
    /// (octet de début, UTF-16 de début, ligne ASCII ?)
    lignes: Vec<(usize, usize, bool)>,
}

impl<'a> Convertisseur<'a> {
    /// Un convertisseur valable pour les positions de `[de, a]`, sachant que `de` (un début de
    /// ligne) vaut `u16_de` en UTF-16.
    pub(super) fn sur(texte: &'a str, de: usize, a: usize, u16_de: usize) -> Self {
        let mut lignes = Vec::new();
        let (mut octet, mut u16) = (de, u16_de);
        for l in texte.get(de..a).unwrap_or("").split('\n') {
            let ascii = l.is_ascii();
            lignes.push((octet, u16, ascii));
            octet += l.len() + 1;
            u16 += if ascii {
                l.len()
            } else {
                fragment_text::utf16::utf16_len(l)
            } + 1;
        }
        Convertisseur { texte, lignes }
    }

    pub(super) fn utf16(&self, b: usize) -> usize {
        let k = self.lignes.partition_point(|l| l.0 <= b).saturating_sub(1);
        let (debut, u16, ascii) = self.lignes[k];
        if ascii {
            u16 + (b - debut)
        } else {
            u16 + fragment_text::utf16::utf16_len(self.texte.get(debut..b).unwrap_or(""))
        }
    }

    fn etendue(&self, e: Etendue) -> Etendue {
        Etendue::new(self.utf16(e.from), self.utf16(e.to))
    }
}
