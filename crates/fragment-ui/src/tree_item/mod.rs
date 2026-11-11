// Port de app/src/ui/views/TreeItem.tsx et treeItem.css
//! Le gabarit d'item d'arbre d'Obsidian, partagé par l'explorateur (T2) et la table des matières du
//! PDF (T4) : [`geometrie`] (les mesures, sans GPUI), [`couleurs`] (les états lus dans les jetons) et
//! [`rendu`] (la ligne et ses enfants). L'appelant fournit les nœuds ; ce module ne connaît aucun
//! modèle de données.

use fragment_core::CommandManager;

pub mod couleurs;
pub mod geometrie;
pub mod rendu;

pub use couleurs::{CouleursLigne, Etat, couleur_guide, couleurs_ligne};
pub use rendu::{Ligne, hauteurs_calees, rendre_item};

/// Enregistre les commandes de scène `pixels:tree_item` : aucune ici. Les scènes de l'arbre sont
/// celles de l'explorateur (`fr-t2-explorateur`), qui monte ce gabarit.
pub fn commandes_de_scene(_commandes: &CommandManager) {}
