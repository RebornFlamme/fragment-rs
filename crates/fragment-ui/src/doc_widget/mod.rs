//! Cadre DocWidget (en-tête, replié, pastille, actions, actif) et service `doc-widget:host@1` en
//! mémoire (tâche `fr-t5-doc-widget`).
//!
//! - [`api`] : le contrat du service et ce que la vue hébergée reçoit ;
//! - [`service`] : le service en mémoire (la persistance au journal est à T6) ;
//! - [`cadre`] : le modèle du widget (replié / ouvert, actif, actions, menus, passage surligné) ;
//! - [`icones_vue`] : le modèle des icônes d'une vue réduite à elles ;
//! - [`rendu`] : le dessin GPUI (en-tête de 37 px, bandeau replié de 38 px, anneau d'actif).

pub mod api;
pub mod cadre;
pub mod icones_vue;
pub mod pin;
pub mod rendu;
pub mod service;

pub use api::{
    DocWidgetAction, DocWidgetEntry, DocWidgetHost, DocWidgetOpen, DocWidgetPlacement,
    etat_de_place, place_de_l_etat,
};
pub use cadre::{
    ActionEntete, Ailleurs, Crochets, DocWidget, OptionsCadre, OptionsOuverture, PlacementWidget,
};
pub use rendu::commandes_de_scene;
pub use service::{EvenementService, ServiceDocWidget, etat_d_ouverture};
