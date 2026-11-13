//! Fil de la conversation : messages, parts vivants, champ (tâche `fr-t7-fil`).
//!
//! - [`modele`] : le modèle pur (forme et état d'un message, avatar, libellés) ;
//! - [`registre`] : les points de branchement (`chat:part@1`, `chat:text@1`, slots, extensions du
//!   champ `chat:composer@1`) — c'est là que `fr-t7-trace` branche le part « trace » ;
//! - [`texte`] : le texte d'un part, par l'éditeur de la note en lecture seule, mis à jour en flux ;
//! - [`primitives`] : le bouton-icône et l'avatar ;
//! - [`message`] : un message ([`MessageVue`]) ;
//! - [`liste`] : la liste ([`ListeMessages`]), qui ne défile jamais seule ;
//! - [`champ`] : le champ de saisie ([`Champ`]).
//!
//! Le défilement, la surface de document, la vue chat et le montage sont les tâches aval
//! (`fr-t7-defilement`, `fr-t7-surface`, `fr-t7-vue`).

pub mod champ;
pub mod liste;
pub mod message;
pub mod modele;
pub mod primitives;
pub mod registre;
pub mod texte;

pub use champ::{Champ, DepsChamp, EnvoiChamp, EtatBouton, EvenementChamp, etat_bouton};
pub use liste::ListeMessages;
pub use message::{DepsFil, MessageVue, PartRendue};
pub use registre::{
    ContributionsFil, ExtensionChamp, PartVivant, RegistreFil, RenduDePart, RenduDeTexte, SlotChat,
    ZoneChamp, registre_vide,
};
pub use texte::TexteLecture;

use fragment_core::CommandManager;

/// Enregistre les commandes de scène `pixels:chat_fil` : aucune ici (les scènes du fil passent par
/// la vue chat montée, `fr-t7-vue` et `fr-t7-scenes-a`).
pub fn commandes_de_scene(_commandes: &CommandManager) {}
