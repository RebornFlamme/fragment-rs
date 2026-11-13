// Port de app/src/ui/core/settings/components/ColorComponent.ts (le comportement : une valeur
// `#rrggbb` qui change à chaque geste, `input` et non `change`).
//! Le sélecteur de couleur libre d'un réglage (l'accent).
//!
//! ★ Fragment utilise `<input type="color">`, le sélecteur NATIF de Chromium : GPUI n'en a pas, et
//! `gpui-component::ColorPicker` (gpui-kit 0.9.1) exige la feature `component` et son propre thème
//! (hors de nos jetons, ADR-001). On dessine donc un sélecteur minimal équivalent : un carré
//! saturation/luminosité et une barre de teinte, qui rendent un `#rrggbb` quelconque. L'écart est
//! noté dans `parite.toml` ; la fenêtre native n'est pas imitée.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use fragment_theme::Couleur;
use gpui_kit::{
    App, Bounds, Context, Div, InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent,
    MouseMoveEvent, ParentElement as _, Pixels, SharedString, Styled, Window, canvas, div,
    linear_color_stop, linear_gradient,
};

use super::fenetre::{couleur_depuis_hex, hex_de};
use crate::echelle::u;
use crate::flottant::{arrondir, bordure};
use crate::theme_global::couleur;

/// Largeur du carré saturation/luminosité et de la barre de teinte (px logiques).
pub const LARGEUR_CARRE: f32 = 176.0;
/// Hauteur du carré.
pub const HAUTEUR_CARRE: f32 = 112.0;
/// Hauteur de la barre de teinte.
pub const HAUTEUR_TEINTE: f32 = 14.0;
/// Diamètre du repère (anneau).
const REPERE: f32 = 10.0;

/// Une couleur en teinte (0..360), saturation et valeur (0..1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hsv {
    pub teinte: f32,
    pub saturation: f32,
    pub valeur: f32,
}

impl Hsv {
    /// Depuis un `#rrggbb` (une valeur illisible donne le noir).
    pub fn depuis_hex(texte: &str) -> Self {
        let c = couleur_depuis_hex(texte);
        Self::depuis_rvb(c.r, c.g, c.b)
    }

    pub fn depuis_rvb(r: u8, g: u8, b: u8) -> Self {
        let (r, g, b) = (
            f32::from(r) / 255.0,
            f32::from(g) / 255.0,
            f32::from(b) / 255.0,
        );
        let max = r.max(g).max(b);
        let d = max - r.min(g).min(b);
        let teinte = if d <= f32::EPSILON {
            0.0
        } else if (max - r).abs() <= f32::EPSILON {
            60.0 * ((g - b) / d).rem_euclid(6.0)
        } else if (max - g).abs() <= f32::EPSILON {
            60.0 * ((b - r) / d + 2.0)
        } else {
            60.0 * ((r - g) / d + 4.0)
        };
        Self {
            teinte,
            saturation: if max <= f32::EPSILON { 0.0 } else { d / max },
            valeur: max,
        }
    }

    /// Les canaux 8 bits (arrondi au plus proche).
    pub fn en_rvb(self) -> (u8, u8, u8) {
        let c = self.valeur * self.saturation;
        let h = self.teinte.rem_euclid(360.0) / 60.0;
        let x = c * (1.0 - (h % 2.0 - 1.0).abs());
        let (r, g, b) = match h as u32 {
            0 => (c, x, 0.0),
            1 => (x, c, 0.0),
            2 => (0.0, c, x),
            3 => (0.0, x, c),
            4 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };
        let m = self.valeur - c;
        let o = |v: f32| ((v + m) * 255.0 + 0.5).floor().clamp(0.0, 255.0) as u8;
        (o(r), o(g), o(b))
    }

    pub fn en_hex(self) -> String {
        let (r, g, b) = self.en_rvb();
        hex_de(Couleur::octets(r, g, b, 255))
    }

    /// La couleur pleine de cette teinte (saturation et valeur à 1).
    pub fn teinte_pleine(teinte: f32) -> Couleur {
        let (r, g, b) = Self {
            teinte,
            saturation: 1.0,
            valeur: 1.0,
        }
        .en_rvb();
        Couleur::octets(r, g, b, 255)
    }
}

/// Fraction (0..1) de `p` dans `[debut, debut + taille]`.
pub fn fraction(p: f32, debut: f32, taille: f32) -> f32 {
    if taille <= 0.0 {
        return 0.0;
    }
    ((p - debut) / taille).clamp(0.0, 1.0)
}

/// Le résultat d'un geste dans le carré : saturation à droite, valeur en haut.
pub fn dans_carre(teinte: f32, fx: f32, fy: f32) -> Hsv {
    Hsv {
        teinte,
        saturation: fx,
        valeur: 1.0 - fy,
    }
}

/// Boîtes relevées à la peinture (clé → boîte fenêtre).
pub type Bornes = Rc<RefCell<HashMap<String, Bounds<Pixels>>>>;
/// Appelé avec le nouvel hex à chaque geste.
pub type Appliquer<T> = Rc<dyn Fn(String, &mut Context<T>)>;

/// La position d'un pointeur en fractions (0..1) de la boîte relevée sous `cle`.
fn fractions(bornes: &Bornes, cle: &str, x: f32, y: f32) -> Option<(f32, f32)> {
    let r = *bornes.borrow().get(cle)?;
    Some((
        fraction(x, f32::from(r.origin.x), f32::from(r.size.width)),
        fraction(y, f32::from(r.origin.y), f32::from(r.size.height)),
    ))
}

/// Un canevas invisible qui relève la boîte de son parent sous `cle`.
fn releveur(bornes: &Bornes, cle: &str) -> impl IntoElement {
    let (b, k) = (bornes.clone(), cle.to_owned());
    canvas(
        move |bo: Bounds<Pixels>, _: &mut Window, _: &mut App| {
            b.borrow_mut().insert(k.clone(), bo);
        },
        |_, (), _, _| {},
    )
    .absolute()
    .size_full()
}

/// Un anneau blanc de `REPERE` px, centré en (`x`, `y`) dans son parent.
fn repere(cx: &App, x: f32, y: f32) -> impl IntoElement {
    let anneau = div()
        .absolute()
        .left(u(cx, x - REPERE / 2.0))
        .top(u(cx, y - REPERE / 2.0))
        .size(u(cx, REPERE))
        .border_color(couleur(Couleur::BLANC));
    bordure(
        arrondir(anneau, cx, u(cx, REPERE / 2.0), u(cx, 2.0)),
        u(cx, 2.0),
    )
}

/// Les deux zones du sélecteur (carré puis barre de teinte), pour le réglage `cle` de valeur `hex`.
///
/// `teinte_memo` garde la teinte choisie : un gris (saturation nulle) n'en porte plus dans son hex,
/// et la barre sauterait à 0 au premier geste dans le carré.
pub fn zones<T: 'static>(
    cle: &str,
    hex: &str,
    teinte_memo: &Rc<Cell<Option<f32>>>,
    bornes: &Bornes,
    appliquer: &Appliquer<T>,
    cx: &mut Context<T>,
) -> Div {
    let hsv = Hsv::depuis_hex(hex);
    // Le mémo ne vaut que s'il décrit la couleur affichée (sinon, valeur changée ailleurs).
    let teinte = match teinte_memo.get() {
        Some(t) if (Hsv { teinte: t, ..hsv }).en_hex() == hex => t,
        _ => hsv.teinte,
    };
    let (cle_carre, cle_barre) = (format!("sv-{cle}"), format!("teinte-{cle}"));

    // Carré : teinte pleine, voile blanc vers la droite, voile noir vers le bas.
    let blanc_nul = couleur(Couleur::octets(255, 255, 255, 0));
    let noir_nul = couleur(Couleur::octets(0, 0, 0, 0));
    let geste_carre = {
        let (b, k, m, a) = (
            bornes.clone(),
            cle_carre.clone(),
            teinte_memo.clone(),
            appliquer.clone(),
        );
        Rc::new(move |x: f32, y: f32, cx: &mut Context<T>| {
            if let Some((fx, fy)) = fractions(&b, &k, x, y) {
                m.set(Some(teinte));
                a(dans_carre(teinte, fx, fy).en_hex(), cx);
            }
        })
    };
    let (bas, deplace) = (geste_carre.clone(), geste_carre);
    let carre = div()
        .id(SharedString::from(cle_carre.clone()))
        .relative()
        .flex_none()
        .overflow_hidden()
        .w(u(cx, LARGEUR_CARRE))
        .h(u(cx, HAUTEUR_CARRE))
        .cursor_crosshair()
        .bg(couleur(Hsv::teinte_pleine(teinte)))
        .child(releveur(bornes, &cle_carre))
        .child(div().absolute().size_full().bg(linear_gradient(
            90.0,
            linear_color_stop(couleur(Couleur::BLANC), 0.0),
            linear_color_stop(blanc_nul, 1.0),
        )))
        .child(div().absolute().size_full().bg(linear_gradient(
            180.0,
            linear_color_stop(noir_nul, 0.0),
            linear_color_stop(couleur(Couleur::NOIR), 1.0),
        )))
        .child(repere(
            cx,
            LARGEUR_CARRE * hsv.saturation,
            HAUTEUR_CARRE * (1.0 - hsv.valeur),
        ))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |_, ev: &MouseDownEvent, _, cx| {
                bas(f32::from(ev.position.x), f32::from(ev.position.y), cx);
            }),
        )
        .on_mouse_move(cx.listener(move |_, ev: &MouseMoveEvent, _, cx| {
            if ev.dragging() {
                deplace(f32::from(ev.position.x), f32::from(ev.position.y), cx);
            }
        }));

    // Barre de teinte : six segments du cercle chromatique.
    let geste_barre = {
        let (b, k, m, a) = (
            bornes.clone(),
            cle_barre.clone(),
            teinte_memo.clone(),
            appliquer.clone(),
        );
        Rc::new(move |x: f32, y: f32, cx: &mut Context<T>| {
            if let Some((fx, _)) = fractions(&b, &k, x, y) {
                let t = fx * 360.0;
                m.set(Some(t));
                a(Hsv { teinte: t, ..hsv }.en_hex(), cx);
            }
        })
    };
    let (bas, deplace) = (geste_barre.clone(), geste_barre);
    let mut segments = div().absolute().size_full().flex();
    for i in 0..6 {
        let (de, vers) = (i as f32 * 60.0, (i + 1) as f32 * 60.0);
        segments = segments.child(div().flex_1().h_full().bg(linear_gradient(
            90.0,
            linear_color_stop(couleur(Hsv::teinte_pleine(de)), 0.0),
            linear_color_stop(couleur(Hsv::teinte_pleine(vers)), 1.0),
        )));
    }
    let barre = div()
        .id(SharedString::from(cle_barre.clone()))
        .relative()
        .flex_none()
        .overflow_hidden()
        .w(u(cx, LARGEUR_CARRE))
        .h(u(cx, HAUTEUR_TEINTE))
        .cursor_pointer()
        .child(releveur(bornes, &cle_barre))
        .child(segments)
        .child(repere(
            cx,
            LARGEUR_CARRE * teinte / 360.0,
            HAUTEUR_TEINTE / 2.0,
        ))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |_, ev: &MouseDownEvent, _, cx| {
                bas(f32::from(ev.position.x), f32::from(ev.position.y), cx);
            }),
        )
        .on_mouse_move(cx.listener(move |_, ev: &MouseMoveEvent, _, cx| {
            if ev.dragging() {
                deplace(f32::from(ev.position.x), f32::from(ev.position.y), cx);
            }
        }));

    div()
        .flex()
        .flex_col()
        .gap(u(cx, 8.0))
        .p(u(cx, 8.0))
        .child(carre)
        .child(barre)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_hex_fait_l_aller_retour() {
        for h in [
            "#7c3aed", "#ff0000", "#00ff00", "#0000ff", "#123456", "#ffffff", "#000000",
        ] {
            assert_eq!(Hsv::depuis_hex(h).en_hex(), h);
        }
    }

    #[test]
    fn les_primaires_ont_leur_teinte() {
        assert_eq!(Hsv::depuis_hex("#ff0000").teinte, 0.0);
        assert_eq!(Hsv::depuis_hex("#00ff00").teinte, 120.0);
        assert_eq!(Hsv::depuis_hex("#0000ff").teinte, 240.0);
    }

    #[test]
    fn un_gris_n_a_pas_de_saturation() {
        let g = Hsv::depuis_hex("#808080");
        assert_eq!(g.saturation, 0.0);
        assert!((g.valeur - 128.0 / 255.0).abs() < 1e-6);
    }

    #[test]
    fn le_carre_va_du_blanc_au_noir() {
        assert_eq!(dans_carre(200.0, 0.0, 0.0).en_hex(), "#ffffff");
        assert_eq!(dans_carre(200.0, 0.5, 1.0).en_hex(), "#000000");
        assert_eq!(dans_carre(0.0, 1.0, 0.0).en_hex(), "#ff0000");
    }

    #[test]
    fn les_fractions_sont_bornees() {
        assert_eq!(fraction(-5.0, 10.0, 100.0), 0.0);
        assert_eq!(fraction(500.0, 10.0, 100.0), 1.0);
        assert_eq!(fraction(60.0, 10.0, 100.0), 0.5);
    }
}
