//! L'élément éditeur du live preview et sa surface de texte (tâche `fr-t3-surface`).
//!
//! - [`element`] : [`EditeurLive`], l'`EditorState` du fork (ADR-006) piloté par le modèle du live
//!   preview, recalculé à chaque changement de texte, de sélection ou de focus ;
//! - [`modele`] : la projection pure du modèle (`fragment_markdown::decorations`, coloration de
//!   `syntaxe::imbrique`) sur le fork ;
//! - [`jetons`] : les noms de jetons résolus dans le thème, en styles de texte GPUI ;
//! - [`surface`] : [`SurfaceLive`], le contrat `TextSurface` (UTF-16) de `fragment-text` ;
//! - [`changement`] : le `map_pos` d'un changement de texte ;
//! - [`constantes`] : la colonne de lecture, lue dans les CSS de Fragment.
//!
//! Les widgets (en ligne, de bloc), la gouttière de repli et le montage dans la vue Markdown sont
//! le travail des tâches aval (`fr-t3-widgets-*`, `fr-t3-vue`).

pub mod changement;
pub mod constantes;
pub mod element;
pub mod jetons;
pub mod modele;
pub mod surface;

pub use element::{Calque, EditeurLive, OptionsEditeur, style_de_l_editeur, style_projete};
pub use modele::{ComplementRangees, EntreeComplement};
pub use surface::SurfaceLive;

use fragment_core::CommandManager;

/// Enregistre les commandes de scène `pixels:editeur` : aucune pour l'instant (les scènes du live
/// preview passent par la vue Markdown montée, `fr-t3-vue`).
pub fn commandes_de_scene(_commandes: &CommandManager) {}
