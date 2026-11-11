//! Thème : jetons (noms d'Obsidian, valeurs de Fragment), themeModel, cascade (S4).
//!
//! `fr-p1-divers` a porté le MODÈLE (`theme_model`) et le contrat `theme:manager@1`
//! (`manager`). `fr-t1-squelette` y ajoute la `struct Theme` (un champ par jeton), ses deux
//! constructeurs `Theme::sombre()/clair()` et la couleur 8 bits avec son `color-mix`
//! (ADR-005). La cascade de `theme.json` d'un coffre reste une tâche de T11.

pub mod clair;
pub mod couleur;
pub mod manager;
pub mod sombre;
pub mod theme;
pub mod theme_model;

pub use couleur::{Couleur, Ombre};
pub use theme::{Aplat, Forme, Mode, Opacites, Palette, Pile, Theme, Typo};

pub use manager::{THEME_MANAGER_ID, ThemeManager, theme_manager};
pub use theme_model::{
    EffetDUnChangement, FICHIER_DU_THEME, FICHIER_DU_THEME_JSON, chemin_du_theme,
    chemin_du_theme_json, dossier_des_themes, effet_d_un_changement, lister_themes, nom_valide,
};
