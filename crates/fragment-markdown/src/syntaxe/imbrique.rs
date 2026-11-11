// Port de app/src/ui/views/markdown/language.ts (`MathHighlight` : parseMixed sur InlineMath,
// InlineDisplayMath et BlockMath) et de `codeLanguages` (livePreview.ts : un langage par bloc
// ``` selon son info) ; principe de @lezer/common `parseMixed` (MIT, © Marijn Haverbeke et
// contributeurs), traduit : une couche de jetons posée SUR l'arbre Markdown (ADR-007).
//! Les langages imbriqués : la coloration des blocs de code et des maths, posée sur l'arbre
//! Markdown.
//!
//! Fragment imbrique deux sortes de langages dans le Markdown : le code d'un bloc ``` (langage lu
//! dans l'info), le LaTeX des formules `$…$` et `$$…$$` (sans leurs délimiteurs). Ici l'arbre
//! Markdown reste celui de `Arbre` ; les langages imbriqués sont une COUCHE à côté : une liste de
//! régions, chacune avec ses jetons (des noms de jetons du thème, [`JetonCode`], jamais une couleur).
//!
//! # Incrémental
//!
//! [`Imbrique::reanalyser`] prend le nouvel arbre (déjà réanalysé) et le [`Changement`] :
//!
//! - les régions d'AVANT les derniers blocs de tête touchés sont gardées telles quelles ;
//! - on parcourt ensuite les blocs de tête du nouvel arbre ; une région qui n'a pas bougé (même
//!   plage, même langage, hors de la modification) est reprise de l'ancienne couche, sinon elle est
//!   recolorée ENTIÈRE ;
//! - dès qu'un bloc de tête situé après la modification a, dans l'ancien arbre, un bloc de tête au
//!   même endroit (décalé), tout ce qui suit est identique (l'analyse de blocs repart d'un état de
//!   racine seule, qui ne dépend que du texte suivant) : les régions suivantes sont reprises et
//!   décalées de leur seul `base`, leurs jetons étant RELATIFS à ce `base`.
//!
//! ★ Exact : le résultat est égal à [`Imbrique::analyser`] (oracle de `tests/syntaxe_imbrique.rs`).
//!   Le prix : une frappe dans un grand bloc de code recolore ce bloc entier.

use std::ptr;
use std::sync::Arc;

use super::arbre::Arbre;
use super::genre::Genre;
use super::incremental::Changement;
use super::modele_rangees::{Etendue, Plage};

#[path = "imbrique_code.rs"]
mod code;
#[path = "imbrique_jetons.rs"]
mod jetons;
#[path = "imbrique_langues.rs"]
mod langues;
#[path = "imbrique_stex.rs"]
mod stex;

pub use jetons::JetonCode;
use langues::{Langue, langue};
use stex::Stex;

/// Un jeton posé dans le texte (plages UTF-16 et octets, comme les nœuds de l'arbre).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JetonPose {
    pub plage: Plage,
    pub jeton: JetonCode,
}

/// Ce que contient une région : un langage de programmation, ou du LaTeX.
#[derive(Debug, Clone, Copy)]
enum Nature {
    Code(&'static Langue),
    Maths,
}

impl PartialEq for Nature {
    fn eq(&self, autre: &Nature) -> bool {
        match (self, autre) {
            (Nature::Code(a), Nature::Code(b)) => ptr::eq(*a, *b),
            (Nature::Maths, Nature::Maths) => true,
            _ => false,
        }
    }
}
impl Eq for Nature {}

/// Le contenu d'une région, partagé (`Arc`) entre les couches successives : reprendre une région
/// d'avant ou d'après la modification ne copie aucun jeton.
#[derive(Debug, PartialEq, Eq)]
struct Corps {
    nature: Nature,
    /// Les segments colorés, relatifs à `base` (plusieurs si le bloc est dans une citation ou une
    /// liste : les préfixes de ligne n'en font pas partie).
    segments: Vec<(usize, usize)>,
    /// Les jetons (début, fin, jeton), relatifs à `base`, triés.
    jetons: Vec<(usize, usize, JetonCode)>,
}

/// Une région colorée : un bloc de code (ses `CodeText`) ou l'intérieur d'une formule.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Region {
    /// Début, en octets : le contenu est RELATIF à ce début (un décalage ne touche que lui).
    base: usize,
    corps: Arc<Corps>,
}

impl std::ops::Deref for Region {
    type Target = Corps;
    fn deref(&self) -> &Corps {
        &self.corps
    }
}

impl Region {
    fn fin(&self) -> usize {
        self.base + self.segments.last().map_or(0, |s| s.1)
    }
}

/// La couche des langages imbriqués d'un document.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Imbrique {
    regions: Vec<Region>,
    /// Début (octets) des blocs de tête de l'arbre, pour retrouver le raccord à la réanalyse.
    tetes: Vec<usize>,
    /// Longueur du texte analysé, en octets.
    longueur: usize,
}

/// Une région trouvée dans l'arbre, avant coloration : sa nature et ses segments (octets absolus).
struct Candidate {
    nature: Nature,
    segments: Vec<(usize, usize)>,
}

/// La région que porte le nœud `i`, s'il en porte une.
fn candidate(arbre: &Arbre, texte: &str, i: usize) -> Option<Candidate> {
    let n = arbre.noeud(i);
    let (de, a) = (n.plage.octets.from, n.plage.octets.to);
    match n.genre {
        Genre::FencedCode => {
            let mut info = "";
            let mut segments = Vec::new();
            for e in arbre.enfants(i) {
                let f = arbre.noeud(e);
                let (d, t) = (f.plage.octets.from, f.plage.octets.to);
                match f.genre {
                    Genre::CodeInfo => info = texte.get(d..t)?,
                    Genre::CodeText if d < t => segments.push((d, t)),
                    _ => {}
                }
            }
            let langue = langue(info)?;
            (!segments.is_empty()).then_some(Candidate {
                nature: Nature::Code(langue),
                segments,
            })
        }
        Genre::InlineMath | Genre::InlineDisplayMath | Genre::BlockMath => {
            // ★ Comme `language.ts` : on enlève 1 ($) ou 2 ($$) caractères de chaque côté, SANS
            //   vérifier que le nœud est fermé (un `$$` jamais fermé perd ses deux derniers octets).
            let delim = if n.genre == Genre::InlineMath { 1 } else { 2 };
            let (d, t) = (de + delim, a.checked_sub(delim)?);
            (d < t && texte.get(d..t).is_some()).then_some(Candidate {
                nature: Nature::Maths,
                segments: vec![(d, t)],
            })
        }
        _ => None,
    }
}

/// Colore une région.
fn colorer(texte: &str, c: &Candidate) -> Region {
    let base = c.segments[0].0;
    let absolus = match c.nature {
        Nature::Code(l) => code::colorer(l, texte, &c.segments),
        Nature::Maths => {
            let (de, a) = c.segments[0];
            Stex::colorer(&texte[de..a])
                .into_iter()
                .map(|(x, y, j)| (x + de, y + de, j))
                .collect()
        }
    };
    Region {
        base,
        corps: Arc::new(Corps {
            nature: c.nature,
            segments: c
                .segments
                .iter()
                .map(|&(d, a)| (d - base, a - base))
                .collect(),
            jetons: absolus
                .into_iter()
                .map(|(d, a, j)| (d - base, a - base, j))
                .collect(),
        }),
    }
}

/// Les candidates du sous-arbre `[i, fin)`.
fn chercher(arbre: &Arbre, texte: &str, i: usize, fin: usize, sortie: &mut Vec<Candidate>) {
    let mut j = i;
    while j < fin {
        if let Some(c) = candidate(arbre, texte, j) {
            sortie.push(c);
        }
        j += 1;
    }
}

impl Imbrique {
    /// Colore `texte` en entier ; `arbre` est son arbre Markdown.
    #[must_use]
    pub fn analyser(texte: &str, arbre: &Arbre) -> Imbrique {
        let mut resultat = Imbrique {
            regions: Vec::new(),
            tetes: Vec::new(),
            longueur: texte.len(),
        };
        let noeuds = arbre.noeuds();
        let mut i = 1;
        let mut trouvees = Vec::new();
        while i < noeuds.len() {
            let n = &noeuds[i];
            if n.genre == Genre::FrontMatter {
                i += 1;
                continue;
            }
            resultat.tetes.push(n.plage.octets.from);
            chercher(arbre, texte, i, n.fin_sous_arbre, &mut trouvees);
            i = n.fin_sous_arbre.max(i + 1);
        }
        resultat.regions = trouvees.iter().map(|c| colorer(texte, c)).collect();
        resultat
    }

    /// La couche de `texte`, obtenue de `self` (la couche de l'ancien texte) après `c`. `arbre` est
    /// l'arbre de `texte`. Égale à `Imbrique::analyser(texte, arbre)`.
    #[must_use]
    pub fn reanalyser(self, texte: &str, arbre: &Arbre, c: Changement) -> Imbrique {
        let coherent = c.de <= c.fin_ancien
            && c.de <= c.fin_nouveau
            && c.fin_ancien <= self.longueur
            && c.fin_nouveau <= texte.len()
            && self.longueur - c.fin_ancien == texte.len() - c.fin_nouveau;
        if !coherent {
            return Imbrique::analyser(texte, arbre);
        }
        let delta = c.fin_nouveau as isize - c.fin_ancien as isize;
        let noeuds = arbre.noeuds();

        // Départ : trois blocs de tête avant la modification (un paragraphe peut devenir un titre
        // par la ligne qui suit, un tableau peut s'allonger : les blocs d'avant dépendent de ce qui
        // les suit d'une ligne).
        let k = self.tetes.partition_point(|&p| p <= c.de);
        let k0 = k.saturating_sub(3);
        let depart = if k0 == 0 { 0 } else { self.tetes[k0] };
        let mut i = if k0 == 0 {
            1
        } else {
            noeuds.partition_point(|n| n.plage.octets.from < depart)
        };
        if k0 > 0 && noeuds.get(i).is_none_or(|n| n.plage.octets.from != depart) {
            return Imbrique::analyser(texte, arbre);
        }

        let mut tetes: Vec<usize> = self.tetes[..k0].to_vec();
        let mut trouvees: Vec<Candidate> = Vec::new();
        let mut raccord = None;
        while i < noeuds.len() {
            let n = &noeuds[i];
            if n.genre == Genre::FrontMatter {
                i += 1;
                continue;
            }
            let p = n.plage.octets.from;
            if p >= c.fin_nouveau {
                let ancien = p.saturating_add_signed(-delta);
                if let Ok(j) = self.tetes.binary_search(&ancien) {
                    raccord = Some(j);
                    break;
                }
            }
            tetes.push(p);
            chercher(arbre, texte, i, n.fin_sous_arbre, &mut trouvees);
            i = n.fin_sous_arbre.max(i + 1);
        }

        // Les régions : celles d'avant le départ, les régions parcourues (reprises ou recolorées),
        // puis celles qui suivent le raccord, décalées.
        let anciennes = &self.regions;
        let avant = anciennes.partition_point(|r| r.fin() <= depart);
        let apres = raccord.map(|j| {
            let debut = self.tetes[j];
            anciennes.partition_point(|r| r.base < debut)
        });
        let mut regions: Vec<Region> = anciennes[..avant].to_vec();
        for cand in &trouvees {
            regions
                .push(reprendre(anciennes, cand, c, delta).unwrap_or_else(|| colorer(texte, cand)));
        }
        if let (Some(j), Some(a)) = (raccord, apres) {
            let dec = |v: usize| v.saturating_add_signed(delta);
            regions.extend(anciennes[a..].iter().map(|r| Region {
                base: dec(r.base),
                ..r.clone()
            }));
            tetes.extend(self.tetes[j..].iter().map(|&p| dec(p)));
        }
        Imbrique {
            regions,
            tetes,
            longueur: texte.len(),
        }
    }

    /// Tous les jetons, en octets absolus, dans l'ordre du texte.
    pub fn jetons(&self) -> impl Iterator<Item = (Etendue, JetonCode)> + '_ {
        self.regions.iter().flat_map(|r| {
            r.jetons
                .iter()
                .map(move |&(d, a, j)| (Etendue::new(r.base + d, r.base + a), j))
        })
    }

    /// Les jetons qui rencontrent `[de, a)` (octets), avec leurs plages UTF-16 : ce que lit le rendu
    /// pour le viewport.
    #[must_use]
    pub fn jetons_dans(&self, texte: &str, de: usize, a: usize) -> Vec<JetonPose> {
        let mut sortie = Vec::new();
        let premiere = self.regions.partition_point(|r| r.fin() <= de);
        // Curseur d'octets → UTF-16, qui n'avance que vers l'avant.
        let (mut octet, mut u16) = (0usize, 0usize);
        let mut vers_u16 = |cible: usize| -> usize {
            if cible > octet {
                u16 += fragment_text::utf16::utf16_len(texte.get(octet..cible).unwrap_or(""));
                octet = cible;
            }
            u16
        };
        for r in &self.regions[premiere..] {
            if r.base >= a {
                break;
            }
            let debut = r.jetons.partition_point(|&(_, fin, _)| r.base + fin <= de);
            for &(d, f, j) in &r.jetons[debut..] {
                let (d, f) = (r.base + d, r.base + f);
                if d >= a {
                    break;
                }
                let d16 = vers_u16(d);
                let f16 = vers_u16(f);
                sortie.push(JetonPose {
                    plage: Plage {
                        utf16: Etendue::new(d16, f16),
                        octets: Etendue::new(d, f),
                    },
                    jeton: j,
                });
            }
        }
        sortie
    }

    /// Le nombre de régions colorées.
    #[must_use]
    pub fn nombre_de_regions(&self) -> usize {
        self.regions.len()
    }
}

/// La région de l'ancienne couche qui équivaut à `cand`, déplacée à sa place : une région ne change
/// pas si elle est entièrement avant la modification (même place) ou entièrement après (décalée),
/// avec les mêmes segments et le même langage.
fn reprendre(
    anciennes: &[Region],
    cand: &Candidate,
    c: Changement,
    delta: isize,
) -> Option<Region> {
    let base = cand.segments[0].0;
    let fin = cand.segments.last()?.1;
    let ancienne_base = if fin <= c.de {
        base
    } else if base >= c.fin_nouveau {
        base.checked_add_signed(-delta)?
    } else {
        return None;
    };
    let k = anciennes
        .binary_search_by_key(&ancienne_base, |r| r.base)
        .ok()?;
    let r = &anciennes[k];
    let memes_segments = r.segments.len() == cand.segments.len()
        && r.segments
            .iter()
            .zip(&cand.segments)
            .all(|(&(d, a), &(cd, ca))| d == cd - base && a == ca - base);
    (r.nature == cand.nature && memes_segments).then(|| Region { base, ..r.clone() })
}
