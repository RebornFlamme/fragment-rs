//! Les icônes peintes à la main (quads, chemins) que la scène de GPUI ne montre pas comme un sprite
//! (fr-p3.1, sas d'ADR-009).
//!
//! `fragment-pixels` lit les glyphes et les icônes SVG dans la scène peinte ; une icône dessinée par
//! des quads (le « − » des boutons de fenêtre) n'y est qu'un aplat parmi d'autres. Son dessin la
//! DÉCLARE ici pendant sa peinture, en pixels physiques ; la capture vide le registre juste avant
//! de peindre et le relit juste après (même discipline que les ombres de [`crate::flottant`]).

use std::cell::RefCell;

/// Une icône peinte : sa boîte en pixels physiques (x, y, largeur, hauteur).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IconePeinte {
    pub x: f32,
    pub y: f32,
    pub largeur: f32,
    pub hauteur: f32,
}

thread_local! {
    /// Les icônes déclarées depuis le dernier [`vider_icones`].
    static ICONES_PEINTES: RefCell<Vec<IconePeinte>> = const { RefCell::new(Vec::new()) };
}

/// Vide le registre (avant la peinture d'une capture).
pub fn vider_icones() {
    ICONES_PEINTES.with(|i| i.borrow_mut().clear());
}

/// Déclare une icône peinte à la main, en pixels physiques.
pub fn declarer_icone(icone: IconePeinte) {
    ICONES_PEINTES.with(|i| i.borrow_mut().push(icone));
}

/// Les icônes déclarées depuis le dernier [`vider_icones`], dans l'ordre de peinture.
pub fn icones_peintes() -> Vec<IconePeinte> {
    ICONES_PEINTES.with(|i| i.borrow().clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_registre_se_vide_et_se_relit() {
        vider_icones();
        let i = IconePeinte {
            x: 1.0,
            y: 2.0,
            largeur: 16.0,
            hauteur: 16.0,
        };
        declarer_icone(i);
        assert_eq!(icones_peintes(), vec![i]);
        vider_icones();
        assert!(icones_peintes().is_empty());
    }
}
