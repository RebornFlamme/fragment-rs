//! La conversation est un document (tâche `fr-t7-surface`) : la `TextSurface` en lecture seule à
//! pas fixe d'une conversation, sur les éditeurs de ses parts.
//!
//! - [`surface`] : l'entité [`ChatSurface`] (unités, synchronisation avec le fil, changements),
//!   l'emprunt [`SurfaceConversation`] (le contrat de surface) et [`plan_observe`] (le plan suivi
//!   à chaque image) ;
//! - [`garde`] : « / », Ctrl+Z et Échap tapés au champ restent au champ ([`GardeChamp`]).
//!
//! Le montage (la vue chat en fait son `get_editor`, les calques s'y posent) est `fr-t7-montage`.

pub mod garde;
pub mod surface;

pub use garde::{GardeChamp, focus_dans_un_champ};
pub use surface::{ChatSurface, SurfaceConversation, plan_observe};

use fragment_core::CommandManager;

/// Enregistre les commandes de scène `pixels:chat_surface` : aucune (la surface ne se peint pas ;
/// ses scènes sont celles des calques de la vue chat montée).
pub fn commandes_de_scene(_commandes: &CommandManager) {}
