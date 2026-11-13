// Port de app/src/ui/core/workspace/view-icons.css (le motif « bulles ») et de la part de
// app/src/ui/core/workspace/viewIcons.ts qui range les bulles dans leurs emplacements.
//! La GÉOMÉTRIE des bulles d'une grappe repliée : où se pose chaque pastille, à quelle taille, dans
//! quelle boîte. Pur, sans GPUI : [`super::rendu`] le peint, les tests le mesurent.
//!
//! - 2 à 4 membres : un CARRÉ de 41 px, chaque bulle à la taille d'une icône de vue (22 px), sans
//!   fond. Emplacements a (haut-gauche), b (haut-droite), c (bas-droite), d (bas-gauche) ; à deux, b
//!   passe en bas à droite (la diagonale).
//! - 5 et plus : le DISQUE de la maquette. Un rond gris et trois bulles de tailles différentes,
//!   coordonnées relevées sur la maquette (repère du disque) puis ramenées à l'écran par UNE échelle
//!   `K` : la plus grande (Ø 240) vaut 22 px. La grande bas-droite (a) porte le « +N » ; les membres
//!   vont dans b (gauche), c (haut), d (petite, haut-droite).

use crate::doc_widget::icones_vue::Pastille;
use crate::doc_widget::icones_vue::{
    self, BULLES, DECALAGE_BULLE, HAUTEUR_BULLES, Motif, PASTILLE,
};

/// L'échelle de la maquette du disque : la plus grande bulle (Ø 240 unités) = une icône de vue.
pub const K: f32 = PASTILLE / 240.0;
/// Le diamètre du disque, en unités de la maquette (la boîte entière : rien ne dépasse dessous).
pub const DISQUE_MAQUETTE: f32 = 522.0;
/// La marge gauche du bouton d'icônes (`padding: 0 7px 0 11px`).
pub const MARGE_GAUCHE: f32 = 11.0;
/// La marge droite du bouton d'icônes.
pub const MARGE_DROITE: f32 = 7.0;
/// L'opacité du gris du disque : `color-mix(text-normal 10 %, transparent)`.
pub const OPACITE_DISQUE: f32 = 0.10;
/// La taille du texte d'une bulle du disque, relative à son diamètre (`font-size: d·k·0,45`).
pub const TEXTE_BULLE: f32 = 0.45;
/// Celle du « +N » (`d·k·0,48`).
pub const TEXTE_PLUS: f32 = 0.48;
/// Le SVG d'une bulle du disque : 60 % de son diamètre.
pub const SVG_BULLE: f32 = 0.60;

/// Les quatre emplacements des bulles (`data-slot`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Emplacement {
    A,
    B,
    C,
    D,
}

impl Emplacement {
    /// La valeur de `data-slot`.
    pub fn cle(self) -> char {
        match self {
            Self::A => 'a',
            Self::B => 'b',
            Self::C => 'c',
            Self::D => 'd',
        }
    }

    /// Le diamètre et le coin haut-gauche (unités de la maquette) d'une bulle du DISQUE.
    fn maquette(self) -> (f32, f32, f32) {
        match self {
            Self::A => (240.0, 237.5, 228.0),
            Self::B => (216.0, 14.5, 203.5),
            Self::C => (192.0, 149.5, 44.5),
            Self::D => (136.0, 354.0, 105.0),
        }
    }
}

/// La disposition : le carré (2 à 4) ou le disque (5 et plus).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Disposition {
    Carre,
    Disque,
}

impl Disposition {
    /// La disposition de `n` membres (`data-count` = `more` au-delà de 4).
    pub fn de(n: usize) -> Self {
        if n > 4 { Self::Disque } else { Self::Carre }
    }

    /// Le côté de la boîte des bulles : 41 px, ou le disque (522 · K ≈ 47,85 px).
    pub fn cote(self) -> f32 {
        match self {
            Self::Carre => BULLES,
            Self::Disque => DISQUE_MAQUETTE * K,
        }
    }

    /// Le haut de la boîte dans le bandeau de 55 px (centrée verticalement).
    pub fn haut_dans_bandeau(self) -> f32 {
        (HAUTEUR_BULLES - self.cote()) / 2.0
    }
}

/// Une bulle posée : son emplacement, sa boîte (relative à la boîte des bulles) et ce qu'elle montre.
#[derive(Clone, Debug, PartialEq)]
pub struct Bulle {
    pub emplacement: Emplacement,
    pub x: f32,
    pub y: f32,
    /// Le diamètre.
    pub d: f32,
    pub pastille: Pastille,
}

impl Bulle {
    /// La taille du texte de la bulle : la normale d'une icône de vue au carré, relative au disque.
    pub fn taille_texte(&self, normale: f32, disposition: Disposition) -> f32 {
        match (disposition, &self.pastille) {
            (Disposition::Carre, _) => normale,
            (Disposition::Disque, Pastille::Plus(_)) => self.d * TEXTE_PLUS,
            (Disposition::Disque, _) => self.d * TEXTE_BULLE,
        }
    }
}

/// Les bulles d'une grappe dont les membres ont ces icônes (la 1re de chaque membre), dans l'ordre
/// de peinture (celui de `viewIcons.ts` : les membres, puis le « +N »).
pub fn bulles(icones: &[String]) -> Vec<Bulle> {
    let disposition = Disposition::de(icones.len());
    let pastilles = icones_vue::pastilles(icones, Motif::Bulles);
    let mut v = Vec::with_capacity(pastilles.len());
    let mut rang = 0;
    for p in pastilles {
        let emplacement = match (&p, disposition) {
            (Pastille::Plus(_), _) => Emplacement::A,
            (_, Disposition::Disque) => {
                [Emplacement::B, Emplacement::C, Emplacement::D][rang.min(2)]
            }
            (_, Disposition::Carre) => [
                Emplacement::A,
                Emplacement::B,
                Emplacement::C,
                Emplacement::D,
            ][rang.min(3)],
        };
        if !matches!(p, Pastille::Plus(_)) {
            rang += 1;
        }
        let (x, y, d) = match disposition {
            Disposition::Disque => {
                let (d, x, y) = emplacement.maquette();
                (x * K, y * K, d * K)
            }
            Disposition::Carre => {
                let (x, y) = match emplacement {
                    Emplacement::A => (0.0, 0.0),
                    // À deux : la diagonale.
                    Emplacement::B if icones.len() == 2 => (DECALAGE_BULLE, DECALAGE_BULLE),
                    Emplacement::B => (DECALAGE_BULLE, 0.0),
                    Emplacement::C => (DECALAGE_BULLE, DECALAGE_BULLE),
                    Emplacement::D => (0.0, DECALAGE_BULLE),
                };
                (x, y, PASTILLE)
            }
        };
        v.push(Bulle {
            emplacement,
            x,
            y,
            d,
            pastille: p,
        });
    }
    v
}

/// L'infobulle des bulles repliées : « N éléments » (le titre de la vue de grappe).
pub fn infobulle(n: usize) -> String {
    format!("{n} éléments")
}
