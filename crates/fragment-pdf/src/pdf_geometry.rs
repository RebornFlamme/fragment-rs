// Port de app/src/ui/views/pdf/pdfGeometry.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  LA GÉOMÉTRIE DES PAGES, EN MÉMOIRE — le propriétaire est la vue PDF.
//
//  ★ POURQUOI : chaque passe des calques demandait au DOM le coin de chaque page
//    (`getBoundingClientRect`), juste après des écritures — une mise en page
//    FORCÉE par lecture. Or cette géométrie, c'est NOUS qui la fixons : chaque
//    page a la taille qu'on lui donne, elles sont empilées en colonne. Comme la
//    height map de CodeMirror 6 : la position se CALCULE, l'affichage suit.
//
//  Le repère : l'origine du PLAN (au coin haut-gauche du `sizer`). En px ÉCRAN :
//  la colonne a un `padding-top` et un `gap` FIXES en px (ils ne suivent pas le
//  zoom), chaque page mesure sa taille non zoomée × l'échelle, ramenée à un
//  nombre entier de CRANS de Chromium (1/64 de pixel PHYSIQUE, cf. `crans`).
//  En unités du PLAN (non zoomées) : px ÷ échelle.
//
//  Horizontalement : le `sizer` prend max(largeur utile, page la plus large),
//  chaque page y est centrée — donc left = (sizer − page) / 2.
//
//  Module PUR (aucun DOM). Le modèle des crans est conservé tel quel : il fixe
//  les sommes de position, même si le moteur de rendu de fragment-rs n'est plus
//  Chromium (la grille 1/64 reste une convention de mise en page à parité).
// ═══════════════════════════════════════════════════════════════════════════

use std::cell::RefCell;

use crate::pdf_zoom::Taille;

/// Arrondi de JavaScript (`Math.round` : la moitié monte), différent de `f64::round`
/// pour les négatifs. Les crans sont positifs, mais on garde la sémantique du TS.
fn arrondi_js(x: f64) -> f64 {
    (x + 0.5).floor()
}

/// Le nombre de CRANS de mise en page d'une longueur en px CSS.
///
/// ★ Chromium met en page en pixels PHYSIQUES (zoom de l'app et densité de
///   l'écran compris), au 1/64 près (LayoutUnit), en TRONQUANT — mesuré :
///   1153,39 px CSS à 1,5 donnent 1153,3854 px. Le modèle ne devine pas cet
///   arrondi, il le DICTE : il choisit un nombre entier de crans (`crans`), et la
///   vue écrit en CSS une longueur posée un DEMI-cran au-dessus (`vers_css`) — la
///   troncature retombe exactement sur ce nombre. Essayé d'abord : deviner
///   l'arrondi sur `h × e` — 3 px de dérive sur 300 pages. Une seule source de
///   vérité : le modèle.
pub fn crans(px: f64, densite: f64) -> i64 {
    arrondi_js(px * 64.0 * densite) as i64
}

/// La longueur (px CSS) d'un nombre de crans — celle que mesure Chromium.
pub fn depuis_crans(n: i64, densite: f64) -> f64 {
    n as f64 / (64.0 * densite)
}

/// La longueur à ÉCRIRE en CSS pour obtenir exactement `n` crans (un demi-cran au-dessus).
pub fn vers_css(n: i64, densite: f64) -> f64 {
    (n as f64 + 0.5) / (64.0 * densite)
}

/// Le rectangle d'une page en unités du PLAN (non zoomées), relatif à l'origine du plan.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CoinPage {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

/// Cache des sommes cumulées (`tops`, `largeurMaxPx` côté TS).
#[derive(Debug, Clone)]
struct Cache {
    /// Le haut de chaque page en px écran.
    tops: Vec<f64>,
    largeur_max_px: f64,
}

#[derive(Debug)]
pub struct PdfGeometrie {
    /// Tailles NON zoomées (unités PDF à l'échelle 1), une par page (index 0).
    largeurs: Vec<f64>,
    hauteurs: Vec<f64>,
    echelle: f64,
    /// `devicePixelRatio` : la grille de mise en page (pixels physiques / 64).
    densite: f64,
    /// Espacements de la colonne, en px ÉCRAN.
    gap: f64,
    pad_haut: f64,
    /// La largeur utile de la zone qui défile (sans son padding), en px écran.
    largeur_utile: f64,
    /// Recalculé paresseusement ; `RefCell` pour que les lectures restent `&self`.
    cache: RefCell<Option<Cache>>,
}

impl Default for PdfGeometrie {
    fn default() -> Self {
        Self {
            largeurs: Vec::new(),
            hauteurs: Vec::new(),
            echelle: 1.0,
            densite: 1.0,
            gap: 0.0,
            pad_haut: 0.0,
            largeur_utile: 0.0,
            cache: RefCell::new(None),
        }
    }
}

impl PdfGeometrie {
    pub fn new() -> Self {
        Self::default()
    }

    fn invalider(&self) {
        *self.cache.borrow_mut() = None;
    }

    pub fn nombre(&self) -> usize {
        self.hauteurs.len()
    }

    /// Un document neuf : `n` pages, toutes à la taille de la page 1 (corrigées au premier rendu).
    pub fn reinitialiser(&mut self, n: usize, largeur: f64, hauteur: f64) {
        self.largeurs = vec![largeur; n];
        self.hauteurs = vec![hauteur; n];
        self.invalider();
    }

    pub fn set_espacements(&mut self, gap_px: f64, pad_haut_px: f64) {
        if gap_px == self.gap && pad_haut_px == self.pad_haut {
            return;
        }
        self.gap = gap_px;
        self.pad_haut = pad_haut_px;
        self.invalider();
    }

    pub fn set_echelle(&mut self, e: f64) {
        if e == self.echelle {
            return;
        }
        self.echelle = e;
        self.invalider();
    }

    pub fn get_echelle(&self) -> f64 {
        self.echelle
    }

    /// Le zoom de l'app ou l'écran ont changé : les crans changent avec.
    pub fn set_densite(&mut self, d: f64) {
        // ★ `!(d > 0)` du TS : rejette aussi NaN.
        if d.is_nan() || d <= 0.0 || d == self.densite {
            return;
        }
        self.densite = d;
        self.invalider();
    }

    /// Rend vrai si la largeur a changé (les coins horizontaux bougent).
    pub fn set_largeur_utile(&mut self, px: f64) -> bool {
        if (px - self.largeur_utile).abs() < 0.01 {
            return false;
        }
        self.largeur_utile = px;
        true
    }

    /// La taille réelle d'une page (son viewport à l'échelle 1). Rend vrai si elle a changé.
    pub fn set_taille(&mut self, i: usize, largeur: f64, hauteur: f64) -> bool {
        if i >= self.hauteurs.len() {
            return false;
        }
        if self.largeurs[i] == largeur && self.hauteurs[i] == hauteur {
            return false;
        }
        self.largeurs[i] = largeur;
        self.hauteurs[i] = hauteur;
        self.invalider();
        true
    }

    /// Taille NON zoomée d'une page (0 × 0 hors bornes, comme le `?? 0` du TS).
    pub fn taille(&self, i: usize) -> Taille {
        Taille {
            largeur: self.largeurs.get(i).copied().unwrap_or(0.0),
            hauteur: self.hauteurs.get(i).copied().unwrap_or(0.0),
        }
    }

    /// Hauteur d'une page en px écran : un nombre entier de crans.
    pub fn hauteur_px(&self, i: usize) -> f64 {
        depuis_crans(self.crans_hauteur(i), self.densite)
    }

    pub fn largeur_px(&self, i: usize) -> f64 {
        depuis_crans(self.crans_largeur(i), self.densite)
    }

    /// Ce que la vue ÉCRIT en CSS pour que la page tombe à `hauteur_px` / `largeur_px`.
    pub fn hauteur_css(&self, i: usize) -> f64 {
        vers_css(self.crans_hauteur(i), self.densite)
    }

    pub fn largeur_css(&self, i: usize) -> f64 {
        vers_css(self.crans_largeur(i), self.densite)
    }

    fn crans_hauteur(&self, i: usize) -> i64 {
        crans(
            self.hauteurs.get(i).copied().unwrap_or(0.0) * self.echelle,
            self.densite,
        )
    }

    fn crans_largeur(&self, i: usize) -> i64 {
        crans(
            self.largeurs.get(i).copied().unwrap_or(0.0) * self.echelle,
            self.densite,
        )
    }

    /// Le haut d'une page en px écran depuis l'origine du plan (indice borné à la dernière page).
    pub fn top_px(&self, i: usize) -> f64 {
        self.calculer();
        let cache = self.cache.borrow();
        cache
            .as_ref()
            .and_then(|c| c.tops.get(i.min(c.tops.len().saturating_sub(1))).copied())
            .unwrap_or(self.pad_haut)
    }

    /// Le bas de la dernière page + le padding du bas (= celui du haut) : la hauteur de la colonne.
    pub fn hauteur_totale_px(&self) -> f64 {
        let n = self.nombre();
        if n == 0 {
            return 0.0;
        }
        self.top_px(n - 1) + self.hauteur_px(n - 1) + self.pad_haut
    }

    /// Le bord gauche d'une page en px écran depuis l'origine du plan.
    pub fn left_px(&self, i: usize) -> f64 {
        self.calculer();
        let largeur_max = self
            .cache
            .borrow()
            .as_ref()
            .map_or(0.0, |c| c.largeur_max_px);
        let sizer = self.largeur_utile.max(largeur_max);
        ((sizer - self.largeur_px(i)) / 2.0).max(0.0)
    }

    /// Le rectangle d'une page en unités du PLAN (non zoomées).
    pub fn coin(&self, i: usize) -> CoinPage {
        let e = self.echelle;
        let left = self.left_px(i) / e;
        let top = self.top_px(i) / e;
        CoinPage {
            left,
            top,
            right: left + self.largeurs.get(i).copied().unwrap_or(0.0),
            bottom: top + self.hauteurs.get(i).copied().unwrap_or(0.0),
        }
    }

    /// Index (0-based) de la première page dont le bas dépasse `y` (px écran
    /// depuis l'origine du plan) — dichotomie sur les sommes cumulées. Au-delà de
    /// la dernière : la dernière.
    pub fn page_sous_px(&self, y: f64) -> usize {
        self.calculer();
        let n = self.nombre();
        if n == 0 {
            return 0;
        }
        let mut bas = 0;
        let mut haut = n - 1;
        while bas < haut {
            let milieu = (bas + haut) >> 1;
            if self.top_brut(milieu) + self.hauteur_px(milieu) > y {
                haut = milieu;
            } else {
                bas = milieu + 1;
            }
        }
        bas
    }

    fn top_brut(&self, i: usize) -> f64 {
        self.cache
            .borrow()
            .as_ref()
            .and_then(|c| c.tops.get(i).copied())
            .unwrap_or(self.pad_haut)
    }

    /// Remplit le cache s'il est absent (ou de mauvaise taille).
    fn calculer(&self) {
        if self
            .cache
            .borrow()
            .as_ref()
            .is_some_and(|c| c.tops.len() == self.hauteurs.len())
        {
            return;
        }
        let n = self.hauteurs.len();
        let mut tops = Vec::with_capacity(n);
        let mut y = self.pad_haut;
        let mut largeur_max: f64 = 0.0;
        for i in 0..n {
            tops.push(y);
            y += self.hauteur_px(i) + self.gap;
            largeur_max = largeur_max.max(self.largeur_px(i));
        }
        *self.cache.borrow_mut() = Some(Cache {
            tops,
            largeur_max_px: largeur_max,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geo(n: usize, l: f64, h: f64, e: f64) -> PdfGeometrie {
        let mut g = PdfGeometrie::new();
        g.reinitialiser(n, l, h);
        g.set_espacements(16.0, 16.0);
        g.set_echelle(e);
        g.set_largeur_utile(800.0);
        g
    }

    fn geo_defaut(n: usize) -> PdfGeometrie {
        geo(n, 400.0, 600.0, 1.0)
    }

    fn proche(a: f64, b: f64, chiffres: i32) -> bool {
        (a - b).abs() < 10f64.powi(-chiffres) / 2.0
    }

    #[test]
    fn empile_les_pages_padding_puis_hauteur_plus_gap_px_ecran_fixes() {
        let g = geo_defaut(3);
        assert_eq!(g.top_px(0), 16.0);
        assert_eq!(g.top_px(1), 16.0 + 600.0 + 16.0);
        assert_eq!(g.top_px(2), 16.0 + 2.0 * (600.0 + 16.0));
        assert_eq!(
            g.hauteur_totale_px(),
            16.0 + 3.0 * 600.0 + 2.0 * 16.0 + 16.0
        );
    }

    #[test]
    fn au_zoom_les_pages_grandissent_le_gap_non_unites_du_plan_px_div_echelle() {
        let g = geo(3, 400.0, 600.0, 2.0);
        assert_eq!(g.top_px(1), 16.0 + 1200.0 + 16.0);
        let c = g.coin(1);
        assert!(proche(c.top, (16.0 + 1200.0 + 16.0) / 2.0, 6));
        assert_eq!(c.bottom - c.top, 600.0); // non zoomé
    }

    #[test]
    fn centre_chaque_page_dans_max_largeur_utile_page_la_plus_large() {
        let mut g = geo_defaut(2);
        g.set_taille(1, 600.0, 600.0);
        assert_eq!(g.left_px(0), (800.0 - 400.0) / 2.0);
        assert_eq!(g.left_px(1), (800.0 - 600.0) / 2.0);
        g.set_largeur_utile(300.0); // plus étroit que les pages : le sizer prend la plus large
        assert_eq!(g.left_px(0), (600.0 - 400.0) / 2.0);
        assert_eq!(g.left_px(1), 0.0);
    }

    #[test]
    fn une_page_corrigee_decale_toutes_les_suivantes_pas_les_precedentes() {
        let mut g = geo_defaut(4);
        let avant: Vec<f64> = (0..4).map(|i| g.top_px(i)).collect();
        assert!(g.set_taille(1, 400.0, 800.0));
        assert!(!g.set_taille(1, 400.0, 800.0));
        assert_eq!(g.top_px(0), avant[0]);
        assert_eq!(g.top_px(1), avant[1]);
        assert_eq!(g.top_px(2), avant[2] + 200.0);
        assert_eq!(g.top_px(3), avant[3] + 200.0);
    }

    #[test]
    fn les_crans_de_chromium_la_longueur_ecrite_retombe_pile_apres_troncature() {
        for d in [1.0, 1.25, 1.5, 2.0] {
            for px in [1153.3893, 841.89, 600.0, 0.3] {
                let n = crans(px, d);
                assert_eq!((vers_css(n, d) * 64.0 * d).floor() as i64, n);
                assert!(proche(depuis_crans(n, d), px, 1));
            }
        }
    }

    #[test]
    fn pas_de_derive_sur_500_pages_les_sommes_cumulees_sont_des_crans_entiers() {
        let mut g = geo(500, 595.276, 841.89, 1.37);
        g.set_densite(1.5);
        let pas = depuis_crans(crans(841.89 * 1.37, 1.5), 1.5) + 16.0;
        assert!(proche(g.top_px(499), 16.0 + 499.0 * pas, 6));
    }

    #[test]
    fn page_sous_px_la_premiere_page_dont_le_bas_depasse_y_bornes_comprises() {
        let g = geo_defaut(3);
        assert_eq!(g.page_sous_px(0.0), 0);
        assert_eq!(g.page_sous_px(16.0 + 600.0 - 0.01), 0);
        assert_eq!(g.page_sous_px(16.0 + 600.0), 1); // le bas exact appartient à la suivante
        assert_eq!(g.page_sous_px(16.0 + 600.0 + 8.0), 1); // dans le gap : la page d'après
        assert_eq!(g.page_sous_px(1e9), 2);
        assert_eq!(PdfGeometrie::new().page_sous_px(10.0), 0);
    }
}
