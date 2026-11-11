//! La fenêtre de réglages (2e fenêtre, 1000 × 700) : `fr-t1-reglages`.
//!
//! - [`curseur`] : le modèle du glisser d'un curseur (appliqué au relâché pour le zoom) ;
//! - [`fenetre`] : la fenêtre, rendue depuis des `SettingDefinition` déclaratives ;
//! - [`onglets_t1`] : les onglets de T1 (Appearance réel, les autres sont des entrées de rail).
//!
//! ★ La fenêtre est une vue GPUI de plus DANS le même processus : elle partage l'`Apparence` de la
//! fenêtre principale (pas de relais entre renderers, voir `parite.toml`).

use std::rc::Rc;

use fragment_core::{Apparence, CommandManager, definitions_appearance};

pub mod curseur;
pub mod fenetre;

pub use fenetre::{
    Definitions, FenetreReglages, OngletReglages, SectionOnglet, couleur_depuis_hex, hex_de,
};

/// L'onglet « Appearance », lié à l'apparence de l'app.
pub fn onglet_appearance(apparence: &Rc<Apparence>) -> OngletReglages {
    let a = apparence.clone();
    OngletReglages::new(
        "appearance",
        "Appearance",
        "palette",
        SectionOnglet::Options,
    )
    .avec_definitions(Rc::new(move |theme| {
        // La liste des thèmes du coffre vient avec le plugin `theme` : « Default » seulement.
        definitions_appearance(
            &a,
            &hex_de(theme.palette.color_accent),
            Some(themes_par_defaut()),
        )
    }))
}

/// Le choix de thème de T1 : aucun thème dans le coffre ; ↻ et 📂 n'ont rien derrière.
fn themes_par_defaut() -> fragment_core::ThemesDefinition {
    fragment_core::ThemesDefinition {
        liste: Vec::new(),
        relire: Rc::new(|| {}),
        ouvrir_dossier: Rc::new(|| {}),
    }
}

/// Le rail de T1 : les onglets de Fragment dans leur ordre, Appearance seul rempli. Les autres
/// viennent avec leur tranche (General, Editor, Hotkeys…) : ils tiennent leur place au rail pour que
/// la fenêtre ait la forme de celle de Fragment.
pub fn onglets_t1(apparence: &Rc<Apparence>) -> Vec<OngletReglages> {
    use SectionOnglet::{Core, Options};
    vec![
        OngletReglages::new("general", "General", "settings", Options),
        OngletReglages::new("editor", "Editor", "pencil", Options),
        OngletReglages::new("files", "Files and links", "folder-open", Options),
        onglet_appearance(apparence),
        OngletReglages::new("hotkeys", "Hotkeys", "keyboard", Options),
        OngletReglages::new("community-plugins", "Community plugins", "blocks", Options),
        OngletReglages::new("keychain", "Keychain", "key-round", Options),
        OngletReglages::new("agent", "Agent", "sparkles", Core),
    ]
}

/// Les commandes de scène `pixels:*` de la fenêtre de réglages : aucune. Les scènes ouvrent la
/// fenêtre par `settings_window` et la pilotent par clics (le rail), comme `reglages-appearance`.
pub fn commandes_de_scene(_commandes: &CommandManager) {}
