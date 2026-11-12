// Port de app/src/ui/views/explorer/ (FileExplorerView.tsx, FileTree.tsx, explorer.css)
//! L'explorateur de fichiers : l'arbre du coffre, ses dossiers ouverts, le fichier actif,
//! révéler, ouvrir.
//!
//! - [`etat`] : dossiers ouverts, actif, tri, état éphémère (sans GPUI ni coffre) ;
//! - [`arbre`] : l'ordre des enfants et les lignes visibles ;
//! - [`vue`] : la vue `file-explorer` (modèle) et son dessin, sur le gabarit [`crate::tree_item`].
//!
//! Le montage au dock est fait par `fr-t2-montage` ; les menus, la création, le renommage et le
//! glisser-déposer par `fr-t2-explorateur-actions`.

pub mod arbre;
pub mod etat;
pub mod vue;

use fragment_core::CommandManager;

pub use arbre::{LigneVisible, comparer_noms, lignes_visibles, trier_enfants};
pub use etat::EtatArbre;
pub use vue::{
    ContexteExplorateur, FILE_EXPLORER_VIEW_TYPE, FileExplorerView, contenu, fabrique,
    rendre_explorateur,
};

/// Enregistre les commandes de scène `pixels:explorateur` : aucune ici. Les scènes de pixels de
/// l'explorateur sont posées au montage (`fr-t2-montage`), qui a un vrai coffre à montrer.
pub fn commandes_de_scene(_commandes: &CommandManager) {}
