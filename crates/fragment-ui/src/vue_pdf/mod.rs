//! La vue PDF montée (tâche `fr-t4-vue-pdf`) : la vue `pdf` d'une feuille.
//!
//! - [`vue`] : [`VuePdf`], la vue de fichier sans GPUI (octets lus au coffre, garde d'extension,
//!   état éphémère, raccourcis de la portée) ;
//! - [`document`] : [`DocumentPartage`], LE document ouvert une fois, que pages, vignettes, couche
//!   texte et recherche interrogent par leurs fils ;
//! - [`montage`] : [`MontagePdf`], l'assemblage GPUI (barre d'outils, barre latérale, overlayPane).

pub mod document;
pub mod montage;
pub mod vue;

pub use document::DocumentPartage;
pub use montage::{MontagePdf, contenu};
pub use vue::{ContextePdf, EXTENSION_PDF, LiensPdf, PDF_VIEW_TYPE, Touche, VuePdf, fabrique};

use fragment_core::{App as _, AppCoeur, CommandManager, Disposer};

/// Le propriétaire des enregistrements (l'id du plugin cœur).
pub const PLUGIN_PDF: &str = "pdf";

/// Enregistre la vue `pdf` et l'extension `pdf`. Rend les retraits.
pub fn enregistrer(app: &AppCoeur, ctx: ContextePdf) -> Vec<Disposer> {
    vec![
        app.view_registry()
            .register(PDF_VIEW_TYPE, fabrique(ctx), PLUGIN_PDF),
        app.extension_registry()
            .register(EXTENSION_PDF, PDF_VIEW_TYPE.to_owned(), PLUGIN_PDF),
    ]
}

/// Enregistre les commandes de scène `pixels:vue_pdf` : aucune, les scènes PDF passent par le
/// coffre monté (la vue s'ouvre comme dans l'app).
pub fn commandes_de_scene(_commandes: &CommandManager) {}
