// Port de app/e2e/theme-plugin.spec.ts (test 2 : l'accent CHOISI l'emporte, vide = celui du thème)
// et de app/e2e/light-theme.spec.ts (schéma clair / sombre), vus depuis l'onglet Appearance.
//
//! Tests headless des réglages de thèmes : liste déroulante, accent, schéma. On lit les définitions
//! déclaratives que la fenêtre rend, on déclenche leurs callbacks, et on regarde le `Global`.

#![allow(clippy::unwrap_used, clippy::expect_used)] // fichier de tests

use std::cell::Cell;
use std::rc::Rc;

use fragment_core::{
    Apparence, DataAdapter, Horloge, MemoryAdapter, SchemaCouleurs, SettingColorControl,
    SettingControl, SettingDefinition, SettingDropdownControl, StockageMemoire,
};
use fragment_theme::{Couleur, Mode, ThemeManager, ThemesCoffre};
use fragment_ui::reglages::apparence_themes::{
    appliquer_schema, definitions_apparence_themes, mode_du_schema,
};
use fragment_ui::theme_global::theme;
use fragment_ui::themes::monter;
use gpui_kit::TestAppContext;

const ROSE: &str = r##"{ "--color-accent": "#e8559a", "--background-primary": "#2a1620" }"##;
const VERT: &str = r##"{ "--color-accent": "#22aa44" }"##;

struct Banc {
    apparence: Rc<Apparence>,
    themes: Rc<ThemesCoffre>,
    schemas: Rc<Cell<Option<SchemaCouleurs>>>,
}

fn banc() -> Banc {
    let disque = Rc::new(MemoryAdapter::new());
    disque.put(".fragment/themes/Rose/theme.json", ROSE);
    disque.put(".fragment/themes/Vert/theme.json", VERT);
    let horloge: Horloge = Rc::new(|| 0);
    let apparence = Apparence::new(Rc::new(StockageMemoire::default()), horloge.clone());
    apparence.charger();
    let adaptateur: Rc<dyn DataAdapter> = disque;
    let themes = ThemesCoffre::new(
        adaptateur,
        ".fragment",
        apparence.clone(),
        horloge,
        Rc::new(|_| {}),
        Rc::new(|_| Ok(())),
    );
    let _ = themes.lier_apparence();
    let _ = themes.refresh();
    Banc {
        apparence,
        themes,
        schemas: Rc::new(Cell::new(None)),
    }
}

impl Banc {
    fn defs(&self, mode: Mode) -> Vec<SettingDefinition> {
        let s = self.schemas.clone();
        definitions_apparence_themes(
            &self.apparence,
            &self.themes,
            mode,
            Rc::new(move |v| s.set(Some(v))),
        )
    }

    fn ligne(&self, nom: &str) -> Vec<SettingControl> {
        self.defs(Mode::Sombre)
            .into_iter()
            .find_map(|d| match d {
                SettingDefinition::Control(c) if c.name == nom => Some(vec![c.control]),
                SettingDefinition::Item(i) if i.name == nom => Some(i.controls),
                _ => None,
            })
            .expect("ligne de réglage")
    }

    fn liste_deroulante(&self, nom: &str) -> SettingDropdownControl {
        self.ligne(nom)
            .into_iter()
            .find_map(|c| match c {
                SettingControl::Dropdown(d) => Some(d),
                _ => None,
            })
            .expect("liste déroulante")
    }

    fn accent(&self, mode: Mode) -> (SettingColorControl, Rc<dyn Fn()>) {
        let item = self
            .defs(mode)
            .into_iter()
            .find_map(|d| match d {
                SettingDefinition::Item(i) if i.name == "Accent color" => Some(i),
                _ => None,
            })
            .expect("ligne accent");
        let mut couleur = None;
        let mut reset = None;
        for c in item.controls {
            match c {
                SettingControl::Color(c) => couleur = Some(c),
                SettingControl::ExtraButton(b) => reset = Some(b.on_click),
                _ => {}
            }
        }
        (couleur.expect("couleur"), reset.expect("reset"))
    }
}

#[test]
fn la_liste_des_themes_propose_default_puis_ceux_du_coffre() {
    let b = banc();
    let d = b.liste_deroulante("Themes");
    let valeurs: Vec<&str> = d.options.iter().map(|(v, _)| v.as_str()).collect();
    assert_eq!(valeurs, ["", "Rose", "Vert"]);
    assert_eq!(d.options[0].1, "Default");
    assert_eq!(d.value, "");
}

#[test]
fn choisir_un_theme_pose_sa_palette_et_ecrit_le_reglage() {
    let b = banc();
    let mut cx = TestAppContext::single();
    cx.update(|cx| monter(cx, &b.themes));
    let d = b.liste_deroulante("Themes");
    (d.on_change.expect("on_change"))("Rose".into());
    assert_eq!(b.apparence.theme_css(), "Rose");
    assert_eq!(b.liste_deroulante("Themes").value, "Rose");
    assert_eq!(
        b.themes.theme(Mode::Sombre).palette.background_primary,
        Couleur::hex(0x2a1620)
    );
}

#[test]
fn les_boutons_relire_et_ouvrir_ne_font_pas_tomber_la_ligne() {
    let b = banc();
    let boutons: Vec<_> = b
        .ligne("Themes")
        .into_iter()
        .filter_map(|c| match c {
            SettingControl::ExtraButton(b) => Some(b.on_click),
            _ => None,
        })
        .collect();
    assert_eq!(boutons.len(), 2);
    for f in boutons {
        f();
    }
}

#[test]
fn laccent_choisi_lemporte_sur_celui_du_theme() {
    let b = banc();
    b.apparence.set_theme_css("Vert");
    let (c, _) = b.accent(Mode::Sombre);
    assert_eq!(c.value, "#22aa44", "non choisi : celui du thème");
    (c.on_change.expect("on_change"))("#e8559a".into());
    assert_eq!(b.apparence.accent(), "#e8559a");
    assert_eq!(
        b.themes.theme(Mode::Sombre).palette.color_accent,
        Couleur::hex(0xe8559a)
    );
    let (c, _) = b.accent(Mode::Sombre);
    assert_eq!(c.value, "#e8559a", "le sélecteur montre le choix");
}

#[test]
fn un_accent_vide_ramene_celui_du_theme() {
    let b = banc();
    b.apparence.set_theme_css("Vert");
    let (c, reset) = b.accent(Mode::Sombre);
    (c.on_change.expect("on_change"))("#e8559a".into());
    reset();
    assert_eq!(b.apparence.accent(), "");
    assert_eq!(
        b.themes.theme(Mode::Sombre).palette.color_accent,
        Couleur::hex(0x22aa44)
    );
    assert_eq!(b.accent(Mode::Sombre).0.value, "#22aa44");
}

#[test]
fn le_schema_ecrit_le_reglage_et_previent_lappelant() {
    let b = banc();
    let d = b.liste_deroulante("Base color scheme");
    assert_eq!(d.value, "system");
    (d.on_change.expect("on_change"))("light".into());
    assert_eq!(b.apparence.schema(), SchemaCouleurs::Light);
    assert_eq!(b.schemas.get(), Some(SchemaCouleurs::Light));
}

#[test]
fn le_schema_clair_ou_sombre_garde_le_theme_du_coffre() {
    assert_eq!(mode_du_schema(SchemaCouleurs::System, true), Mode::Sombre);
    assert_eq!(mode_du_schema(SchemaCouleurs::System, false), Mode::Clair);
    assert_eq!(mode_du_schema(SchemaCouleurs::Light, true), Mode::Clair);
    assert_eq!(mode_du_schema(SchemaCouleurs::Dark, false), Mode::Sombre);

    let b = banc();
    b.apparence.set_theme_css("Rose");
    let mut cx = TestAppContext::single();
    cx.update(|cx| monter(cx, &b.themes));
    cx.update(|cx| appliquer_schema(cx, &b.themes, SchemaCouleurs::Light, true));
    let (mode, accent) = cx.update(|cx| (theme(cx).mode, theme(cx).palette.color_accent));
    assert_eq!(mode, Mode::Clair);
    assert_eq!(accent, Couleur::hex(0xe8559a), "le thème du coffre reste");
    cx.update(|cx| appliquer_schema(cx, &b.themes, SchemaCouleurs::Dark, false));
    assert_eq!(cx.update(|cx| theme(cx).mode), Mode::Sombre);
}
