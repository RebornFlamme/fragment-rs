//! Chat (modèles) : ChatSession, chatSurfaceModel, followModel, rail, recherche, historique, format.
//!
//! Modèles PURS (aucun DOM, aucune vue, pas de gpui-kit). Chaque module porte un fichier de
//! `app/src/ui/views/chat/…` de Fragment ; la table de parité est `parite.toml`.
//!
//! ★ Les positions de la surface d'une conversation sont des index en unités de code UTF-16
//!   (ADR-004) ; la conversion avec les `&str` passe par `fragment_text::utf16`.

pub mod api;
pub mod calendrier;
pub mod chat_key;
pub mod chat_session;
pub mod chat_surface_model;
pub mod follow_model;
pub mod format;
pub mod history_model;
pub mod post_model;
pub mod rail;
pub mod search_model;
