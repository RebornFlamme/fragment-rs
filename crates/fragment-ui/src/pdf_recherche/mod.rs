//! La barre de recherche du PDF (tâche `fr-t4-recherche-ui`) : Ctrl+F, champ, Aa, mots entiers dans
//! un menu, compte « n of m », navigation, surlignages posés par les coordonnées de la couche texte.
//!
//! - [`modele`] : [`BarreRecherche`], l'état de la barre (champ, réglages, compteur) au-dessus de
//!   la recherche pure de `fragment-pdf`, SANS GPUI ;
//! - [`rendu`] : la barre dessinée (36 px, boutons de 26 px) et les rects de surlignage ;
//! - [`vue`] : [`RecherchePdf`], l'entité qui enveloppe la couche texte ([`crate::pdf_texte`]) :
//!   lecture du texte de toutes les pages, portée clavier empilée au focus, navigation vers
//!   l'occurrence.
//!
//! Le montage dans la vue de fichier PDF est fait par la tâche aval (`fr-t4-vue-pdf`).

pub mod modele;
pub mod rendu;
pub mod vue;

use fragment_core::CommandManager;

pub use modele::{BarreRecherche, LIBELLES};
pub use rendu::{Bouton as BoutonRecherche, RectSurlignage, rendre_barre};
pub use vue::RecherchePdf;

/// Enregistre les commandes de scène `pixels:pdf_recherche` : vide jusqu'aux scènes de la tâche
/// aval.
pub fn commandes_de_scene(_commandes: &CommandManager) {}
