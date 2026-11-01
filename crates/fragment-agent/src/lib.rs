//! Agent (modèles) : événements, trace, politiques, permissions, actions, skills, harness claude stream-json.
//!
//! Modèles PURS (aucun processus `claude` ici). Chaque module porte un fichier de
//! `app/src/ui/views/agent/…` de Fragment ; la table de parité est `parite.toml`.

pub mod actions_model;
pub mod attachment_blocks;
pub mod contrat;
pub mod events;
pub(crate) mod motif;
pub mod quick_model;
pub mod trace_model;
pub mod utf16;
