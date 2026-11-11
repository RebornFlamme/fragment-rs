//! La barre d'outils de la vue PDF : bouton de barre latérale et son menu, zoom − / + et menu
//! des modes, champ de page « of N », raccourcis Ctrl + − 0.
//!
//! Découpage :
//! - [`modele`] : [`BarrePdf`], l'état de la barre (barre latérale, menu ouvert) et tout ce qui
//!   en découle, SANS GPUI : menus construits, touches, saisie du champ. Tout zoom et toute
//!   navigation passent par le [`fragment_pdf::vue_modele::VueModele`] de la vue ;
//! - [`rendu`] : la barre dessinée (36 px de haut, boutons de 26 px).
//!
//! La barre ne possède pas le modèle de la vue : la vue montée (fr-t4-vue-pdf) le lui prête à
//! chaque appel, et le montage dans le shell est fait par cette tâche aval.

pub mod modele;
pub mod rendu;

use fragment_core::CommandManager;

pub use modele::{ActionBarre, BarrePdf, Bouton, LIBELLES_BOUTONS, ModeBarre};
pub use rendu::rendre_barre;

/// Enregistre les commandes de scène `pixels:pdf_barre` : vide jusqu'aux scènes de la tâche aval.
pub fn commandes_de_scene(_commandes: &CommandManager) {}
