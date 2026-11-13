//! Bulles, disque et vue de grappe ouverte en place (tâche `fr-t5-grappes`).
//!
//! Le cœur (`fragment-layers` : `cluster_gutter`, l'arbitre) GROUPE les réduits tombés au même
//! endroit, les CACHE et ANNONCE la grappe ; ce module la PRÉSENTE :
//! - [`bulles`] : la géométrie des bulles (carré de 2 à 4, disque de 5 et plus) ;
//! - [`vue`] : la vue `doc-widget-cluster` et son modèle (liste, recherche, filtre, ⋮, « Aller à ») ;
//! - [`presentateur`] : un DocWidget de grappe par grappe annoncée, le masquage par position ;
//! - [`rendu`] : le dessin GPUI et la scène `pixels:grappe`.

pub mod bulles;
pub mod presentateur;
pub mod rendu;
pub mod vue;

pub use presentateur::{
    GrappePresentee, PresentateurGrappes, RAYON_GRAPPE, SurfaceGrappes, enregistrer_vue,
};
pub use rendu::commandes_de_scene;
pub use vue::{Apercu, Ligne, ModeleGrappe, VueGrappe};
