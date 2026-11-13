// Port de app/src/ui/settings/SettingsPlugin.ts (`EditorTab`), app/src/ui/core/editor/editorConfig.ts
// et app/src/ui/core/settings/config.ts (`DEFAULT_APP`)
//
//! L'onglet « Editor » de la fenêtre de réglages (`fr-t3-reglages-editeur`) : cinq réglages
//! déclaratifs (largeur de lecture, numéros de ligne, retour à la ligne, correcteur, taille de
//! tabulation) lus et écrits dans `app.json` du coffre.
//!
//! - [`ReglagesApp`] : `app.json`, avec ses accesseurs typés ;
//! - [`ConfigEditeur`] : l'instantané que la vue Markdown relit à chaque changement (l'effet « à
//!   chaud » : la vue s'abonne par [`ReglagesApp::sur_changement`], elle ne recrée pas l'éditeur) ;
//! - [`definitions_editor`] / [`onglet_editor`] : les définitions et l'entrée du rail.
//!
//! ★ Le magasin est UN seul pour toutes les fenêtres (même processus) : pas de relais entre
//!   renderers comme dans Fragment, l'abonnement local suffit.
//! ★ Les définitions relisent le magasin à CHAQUE rendu de la fenêtre : un `app.json` modifié
//!   ailleurs (autre onglet, commande) y est vu sans rien rebrancher.

use std::rc::Rc;

use fragment_core::{
    ConfigStore, Donnees, EventRef, Horloge, OrigineChangement, SettingControl, SettingDefinition,
    SettingDefinitionBase, SettingDefinitionControl, SettingSliderControl, SettingToggleControl,
    StockageConfig, StockageMemoire,
};
use serde_json::{Value, json};

use super::{OngletReglages, SectionOnglet};

/// Le fichier des réglages d'application, dans `.fragment/`.
pub const FICHIER_APP: &str = "app.json";

/// Taille de tabulation par défaut, en espaces.
pub const TAB_PAR_DEFAUT: u32 = 4;
/// Bornes du curseur de tabulation.
pub const TAB_MIN: u32 = 2;
pub const TAB_MAX: u32 = 8;

/// Les valeurs par défaut de `app.json` (`DEFAULT_APP`), dans l'ordre du TS.
pub fn defauts_app() -> Donnees {
    let mut d = Donnees::new();
    d.insert("automaticUpdates".into(), json!(true));
    d.insert("language".into(), json!("en"));
    d.insert("readableLineLength".into(), json!(true));
    d.insert("lineNumbers".into(), json!(false));
    d.insert("lineWrap".into(), json!(true));
    d.insert("tabSize".into(), json!(TAB_PAR_DEFAUT));
    d.insert("spellcheck".into(), json!(false));
    d.insert("trashOption".into(), json!("system"));
    d.insert("newFileLocation".into(), json!("root"));
    d.insert("attachmentLocation".into(), json!("root"));
    d.insert("attachmentFolder".into(), json!("attachments"));
    d
}

/// Ce que l'éditeur lit des réglages : les quatre extensions à compartiment du TS plus la
/// largeur de colonne.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConfigEditeur {
    /// Colonne de lecture bornée ; faux : pleine largeur.
    pub largeur_lisible: bool,
    /// Numéros de ligne dans la gouttière.
    pub numeros_de_ligne: bool,
    /// Retour à la ligne automatique.
    pub retour_ligne: bool,
    /// Correcteur orthographique du système.
    pub correcteur: bool,
    /// Taille d'une tabulation, en espaces.
    pub taille_tab: u32,
}

impl ConfigEditeur {
    /// La largeur de colonne posée sur la vue : `None` = la colonne de lecture du thème (la
    /// variable est vidée), `Some("100%")` = pleine largeur.
    pub fn largeur_colonne(&self) -> Option<&'static str> {
        if self.largeur_lisible {
            None
        } else {
            Some("100%")
        }
    }

    /// Ce qu'une touche Tab insère : `taille_tab` espaces, au moins un.
    pub fn unite_indentation(&self) -> String {
        " ".repeat(self.taille_tab.max(1) as usize)
    }
}

/// `app.json`, avec ses accesseurs typés.
pub struct ReglagesApp {
    magasin: ConfigStore,
}

impl std::fmt::Debug for ReglagesApp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReglagesApp")
            .field("editeur", &self.editeur())
            .finish_non_exhaustive()
    }
}

impl ReglagesApp {
    /// Les réglages d'un coffre ; [`Self::charger`] lit le fichier.
    pub fn new(stockage: Rc<dyn StockageConfig>, horloge: Horloge) -> Rc<Self> {
        Rc::new(Self {
            magasin: ConfigStore::new(FICHIER_APP, defauts_app(), stockage, horloge),
        })
    }

    /// Des réglages sans disque (les défauts) : scènes de pixels et bancs.
    pub fn en_memoire() -> Rc<Self> {
        Self::new(Rc::new(StockageMemoire::default()), Rc::new(|| 0))
    }

    pub fn magasin(&self) -> &ConfigStore {
        &self.magasin
    }

    pub fn charger(&self) {
        self.magasin.charger();
    }

    fn booleen(&self, cle: &str, defaut: bool) -> bool {
        self.magasin
            .get(cle)
            .and_then(|v| v.as_bool())
            .unwrap_or(defaut)
    }

    pub fn largeur_lisible(&self) -> bool {
        self.booleen("readableLineLength", true)
    }

    pub fn numeros_de_ligne(&self) -> bool {
        self.booleen("lineNumbers", false)
    }

    pub fn retour_ligne(&self) -> bool {
        self.booleen("lineWrap", true)
    }

    pub fn correcteur(&self) -> bool {
        self.booleen("spellcheck", false)
    }

    /// La taille de tabulation ; une valeur illisible (fichier édité à la main) retombe sur 4.
    pub fn taille_tab(&self) -> u32 {
        self.magasin
            .get("tabSize")
            .and_then(|v| v.as_f64())
            .filter(|n| n.is_finite() && *n >= 1.0)
            .map_or(TAB_PAR_DEFAUT, |n| n.round() as u32)
    }

    pub fn set_largeur_lisible(&self, on: bool) {
        self.magasin.set("readableLineLength", json!(on));
    }

    pub fn set_numeros_de_ligne(&self, on: bool) {
        self.magasin.set("lineNumbers", json!(on));
    }

    pub fn set_retour_ligne(&self, on: bool) {
        self.magasin.set("lineWrap", json!(on));
    }

    pub fn set_correcteur(&self, on: bool) {
        self.magasin.set("spellcheck", json!(on));
    }

    pub fn set_taille_tab(&self, n: u32) {
        self.magasin.set("tabSize", Value::from(n));
    }

    /// L'instantané que la vue Markdown applique (à sa création, puis à chaque changement).
    pub fn editeur(&self) -> ConfigEditeur {
        ConfigEditeur {
            largeur_lisible: self.largeur_lisible(),
            numeros_de_ligne: self.numeros_de_ligne(),
            retour_ligne: self.retour_ligne(),
            correcteur: self.correcteur(),
            taille_tab: self.taille_tab(),
        }
    }

    /// S'abonne aux changements (locaux OU distants) : la fonction reçoit l'instantané à jour.
    /// ★ La vue n'est pas recréée, elle réapplique la configuration (`reconfigureEditors`).
    pub fn sur_changement(self: &Rc<Self>, cb: impl Fn(ConfigEditeur) + 'static) -> EventRef {
        let moi = Rc::downgrade(self);
        self.magasin.sur_changement(move |_: OrigineChangement| {
            if let Some(moi) = moi.upgrade() {
                cb(moi.editeur());
            }
        })
    }
}

fn base() -> SettingDefinitionBase {
    SettingDefinitionBase { id: None }
}

fn bascule(
    nom: &str,
    description: &str,
    valeur: bool,
    ecrire: impl Fn(bool) + 'static,
) -> SettingDefinition {
    SettingDefinition::Control(SettingDefinitionControl {
        base: base(),
        name: nom.into(),
        description: Some(description.into()),
        control: SettingControl::Toggle(SettingToggleControl {
            value: valeur,
            on_change: Some(Rc::new(ecrire)),
        }),
    })
}

/// Les cinq lignes de l'onglet, dans l'ordre de Fragment.
pub fn definitions_editor(reglages: &Rc<ReglagesApp>) -> Vec<SettingDefinition> {
    let r = reglages.clone();
    let mut defs = vec![bascule(
        "Readable line length",
        "Limit the editor line width for readability.",
        reglages.largeur_lisible(),
        move |v| r.set_largeur_lisible(v),
    )];
    let r = reglages.clone();
    defs.push(bascule(
        "Show line numbers",
        "Display line numbers in the gutter.",
        reglages.numeros_de_ligne(),
        move |v| r.set_numeros_de_ligne(v),
    ));
    let r = reglages.clone();
    defs.push(bascule(
        "Line wrap",
        "Wrap long lines instead of scrolling horizontally.",
        reglages.retour_ligne(),
        move |v| r.set_retour_ligne(v),
    ));
    let r = reglages.clone();
    defs.push(bascule(
        "Spellcheck",
        "Check spelling as you type.",
        reglages.correcteur(),
        move |v| r.set_correcteur(v),
    ));
    let r = reglages.clone();
    defs.push(SettingDefinition::Control(SettingDefinitionControl {
        base: base(),
        name: "Tab size".into(),
        description: Some("Number of spaces a tab is worth.".into()),
        control: SettingControl::Slider(SettingSliderControl {
            value: f64::from(reglages.taille_tab()),
            min: f64::from(TAB_MIN),
            max: f64::from(TAB_MAX),
            step: Some(1.0),
            instant: None,
            format: None,
            on_change: Some(Rc::new(move |v: f64| {
                r.set_taille_tab(v.round().max(1.0) as u32);
            })),
        }),
    }));
    defs
}

/// L'onglet « Editor » du rail (icône `pencil`, section Options).
pub fn onglet_editor(reglages: &Rc<ReglagesApp>) -> OngletReglages {
    let r = reglages.clone();
    OngletReglages::new("editor", "Editor", "pencil", SectionOnglet::Options)
        .avec_definitions(Rc::new(move |_theme| definitions_editor(&r)))
}
