//! Les widgets en ligne du live preview (tâche `fr-t3-widgets-inline`) : case à cocher cliquable,
//! puce liée au repli, barres d'indentation et maths en ligne.
//!
//! - [`geometrie`] : la partie pure (largeurs dictées au fork, bascule d'une case, sélection du
//!   LaTeX, aspect d'une puce) ;
//! - [`calque`] : la peinture par-dessus l'éditeur et la capture des clics ([`installer`]).
//!
//! Chaque widget occupe la plage de sa source avec SA largeur (`set_widget_ranges` du fork,
//! ADR-006) ; les maths viennent de `fragment-maths` (ADR-011). Le montage dans la vue Markdown est
//! le travail de `fr-t3-vue`.

pub mod calque;
pub mod geometrie;

pub use calque::{CAPACITE_MOTEUR, LiaisonRepli, MdInline, installer};

use fragment_core::CommandManager;

/// Enregistre les commandes de scène `pixels:md_inline` : aucune pour l'instant (les scènes du live
/// preview passent par la vue Markdown montée, `fr-t3-vue`).
pub fn commandes_de_scene(_commandes: &CommandManager) {}
