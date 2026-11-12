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
/// ★ L'alpha final se quantifie à l'OCTET deux fois, comme Skia puis le calque d'opacité du
///   bouton : `alpha = ⌊épaisseur × 255⌉ ⊗ ⌊couverture × 255⌉`, puis `⊗ ⌊opacité × 255⌉` (`⊗` =
///   produit divisé par 255 arrondi, [`mul_div_255`]). Pour 0,875 × 0,5 × 0,6 : 223 ⊗ 128 = 112,
///   112 ⊗ 153 = 67, soit 67/255 = 0,2627 ; le produit flottant (0,2602 après la part en 256es de
///   [`geometrie::filet`]) tombait un niveau sous la référence en sombre (68 contre 69, run
///   43086926887). Le canvas couvre tout le bouton pour savoir s'il est survolé (opacité 1).
fn trait_reduire(
    teinte: gpui_kit::Rgba,
    decalage: (Pixels, Pixels),
    cote: Pixels,
) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |bouton: Bounds<Pixels>, (), window: &mut Window, _: &mut App| {
            let survole = bouton.contains(&window.mouse_position());
            let opacite = if survole { 1.0 } else { OPACITE_REPOS };
            let bornes = Bounds::new(
                point(bouton.origin.x + decalage.0, bouton.origin.y + decalage.1),
                gpui_kit::size(cote, cote),
            );
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
                for (ligne, couverture) in
                    geometrie::couvertures_filet(f64::from(yc - w / 2.0), f64::from(yc + w / 2.0))
                {
                    let alpha = alpha_au_calque(f64::from(w), couverture, f64::from(opacite));
                    let mut t = source_premultipliee(teinte, alpha);
                    // GPUI multiplie encore par l'opacité du bouton : on la retire de la cible.
                    t.a = alpha as f32 / opacite;
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
    .absolute()
    .top(px(0.0))
    .left(px(0.0))
    .size_full()
}

/// `a × b / 255` arrondi, comme `SkMulDiv255Round`.
fn mul_div_255(a: u32, b: u32) -> u32 {
    let p = a * b + 128;
    (p + (p >> 8)) >> 8
}

/// L'alpha final (0 à 1) d'un pixel de filet d'épaisseur `epaisseur` (px physiques) et de
/// couverture `couverture`, sous un calque d'opacité `opacite` : voir [`trait_reduire`].
fn alpha_au_calque(epaisseur: f64, couverture: f64, opacite: f64) -> f64 {
    let octet = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
    let skia = mul_div_255(octet(epaisseur), octet(couverture));
    f64::from(mul_div_255(skia, octet(opacite))) / 255.0
}

/// Une couleur de GPUI (l'alias garde le motif de construction littérale réservé au thème).
type Teinte = gpui_kit::Rgba;

/// ★ La couleur à donner à GPUI pour que son mélange flottant (`c × a + fond × (1 − a)`, arrondi à
/// la fin) rende celui de Chromium, dont la source est PRÉMULTIPLIÉE à l'octet : `⌊c × A / 255⌉ +
/// fond × (255 − A) / 255`. Chaque canal devient `⌊c × A / 255⌉ / A`. Mesuré au croisement du « − »
/// et du trait de la poignée (`t1-boutons-fenetre` sombre, (1166,16) : 86, 85, 87) : aucun alpha
/// unique ne rendait les trois canaux sur les deux fonds.
fn source_premultipliee(teinte: Teinte, alpha: f64) -> Teinte {
    let a = (alpha * 255.0).round();
    if a <= 0.0 {
        return teinte;
    }
    let canal = |c: f32| {
        let p = (f64::from(c.clamp(0.0, 1.0)) * 255.0).round() * a / 255.0;
        (p.round() / a) as f32
    };
    let mut s = teinte;
    s.r = canal(teinte.r);
    s.g = canal(teinte.g);
    s.b = canal(teinte.b);
    s
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
            trait_reduire(
                normal,
                (
                    moitie(cx, f.frame_button_width - f.icon_xs),
                    moitie(cx, HAUTEUR_BOUTON - f.icon_xs),
                ),
                u(cx, f.icon_xs),
            )
            .into_any_element()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_moins_au_repos_sort_a_67_sur_255_comme_chromium() {
        assert_eq!(mul_div_255(223, 128), 112);
        assert_eq!(mul_div_255(112, 153), 67);
        let a = alpha_au_calque(0.875, 0.5, f64::from(OPACITE_REPOS));
        assert!((a - 67.0 / 255.0).abs() < 1e-9, "{a}");
        // Sur le fond sombre (14, 15, 17) et le texte (223, 222, 217) : 69, 69, 70 (référence).
        let mel = |f: f64, t: f64| (f + (t - f) * a).round();
        assert_eq!(
            [mel(14.0, 223.0), mel(15.0, 222.0), mel(17.0, 217.0)],
            [69.0, 69.0, 70.0]
        );
        // En clair (232, 231, 225) sous (26, 27, 30) : 178, 177, 174.
        assert_eq!(
            [mel(232.0, 26.0), mel(231.0, 27.0), mel(225.0, 30.0)],
            [178.0, 177.0, 174.0]
        );
        // Source prémultipliée : les deux fonds du croisement (fond sombre, trait de poignée).
        let s = source_premultipliee(
            // `--text-normal` du thème sombre (223, 222, 217).
            crate::theme_global::couleur(fragment_theme::Couleur::hex(0xdfded9)),
            a,
        );
        let gpui = |c: f32, f: f64| (f64::from(c) * a * 255.0 + f * (1.0 - a)).round();
        assert_eq!(
            [gpui(s.r, 14.0), gpui(s.g, 15.0), gpui(s.b, 17.0)],
            [69.0, 69.0, 70.0]
        );
        assert_eq!(
            [gpui(s.r, 36.0), gpui(s.g, 37.0), gpui(s.b, 41.0)],
            [86.0, 85.0, 87.0]
        );
        assert_eq!(
            [gpui(s.r, 33.0), gpui(s.g, 34.0), gpui(s.b, 38.0)],
            [83.0, 83.0, 85.0]
        );
        // Survolé (opacité 1) : 112/255.
        assert!((alpha_au_calque(0.875, 0.5, 1.0) - 112.0 / 255.0).abs() < 1e-9);
    }
}
