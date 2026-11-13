//! Présentation du WidgetLayer (tâche `fr-t5-widgets`) : les widgets ancrés de TOUS les calques d'une
//! vue, posés sur son hôte (`crate::calques::HoteCalques`).
//!
//! - [`couche`] : la couche d'une vue — ses `WidgetLayer` (un par calque), la boucle d'image qui
//!   rejoue leurs microtâches, l'appui et le relâché du pointeur traduits pour l'actif, le focus
//!   clavier suivi, et l'équivalent du `ResizeObserver` (une taille de marge qui change relance
//!   l'arbitre) ;
//! - [`paresseux`] : un `LazyWidget` GPUI, construit quand son ancre entre dans la bande et détruit
//!   quand elle en sort (instance NEUVE au retour) ;
//! - [`essai`] : les widgets de TEST (rigide, flex, seuil, vue nue), écrits pour fragment-rs ;
//! - [`scene`] : les commandes `pixels:widget-test` et `pixels:activer`.
//!
//! Toute la logique (ancres, bande, arbitre, actif) est dans `fragment-layers` ; ici, seulement ce
//! qui touche GPUI.

pub mod couche;
pub mod essai;
pub mod globe;
pub mod paresseux;
pub mod scene;

pub use couche::{CalqueWidgets, CoucheWidgets};
pub use essai::{SorteEssai, SpecEssai, WidgetEssai, poser_essai};
pub use paresseux::{Fabrique, WidgetDeclare};
pub use scene::{commandes_de_scene, exposer_aux_scenes};
