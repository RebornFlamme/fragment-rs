//! Les tableaux Markdown du live preview (fr-t3-tableaux).
//!
//! - [`geometrie`] : la mise en page pure (colonnes, grille, poignées, bandes), lue dans `table.css` ;
//! - [`controle`] : la logique pure du contrôleur (opérations, gestes des cellules, diff minimal) ;
//! - [`menus`] : les menus de poignée, du tableau et d'une cellule ;
//! - [`widget`] : [`Tableau`], la grille de cellules éditables imbriquées ;
//! - [`hote`] : [`HoteTableaux`], l'hôte des blocs augmenté de ses tableaux (hauteur, pont curseur,
//!   clavier et clic autour du tableau).

pub mod controle;
pub mod geometrie;
pub mod hote;
pub mod menus;
pub mod widget;

pub use hote::HoteTableaux;
pub use widget::{Bande, Choix, EvenementTableau, GeoTableau, OptionsTableau, Tableau};

use fragment_core::CommandManager;

/// Enregistre les commandes de scène `pixels:md_tableau` : aucune (les scènes passent par la vue
/// Markdown montée, `fr-t3-vue`).
pub fn commandes_de_scene(_commandes: &CommandManager) {}
