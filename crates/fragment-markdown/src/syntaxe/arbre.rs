//! L'arbre syntaxique d'un document : nœuds à plat, en ordre préfixe, plages UTF-16 ET octets.
//!
//! C'est le `syntaxTree(state)` que Fragment lit dans CodeMirror, réduit à ce que lisent ses
//! consommateurs : le genre (`node.name`), la plage (`from`/`to`), la parenté, et un parcours LIMITÉ
//! à une plage (le viewport : `tree.iterate({ from, to, enter })`).
//!
//! ★ À plat plutôt qu'en arbre de boîtes : un parcours est une boucle sur un tableau, et « sauter un
//!   sous-arbre » est un saut à `fin_sous_arbre` (aucune récursion, aucune allocation).

use super::analyse_blocs;
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

/// L'arbre syntaxique Markdown d'un document (Lezer et les extensions de Fragment).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arbre {
    noeuds: Vec<Noeud>,
    front_matter: Option<Plage>,
}

impl Arbre {
    /// Analyse `texte` en entier.
    ///
    /// ★ Pas d'analyse incrémentale : Lezer réutilise les fragments de l'arbre précédent, ici on
    ///   réanalyse tout. Mesuré (ADR-010) : quelques millisecondes pour 10 000 lignes en `--release`,
    ///   sous le budget d'une image ; à revoir si `fr-t3-surface` mesure autre chose sur DirectWrite.
    #[must_use]
    pub fn analyser(texte: &str) -> Arbre {
        let mut racine = analyse_blocs::analyser(texte);
        let front_matter = envelopper_front_matter(texte, &mut racine);
        let conv = Convertisseur::new(texte);
        let mut noeuds = Vec::new();
        aplatir(&racine, None, 0, &conv, &mut noeuds);
        Arbre {
            noeuds,
            front_matter: front_matter.map(|e| Plage {
                utf16: conv.etendue(e),
                octets: e,
            }),
        }
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

/// Ligne 1 = `---` (espaces permis derrière) et une ligne `---` plus loin : la règle de
/// `frontmatterModel`. Enveloppe les nœuds de tête qui tiennent dedans ; rend la plage en octets.
fn envelopper_front_matter(texte: &str, racine: &mut Elt) -> Option<Etendue> {
    let est_delim = |l: &str| {
        l.strip_prefix("---")
            .is_some_and(|r| r.chars().all(char::is_whitespace))
    };
    let mut lignes = texte.split('\n');
    if !lignes.next().is_some_and(est_delim) {
        return None;
    }
    let mut pos = texte.find('\n')? + 1;
    let mut fin = None;
    for l in lignes {
        if est_delim(l) {
            fin = Some(pos + l.len());
            break;
        }
        pos += l.len() + 1;
    }
    let fin = fin?;
    let n = racine.enfants.iter().take_while(|e| e.to <= fin).count();
    let deborde = racine.enfants.get(n).is_some_and(|e| e.from < fin);
    if !deborde && n > 0 {
        let dedans: Vec<Elt> = racine.enfants.drain(..n).collect();
        racine
            .enfants
            .insert(0, Elt::avec(Genre::FrontMatter, 0, fin, dedans));
    }
    Some(Etendue::new(0, fin))
}

/// Conversion octets → UTF-16, par ligne (identité sur une ligne ASCII).
struct Convertisseur<'a> {
    texte: &'a str,
    /// (octet de début, UTF-16 de début, ligne ASCII ?)
    lignes: Vec<(usize, usize, bool)>,
}

impl<'a> Convertisseur<'a> {
    fn new(texte: &'a str) -> Self {
        let mut lignes = Vec::new();
        let (mut octet, mut u16) = (0, 0);
        for l in texte.split('\n') {
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

    fn utf16(&self, b: usize) -> usize {
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
