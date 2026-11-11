//! La couche texte d'un PDF (tâche `fr-t4-texte`) exposée comme `TextSurface`.
//!
//! - [`modele`] : le texte d'une page, ses glyphes (unités de page non zoomées) et ses liens,
//!   sans GPUI ;
//! - [`surface`] : [`SurfacePdf`], la `TextSurface` (positions `(page − 1) × 1e6 + 1 + car.`) ;
//! - [`source`] : la lecture du texte hors du fil de l'interface (pdfium, ou en ligne) ;
//! - [`vue`] : [`TextePdf`], l'entité qui couvre la colonne des pages (sélection, liens, outil
//!   armé).
//!
//! Le montage dans la vue de fichier est fait par `fr-t4-vue-pdf`.

pub mod modele;
pub mod source;
pub mod surface;
pub mod vue;

use fragment_core::CommandManager;

pub use modele::{Glyphe, LienPage, TextePage};
pub use source::{FilDeTexte, SourceTexte, SourceTextePdfium};
pub use surface::{SurfacePdf, index_de_pos, page_de_pos, pos_de_page};
pub use vue::{PhasePointeur, PointeurPdf, TextePdf};

/// Enregistre les commandes de scène `pixels:pdf_texte` : aucune (la scène `t4-selection` passe
/// par la vue montée).
pub fn commandes_de_scene(_commandes: &CommandManager) {}
