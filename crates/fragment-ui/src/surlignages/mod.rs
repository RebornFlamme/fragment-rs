//! Les rangeHighlights dessinés sous le texte, un calque par surface (tâche `fr-t5-surlignages`).
//!
//! - [`CalqueSurlignages`] : la table d'une surface (celle de `fragment-text`), sa géométrie et son
//!   dessin ; le crochet [`SousLeTexte`] est ce que l'élément d'une surface appelle avant ses
//!   glyphes. Le branchement réel (éditeur, pages PDF) est fait au montage (`fr-t5-montage-note`,
//!   `fr-t5-montage-pdf`) ; ici, il n'existe que le crochet et une fausse surface en test.
//! - [`style`] : les classes (passage, actif, survol) résolues en jetons de thème.
//! - Commande de scène `pixels:survol` `{id, from, to, active?}` : pose le surlignage d'un passage
//!   survolé sur le calque exposé aux scènes ([`exposer_aux_scenes`]).

mod calque;
mod scene;
pub mod style;

pub use calque::{CalqueSurlignages, ModeCalque, Rectangle, SousLeTexte};
pub use scene::{commandes_de_scene, exposer_aux_scenes};
