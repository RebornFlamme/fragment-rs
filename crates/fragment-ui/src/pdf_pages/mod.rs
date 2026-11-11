//! Les pages d'un PDF (tâche `fr-t4-pages`) : liste virtualisée, une image par page rendue HORS
//! du fil de l'interface, à un pixel de raster par pixel physique, repeinte sans clignotement.
//!
//! - [`vue`] : [`PagesPdf`], la colonne des pages (entité GPUI) au-dessus du `VueModele` ;
//! - [`emplacements`] : les images de chaque page (courante, périmée, rendu en cours), sans GPUI ;
//! - [`fil`] : le fil de rendu (pdfium, ou un rastériseur en ligne pour les tests).
//!
//! Le montage dans une vue de fichier (barre d'outils, couche texte, état éphémère) est fait par
//! les tâches aval (`fr-t4-texte`, `fr-t4-vue-pdf`).

pub mod emplacements;
pub mod fil;
pub mod vue;

use fragment_core::CommandManager;

pub use emplacements::{Demande, EtatImages};
pub use fil::{FilDeRendu, Rasteriseur, RasteriseurPdfium};
pub use vue::{PagesPdf, Pixels, pixels_peints};

/// `.pdf-pages { gap: 1rem }` : l'espace entre deux pages, en px CSS.
pub const ESPACE_PAGES: f64 = 16.0;
/// `.pdf-pages { padding: 1rem 0 }` : la marge au-dessus de la première page, en px CSS.
pub const MARGE_HAUTE: f64 = 16.0;
/// Une ligne de molette (`deltaMode` 1) vaut 16 px CSS, comme dans le modèle.
pub const LIGNE_MOLETTE: f32 = 16.0;

/// Enregistre les commandes de scène `pixels:pdf_pages` : aucune (les scènes PDF passent par
/// `pixels:pdf-zoom` et `pixels:pdf-page`, posées par la vue montée).
pub fn commandes_de_scene(_commandes: &CommandManager) {}
