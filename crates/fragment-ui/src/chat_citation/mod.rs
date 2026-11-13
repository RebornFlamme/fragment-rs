//! RÉPONDRE : la citation d'une partie de la conversation, dans le champ puis dans le message
//! (tâche `fr-t7-pieces-citation`).
//!
//! Une fonctionnalité GÉNÉRALE de la vue chat (tout fournisseur, tout mode), un COMPORTEMENT du champ
//! contribué à `chat:composer@1` sous l'id `quote` :
//!
//! - [`modele`] : l'extrait, la citation en tête du prompt, l'état d'un champ, l'extension
//!   ([`ExtensionCitation`]) et la cible de « Répondre » ([`CibleReponse`]) ;
//! - [`vue`] : la puce au-dessus du champ (icône, extrait ou vignette, ✕) et le rendu du part `quote`
//!   (une citation cliquable qui ramène au passage).
//!
//! Le part `quote` s'affiche AU-DESSUS du texte (CSS `order: -1` en TS) sans changer sa place dans le
//! message, donc sans toucher aux positions de la surface : [`modele::rang_d_affichage`] dit où le
//! message doit le peindre.

pub mod modele;
pub mod vue;

pub use modele::{
    CibleReponse, EXTRAIT_MAX, EtatCitation, ExtensionCitation, PRIORITE_CITATION, QUOTE_EXTENSION,
    donnees, extrait, part_quote, quote_depuis_json, quote_for_prompt, quote_json,
    rang_d_affichage,
};
pub use vue::{RenduCitation, zone_citation};

use fragment_core::CommandManager;

/// Enregistre les commandes de scène `pixels:chat_citation` : aucune ici (la scène `t7-citation`
/// passe par la vue chat montée, `fr-t7-scenes-b`).
pub fn commandes_de_scene(_commandes: &CommandManager) {}
