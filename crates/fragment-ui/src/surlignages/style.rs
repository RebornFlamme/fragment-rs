// Port de app/src/ui/core/editor/range-highlight.css (les règles, traduites en jetons de thème)
//
//! Le style d'un surlignage : ce que disent les classes de sa plage, résolu en jetons.
//!
//! Trois familles, comme dans la feuille de Fragment :
//! - le surlignage nu (`range-highlight`) : l'accent à 18 %, coins `radius_xs` ;
//! - le PASSAGE (`is-passage`, et `is-hover` : un passage survolé) : l'accent à 14 %, coins
//!   `radius_xxs`, souligné d'un trait intérieur de 2 px en `color_accent_2` ;
//! - l'ACTIF (`is-active`) : le même, plus marqué (28 % pour le nu, 24 % pour un passage).
//!
//! Une classe inconnue (celle d'un plugin) ne change rien ici : son style est celui du plugin.

use fragment_theme::{Couleur, Theme};

/// L'épaisseur du trait sous un passage (px CSS, trait intérieur : sans incidence sur la géométrie).
pub const TRAIT_PASSAGE: f32 = 2.0;

/// Le style résolu d'une boîte.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StyleSurlignage {
    pub fond: Couleur,
    /// Rayon des coins (px CSS).
    pub rayon: f32,
    /// Le trait du bas d'un passage : sa couleur.
    pub trait_bas: Option<Couleur>,
}

fn a_la_classe(cls: &str, nom: &str) -> bool {
    cls.split_whitespace().any(|c| c == nom)
}

/// Le style des classes `cls` (la chaîne complète d'une `HighlightBox`, `range-highlight` compris).
pub fn style_de(cls: &str, theme: &Theme) -> StyleSurlignage {
    let p = &theme.palette;
    let passage = a_la_classe(cls, "is-passage") || a_la_classe(cls, "is-hover");
    let actif = a_la_classe(cls, "is-active");
    let part = match (passage, actif) {
        (false, false) => 18.0,
        (false, true) => 28.0,
        (true, false) => 14.0,
        (true, true) => 24.0,
    };
    StyleSurlignage {
        fond: p.color_accent.melange(part, Couleur::TRANSPARENT),
        rayon: if passage {
            theme.forme.radius_xxs
        } else {
            theme.forme.radius_xs
        },
        trait_bas: passage.then_some(p.color_accent_2),
    }
}
