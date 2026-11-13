//! Montage des calques dans les vues à surface (tâche `fr-t5-hote`) : l'hôte [`HoteCalques`]
//! (contrat `OverlayHost` de `fragment-layers`, registre des éléments, arbitre et actif de la
//! vue) et le dessin de ses plans ([`plans_calques`] : document, marge non zoomée, viewport).
//!
//! La géométrie pure (conversions, bandes de marge, clé, viewport) est dans
//! `fragment_layers::view_overlays` ; ici, seulement ce qui touche GPUI.

pub mod element;
pub mod hote;

pub use element::{SourcePlans, plans_calques};
pub use hote::{Contenu, Fiche, HoteCalques};

use fragment_core::CommandManager;

/// Enregistre les commandes de scène `pixels:calques` : aucune pour l'hôte (les widgets de test
/// et leurs commandes viennent avec `fr-t5-widgets`).
pub fn commandes_de_scene(_commandes: &CommandManager) {}
