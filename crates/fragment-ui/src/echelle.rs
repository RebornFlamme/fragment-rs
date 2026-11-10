//! Le zoom de l'application et `u(n)` : l'unique porte d'entrée des dimensions de rendu.
//!
//! GPUI n'a pas de zoom global (Chromium en a un : `webFrame.setZoomFactor`). Toute longueur de
//! rendu passe donc par [`u`] (`n × zoom`) ; un `px(` brut hors de ce fichier est un défaut de revue.
//! Le facteur vit dans un `Global` GPUI, que le réglage de zoom remplace.

use fragment_core::normaliser_zoom;
use gpui_kit::{App, Global, Pixels, px};

/// Le facteur de zoom courant (1 = 100 %).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Echelle {
    zoom: f32,
}

impl Global for Echelle {}

impl Default for Echelle {
    fn default() -> Self {
        Self::NEUTRE
    }
}

impl Echelle {
    /// Zoom à 100 % : `u(n)` vaut `n`.
    pub const NEUTRE: Self = Self { zoom: 1.0 };

    /// Une échelle pour `zoom`, ramené par la normalisation du cœur (bornes 50 %–200 %).
    pub fn new(zoom: f64) -> Self {
        Self {
            zoom: normaliser_zoom(zoom) as f32,
        }
    }

    /// Le facteur de zoom (1 = 100 %).
    pub fn zoom(self) -> f32 {
        self.zoom
    }

    /// Une longueur logique `n` mise à l'échelle : `n × zoom`.
    pub fn u(self, n: f32) -> Pixels {
        px(n * self.zoom)
    }

    /// Installe (ou remplace) l'échelle de l'application et redessine les fenêtres.
    pub fn installer(cx: &mut App, zoom: f64) {
        cx.set_global(Self::new(zoom));
        cx.refresh_windows();
    }

    /// L'échelle courante ; à 100 % tant qu'aucune n'est installée.
    pub fn courante(cx: &App) -> Self {
        cx.try_global::<Self>().copied().unwrap_or(Self::NEUTRE)
    }
}

/// `n × zoom` : LA façon d'écrire une dimension de rendu.
pub fn u(cx: &App, n: f32) -> Pixels {
    Echelle::courante(cx).u(n)
}

/// ★ Positions fractionnaires : des hauteurs de boîtes empilées, calées comme le fait Chromium.
///
/// Chromium cumule les longueurs en fractions exactes (1/64 px) et n'arrondit qu'à la peinture, arête
/// par arête, à l'entier le plus proche (les égalités vers le haut). GPUI arrondit chaque longueur
/// écrite (`h`, `p`, `gap`…) AVANT la mise en page, la moitié vers zéro : 29,5 donne 29 sur toute
/// la pile, soit une dérive d'un demi-pixel par ligne. Ces longueurs-là ne se corrigent donc pas ; on
/// donne à GPUI des entiers déjà calés.
///
/// `origine` est la position exacte (logique, avant zoom) de l'arête haute de la première boîte ;
/// `hauteurs` les hauteurs exactes (logiques) des boîtes, bout à bout. Rend les hauteurs entières
/// (mises à l'échelle du zoom) à écrire : leur cumul retombe sur l'arrondi de chaque arête exacte,
/// comme Chromium, et leur somme est `arrondi(fin) - arrondi(début)`. Pour 29,5 depuis 5 : 30, 29,
/// 30, 29… Hypothèse : un pixel logique = un pixel écran (dpr 1), celui des scènes de pixels.
pub fn calees(cx: &App, origine: f32, hauteurs: &[f32]) -> Vec<Pixels> {
    let zoom = Echelle::courante(cx).zoom();
    calees_pures(zoom, origine, hauteurs)
        .into_iter()
        .map(px)
        .collect()
}

/// [`calees`] sans GPUI : les hauteurs entières, en pixels.
fn calees_pures(zoom: f32, origine: f32, hauteurs: &[f32]) -> Vec<f32> {
    // Marge contre l'erreur de flottant : une égalité exacte (x,5) doit s'arrondir vers le haut.
    let arrondi = |v: f32| (v * zoom + 0.5 + 1e-3).floor();
    let mut bord_haut = arrondi(origine);
    let mut exact = origine;
    hauteurs
        .iter()
        .map(|h| {
            exact += h;
            let bord_bas = arrondi(exact);
            let calee = bord_bas - bord_haut;
            bord_haut = bord_bas;
            calee
        })
        .collect()
}

/// ★ La moitié d'un reste à répartir (centrage), l'égalité vers le HAUT comme Chromium : un contenu de
/// 15 px centré dans 18 px se pose à 2 px du bord (1,5 arrondi à 2). GPUI arrondit 1,5 à 1 (moitié
/// vers zéro) : on pose donc le décalage soi-même (`.pl(moitie(cx, reste))`) au lieu de `justify_center`.
/// Un reste entier se centre pareil des deux côtés, GPUI suffit alors.
pub fn moitie(cx: &App, reste: f32) -> Pixels {
    px((reste * Echelle::courante(cx).zoom() / 2.0 + 0.5 + 1e-3).floor())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn en_pixels(p: Pixels) -> f32 {
        f32::from(p)
    }

    #[test]
    fn zoom_a_100_pour_cent_ne_change_rien() {
        let e = Echelle::new(1.0);
        assert_eq!(en_pixels(e.u(36.0)), 36.0);
        assert_eq!(en_pixels(e.u(0.5)), 0.5);
        assert_eq!(e, Echelle::NEUTRE);
    }

    #[test]
    fn zoom_a_150_pour_cent_agrandit() {
        let e = Echelle::new(1.5);
        assert_eq!(e.zoom(), 1.5);
        assert_eq!(
            en_pixels(e.u(36.0)),
            54.0,
            "la hauteur d'en-tête passe de 36 à 54"
        );
        assert_eq!(en_pixels(e.u(8.0)), 12.0);
    }

    #[test]
    fn zoom_a_50_pour_cent_reduit() {
        let e = Echelle::new(0.5);
        assert_eq!(en_pixels(e.u(36.0)), 18.0);
        assert_eq!(en_pixels(e.u(1.0)), 0.5);
    }

    #[test]
    fn le_zoom_est_borne_comme_dans_le_coeur() {
        assert_eq!(Echelle::new(5.0).zoom(), 2.0);
        assert_eq!(Echelle::new(0.1).zoom(), 0.5);
        assert_eq!(Echelle::new(f64::NAN).zoom(), 1.0);
    }

    #[test]
    fn calees_alternent_30_et_29_pour_29_5_depuis_5() {
        let h = calees_pures(1.0, 5.0, &[29.5; 5]);
        assert_eq!(h, vec![30.0, 29.0, 30.0, 29.0, 30.0]);
        // La somme est l'arrondi de la fin moins celui du début : 5 + 147,5 = 152,5 -> 153.
        assert_eq!(h.iter().sum::<f32>(), 153.0 - 5.0);
    }

    #[test]
    fn calees_gardent_les_entiers_et_suivent_le_zoom() {
        assert_eq!(calees_pures(1.0, 0.0, &[9.0, 20.0]), vec![9.0, 20.0]);
        assert_eq!(calees_pures(2.0, 0.0, &[29.5, 29.5]), vec![59.0, 59.0]);
    }

    #[test]
    fn le_zoom_par_defaut_est_neutre() {
        assert_eq!(Echelle::default(), Echelle::new(fragment_core::ZOOM_DEFAUT));
    }
}
