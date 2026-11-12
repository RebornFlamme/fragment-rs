// Port de app/src/ui/core/workspace/StatusBar.ts et du bloc `.status-bar` de app/src/ui/workspace.css.
//
//! La barre d'état : un calque flottant en bas à droite, HORS du flux du workspace (rien n'est
//! décalé), qui rend les éléments posés par les plugins (`addStatusBarItem`). Sans élément, rien
//! n'est peint (`.status-bar:empty`).
//!
//! Gabarit (workspace.css, `.status-bar`) : 20 px de haut, `padding: 0 --size-2-3`, fond
//! `--tab-bar-bg`, trait `--tab-outline-color` en haut et à gauche, coin haut-gauche
//! `--radius-sm`, texte `--text-muted` en `--font-ui-smaller` ; chaque élément a son propre
//! `padding: 0 --size-2-3`. La largeur suit le contenu, plafonnée à 400 px.

use gpui_kit::{
    AbsoluteLength, AnyElement, App, Bounds, FontWeight, IntoElement, ParentElement as _, Pixels,
    SharedString, Styled as _, TextAlign, Visibility, Window, canvas, div, point, px,
};

use crate::echelle::{Echelle, u};
use crate::flottant::{arrondir_coins, texte};
use crate::polices::{largeur_ajustee, largeur_texte};
use crate::splits::geometrie;
use crate::theme_global::{couleur, theme_ou_defaut};

/// La hauteur de la barre (`.status-bar`, `height: 20px`).
pub const HAUTEUR: f32 = 20.0;
/// La largeur maximale (`--status-bar-max-width`, défaut 400 px).
pub const LARGEUR_MAX: f32 = 400.0;

/// La barre d'état, avec ses éléments dans l'ordre des plugins ; rien si `elements` est vide.
pub fn rendre(elements: &[SharedString], cx: &App) -> AnyElement {
    if elements.is_empty() {
        return div().into_any_element();
    }
    let t = theme_ou_defaut(cx);
    let (c, f, ty) = (&t.palette, &t.forme, &t.typo);
    // ★ Le trait (haut et gauche) fait un nombre entier de pixels d'écran, comme chez Chromium : à
    //   80 % 1 px et non 0,8, à 150 % 1 px et non 1,5. Il entre dans la largeur ci-dessous : avec
    //   l'épaisseur zoomée, la barre sortait 1 px trop étroite à 80 % et trop large à 150 %.
    let bordure = geometrie::epaisseur_bordure(
        f64::from(f.border_width),
        f64::from(Echelle::courante(cx).zoom()),
    ) as f32;
    let trait_ = u(cx, bordure);
    // ★ La largeur est celle de Chromium : le contenu (textes exacts + rembourrages des éléments),
    //   plus le rembourrage et le trait gauche de la barre (`border-box`), arrondie au pixel le plus
    //   proche. GPUI arrondit chaque texte au pixel SUPÉRIEUR : ancrée à droite, la barre (et son
    //   texte) partait jusqu'à un pixel trop à gauche.
    let textes: f32 = elements
        .iter()
        .map(|e| largeur_texte(cx, e, FontWeight::NORMAL, ty.font_ui_smaller))
        .sum();
    let ajout = (elements.len() as f32 + 1.0) * 2.0 * f.size_2_3 + bordure;
    let largeur = largeur_ajustee(cx, textes, ajout).min(u(cx, LARGEUR_MAX));
    let mut barre = div();
    {
        let s = barre.style();
        s.border_widths.top = Some(AbsoluteLength::from(trait_));
        s.border_widths.left = Some(AbsoluteLength::from(trait_));
    }
    let items = elements.iter().map(|e| {
        div()
            .flex()
            .flex_none()
            .items_center()
            .h_full()
            .px(u(cx, f.size_2_3))
            .whitespace_nowrap()
            .relative()
            .child(texte_cale(
                e.clone(),
                u(cx, f.size_2_3),
                ty.line_height_normal,
                cx,
            ))
    });
    let barre = texte(barre, cx, ty.font_ui_smaller, ty.line_height_normal)
        .absolute()
        .right(u(cx, 0.0))
        .bottom(u(cx, 0.0))
        .flex()
        .items_center()
        .w(largeur)
        .overflow_hidden()
        .h(u(cx, HAUTEUR))
        .px(u(cx, f.size_2_3))
        .bg(couleur(c.tab_bar_bg))
        .border_color(couleur(c.tab_outline_color))
        .text_color(couleur(c.text_muted))
        .children(items);
    // Le coin haut-gauche, seul arrondi et seul déclaré (catégorie bord d'arrondi d'ADR-009), sous
    // l'exception nommée « coin de la barre d'état » (arc intérieur du quad GPUI, delta 9 à 11 en
    // sombre : mélange alpha, à retirer par fr-p4.1).
    arrondir_coins(
        barre,
        cx,
        u(cx, f.radius_sm),
        trait_,
        [true, false, false, false],
        ELEMENT_COIN,
    )
    .into_any_element()
}

/// L'élément de l'exception nommée du coin (clé du cliquet, `xtask/src/pixels/exceptions.rs`,
/// `COIN_BARRE_ETAT`).
pub const ELEMENT_COIN: &str = "coin de la barre d'état";

/// ★ Le texte d'un élément, sa LIGNE DE BASE calée comme Chromium. Chromium arrondit l'ascendante et
/// la descendante de la police au pixel, pose le haut de la ligne au 1/64 (centrage d'une ligne de
/// `interligne` × taille dans la barre), ajoute la DEMI-INTERLIGNE ARRONDIE À L'ENTIER INFÉRIEUR
/// puis l'ascendante, et peint au pixel le plus proche. GPUI centre une hauteur de ligne arrondie
/// et pose la ligne de base à l'ascendante exacte : 1 px trop bas à 80 et 150 % (`t1-zoom-80`,
/// (1208,788) ; `t1-zoom-150`, (1147,778) : sonde DOM, haut du texte à 785,906 et 774,125).
///
/// Le texte garde sa place par une copie invisible ; il est peint par-dessus, à l'abscisse de la
/// copie (`gauche` : le rembourrage de l'élément) et sur toute la hauteur de l'élément.
fn texte_cale(texte: SharedString, gauche: Pixels, interligne: f32, cx: &App) -> impl IntoElement {
    let mut gabarit = div().child(texte.clone());
    gabarit.style().visibility = Some(Visibility::Hidden);
    let _ = cx;
    div().child(gabarit).child(
        canvas(
            |_, _, _| {},
            move |bornes: Bounds<Pixels>, (), window: &mut Window, cx: &mut App| {
                let style = window.text_style();
                let taille = style.font_size.to_pixels(window.rem_size());
                let run = style.to_run(texte.len());
                let ligne = window
                    .text_system()
                    .shape_line(texte.clone(), taille, &[run], None);
                let k = window.scale_factor();
                let (a, d) = (f32::from(ligne.ascent) * k, f32::from(ligne.descent) * k);
                let (ar, dr) = (a.round(), d.round());
                let lu = |v: f32| (v * 64.0 + 1e-3).floor() / 64.0;
                let hauteur_ligne = lu(f32::from(taille) * interligne * k);
                let (haut, h) = (
                    f32::from(bornes.origin.y) * k,
                    f32::from(bornes.size.height) * k,
                );
                let ligne_haut = haut + lu((h - hauteur_ligne) / 2.0);
                let demi = ((hauteur_ligne - (ar + dr)) / 2.0).floor();
                let base = (ligne_haut + demi + ar + 0.5).floor();
                // Interligne de peinture = a + d : GPUI pose alors la ligne de base à `y + a`.
                let y = (base - a) / k;
                let _ = ligne.paint(
                    point(bornes.origin.x, px(y)),
                    px((a + d) / k),
                    TextAlign::Left,
                    None,
                    window,
                    cx,
                );
            },
        )
        .absolute()
        .top(px(0.0))
        .bottom(px(0.0))
        .left(gauche)
        .right(px(0.0)),
    )
}
