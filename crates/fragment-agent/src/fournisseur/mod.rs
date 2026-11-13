//! Fournisseur agent du chat (`AgentChatProvider`) et faux harness scripté (tâche `fr-t7-fournisseur`).
//!
//! - [`hote`] : le TRAIT que le fournisseur appelle pour tout ce qui n'est pas lui (sessions,
//!   registre des moteurs, run commun) ; `fr-t7-service` l'implémente, [`faux::FauxHote`] le double ;
//! - [`agent_chat`] : `AgentChatProvider`, qui implémente `fragment_chat::ChatProvider` ;
//! - [`pieces`] : le rangement des pièces jointes (réduction d'image, fichier « autre ») ;
//! - [`parts`] : les parts du fil (trace, carte de permission) en JSON, comme les relit la vue ;
//! - [`faux`] : `FauxHarness` (tours scriptés, aucun appel à `claude`), `FauxHote`, `CodecImageFaux`.

pub mod agent_chat;
pub mod faux;
pub mod hote;
pub mod parts;
pub mod pieces;

pub use agent_chat::{
    AGENT_PERMISSION_PART, AgentChatProvider, EnvoiAgent, ID_FOURNISSEUR, ProfilAgent, titre_depuis,
};
pub use faux::{CodecImageFaux, FauxHarness, FauxHote, Pas};
pub use hote::{AgentHote, Auteur, DemandeTour, SessionAgent, SortieTour};
