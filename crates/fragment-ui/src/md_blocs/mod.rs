//! L'hôte des widgets de bloc du live preview (fr-t3-widgets-blocs) : séparateur, bloc de code, maths
//! en bloc, cadre réservé des embeds, et la gouttière de repli (`md_repli`).
//!
//! - [`geometrie`] : les règles de mise en page des blocs, lues dans `editor.css` ;
//! - [`complement`] : ce que l'hôte ajoute au modèle de rangées (blocs, repli, lignes de code).
//! - [`hote`] : [`HoteBlocs`], l'éditeur enveloppé, ses fonds, traits, badges et chevrons.

pub mod complement;
pub mod geometrie;
pub mod hote;
pub mod maths;

pub use hote::{Geometrie, HoteBlocs, LigneEcran, Mesureur};

use fragment_core::CommandManager;

/// Enregistre les commandes de scène `pixels:md_blocs` : aucune (les scènes passent par la vue
/// Markdown montée, `fr-t3-vue`).
pub fn commandes_de_scene(_commandes: &CommandManager) {}
