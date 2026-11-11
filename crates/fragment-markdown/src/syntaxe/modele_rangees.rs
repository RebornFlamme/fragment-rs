//! Les types PARTAGÉS du live preview : ce que `decorations` (en ligne) et `blocs` (blocs, repli)
//! produisent, et ce que la surface d'édition (`fr-t3-surface`) donne au fork de l'éditeur
//! (`InputBaseState::set_row_model` et `set_line_font_sizes`, ADR-006).
//!
//! Pas de port : Fragment confie tout cela à CodeMirror (`Decoration.replace`, `Decoration.widget`,
//! `Decoration.line`). Ici le modèle est pur et explicite, pour être testé sans GPUI.
//!
//! Règles :
//! - une POSITION est en unités UTF-16 (ADR-004) ; chaque plage porte AUSSI ses octets, parce que le
//!   fork parle en octets UTF-8 de la source (`Range<usize>`) : la conversion est faite une fois, à la
//!   construction (`Plage::depuis_octets` / `Plage::depuis_utf16`), jamais dans la boucle de rendu ;
//! - une LIGNE est un numéro de ligne logique, à partir de 0 (la convention du fork : `blocks =
//!   [(1, 120 px)]` met le bloc sous la deuxième ligne) ;
//! - une hauteur ou une taille est en pixels logiques (`f32`), le `Pixels` de GPUI, sans dépendre de
//!   GPUI (ce module est testé sur Linux).
//!
//! ★ Le fork n'a que deux façons de placer un bloc : de la HAUTEUR AJOUTÉE sous une ligne (le curseur
//!   ne peut pas y entrer, Haut/Bas la traversent d'un coup), ou « à la place » = la source masquée et
//!   une hauteur dictée (une rangée, un arrêt du curseur). `Placement` nomme les deux ;
//!   `ModeleRangees::vers_fork` fait la traduction.

use fragment_text::utf16::{byte_to_utf16, utf16_to_byte};

/// Un intervalle `[from, to)` dans UNE unité (UTF-16 ou octets, selon le champ qui le porte).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
pub struct Etendue {
    pub from: usize,
    pub to: usize,
}

impl Etendue {
    #[must_use]
    pub fn new(from: usize, to: usize) -> Self {
        Etendue { from, to }
    }
    #[must_use]
    pub fn len(self) -> usize {
        self.to.saturating_sub(self.from)
    }
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.to <= self.from
    }
    /// `from <= pos <= to` (bornes comprises, comme le `touches` du curseur de Fragment).
    #[must_use]
    pub fn touche(self, pos: usize) -> bool {
        self.from <= pos && pos <= self.to
    }
    /// Les deux intervalles se touchent ou se chevauchent (`a.to >= b.from && a.from <= b.to`).
    #[must_use]
    pub fn touche_etendue(self, autre: Etendue) -> bool {
        self.to >= autre.from && self.from <= autre.to
    }
    #[must_use]
    pub fn contient(self, autre: Etendue) -> bool {
        self.from <= autre.from && autre.to <= self.to
    }
    /// L'intervalle `Range` de Rust.
    #[must_use]
    pub fn range(self) -> std::ops::Range<usize> {
        self.from..self.to
    }
}

/// Une plage de la source, en UTF-16 (le contrat) ET en octets UTF-8 (pour le fork et le découpage
/// d'un `&str`). Les deux décrivent EXACTEMENT les mêmes caractères.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
pub struct Plage {
    pub utf16: Etendue,
    pub octets: Etendue,
}

impl Plage {
    /// Une plage donnée en octets de `texte` (bornes sur des frontières de caractères).
    #[must_use]
    pub fn depuis_octets(texte: &str, from: usize, to: usize) -> Plage {
        Plage {
            utf16: Etendue::new(byte_to_utf16(texte, from), byte_to_utf16(texte, to)),
            octets: Etendue::new(from, to),
        }
    }

    /// Une plage donnée en UTF-16 ; `None` si une borne est hors du texte ou au milieu d'une paire de
    /// substitution.
    #[must_use]
    pub fn depuis_utf16(texte: &str, from: usize, to: usize) -> Option<Plage> {
        Some(Plage {
            utf16: Etendue::new(from, to),
            octets: Etendue::new(utf16_to_byte(texte, from)?, utf16_to_byte(texte, to)?),
        })
    }

    /// Le texte de la plage.
    #[must_use]
    pub fn texte<'a>(&self, source: &'a str) -> &'a str {
        source.get(self.octets.range()).unwrap_or("")
    }
}

/// Les lignes logiques de `texte` (sans leur saut de ligne), en UTF-16 et en octets.
#[must_use]
pub fn lignes_de(texte: &str) -> Vec<Plage> {
    let mut res = Vec::new();
    let (mut octet, mut u16) = (0, 0);
    for l in texte.split('\n') {
        let n16 = fragment_text::utf16::utf16_len(l);
        res.push(Plage {
            utf16: Etendue::new(u16, u16 + n16),
            octets: Etendue::new(octet, octet + l.len()),
        });
        octet += l.len() + 1;
        u16 += n16 + 1;
    }
    res
}

/// La hauteur DICTÉE d'une ligne logique (un titre, une ligne remplacée par un widget).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HauteurLigne {
    pub ligne: usize,
    pub hauteur: f32,
}

/// La taille de police DICTÉE d'une ligne logique (un H1 plus gros, pas seulement plus haut).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PoliceLigne {
    pub ligne: usize,
    pub taille: f32,
}

/// Ce qu'un bloc rend (le widget que l'hôte des blocs, `fr-t3-widgets-blocs`, instancie).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GenreBloc {
    /// `$$…$$` (`BlockMath`).
    Maths,
    /// `![[…]]` seul sur sa ligne.
    Embed,
    /// Tableau GFM (toujours remplacé, jamais révélé : on édite les cellules).
    Tableau,
    /// Le frontmatter (`FrontMatter`), rendu en Properties.
    Proprietes,
    /// Bloc HTML (`HTMLBlock`), assaini.
    Html,
    /// Bloc de code (badge de langue, coloration).
    Code,
    /// Règle horizontale.
    Separateur,
}

/// Où se place un bloc par rapport à sa source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Placement {
    /// La source reste visible (curseur dedans) ; le rendu est AJOUTÉ sous sa dernière ligne.
    /// (Fragment : `Decoration.widget({ block: true, side: 1 })`.)
    Dessous,
    /// La source est masquée ; le rendu occupe sa place, à la hauteur du bloc.
    /// (Fragment : `Decoration.replace({ block: true })`.)
    ALaPlace,
}

/// Un bloc rendu (widget de bloc) et la source qu'il représente.
#[derive(Debug, Clone, PartialEq)]
pub struct Bloc {
    pub genre: GenreBloc,
    /// La source du bloc (lignes entières, sans le saut de ligne final).
    pub source: Plage,
    /// Première et dernière lignes logiques de la source (bornes comprises).
    pub premiere_ligne: usize,
    pub derniere_ligne: usize,
    pub placement: Placement,
    /// Hauteur du rendu, en pixels logiques (mesurée par le widget ; 0 tant qu'elle ne l'est pas).
    pub hauteur: f32,
}

/// Le modèle de rangées d'un document : ce qui est masqué, les hauteurs et tailles dictées, les blocs.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ModeleRangees {
    /// Plages de source dessinées en largeur 0 (marques cachées hors curseur).
    pub masques: Vec<Plage>,
    pub hauteurs: Vec<HauteurLigne>,
    pub polices: Vec<PoliceLigne>,
    pub blocs: Vec<Bloc>,
}

/// Les arguments de `set_row_model` et `set_line_font_sizes` du fork, en octets et lignes.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ArgumentsFork {
    /// `hidden` : plages d'octets triées, disjointes.
    pub masques: Vec<std::ops::Range<usize>>,
    /// `line_heights` : (ligne, px), triées, une par ligne.
    pub hauteurs: Vec<(usize, f32)>,
    /// `blocks` : (ligne, px) de hauteur ajoutée SOUS la ligne.
    pub blocs: Vec<(usize, f32)>,
    /// `set_line_font_sizes` : (ligne, px).
    pub polices: Vec<(usize, f32)>,
}

impl ModeleRangees {
    /// Trie et fusionne : masques triés et disjoints (deux masques qui se touchent n'en font qu'un),
    /// une hauteur et une police par ligne (la PLUS GRANDE l'emporte), blocs triés par ligne.
    pub fn normaliser(&mut self) {
        self.masques.retain(|m| !m.octets.is_empty());
        self.masques.sort_by_key(|p| (p.octets.from, p.octets.to));
        let mut fusion: Vec<Plage> = Vec::with_capacity(self.masques.len());
        for m in self.masques.drain(..) {
            match fusion.last_mut() {
                Some(d) if m.octets.from <= d.octets.to => {
                    if m.octets.to > d.octets.to {
                        d.octets.to = m.octets.to;
                        d.utf16.to = m.utf16.to;
                    }
                }
                _ => fusion.push(m),
            }
        }
        self.masques = fusion;
        self.hauteurs
            .sort_by(|a, b| a.ligne.cmp(&b.ligne).then(b.hauteur.total_cmp(&a.hauteur)));
        self.hauteurs.dedup_by_key(|h| h.ligne);
        self.polices
            .sort_by(|a, b| a.ligne.cmp(&b.ligne).then(b.taille.total_cmp(&a.taille)));
        self.polices.dedup_by_key(|p| p.ligne);
        self.blocs
            .sort_by_key(|b| (b.premiere_ligne, b.derniere_ligne));
    }

    /// Traduit le modèle pour le fork. `lignes` = la plage de chaque ligne logique, sans son saut de
    /// ligne (`lignes_de`), pour masquer la source d'un bloc `ALaPlace` ligne par ligne.
    ///
    /// - `Dessous` → `blocks = (derniere_ligne, hauteur)`.
    /// - `ALaPlace` → chaque ligne de la source masquée ; `hauteur` dictée à la PREMIÈRE ligne, 0 aux
    ///   suivantes. ★ Le fork a été éprouvé sur une ligne remplacée (S1b, « bloc à la place ») ; une
    ///   rangée de hauteur 0 pour les lignes suivantes reste à confirmer par `fr-t3-widgets-blocs`.
    #[must_use]
    pub fn vers_fork(&self, lignes: &[Plage]) -> ArgumentsFork {
        let mut m = self.clone();
        let mut blocs = Vec::new();
        for b in &self.blocs {
            match b.placement {
                Placement::Dessous => blocs.push((b.derniere_ligne, b.hauteur)),
                Placement::ALaPlace => {
                    for l in b.premiere_ligne..=b.derniere_ligne {
                        if let Some(p) = lignes.get(l) {
                            m.masques.push(*p);
                        }
                        let hauteur = if l == b.premiere_ligne {
                            b.hauteur
                        } else {
                            0.0
                        };
                        m.hauteurs.push(HauteurLigne { ligne: l, hauteur });
                    }
                }
            }
        }
        m.normaliser();
        blocs.sort_by_key(|&(l, _)| l);
        ArgumentsFork {
            masques: m.masques.iter().map(|p| p.octets.range()).collect(),
            hauteurs: m.hauteurs.iter().map(|h| (h.ligne, h.hauteur)).collect(),
            blocs,
            polices: m.polices.iter().map(|p| (p.ligne, p.taille)).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plage_utf16_et_octets_coincident_sur_le_meme_texte() {
        let t = "é😀ab";
        let p = Plage::depuis_octets(t, 2, 6);
        assert_eq!(p.utf16, Etendue::new(1, 3));
        assert_eq!(p.texte(t), "😀");
        assert_eq!(Plage::depuis_utf16(t, 1, 3), Some(p));
        // Au milieu d'une paire de substitution : refusé.
        assert_eq!(Plage::depuis_utf16(t, 2, 3), None);
    }

    #[test]
    fn lignes_logiques() {
        let l = lignes_de("a\néb\n");
        assert_eq!(l.len(), 3);
        assert_eq!(l[1].octets, Etendue::new(2, 5));
        assert_eq!(l[1].utf16, Etendue::new(2, 4));
        assert!(l[2].octets.is_empty());
    }

    #[test]
    fn normaliser_fusionne_les_masques_et_garde_la_plus_grande_hauteur() {
        let t = "## T **g**";
        let mut m = ModeleRangees {
            masques: vec![
                Plage::depuis_octets(t, 5, 7),
                Plage::depuis_octets(t, 0, 3),
                Plage::depuis_octets(t, 2, 3),
            ],
            hauteurs: vec![
                HauteurLigne {
                    ligne: 0,
                    hauteur: 20.0,
                },
                HauteurLigne {
                    ligne: 0,
                    hauteur: 36.0,
                },
            ],
            polices: vec![PoliceLigne {
                ligne: 0,
                taille: 26.0,
            }],
            blocs: Vec::new(),
        };
        m.normaliser();
        assert_eq!(
            m.masques,
            [Plage::depuis_octets(t, 0, 3), Plage::depuis_octets(t, 5, 7)]
        );
        assert_eq!(
            m.hauteurs,
            [HauteurLigne {
                ligne: 0,
                hauteur: 36.0
            }]
        );
    }

    #[test]
    fn vers_fork_dessous_et_a_la_place() {
        let t = "a\n$$\nx\n$$\nb";
        let lignes = lignes_de(t);
        let source = Plage::depuis_octets(t, 2, 9);
        let bloc = |placement| Bloc {
            genre: GenreBloc::Maths,
            source,
            premiere_ligne: 1,
            derniere_ligne: 3,
            placement,
            hauteur: 48.0,
        };
        let m = ModeleRangees {
            blocs: vec![bloc(Placement::Dessous)],
            ..Default::default()
        };
        let f = m.vers_fork(&lignes);
        assert_eq!(f.blocs, [(3, 48.0)]);
        assert!(f.masques.is_empty());
        let m = ModeleRangees {
            blocs: vec![bloc(Placement::ALaPlace)],
            ..Default::default()
        };
        let f = m.vers_fork(&lignes);
        assert!(f.blocs.is_empty());
        assert_eq!(f.masques, [2..4, 5..6, 7..9]);
        assert_eq!(f.hauteurs, [(1, 48.0), (2, 0.0), (3, 0.0)]);
    }
}
