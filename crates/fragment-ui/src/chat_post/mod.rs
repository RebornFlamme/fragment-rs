//! Présentation « post » d'une conversation : le tweet d'une grappe ouverte (tâche `fr-t7-post`).
//!
//! - [`modele`] : le modèle pur (mode compact, hauteur coupée, heure, citation, pièces) ;
//! - [`vue`] : la vue ([`PostVue`]), dont l'hôte (la grappe) branche les [`Rappels`] au montage.
//!
//! Le montage dans la grappe est `fr-t7-montage` ; les scènes de pixels, `fr-t7-scenes-b`.

pub mod modele;
pub mod vue;

pub use modele::{Origine, PostMessage, PostModel, post_model};
pub use vue::{DepsPost, PostVue, Rappel, Rappels};

use fragment_core::CommandManager;

/// Enregistre les commandes de scène `pixels:chat_post` : aucune ici (`fr-t7-scenes-b`).
pub fn commandes_de_scene(_commandes: &CommandManager) {}
