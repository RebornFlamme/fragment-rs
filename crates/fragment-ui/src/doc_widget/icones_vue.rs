// Port de app/src/ui/core/workspace/viewIcons.ts et de view-icons.css (la part qui sert le cadre).
//! Les icônes d'une vue réduite à elles (`View::get_icons`) : le MODÈLE (quelles pastilles, quelle
//! teinte, quelle largeur) — pur, testable sans fenêtre — que [`super::rendu`] peint.
//!
//! Chaque icône est une chaîne : un nom d'icône (Lucide, ou inscrite) donne son SVG ; sinon un
//! court texte (emoji, initiales) sur une pastille ronde, teintée de façon stable par le texte.
//! Plusieurs icônes se chevauchent ; au-delà du maximum, une pastille « +N ».

use crate::icones;

/// Les teintes des pastilles (angles OKLCH) : huit couleurs à luminosité perçue égale — corail,
/// ambre, olive, vert, sarcelle, ciel, indigo, magenta.
pub const TEINTES: [f32; 8] = [25.0, 60.0, 100.0, 150.0, 190.0, 235.0, 275.0, 325.0];

/// Le côté d'une pastille (view-icons.css : `.view-icon`).
pub const PASTILLE: f32 = 22.0;
/// Le chevauchement de deux pastilles voisines d'une pile.
pub const CHEVAUCHEMENT: f32 = 7.0;
/// Le carré du motif « bulles » : quatre pastilles de taille normale.
pub const BULLES: f32 = 41.0;
/// Le décalage d'une bulle du carré (`data-slot` b, c, d : 19 px).
pub const DECALAGE_BULLE: f32 = 19.0;
/// Le disque de 5 membres et plus : 522 unités de la maquette, la plus grande bulle (Ø 240) valant 22 px.
/// ★ 522 · 22/240 = 47,85 ; Chromium garde la mise en page en `LayoutUnit` (1/64 px, arrondi vers le
/// bas) : 3062/64 = 47,84375. La largeur du bouton (18 + disque) reste fractionnaire, seule la peinture
/// du disque est calée au pixel. 48 décalait de 0,16 px tout ce qui suit le bouton.
pub const DISQUE: f32 = 47.84375;

/// `calc(unités * var(--k))` tel que Chromium le tient en mise en page : `--k` = 22px/240, produit
/// ramené à la grille de 1/64 px (LayoutUnit), arrondi vers le bas. Calculé en f64 pour que 1/64 soit
/// exact et que les produits tombant juste (240 · k = 22) ne perdent pas un cran.
pub fn layout_unit_k(unites: f32) -> f32 {
    let px = f64::from(unites) * f64::from(PASTILLE) / 240.0;
    ((px * 64.0 + 1e-9).floor() / 64.0) as f32
}
/// La ligne normale du bandeau d'icônes (`--doc-widget-icons-height`).
pub const HAUTEUR_LIGNE: f32 = 38.0;
/// Le bandeau d'une grappe : le carré des bulles plus 7 px de marge en haut et en bas.
pub const HAUTEUR_BULLES: f32 = BULLES + 14.0;

/// Le motif des icônes : la pile (défaut) ou les bulles d'une grappe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motif {
    Pile,
    Bulles,
}

/// Une pastille à peindre.
#[derive(Clone, Debug, PartialEq)]
pub enum Pastille {
    /// Un nom d'icône connu : son SVG, sobre, sur l'encre du texte.
    Icone(String),
    /// Un court texte (emoji, initiales) sur une pastille teintée (angle OKLCH, en degrés).
    Texte { texte: String, teinte: f32 },
    /// « +N » : les icônes qui ne tiennent pas.
    Plus(usize),
}

impl Pastille {
    /// Le texte visible (le contenu de `.view-icon`) : celui d'une icône SVG est vide.
    pub fn texte(&self) -> String {
        match self {
            Self::Icone(_) => String::new(),
            Self::Texte { texte, .. } => texte.clone(),
            Self::Plus(n) => format!("+{n}"),
        }
    }
}

/// Une teinte stable tirée d'un texte (même texte, même teinte, partout).
pub fn teinte(texte: &str) -> f32 {
    let mut h: u32 = 0;
    for c in texte.chars() {
        // `charCodeAt(0)` d'un point de code : sa première unité UTF-16.
        let mut tampon = [0u16; 2];
        let unite = u32::from(c.encode_utf16(&mut tampon)[0]);
        h = (h * 31 + unite) % 9973;
    }
    TEINTES[(h % 8) as usize]
}

/// Combien de pastilles d'icônes (hors « +N ») le motif en montre.
fn maximum(n: usize, motif: Motif) -> usize {
    match motif {
        // Bulles : jusqu'à 4, toutes ; au-delà (le disque), 3 plus « +N » dans la grande.
        Motif::Bulles => {
            if n <= 4 {
                4
            } else {
                3
            }
        }
        Motif::Pile => 3,
    }
}

/// Les pastilles d'une liste d'icônes, dans l'ordre de peinture.
pub fn pastilles(icones: &[String], motif: Motif) -> Vec<Pastille> {
    let max = maximum(icones.len(), motif);
    let mut v: Vec<Pastille> = icones
        .iter()
        .take(max)
        .map(|i| {
            if icones::svg(i).is_some() {
                Pastille::Icone(i.clone())
            } else {
                Pastille::Texte {
                    texte: i.clone(),
                    teinte: teinte(i),
                }
            }
        })
        .collect();
    if icones.len() > max {
        v.push(Pastille::Plus(icones.len() - max));
    }
    v
}

/// La largeur du bouton d'icônes d'une pile de `n` icônes : 11 px de marge à gauche, 7 à droite,
/// des pastilles de 22 px qui se chevauchent de 7, au plus 3 plus « +N ».
pub fn largeur_pile(n: usize) -> f32 {
    let k = n.clamp(1, 3) + usize::from(n > 3);
    18.0 + PASTILLE + (k.max(1) - 1) as f32 * (PASTILLE - CHEVAUCHEMENT)
}

/// La place que prend la boîte d'icônes (largeur du bouton) : pile, ou carré / disque d'une grappe.
pub fn largeur_boite(n: usize, motif: Motif) -> f32 {
    match motif {
        Motif::Pile => largeur_pile(n),
        Motif::Bulles => 18.0 + if n > 4 { DISQUE } else { BULLES },
    }
}

/// La hauteur du bandeau d'icônes : la ligne normale, ou, pour une grappe, un bandeau plus haut.
pub fn hauteur_bandeau(motif: Motif) -> f32 {
    match motif {
        Motif::Pile => HAUTEUR_LIGNE,
        Motif::Bulles => HAUTEUR_BULLES,
    }
}

/// Une couleur OKLCH en sRGB 8 bits (le calcul de Chromium : OKLab, matrice LMS, gamma sRGB).
pub fn oklch_vers_rgb(l: f32, c: f32, h_deg: f32) -> [u8; 3] {
    let h = h_deg.to_radians();
    let (a, b) = (c * h.cos(), c * h.sin());
    let l_ = l + 0.396_337_78 * a + 0.215_803_76 * b;
    let m_ = l - 0.105_561_346 * a - 0.063_854_17 * b;
    let s_ = l - 0.089_484_18 * a - 1.291_485_5 * b;
    let (l3, m3, s3) = (l_ * l_ * l_, m_ * m_ * m_, s_ * s_ * s_);
    let r = 4.076_741_7 * l3 - 3.307_711_6 * m3 + 0.230_969_94 * s3;
    let g = -1.268_438 * l3 + 2.609_757_4 * m3 - 0.341_319_4 * s3;
    let bl = -0.004_196_086_3 * l3 - 0.703_418_6 * m3 + 1.707_614_7 * s3;
    let gamma = |x: f32| {
        let x = x.clamp(0.0, 1.0);
        let v = if x <= 0.003_130_8 {
            12.92 * x
        } else {
            1.055 * x.powf(1.0 / 2.4) - 0.055
        };
        (v * 255.0).round() as u8
    };
    [gamma(r), gamma(g), gamma(bl)]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 522 · 22/240 passé à la grille de 1/64 px de Chromium (LayoutUnit, vers le bas).
    #[test]
    fn disque_est_en_layout_unit() {
        let attendu = (522.0_f32 * 22.0 / 240.0 * 64.0).floor() / 64.0;
        assert_eq!(DISQUE, attendu);
        assert_eq!(layout_unit_k(522.0), DISQUE);
        assert_eq!(largeur_boite(5, Motif::Bulles), 18.0 + 47.84375);
    }
}
