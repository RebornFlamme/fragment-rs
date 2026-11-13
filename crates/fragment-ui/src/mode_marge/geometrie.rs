// Port de app/src/ui/coreCommands/marginMode.css (la réserve de la zone qui défile) et de la
// colonne de app/src/ui/views/markdown/markdown-view.css (`.doc-sizer`, centrée, 720 px au plus).
//
//! La géométrie PURE de la marge de lecture : ce que la réserve fait à la zone qui défile.
//!
//! ★ La colonne NE S'ÉLARGIT PAS (bug de la v1 de Fragment : `width: 75%` élargissait la note) :
//!   la zone qui défile prend un padding à droite (un pourcentage de SA largeur, la boîte reste
//!   `border-box`) et la colonne garde sa largeur, centrée dans ce qui reste. La marge droite
//!   s'élargit d'autant : c'est elle que mesurent l'arbitre et les widgets.

use crate::editeur::constantes::{LARGEUR_BARRE, LARGEUR_DOC};

/// Où tombent la réserve et la colonne d'une note, pour une zone qui défile de `largeur` px.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MiseEnPage {
    /// Le padding droit de la zone (la réserve, en px).
    pub reserve: f32,
    /// Ce qui reste pour la colonne : largeur − réserve − barre de défilement (toujours réservée).
    pub largeur_utile: f32,
    /// Le bord gauche de la colonne, depuis le bord gauche de la zone.
    pub colonne_gauche: f32,
    /// La largeur de la colonne : celle de la note, jamais plus que la place utile.
    pub colonne_largeur: f32,
    /// Du bord droit de la colonne au bord droit de la zone : la marge droite élargie.
    pub marge_droite: f32,
}

/// La réserve en px d'une zone de `largeur` px pour une `fraction` (0,25 note, 0,40 PDF).
pub fn reserve(largeur: f32, fraction: f32) -> f32 {
    (largeur * fraction.clamp(0.0, 1.0)).max(0.0)
}

/// La largeur UTILE d'un PDF (ce sur quoi s'ajuste le zoom « largeur de page ») : sans la réserve,
/// sinon la page ajustée déborde dans la marge.
pub fn largeur_utile_pdf(largeur: f32, fraction: f32) -> f32 {
    (largeur - reserve(largeur, fraction)).max(0.0)
}

/// La mise en page d'une note : la colonne (au plus [`LARGEUR_DOC`]) centrée dans la place utile.
pub fn mise_en_page_note(largeur: f32, fraction: f32) -> MiseEnPage {
    let r = reserve(largeur, fraction);
    let utile = (largeur - r - LARGEUR_BARRE).max(0.0);
    let colonne = utile.min(LARGEUR_DOC);
    let gauche = (utile - colonne) / 2.0;
    MiseEnPage {
        reserve: r,
        largeur_utile: utile,
        colonne_gauche: gauche,
        colonne_largeur: colonne,
        marge_droite: largeur - gauche - colonne,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_colonne_garde_sa_largeur_et_se_recentre_dans_les_trois_quarts() {
        let sans = mise_en_page_note(1200.0, 0.0);
        let avec = mise_en_page_note(1200.0, 0.25);
        assert_eq!(sans.colonne_largeur, 720.0);
        assert_eq!(avec.colonne_largeur, 720.0);
        assert_eq!(avec.reserve, 300.0);
        // 1 200 − 300 − 6 = 894 utiles : (894 − 720) / 2 = 87 à gauche.
        assert!((avec.colonne_gauche - 87.0).abs() < 1e-4);
        // La marge droite passe de 243 (6 + 237) à 393 (300 + 6 + 87).
        assert!((sans.marge_droite - 243.0).abs() < 1e-4);
        assert!((avec.marge_droite - 393.0).abs() < 1e-4);
    }

    #[test]
    fn trop_etroit_la_colonne_retrecit_aux_trois_quarts() {
        let m = mise_en_page_note(800.0, 0.25);
        assert_eq!(m.colonne_largeur, 800.0 - 200.0 - 6.0);
        assert_eq!(m.colonne_gauche, 0.0);
    }

    #[test]
    fn un_pdf_s_ajuste_a_sa_largeur_utile() {
        assert!((largeur_utile_pdf(1000.0, 0.40) - 600.0).abs() < 1e-3);
        assert_eq!(largeur_utile_pdf(1000.0, 0.0), 1000.0);
    }
}
