// Port de app/src/ui/settings/SettingsPlugin.ts (l'onglet Appearance : Themes, accent, schéma)
// et de app/src/ui/core/settings/theme.ts (le schéma effectif)
//
//! L'onglet Appearance, côté thèmes : les définitions déclaratives des réglages « Base color
//! scheme », « Themes » et « Accent color », branchées sur le [`ThemesCoffre`].
//!
//! ★ Un seul chemin d'écriture : un réglage écrit dans l'[`Apparence`] (`appearance.json`), et
//!   c'est le `ThemesCoffre` (via `lier_apparence`) qui réapplique le thème. Ce module ne touche
//!   jamais le `Global` de lui-même, sauf pour le schéma, qui n'est pas l'affaire du coffre.
//! ★ Les callbacks des réglages n'ont pas de `cx` : un changement de schéma est rendu à l'appelant
//!   (`apres_schema`), qui le pose avec [`appliquer_schema`] quand il a la main.

use std::rc::Rc;

use fragment_core::{
    Apparence, SchemaCouleurs, SchemaEffectif, SettingControl, SettingDefinition, ThemesDefinition,
    definitions_appearance, schema_effectif,
};
use fragment_theme::{Mode, ThemeManager, ThemesCoffre};
use gpui_kit::App;

use super::fenetre::hex_de;
use crate::themes::basculer_avec_themes;

/// Le libellé de la ligne du schéma de couleurs.
const NOM_SCHEMA: &str = "Base color scheme";

/// Le mode du thème pour un schéma (`system` suit l'OS).
pub fn mode_du_schema(schema: SchemaCouleurs, os_sombre: bool) -> Mode {
    match schema_effectif(schema, os_sombre) {
        SchemaEffectif::Dark => Mode::Sombre,
        SchemaEffectif::Light => Mode::Clair,
    }
}

/// Les réglages de l'onglet Appearance, thèmes compris.
///
/// - `mode` : le mode affiché ; l'accent montré quand aucun n'est choisi est celui du thème dans
///   ce mode ;
/// - `apres_schema` : appelé avec le schéma choisi, après son écriture dans l'apparence.
pub fn definitions_apparence_themes(
    apparence: &Rc<Apparence>,
    themes: &Rc<ThemesCoffre>,
    mode: Mode,
    apres_schema: Rc<dyn Fn(SchemaCouleurs)>,
) -> Vec<SettingDefinition> {
    let (relire, ouvrir) = (themes.clone(), themes.clone());
    let definition_themes = ThemesDefinition {
        liste: themes.list(),
        // ★ Un échec de lecture ou d'ouverture n'est pas une panne du réglage : la ligne reste.
        relire: Rc::new(move || {
            let _ = relire.refresh();
        }),
        ouvrir_dossier: Rc::new(move || {
            let _ = ouvrir.open_folder();
        }),
    };
    let accent = hex_de(themes.theme(mode).palette.color_accent);
    let mut defs = definitions_appearance(apparence, &accent, Some(definition_themes));
    for d in &mut defs {
        if let SettingDefinition::Control(c) = d
            && c.name == NOM_SCHEMA
            && let SettingControl::Dropdown(dd) = &mut c.control
        {
            let ecrire = dd.on_change.take();
            let suite = apres_schema.clone();
            dd.on_change = Some(Rc::new(move |v: String| {
                if let Some(e) = &ecrire {
                    e(v.clone());
                }
                suite(SchemaCouleurs::depuis_reglage(Some(&v)));
            }));
        }
    }
    defs
}

/// Pose le schéma de couleurs sur l'écran en gardant le thème du coffre.
pub fn appliquer_schema(
    cx: &mut App,
    themes: &ThemesCoffre,
    schema: SchemaCouleurs,
    os_sombre: bool,
) {
    basculer_avec_themes(cx, themes, mode_du_schema(schema, os_sombre));
}
