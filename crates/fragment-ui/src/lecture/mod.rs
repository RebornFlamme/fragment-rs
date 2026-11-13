//! La position de lecture aux trois étages (état éphémère, clé `scroll`) : le défaut générique
//! d'une vue (`Position`, `Defileur`, `Reprise`), la rétention quand la vue est cachée ou
//! réinsérée, et l'état éphémère qui part dans workspace.json (`Lecture`).
//!
//! Les vues qui ré-agencent leur contenu (note : `{line, dy}`, PDF : `{page, fy, fx}`,
//! conversation) surchargent la lecture et la reprise, et gardent la rétention : elles sont
//! héritées par fr-t3-vue, fr-t4-vue-pdf et T7. L'explorateur porte déjà sa clé `scroll`.

use fragment_core::CommandManager;

mod etat;
mod position;
mod reprise;

pub use etat::Lecture;
pub use position::{CLE_SCROLL, Defileur, Position};
pub use reprise::{DELAI_REPRISE_MS, IssueReprise, Reprise};

/// Enregistre les commandes de scène `pixels:lecture` : vide, la position de lecture n'a pas de
/// rendu propre.
pub fn commandes_de_scene(_commandes: &CommandManager) {}
