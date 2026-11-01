//! Snap : le modèle des vues repliées en tranches (`snapModel`).
//!
//! La présentation (tranches CSS → éléments GPUI, raccourcis Ctrl+Alt+flèche,
//! persistance dans `workspace.json`) est `fragment-ui` : `SnapPlugin.ts` n'est pas porté ici.

pub mod snap_model;

pub use snap_model::{
    Arbre, GrowApresSnap, PartsDuSplit, Snap, SnapAxis, SnapDir, SnapEnregistre, SnapSide,
    ancetre_commun, autre_cote, axe_de, contient, cote_de, feuilles, grow_apres_restauration,
    grow_apres_snap, reidentifier, transfert_present, trouver_restauration, trouver_snap,
};
