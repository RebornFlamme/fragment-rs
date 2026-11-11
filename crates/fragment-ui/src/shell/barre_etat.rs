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
    AbsoluteLength, AnyElement, App, FontWeight, IntoElement, ParentElement as _, SharedString,
    Styled as _, div,
};

use crate::echelle::u;
use crate::flottant::{arrondir, texte};
use crate::polices::{largeur_ajustee, largeur_texte};
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
    let trait_ = u(cx, f.border_width);
    // ★ La largeur est celle de Chromium : le contenu (textes exacts + rembourrages des éléments),
    //   plus le rembourrage et le trait gauche de la barre (`border-box`), arrondie au pixel le plus
    //   proche. GPUI arrondit chaque texte au pixel SUPÉRIEUR : ancrée à droite, la barre (et son
    //   texte) partait jusqu'à un pixel trop à gauche.
    let textes: f32 = elements
        .iter()
        .map(|e| largeur_texte(cx, e, FontWeight::NORMAL, ty.font_ui_smaller))
        .sum();
    let ajout = (elements.len() as f32 + 1.0) * 2.0 * f.size_2_3 + f.border_width;
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
            .child(e.clone())
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
    // Le coin haut-gauche, déclaré (catégorie bord d'arrondi d'ADR-009). ★ La déclaration de
    // `arrondir` vaut pour les quatre coins : les trois autres, carrés, sont remis à zéro ensuite ;
    // leurs bandes déclarées tombent hors de l'image (droite, bas) ou sur un coin sans écart.
    arrondir(barre, cx, u(cx, f.radius_sm), trait_)
        .rounded_tr(u(cx, 0.0))
        .rounded_bl(u(cx, 0.0))
        .rounded_br(u(cx, 0.0))
        .into_any_element()
}
