// Port de app/src/ui/windowHost.ts (le pont RÉEL de `app.windowHost`, couche d'assemblage).
//
//! L'hôte fenêtre de GPUI : ce que seule la fenêtre sait faire, derrière le contrat
//! [`fragment_core::WindowHost`].
//!
//! - Zoom : `set_zoom` retient le facteur ; [`HoteFenetre::appliquer`] l'installe dans
//!   [`fragment_ui::echelle::Echelle`] (toute dimension de rendu passe par `u()`).
//! - Capture : indisponible dans le binaire (GPUI ne rend une fenêtre en image que sous sa feature de
//!   test, celle du lecteur de scènes) : une erreur nommée, que l'appelant affiche.
//!
//! ★ Le contrat est SYNCHRONE et sans contexte GPUI : une commande (`app:zoom-in`, lancée par une
//!   frappe ou par la palette) écrit l'apparence, qui appelle `set_zoom` au milieu d'une mise à jour
//!   de l'application. On ne peut pas y toucher aux globaux GPUI : le facteur attend, et
//!   `appliquer` l'installe au point de synchronisation suivant (après la frappe, à chaque tic de
//!   l'application, avant chaque image du lecteur de scènes).

use std::cell::Cell;

use fragment_core::{CaptureRect, WindowHost, WindowHostError, ZOOM_DEFAUT};
use fragment_ui::echelle::Echelle;
use gpui_kit::App;

/// L'hôte fenêtre de la fenêtre principale.
#[derive(Debug)]
pub struct HoteFenetre {
    zoom: Cell<f64>,
    en_attente: Cell<bool>,
}

impl Default for HoteFenetre {
    fn default() -> Self {
        Self {
            zoom: Cell::new(ZOOM_DEFAUT),
            en_attente: Cell::new(true),
        }
    }
}

impl HoteFenetre {
    pub fn new() -> Self {
        Self::default()
    }

    /// Un zoom attend-il d'être installé ?
    pub fn en_attente(&self) -> bool {
        self.en_attente.get()
    }

    /// Installe le zoom en attente dans l'échelle de l'application ; vrai s'il y en avait un.
    pub fn appliquer(&self, cx: &mut App) -> bool {
        if !self.en_attente.replace(false) {
            return false;
        }
        Echelle::installer(cx, self.zoom.get());
        true
    }
}

impl WindowHost for HoteFenetre {
    fn capture(&self, _rect: Option<CaptureRect>) -> Result<Vec<u8>, WindowHostError> {
        Err(WindowHostError::Hote(
            "la capture de la fenêtre n'existe que dans le lecteur de scènes (fragment-pixels)"
                .into(),
        ))
    }

    fn set_zoom(&self, facteur: f64) -> Result<(), WindowHostError> {
        if (facteur - self.zoom.get()).abs() > f64::EPSILON {
            self.zoom.set(facteur);
            self.en_attente.set(true);
        }
        Ok(())
    }

    fn get_zoom(&self) -> Result<f64, WindowHostError> {
        Ok(self.zoom.get())
    }
}
