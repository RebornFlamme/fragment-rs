//! Le HTML dans le Markdown du live preview (tâche `fr-t3-html-rendu`, ADR-012), comme Obsidian :
//! rendu natif d'un HTML ASSAINI, la source au curseur ou par le bouton « </> ».
//!
//! - [`arbre`] : le HTML analysé (html5ever) et assaini par la politique de
//!   `fragment_markdown::sanitize_html` AVANT toute traduction ;
//! - [`css`] : le sous-ensemble des déclarations `style="…"`, variables du thème résolues ;
//! - [`smil`] et [`svg`] : le SVG figé à l'instant `t`, habillé du thème, rastérisé par GPUI ;
//! - [`en_ligne`] : balises en ligne masquées, texte balisé, éléments entiers (calcul pur) ;
//! - [`bloc`] : [`BlocHtml`], le widget d'un bloc (rendu interactif, bouton « </> », hauteur) ;
//! - [`hote`] : [`HoteHtml`], l'hôte des blocs augmenté du HTML.
//!
//! Le montage dans la vue Markdown est le travail d'une tâche aval.

pub mod arbre;
pub mod bloc;
pub mod css;
pub mod en_ligne;
pub mod hote;
pub mod smil;
pub mod svg;

pub use bloc::{BlocHtml, EvenementBloc, GeoBloc};
pub use hote::HoteHtml;

use fragment_core::CommandManager;

/// Enregistre les commandes de scène `pixels:md_html` : aucune (les scènes passent par la vue
/// Markdown montée).
pub fn commandes_de_scene(_commandes: &CommandManager) {}
