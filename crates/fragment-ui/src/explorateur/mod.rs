// Port de app/src/ui/views/explorer/ (FileExplorerView.tsx, FileTree.tsx, explorer.css)
//! L'explorateur de fichiers : l'arbre du coffre, ses dossiers ouverts, le fichier actif,
//! révéler, ouvrir.
//!
//! - [`etat`] : dossiers ouverts, actif, tri, état éphémère (sans GPUI ni coffre) ;
//! - [`arbre`] : l'ordre des enfants et les lignes visibles ;
//! - [`vue`] : la vue `file-explorer` (modèle) et son dessin, sur le gabarit [`crate::tree_item`] ;
//! - [`menu`] : les menus fichier, dossier et racine, la création, la copie, la suppression ;
//! - [`renommer`] : le renommage en place ;
//! - [`deplacer`] : la modale « Déplacer vers… » ;
//! - [`glisser`] : glisser un fichier sur un dossier, import des fichiers du système, ressort.
//!
//! Le montage au dock est fait par `fr-t2-montage`.

pub mod arbre;
pub mod deplacer;
pub mod etat;
pub mod glisser;
pub mod menu;
pub mod renommer;
pub mod vue;

use fragment_core::CommandManager;

pub use arbre::{LigneVisible, comparer_noms, lignes_visibles, trier_enfants};
pub use etat::EtatArbre;
pub use menu::{ActionsExplorateur, GesteVue};
pub use renommer::Renommage;
pub use vue::{
    ContexteExplorateur, FILE_EXPLORER_VIEW_TYPE, FileExplorerView, contenu, fabrique,
    rendre_explorateur,
};

/// Enregistre les commandes de scène `pixels:explorateur` : aucune ici. Les scènes de pixels de
/// l'explorateur sont posées au montage (`fr-t2-montage`), qui a un vrai coffre à montrer.
pub fn commandes_de_scene(_commandes: &CommandManager) {}
