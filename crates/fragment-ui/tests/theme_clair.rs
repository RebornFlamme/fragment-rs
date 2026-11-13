// Port de app/e2e/light-theme.spec.ts (curseur, menus, contour de focus, contre-épreuve en sombre) :
// le DOM n'existant pas, on lit les mêmes grandeurs là où le rendu les prend — le style projeté sur
// l'éditeur (`style_projete`) et les jetons du menu (`menu_bg`, `text_normal`, ADR-005).
//
//! Test headless du thème clair : curseur visible, menus clairs et lisibles, pas de cadre de focus ;
//! sombre inchangé. Aucune fenêtre ni GPU.

#![allow(clippy::unwrap_used, clippy::expect_used)] // fichier de tests

use fragment_theme::{Aplat, Couleur, Theme};
use fragment_ui::editeur::style_projete;
use gpui_kit::Hsla;

/// Luminance relative (0 = noir, 1 = blanc) d'une couleur opaque, formule WCAG.
fn luminance(c: Couleur) -> f64 {
    let lin = |v: u8| {
        let v = f64::from(v) / 255.0;
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(c.r) + 0.7152 * lin(c.g) + 0.0722 * lin(c.b)
}

fn contraste(a: Couleur, b: Couleur) -> f64 {
    let (haut, bas) = if luminance(a) >= luminance(b) {
        (luminance(a), luminance(b))
    } else {
        (luminance(b), luminance(a))
    };
    (haut + 0.05) / (bas + 0.05)
}

/// Luminance d'une teinte GPUI (le curseur du style projeté), via ses canaux sRGB.
fn luminance_hsla(h: Hsla) -> f64 {
    let rgba = gpui_kit::Rgba::from(h);
    let octet = |v: f32| (v * 255.0).round() as u8;
    luminance(Couleur::octets(
        octet(rgba.r),
        octet(rgba.g),
        octet(rgba.b),
        255,
    ))
}

#[test]
fn theme_clair_curseur_visible_menus_clairs_et_lisibles_pas_de_cadre_de_focus_sombre_inchange() {
    let clair = Theme::clair();
    let fond_note = clair.palette.background_primary;
    let style = style_projete(&clair);

    // Curseur : sombre, et lisible sur le fond de la note (il était `white`).
    assert!(luminance_hsla(style.caret) < 0.2);
    assert_eq!(style.caret.a, 1.0);
    let caret = clair.palette.text_normal;
    assert!(luminance(caret) < 0.2);
    assert!(contraste(caret, fond_note) > 4.5);

    // Pas de cadre de focus : la seule bordure projetée est l'aplat discret des jetons, jamais le
    // `#212121` pointillé de CodeMirror (noir en clair).
    let bordure = clair.palette.aplat(Aplat::Bordure, fond_note);
    assert!(luminance(bordure) > 0.7);

    // Menus : fond clair et texte lisible (il était `#2a2a2a`).
    let menu = clair.palette.menu_bg;
    assert!(luminance(menu) > 0.8);
    assert!(contraste(clair.palette.text_normal, menu) > 4.5);

    // Contre-épreuve en sombre : curseur clair, menu `#1c1d21`, texte lisible.
    let sombre = Theme::sombre();
    let fond_sombre = sombre.palette.background_primary;
    let style_sombre = style_projete(&sombre);
    assert!(luminance_hsla(style_sombre.caret) > 0.5);
    assert!(luminance(sombre.palette.text_normal) > 0.5);
    assert!(contraste(sombre.palette.text_normal, fond_sombre) > 4.5);
    assert_eq!(sombre.palette.menu_bg, Couleur::hex(0x001c_1d21));
    assert!(contraste(sombre.palette.text_normal, sombre.palette.menu_bg) > 4.5);
}
