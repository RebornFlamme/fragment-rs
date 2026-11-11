// Port de app/src/ui/core/WindowControls.ts et de app/src/ui/core/window-controls.css.
//
//! Les boutons de fenêtre (réduire, agrandir, fermer) : posés par-dessus le coin haut-droit, ils ne
//! participent à AUCUNE mise en page. C'est la pile qui touche le coin qui s'écarte d'eux
//! (`OptionsPile::retrait_droit`, d'après `Workspace::espace_des_boutons`), jamais l'inverse.
//!
//! Le langage visuel de Windows, pas celui des icônes cliquables : 46 px de large, collés, sans
//! arrondi ; la croix vire au rouge au survol, icône blanche.

use gpui_kit::{
    AnyElement, App, Bounds, InteractiveElement as _, IntoElement, ParentElement as _, Pixels,
    StatefulInteractiveElement as _, Styled as _, Window, canvas, div, fill, point, px, svg,
};

use super::geometrie;
use crate::echelle::{moitie, u};
use crate::icones;
use crate::onglets::mesures::OPACITE_REPOS;
use crate::theme_global::{couleur, theme_ou_defaut};

/// La hauteur d'un bouton : celle de la barre d'onglets sous lui, 4 + 3 + 3 + 3 + 3 + 18 = 34 px
/// (window-controls.css:31).
pub const HAUTEUR_BOUTON: f32 = 34.0;

/// Ce que fait un bouton.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoutonFenetre {
    Reduire,
    Agrandir,
    Fermer,
}

impl BoutonFenetre {
    /// Les trois, de gauche à droite.
    pub const TOUS: [Self; 3] = [Self::Reduire, Self::Agrandir, Self::Fermer];

    /// L'icône (trait fin, `--icon-stroke`).
    pub fn icone(self) -> &'static str {
        match self {
            Self::Reduire => "minus-fin",
            Self::Agrandir => "square-fin",
            Self::Fermer => "x-fin",
        }
    }
}

/// Le « − » de `minus-fin` (`M5 12h14`, trait de 1,5 sur 24, bouts ronds), dans une boîte de
/// `--icon-xs`, peint comme Skia le rastérise.
///
/// ★ Plus fin qu'un pixel (0,875 px à 100 %), le trait n'est pas rempli à sa surface : Skia le trace
///   en filet d'un pixel à l'alpha modulé ([`geometrie::filet`]), d'une couverture UNIFORME sur
///   tous les pixels dont le centre tombe dans le trait, bouts ronds compris. Mesuré sur
///   `t1-boutons-fenetre` : 10 px × 2 lignes à 0,4375 (69 en sombre, 178 en clair), là où resvg
///   donnait 50/81×8/50 (décision du chef, fr-t1.5 [A]).
fn trait_reduire(teinte: gpui_kit::Rgba, cx: &App) -> impl IntoElement {
    let f = &theme_ou_defaut(cx).forme;
    canvas(
        |_, _, _| {},
        move |bornes: Bounds<Pixels>, (), window: &mut Window, _: &mut App| {
            let k = window.scale_factor();
            let s = f32::from(bornes.size.width) * k;
            let (ox, oy) = (
                f32::from(bornes.origin.x) * k,
                f32::from(bornes.origin.y) * k,
            );
            let w = 1.5 / 24.0 * s;
            let yc = oy + 12.0 / 24.0 * s;
            let (x0, x1) = (
                ox + 5.0 / 24.0 * s - w / 2.0,
                ox + 19.0 / 24.0 * s + w / 2.0,
            );
            let pt = |x: f32, y: f32| point(px(x / k), px(y / k));
            if w < 1.0 {
                // Les colonnes dont le centre est dans le trait, pleines le long.
                let (c0, c1) = ((x0 - 0.5).ceil(), (x1 - 0.5).floor() + 1.0);
                for (ligne, part) in
                    geometrie::filet(f64::from(yc - w / 2.0), f64::from(yc + w / 2.0))
                {
                    let mut t = teinte;
                    t.a *= part as f32;
                    let y = ligne as f32;
                    window.paint_quad(fill(Bounds::from_corners(pt(c0, y), pt(c1, y + 1.0)), t));
                }
            } else {
                window.paint_quad(fill(
                    Bounds::from_corners(pt(x0, yc - w / 2.0), pt(x1, yc + w / 2.0)),
                    teinte,
                ));
            }
        },
    )
    .size(u(cx, f.icon_xs))
    .flex_none()
}

/// Le conteneur des trois boutons, en haut à droite de la fenêtre.
pub fn rendre_boutons(cx: &App) -> AnyElement {
    let t = theme_ou_defaut(cx);
    let (c, f) = (&t.palette, &t.forme);
    let boutons = BoutonFenetre::TOUS.into_iter().map(|b| {
        let survol = if b == BoutonFenetre::Fermer {
            (couleur(c.window_close_bg), Some(couleur(c.text_on_accent)))
        } else {
            (couleur(c.background_modifier_hover), None)
        };
        let normal = couleur(c.text_normal);
        let groupe = format!("bouton-fenetre-{}", b as u8);
        let icone = if b == BoutonFenetre::Reduire {
            trait_reduire(normal, cx).into_any_element()
        } else {
            let icone = svg()
                .path(icones::chemin(b.icone()))
                .size(u(cx, f.icon_xs))
                .flex_none()
                .text_color(normal);
            match survol.1 {
                Some(blanc) => icone
                    .group_hover(groupe.clone(), move |s| s.text_color(blanc))
                    .into_any_element(),
                None => icone.into_any_element(),
            }
        };
        let fond = survol.0;
        div()
            .id(("bouton-fenetre", b as usize))
            .group(groupe)
            .flex()
            .flex_none()
            .w(u(cx, f.frame_button_width))
            .h(u(cx, HAUTEUR_BOUTON))
            // Le centrage à la Chromium (l'égalité vers le haut) : 16 px et 10 px pour une icône de 14.
            .pl(moitie(cx, f.frame_button_width - f.icon_xs))
            .pt(moitie(cx, HAUTEUR_BOUTON - f.icon_xs))
            .opacity(OPACITE_REPOS)
            .hover(move |s| s.opacity(1.0).bg(fond))
            .on_click(move |_, window, _| match b {
                BoutonFenetre::Reduire => window.minimize_window(),
                BoutonFenetre::Agrandir => window.zoom_window(),
                BoutonFenetre::Fermer => window.remove_window(),
            })
            .child(icone)
    });
    div()
        .absolute()
        .top(u(cx, 0.0))
        .right(u(cx, 0.0))
        .flex()
        .children(boutons)
        .into_any_element()
}
