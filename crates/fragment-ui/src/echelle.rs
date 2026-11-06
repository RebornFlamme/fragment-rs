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
    fn le_zoom_par_defaut_est_neutre() {
        assert_eq!(Echelle::default(), Echelle::new(fragment_core::ZOOM_DEFAUT));
    }
}
