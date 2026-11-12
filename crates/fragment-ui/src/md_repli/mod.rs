//! La gouttière de repli du live preview (fr-t3-widgets-blocs) : chevrons de titres, de listes et de
//! blocs, survol partagé avec la puce liée.
//!
//! - [`etat`] : [`Repli`], l'état de repli d'un éditeur (plis, survol, contrôle) ;
//! - [`gouttiere`] : géométrie et couleurs des chevrons.
//!
//! Le dessin et les clics sont ceux de l'hôte des blocs (`md_blocs::hote`), qui enveloppe l'éditeur.

pub mod etat;
pub mod gouttiere;

pub use etat::{Chevron, Controle, Interaction, Repli};

use fragment_core::CommandManager;

/// Enregistre les commandes de scène `pixels:md_repli` : aucune (`pixels:fold` passe par la vue
/// Markdown montée, `fr-t3-vue`).
pub fn commandes_de_scene(_commandes: &CommandManager) {}
