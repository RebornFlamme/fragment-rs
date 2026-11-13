//! Les Properties (fr-t3-proprietes) : le front matter rendu comme un widget de bloc éditable.
//!
//! - [`modele`] : le contrôleur pur (document + geste → édition), les menus, les types ;
//! - [`saisie`] : les règles du curseur et de la frappe autour du bloc, pures aussi ;
//! - la vue GPUI vit à côté ([`vue`]) et applique les éditions comme des transactions isolées.

pub mod modele;
pub mod saisie;

use fragment_core::CommandManager;

/// Enregistre les commandes de scène `pixels:md_proprietes` : aucune (les scènes passent par la vue
/// Markdown montée, `fr-t3-vue`).
pub fn commandes_de_scene(_commandes: &CommandManager) {}
