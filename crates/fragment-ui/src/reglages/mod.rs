//! La fenêtre de réglages (2e fenêtre, 1000 × 700) : `fr-t1-reglages`.
//!
//! - [`curseur`] : le modèle du glisser d'un curseur (appliqué au relâché pour le zoom) ;
//! - [`fenetre`] : la fenêtre, rendue depuis des `SettingDefinition` déclaratives ;
//! - [`onglets_t1`] : les onglets de T1 (Appearance réel, les autres sont des entrées de rail).
//!
//! ★ La fenêtre est une vue GPUI de plus DANS le même processus : elle partage l'`Apparence` de la
//! fenêtre principale (pas de relais entre renderers, voir `parite.toml`).

use std::cell::Cell;
use std::rc::Rc;

use fragment_core::{Apparence, CommandManager, SchemaCouleurs, definitions_appearance};
use fragment_theme::{Theme, ThemesCoffre};
use gpui_kit::{App, WindowAppearance};

pub mod apparence_themes;
pub mod curseur;
pub mod fenetre;
pub mod selecteur_couleur;

use apparence_themes::{appliquer_schema, definitions_apparence_themes};
pub use fenetre::{
    ApresChangement, Definitions, FenetreReglages, OngletReglages, SectionOnglet,
    couleur_depuis_hex, hex_de,
};

/// L'onglet « Appearance », lié à l'apparence de l'app, sans thèmes de coffre.
pub fn onglet_appearance(apparence: &Rc<Apparence>) -> OngletReglages {
    onglet_appearance_themes(apparence, None)
}

/// L'onglet « Appearance » avec les thèmes du coffre (`Some`) : liste des thèmes, accent et schéma
/// y passent par le `ThemesCoffre`. `None` : le choix de T1 (« Default » seulement).
pub fn onglet_appearance_themes(
    apparence: &Rc<Apparence>,
    themes: Option<Rc<ThemesCoffre>>,
) -> OngletReglages {
    let a = apparence.clone();
    let en_attente: Rc<Cell<Option<SchemaCouleurs>>> = Rc::default();
    let (pour_definitions, file) = (themes.clone(), en_attente.clone());
    let onglet = OngletReglages::new(
        "appearance",
        "Appearance",
        "palette",
        SectionOnglet::Options,
    )
    .avec_definitions(Rc::new(move |theme: &Theme| {
        if let Some(themes) = &pour_definitions {
            let file = file.clone();
            return definitions_apparence_themes(
                &a,
                themes,
                theme.mode,
                Rc::new(move |schema| file.set(Some(schema))),
            );
        }
        // Sans coffre : « Default » seulement.
        definitions_appearance(
            &a,
            &hex_de(theme.palette.color_accent),
            Some(themes_par_defaut()),
        )
    }));
    // ★ Les rappels des réglages n'ont pas de `cx` : le schéma choisi attend dans `en_attente` et se
    //   pose avec la main de la fenêtre (`appliquer_schema` garde le thème du coffre, change le mode).
    match themes {
        Some(themes) => onglet.avec_apres_changement(Rc::new(move |cx: &mut App| {
            if let Some(schema) = en_attente.take() {
                let sombre = matches!(
                    cx.window_appearance(),
                    WindowAppearance::Dark | WindowAppearance::VibrantDark
                );
                appliquer_schema(cx, &themes, schema, sombre);
            }
        })),
        None => onglet,
    }
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
    onglets_t1_themes(apparence, None)
}

/// [`onglets_t1`], Appearance branché sur les thèmes du coffre quand il y en a un.
pub fn onglets_t1_themes(
    apparence: &Rc<Apparence>,
    themes: Option<Rc<ThemesCoffre>>,
) -> Vec<OngletReglages> {
    use SectionOnglet::{Core, Options};
    vec![
        OngletReglages::new("general", "General", "settings", Options),
        OngletReglages::new("editor", "Editor", "pencil", Options),
        OngletReglages::new("files", "Files and links", "folder-open", Options),
        onglet_appearance_themes(apparence, themes),
        OngletReglages::new("hotkeys", "Hotkeys", "keyboard", Options),
        OngletReglages::new("community-plugins", "Community plugins", "blocks", Options),
        OngletReglages::new("keychain", "Keychain", "key-round", Options),
        OngletReglages::new("agent", "Agent", "sparkles", Core),
    ]
}

/// Les commandes de scène `pixels:*` de la fenêtre de réglages : aucune. Les scènes ouvrent la
/// fenêtre par `settings_window` et la pilotent par clics (le rail), comme `reglages-appearance`.
pub fn commandes_de_scene(_commandes: &CommandManager) {}
