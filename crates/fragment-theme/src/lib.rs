//! Thème : jetons (noms d'Obsidian, valeurs de Fragment), themeModel, cascade (S4).
//!
//! Cette tâche (`fr-p1-divers`) porte le MODÈLE (`theme_model`) et le contrat
//! `theme:manager@1` (`manager`). La `struct Theme`, `Theme::sombre()/clair()` et la
//! cascade de `theme.json` (ADR-005) sont une tâche de T11.

pub mod manager;
pub mod theme_model;

pub use manager::{THEME_MANAGER_ID, ThemeManager, theme_manager};
pub use theme_model::{
    EffetDUnChangement, FICHIER_DU_THEME, FICHIER_DU_THEME_JSON, chemin_du_theme,
    chemin_du_theme_json, dossier_des_themes, effet_d_un_changement, lister_themes, nom_valide,
};
