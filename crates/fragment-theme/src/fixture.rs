//! Palette de FIXTURE (fr-t5.1) : les couleurs des plugins de test e2e de Fragment (`e2e-room`,
//! `e2e-globe`, la vue nue de `doc-widget`), relues dans leurs fixtures et réécrites ici.
//!
//! ★ RÉSERVÉE aux widgets de test et aux scènes pixels. Ces couleurs ne sont PAS des jetons du
//! thème : elles ne changent ni avec le mode clair/sombre ni avec les surcharges, exactement comme
//! les littéraux CSS des fixtures de Fragment. Aucune vue de l'application n'a le droit de s'en
//! servir ; elles vivent ici pour que la règle « pas de littéral de couleur hors `fragment-theme` »
//! tienne.

use crate::couleur::Couleur;

/// Le rouge des widgets de marge de test (`#c8473b`) : le contour d'un pixel du flex, du seuil et
/// du rigide du plugin `e2e-room`.
pub const FIXTURE_ROUGE: Couleur = Couleur::hex(0xc8473b);

/// Le fond de ces mêmes widgets : le rouge à 15 % d'opacité (`rgba(200,71,59,.15)`, soit 38/255
/// après l'arrondi de Chromium).
pub const FIXTURE_ROUGE_FOND: Couleur = Couleur::octets(200, 71, 59, 38);

/// La pastille (vue nue repliée) : le rouge à 40 % (`rgba(200,71,59,.4)`, 102/255).
pub const FIXTURE_ROUGE_PASTILLE: Couleur = Couleur::octets(200, 71, 59, 102);

/// Le fond de la vue nue complète : un bleu à 15 % (`rgba(59,130,246,.15)`, 38/255).
pub const FIXTURE_BLEU_VUE: Couleur = Couleur::octets(59, 130, 246, 38);

/// Le centre du dégradé radial du globe rigide (`#2a6fd6`).
pub const FIXTURE_GLOBE_CENTRE: Couleur = Couleur::hex(0x2a6fd6);

/// Le bord du dégradé radial du globe rigide (`#0b1d3a`).
pub const FIXTURE_GLOBE_BORD: Couleur = Couleur::hex(0x0b1d3a);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_opacites_sont_celles_de_chromium() {
        assert_eq!(FIXTURE_ROUGE_FOND, Couleur::css(200, 71, 59, 0.15));
        assert_eq!(FIXTURE_ROUGE_PASTILLE, Couleur::css(200, 71, 59, 0.4));
        assert_eq!(FIXTURE_BLEU_VUE, Couleur::css(59, 130, 246, 0.15));
    }

    #[test]
    fn les_teintes_opaques() {
        assert_eq!(FIXTURE_ROUGE, Couleur::octets(200, 71, 59, 255));
        assert_eq!(FIXTURE_GLOBE_CENTRE, Couleur::octets(42, 111, 214, 255));
        assert_eq!(FIXTURE_GLOBE_BORD, Couleur::octets(11, 29, 58, 255));
    }
}
