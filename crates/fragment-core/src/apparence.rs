// Port de app/src/ui/core/settings/config.ts (`AppearanceConfig`, `DEFAULT_APPEARANCE`,
// `applyAppearance`) et de l'onglet « Appearance » de app/src/ui/settings/SettingsPlugin.ts
// (`AppearanceTab.getSettingDefinitions`).
//
// L'APPARENCE — `appearance.json` : accent, taille de police des notes, zoom de TOUTE
// l'interface, schéma de couleurs, thème CSS du coffre.
//
// ★ UN SEUL chemin d'écriture du zoom : commande / curseur / remise à zéro appellent
//   `Apparence::set_zoom` (`appearance.set('zoom')`) → événement `change` → `appliquer_apparence`
//   → `WindowHost::set_zoom`. Personne ne pilote la fenêtre en direct : sinon le réglage et
//   l'écran divergent.
// ★ Le zoom est TOUJOURS lu par `normaliser_zoom_valeur` : le fichier est éditable à la main
//   (`"abc"` → 100 %, `10` → 200 %, `-1` → 50 %).
// ★ ÉCARTS : l'application de l'accent, du schéma et de la taille de police (le CSS du TS : variables
//   sur `body` / `documentElement`) est une affaire de rendu, donc derrière `EffetsApparence`
//   (fragment-ui). Le contrat `theme:manager@1` (liste des thèmes du coffre) arrive en paramètre de
//   `definitions_appearance`, le cœur ne connaît pas le plugin `theme`.

use std::rc::{Rc, Weak};

use serde_json::{Value, json};

use crate::config::{ConfigStore, Donnees, Horloge, OrigineChangement, StockageConfig};
use crate::events::EventRef;
use crate::settings::{
    SettingColorControl, SettingControl, SettingDefinition, SettingDefinitionBase,
    SettingDefinitionControl, SettingDefinitionGroup, SettingDefinitionItem,
    SettingDropdownControl, SettingExtraButtonControl, SettingSliderControl,
};
use crate::theme_setting::{SchemaCouleurs, accent_choisi};
use crate::window_host::WindowHost;
use crate::zoom::{ZOOM_DEFAUT, ZOOM_MAX, ZOOM_MIN, format_zoom, normaliser_zoom_valeur};

/// Le fichier de l'apparence, dans `.fragment/`.
pub const FICHIER_APPEARANCE: &str = "appearance.json";

/// Taille de police par défaut des notes, en px.
pub const TAILLE_POLICE_DEFAUT: f64 = 16.0;

/// Les valeurs par défaut (`DEFAULT_APPEARANCE`).
pub fn defauts_apparence() -> Donnees {
    let mut d = Donnees::new();
    // '' = l'accent du thème (comme Obsidian) ; voir `accent_choisi`.
    d.insert("accentColor".into(), json!(""));
    d.insert("fontSize".into(), json!(TAILLE_POLICE_DEFAUT));
    d.insert("zoom".into(), json!(ZOOM_DEFAUT));
    d.insert("baseColorScheme".into(), json!("system"));
    d.insert("cssTheme".into(), json!(""));
    d
}

/// Ce que `applyAppearance` écrit dans le DOM et que GPUI rend autrement.
pub trait EffetsApparence {
    /// L'accent choisi ; `None` = celui du thème.
    fn accent(&self, couleur: Option<&str>);
    /// La taille de police des notes (`--font-text-size`), en px.
    fn taille_police(&self, px: f64);
    /// Le schéma de couleurs demandé (`system` suit l'OS).
    fn schema(&self, schema: SchemaCouleurs);
}

/// `appearance.json`, avec ses accesseurs typés.
pub struct Apparence {
    magasin: ConfigStore,
}

impl std::fmt::Debug for Apparence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Apparence")
            .field("zoom", &self.zoom())
            .finish_non_exhaustive()
    }
}

impl Apparence {
    /// L'apparence d'un coffre ; [`Self::charger`] lit le fichier.
    pub fn new(stockage: Rc<dyn StockageConfig>, horloge: Horloge) -> Rc<Self> {
        Rc::new(Self {
            magasin: ConfigStore::new(FICHIER_APPEARANCE, defauts_apparence(), stockage, horloge),
        })
    }

    /// Le magasin sous-jacent (relais, sauvegarde).
    pub fn magasin(&self) -> &ConfigStore {
        &self.magasin
    }

    pub fn charger(&self) {
        self.magasin.charger();
    }

    /// Le zoom de l'interface (facteur, 1 = 100 %), ramené à une valeur sûre.
    pub fn zoom(&self) -> f64 {
        normaliser_zoom_valeur(self.magasin.get("zoom").as_ref())
    }

    /// Le SEUL point d'écriture du zoom.
    pub fn set_zoom(&self, facteur: f64) {
        self.magasin.set("zoom", json!(facteur));
    }

    /// L'accent choisi, ou `""` = celui du thème.
    pub fn accent(&self) -> String {
        let v = self.magasin.get("accentColor");
        accent_choisi(v.as_ref().and_then(Value::as_str))
    }

    pub fn set_accent(&self, couleur: &str) {
        self.magasin.set("accentColor", json!(couleur));
    }

    /// Taille de police des notes, en px (valeur illisible → 16).
    pub fn taille_police(&self) -> f64 {
        self.magasin
            .get("fontSize")
            .and_then(|v| v.as_f64())
            .filter(|n| n.is_finite())
            .unwrap_or(TAILLE_POLICE_DEFAUT)
    }

    pub fn set_taille_police(&self, px: f64) {
        self.magasin.set("fontSize", json!(px));
    }

    /// `applyAppearance` : tout ce qui n'est ni `dark` ni `light` retombe sur `system`.
    pub fn schema(&self) -> SchemaCouleurs {
        let v = self.magasin.get("baseColorScheme");
        SchemaCouleurs::depuis_reglage(v.as_ref().and_then(Value::as_str))
    }

    pub fn set_schema(&self, valeur: &str) {
        self.magasin.set("baseColorScheme", json!(valeur));
    }

    /// Le thème CSS du coffre (`""` = celui de Fragment).
    pub fn theme_css(&self) -> String {
        self.magasin
            .get("cssTheme")
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default()
    }

    pub fn set_theme_css(&self, nom: &str) {
        self.magasin.set("cssTheme", json!(nom));
    }
}

/// `applyAppearance` : applique l'apparence à la fenêtre. Au démarrage ET à chaque changement ;
/// idempotent.
pub fn appliquer_apparence(
    apparence: &Apparence,
    hote: &dyn WindowHost,
    effets: Option<&dyn EffetsApparence>,
) {
    if let Some(e) = effets {
        let accent = apparence.accent();
        e.accent((!accent.is_empty()).then_some(accent.as_str()));
        e.taille_police(apparence.taille_police());
        e.schema(apparence.schema());
    }
    // Le zoom : par le pont fenêtre (le cœur ne connaît pas la plateforme). Un échec de l'hôte ne
    // doit pas faire tomber le réglage (le `void` du TS).
    if let Err(e) = hote.set_zoom(apparence.zoom()) {
        log::warn!("[apparence] zoom non appliqué : {e}");
    }
}

/// Branche `change` → `appliquer_apparence` (le `wire` de sync.ts, pour le magasin d'apparence).
/// L'abonnement vit tant que la référence rendue est gardée.
pub fn cabler_apparence(
    apparence: &Rc<Apparence>,
    hote: Rc<dyn WindowHost>,
    effets: Option<Rc<dyn EffetsApparence>>,
) -> EventRef {
    let faible: Weak<Apparence> = Rc::downgrade(apparence);
    apparence
        .magasin()
        .sur_changement(move |_origine: OrigineChangement| {
            if let Some(a) = faible.upgrade() {
                appliquer_apparence(&a, hote.as_ref(), effets.as_deref());
            }
        })
}

// ── L'onglet « Appearance » : les définitions déclaratives ─────────────────

/// Le choix du thème CSS du coffre (contrat `theme:manager@1`) : absent, pas de ligne.
#[derive(Clone)]
pub struct ThemesDefinition {
    /// Les thèmes du coffre (le thème « Default » est ajouté devant).
    pub liste: Vec<String>,
    /// ↻ relire le dossier des thèmes.
    pub relire: Rc<dyn Fn()>,
    /// 📂 ouvrir le dossier des thèmes.
    pub ouvrir_dossier: Rc<dyn Fn()>,
}

fn base(id: Option<&str>) -> SettingDefinitionBase {
    SettingDefinitionBase {
        id: id.map(str::to_owned),
    }
}

fn bouton_reset(action: impl Fn() + 'static) -> SettingControl {
    SettingControl::ExtraButton(SettingExtraButtonControl {
        icon: "rotate-ccw".into(),
        tooltip: Some("Restore default".into()),
        on_click: Rc::new(action),
    })
}

/// `AppearanceTab.getSettingDefinitions` : les réglages de l'onglet, en données.
///
/// `accent_du_theme` : ce que montre le sélecteur quand aucun accent n'est choisi (`''`).
pub fn definitions_appearance(
    apparence: &Rc<Apparence>,
    accent_du_theme: &str,
    themes: Option<ThemesDefinition>,
) -> Vec<SettingDefinition> {
    let mut defs = Vec::new();

    let a = apparence.clone();
    defs.push(SettingDefinition::Control(SettingDefinitionControl {
        base: base(None),
        name: "Base color scheme".into(),
        description: Some("Choose the default color scheme.".into()),
        control: SettingControl::Dropdown(SettingDropdownControl {
            value: schema_en_texte(apparence.schema()).into(),
            options: vec![
                ("system".into(), "Adapt to system".into()),
                ("dark".into(), "Dark".into()),
                ("light".into(), "Light".into()),
            ],
            // Persisté ; l'effet (surcharge du prefers-color-scheme) vient avec `EffetsApparence`.
            on_change: Some(Rc::new(move |v: String| a.set_schema(&v))),
        }),
    }));

    // Sans gestionnaire de thèmes (plugin `theme` absent), pas de ligne.
    if let Some(t) = themes {
        defs.push(ligne_themes(apparence, &t));
    }

    let (a, a2) = (apparence.clone(), apparence.clone());
    let accent = apparence.accent();
    defs.push(SettingDefinition::Item(SettingDefinitionItem {
        base: base(None),
        name: "Accent color".into(),
        description: Some("Choose the accent color used throughout the app.".into()),
        // Non choisi (`""`), le sélecteur montre l'accent du THÈME ; ↺ y revient.
        controls: vec![
            SettingControl::Color(SettingColorControl {
                value: if accent.is_empty() {
                    accent_du_theme.to_owned()
                } else {
                    accent
                },
                on_change: Some(Rc::new(move |v: String| a.set_accent(&v))),
            }),
            bouton_reset(move || a2.set_accent("")),
        ],
    }));

    let a = apparence.clone();
    defs.push(SettingDefinition::Group(SettingDefinitionGroup {
        base: base(None),
        name: "Font".into(),
        items: vec![SettingDefinition::Control(SettingDefinitionControl {
            base: base(None),
            name: "Font size".into(),
            description: Some("Font size in pixels that affects editing and reading views.".into()),
            control: SettingControl::Slider(SettingSliderControl {
                value: apparence.taille_police(),
                min: 8.0,
                max: 30.0,
                step: Some(1.0),
                instant: None,
                format: None,
                on_change: Some(Rc::new(move |v: f64| a.set_taille_police(v))),
            }),
        })],
    }));

    defs.push(SettingDefinition::Group(SettingDefinitionGroup {
        base: base(None),
        name: "Zoom".into(),
        items: vec![ligne_zoom(apparence)],
    }));
    defs
}

fn schema_en_texte(s: SchemaCouleurs) -> &'static str {
    match s {
        SchemaCouleurs::System => "system",
        SchemaCouleurs::Dark => "dark",
        SchemaCouleurs::Light => "light",
    }
}

fn ligne_themes(apparence: &Rc<Apparence>, t: &ThemesDefinition) -> SettingDefinition {
    let actuel = apparence.theme_css();
    let mut options: Vec<(String, String)> = vec![(String::new(), "Default".into())];
    options.extend(t.liste.iter().map(|n| (n.clone(), n.clone())));
    if !actuel.is_empty() && !options.iter().any(|(v, _)| *v == actuel) {
        options.push((actuel.clone(), format!("{actuel} (missing)")));
    }
    let a = apparence.clone();
    let (relire, ouvrir) = (t.relire.clone(), t.ouvrir_dossier.clone());
    SettingDefinition::Item(SettingDefinitionItem {
        base: base(Some("css-theme")),
        name: "Themes".into(),
        description: Some("A theme is a folder in .fragment/themes with a theme.css.".into()),
        controls: vec![
            SettingControl::Dropdown(SettingDropdownControl {
                value: actuel,
                options,
                on_change: Some(Rc::new(move |v: String| a.set_theme_css(&v))),
            }),
            SettingControl::ExtraButton(SettingExtraButtonControl {
                icon: "refresh-cw".into(),
                tooltip: Some("Reload themes".into()),
                on_click: Rc::new(move || relire()),
            }),
            SettingControl::ExtraButton(SettingExtraButtonControl {
                icon: "folder-open".into(),
                tooltip: Some("Open themes folder".into()),
                on_click: Rc::new(move || ouvrir()),
            }),
        ],
    })
}

/// La ligne « Zoom level » : curseur en pourcents appliqué AU RELÂCHÉ (`instant: false`) et ↺.
///
/// ★ Au relâché : le zoom s'applique à toute la fenêtre, celle des réglages comprise ; appliqué
///   pendant le glisser, la piste se redimensionnerait sous le curseur.
pub fn ligne_zoom(apparence: &Rc<Apparence>) -> SettingDefinition {
    let (a, a2) = (apparence.clone(), apparence.clone());
    SettingDefinition::Item(SettingDefinitionItem {
        base: base(Some("zoom")),
        name: "Zoom level".into(),
        description: Some(
            "Scales the whole interface (panels, tabs, menus, text). Ctrl+= / Ctrl+- / Ctrl+0."
                .into(),
        ),
        controls: vec![
            SettingControl::Slider(SettingSliderControl {
                value: (apparence.zoom() * 100.0).round(),
                min: ZOOM_MIN * 100.0,
                max: ZOOM_MAX * 100.0,
                step: Some(5.0),
                instant: Some(false),
                format: Some(Rc::new(|v: f64| format_zoom(v / 100.0))),
                on_change: Some(Rc::new(move |v: f64| {
                    a.set_zoom(crate::zoom::normaliser_zoom(v / 100.0));
                })),
            }),
            bouton_reset(move || a2.set_zoom(ZOOM_DEFAUT)),
        ],
    })
}
