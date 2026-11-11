// Port de app/e2e/theme-plugin.spec.ts (palette du thème posée partout, accent choisi, thème
// retiré) : on lit le `Global` GPUI que toutes les vues lisent, à la place des styles calculés du DOM.
//
//! Tests headless du montage des thèmes du coffre : le thème installé, la modification à chaud
//! sans rechargement, le retour à la palette de Fragment, le changement de mode.

#![allow(clippy::unwrap_used, clippy::expect_used)] // fichier de tests

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use fragment_core::{Apparence, DataAdapter, Horloge, MemoryAdapter, StockageMemoire};
use fragment_theme::{Couleur, Mode, Theme, ThemesCoffre};
use fragment_ui::theme_global::{self, theme};
use fragment_ui::themes::{PERIODE_MS, basculer_avec_themes, monter};
use gpui_kit::TestAppContext;

const ROSE: &str = r##"{ "--color-accent": "#e8559a", "--background-primary": "#2a1620" }"##;
const ROSE_V2: &str = r##"{ "--color-accent": "#e8559a", "--background-primary": "#301828" }"##;

struct Banc {
    disque: Rc<MemoryAdapter>,
    apparence: Rc<Apparence>,
    themes: Rc<ThemesCoffre>,
    temps: Rc<Cell<u64>>,
}

fn banc() -> Banc {
    let disque = Rc::new(MemoryAdapter::new());
    let temps = Rc::new(Cell::new(0_u64));
    let t = temps.clone();
    let horloge: Horloge = Rc::new(move || t.get());
    let apparence = Apparence::new(Rc::new(StockageMemoire::default()), horloge.clone());
    apparence.charger();
    let adaptateur: Rc<dyn DataAdapter> = disque.clone();
    let themes = ThemesCoffre::new(
        adaptateur,
        ".fragment",
        apparence.clone(),
        horloge,
        Rc::new(|_| {}),
        Rc::new(|_| Ok(())),
    );
    let _ = themes.lier_apparence();
    Banc {
        disque,
        apparence,
        themes,
        temps,
    }
}

/// Le temps passe : l'horloge du cœur ET celle de l'exécuteur de test.
fn attendre(b: &Banc, cx: &mut TestAppContext, ms: u64) {
    b.temps.set(b.temps.get() + ms);
    cx.executor().advance_clock(Duration::from_millis(ms));
    cx.run_until_parked();
}

fn fond(cx: &mut TestAppContext) -> Couleur {
    cx.update(|cx| theme(cx).palette.background_primary)
}

#[test]
fn le_theme_du_coffre_est_installe_dans_le_global_au_montage() {
    let b = banc();
    b.disque.put(".fragment/themes/Rose/theme.json", ROSE);
    b.apparence.set_theme_css("Rose");
    let mut cx = TestAppContext::single();
    cx.update(|cx| monter(cx, &b.themes));
    assert_eq!(fond(&mut cx), Couleur::hex(0x2a1620));
    let accent = cx.update(|cx| theme(cx).palette.color_accent);
    assert_eq!(accent, Couleur::hex(0xe8559a));
}

#[test]
fn modification_a_chaud_le_global_suit_sans_rechargement() {
    let b = banc();
    b.disque.put(".fragment/themes/Rose/theme.json", ROSE);
    b.apparence.set_theme_css("Rose");
    let mut cx = TestAppContext::single();
    cx.update(|cx| monter(cx, &b.themes));
    assert_eq!(fond(&mut cx), Couleur::hex(0x2a1620));

    b.disque.put(".fragment/themes/Rose/theme.json", ROSE_V2);
    b.themes
        .sur_evenement_brut(".fragment/themes/Rose/theme.json");
    // Le regroupement n'est pas écoulé : rien n'a bougé.
    attendre(&b, &mut cx, 10);
    assert_eq!(fond(&mut cx), Couleur::hex(0x2a1620));
    attendre(&b, &mut cx, 2 * PERIODE_MS);
    assert_eq!(fond(&mut cx), Couleur::hex(0x301828));
}

#[test]
fn le_theme_retire_retour_a_la_palette_de_fragment() {
    let b = banc();
    b.disque.put(".fragment/themes/Rose/theme.json", ROSE);
    b.apparence.set_theme_css("Rose");
    let mut cx = TestAppContext::single();
    cx.update(|cx| monter(cx, &b.themes));
    assert_ne!(fond(&mut cx), Couleur::hex(0x141517));

    b.disque.retirer(".fragment/themes/Rose/theme.json");
    b.themes
        .sur_evenement_brut(".fragment/themes/Rose/theme.json");
    attendre(&b, &mut cx, 3 * PERIODE_MS);
    assert_eq!(fond(&mut cx), Couleur::hex(0x141517));
    assert_eq!(cx.update(|cx| theme(cx).clone()), Theme::sombre());
}

#[test]
fn un_accent_choisi_est_redessine_dans_le_global() {
    let b = banc();
    let mut cx = TestAppContext::single();
    cx.update(|cx| monter(cx, &b.themes));
    b.apparence.set_accent("#00aa55");
    attendre(&b, &mut cx, 2 * PERIODE_MS);
    let accent = cx.update(|cx| theme(cx).palette.color_accent);
    assert_eq!(accent, Couleur::hex(0x00aa55));
    b.apparence.set_accent("");
    attendre(&b, &mut cx, 2 * PERIODE_MS);
    assert_eq!(
        cx.update(|cx| theme(cx).palette.color_accent),
        Couleur::hex(0x646cff)
    );
}

#[test]
fn le_mode_clair_garde_le_theme_du_coffre() {
    let b = banc();
    b.disque.put(".fragment/themes/Rose/theme.json", ROSE);
    b.apparence.set_theme_css("Rose");
    let mut cx = TestAppContext::single();
    cx.update(|cx| monter(cx, &b.themes));
    cx.update(|cx| basculer_avec_themes(cx, &b.themes, Mode::Clair));
    let (mode, fond) = cx.update(|cx| (theme(cx).mode, theme(cx).palette.background_primary));
    assert_eq!(mode, Mode::Clair);
    assert_eq!(fond, Couleur::hex(0x2a1620));
    // Sans le thème du coffre, le basculement d'origine repart de la palette de Fragment.
    cx.update(|cx| theme_global::basculer(cx, Mode::Sombre));
    assert_eq!(self::fond(&mut cx), Couleur::hex(0x141517));
}
