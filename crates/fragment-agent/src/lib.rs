//! Agent (modèles) : événements, trace, politiques, permissions, actions, skills, harness claude stream-json.
//!
//! Modèles PURS (aucun processus `claude` ici). Chaque module porte un fichier de
//! `app/src/ui/views/agent/…` de Fragment ; la table de parité est `parite.toml`.

pub mod actions_model;
pub mod attachment_blocks;
pub mod card_model;
pub mod contrat;
pub mod controls_model;
pub mod engine_model;
pub mod events;
pub mod harness_registry;
pub mod lasso_prompt;
pub(crate) mod motif;
pub mod one_shot_options;
pub mod permission_model;
pub mod policy_model;
pub mod quick_model;
pub mod screen_model;
pub mod side_chat_prompt;
pub mod skills_model;
pub mod text_access;
pub mod trace_model;
pub mod utf16;
