// Port de app/src/ui/views/pdf/pdfZoom.ts
//
// Le zoom de la vue PDF, en logique pure.
//
// Deux régimes, comme dans pdf.js et Obsidian :
//   - un MODE (« largeur de page », « page entière »…) : l'échelle se déduit de
//     la place disponible et se recalcule quand le panneau change de taille ;
//   - une échelle LIBRE (zoom +/−, pourcentage choisi) : elle ne bouge plus.
//
// Échelle 1 = 1 px CSS par point PDF — « Actual size ».

/// Les modes de zoom (`ZoomMode` côté TS : `'auto' | 'page-actual' | …`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoomMode {
    Auto,
    PageActual,
    PageFit,
    PageWidth,
}

pub const ECHELLE_MIN: f64 = 0.25;
pub const ECHELLE_MAX: f64 = 4.0;

/// Plafond du mode automatique : au-delà, un texte de page étroite devient énorme.
const ECHELLE_AUTO_MAX: f64 = 1.25;

/// Les crans de zoom +/−. Des paliers plutôt qu'un pas fixe : un pas de 0.25
/// est trop grossier vers 50 % et trop fin vers 400 %.
pub const PALIERS: [f64; 16] = [
    0.25, 0.33, 0.5, 0.67, 0.75, 0.8, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0, 4.0,
];

/// Les pourcentages proposés dans le menu ▾ du zoom.
pub const POURCENTAGES: [f64; 8] = [0.5, 0.75, 1.0, 1.25, 1.5, 2.0, 3.0, 4.0];

/// Le blanc autour d'une page dans `.pdf-pages` (CSS) : 1rem de chaque côté en
/// largeur, le padding vertical + un peu d'air en hauteur. Les modes « fit » le
/// retirent, sinon la page touche le bord — ou fait apparaître une barre.
pub const MARGE_H: f64 = 32.0;
pub const MARGE_V: f64 = 32.0;

/// Une taille (page en unités PDF, ou zone visible en px).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Taille {
    pub largeur: f64,
    pub hauteur: f64,
}

pub fn borner_echelle(e: f64) -> f64 {
    e.clamp(ECHELLE_MIN, ECHELLE_MAX)
}

/// Le sens d'un cran de zoom.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sens {
    Plus,
    Moins,
}

/// Le cran suivant (`Plus`) ou précédent (`Moins`), borné.
pub fn palier_suivant(echelle: f64, sens: Sens) -> f64 {
    // ★ Tolérance : 1.1 affiché « 110 % » ne doit pas se re-sélectionner lui-même.
    let eps = 1e-3;
    match sens {
        Sens::Plus => PALIERS
            .iter()
            .copied()
            .find(|p| *p > echelle + eps)
            .unwrap_or(ECHELLE_MAX),
        Sens::Moins => PALIERS
            .iter()
            .rev()
            .copied()
            .find(|p| *p < echelle - eps)
            .unwrap_or(ECHELLE_MIN),
    }
}

/// L'échelle d'un mode, pour une page (unités PDF, échelle 1) et une zone
/// visible (px). Mêmes règles que pdf.js : « auto » = largeur de page plafonnée
/// pour un document en portrait, page entière pour un paysage.
pub fn echelle_pour_mode(mode: ZoomMode, page: Taille, vue: Taille) -> f64 {
    if page.largeur <= 0.0 || page.hauteur <= 0.0 {
        return 1.0;
    }
    let largeur = (vue.largeur - MARGE_H).max(1.0) / page.largeur;
    let hauteur = (vue.hauteur - MARGE_V).max(1.0) / page.hauteur;

    match mode {
        ZoomMode::PageActual => 1.0,
        ZoomMode::PageWidth => borner_echelle(largeur),
        ZoomMode::PageFit => borner_echelle(largeur.min(hauteur)),
        ZoomMode::Auto => {
            let paysage = page.largeur > page.hauteur;
            borner_echelle(if paysage {
                largeur.min(hauteur)
            } else {
                ECHELLE_AUTO_MAX.min(largeur)
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palier_suivant_monte_et_descend_de_cran_en_cran() {
        assert_eq!(palier_suivant(1.0, Sens::Plus), 1.1);
        assert_eq!(palier_suivant(1.0, Sens::Moins), 0.9);
    }

    #[test]
    fn palier_suivant_se_recale_sur_un_cran_depuis_une_echelle_libre() {
        assert_eq!(palier_suivant(1.18, Sens::Plus), 1.25);
        assert_eq!(palier_suivant(1.18, Sens::Moins), 1.1);
    }

    #[test]
    fn palier_suivant_reste_dans_les_bornes() {
        assert_eq!(palier_suivant(ECHELLE_MAX, Sens::Plus), ECHELLE_MAX);
        assert_eq!(palier_suivant(ECHELLE_MIN, Sens::Moins), ECHELLE_MIN);
    }

    fn portrait() -> Taille {
        Taille {
            largeur: 200.0,
            hauteur: 300.0,
        }
    }

    fn vue() -> Taille {
        Taille {
            largeur: 800.0 + MARGE_H,
            hauteur: 600.0 + MARGE_V,
        }
    }

    #[test]
    fn echelle_pour_mode_page_width_remplit_la_largeur() {
        assert_eq!(
            echelle_pour_mode(ZoomMode::PageWidth, portrait(), vue()),
            4.0
        );
        let p = Taille {
            largeur: 400.0,
            hauteur: 600.0,
        };
        assert_eq!(echelle_pour_mode(ZoomMode::PageWidth, p, vue()), 2.0);
    }

    #[test]
    fn echelle_pour_mode_page_fit_tient_la_page_entiere() {
        assert_eq!(echelle_pour_mode(ZoomMode::PageFit, portrait(), vue()), 2.0);
    }

    #[test]
    fn echelle_pour_mode_auto_plafonne_la_largeur_en_portrait_tient_la_page_en_paysage() {
        assert_eq!(echelle_pour_mode(ZoomMode::Auto, portrait(), vue()), 1.25);
        let paysage = Taille {
            largeur: 1600.0,
            hauteur: 600.0,
        };
        assert_eq!(echelle_pour_mode(ZoomMode::Auto, paysage, vue()), 0.5);
    }

    #[test]
    fn echelle_pour_mode_page_actual_vaut_1_et_tout_est_borne() {
        assert_eq!(
            echelle_pour_mode(ZoomMode::PageActual, portrait(), vue()),
            1.0
        );
        let enorme = Taille {
            largeur: 10000.0,
            hauteur: 10.0,
        };
        assert_eq!(
            echelle_pour_mode(ZoomMode::PageWidth, enorme, vue()),
            ECHELLE_MIN
        );
    }
}
