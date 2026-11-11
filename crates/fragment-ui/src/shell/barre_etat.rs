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
    AbsoluteLength, AnyElement, App, IntoElement, ParentElement as _, SharedString, Styled as _,
    div,
};

use crate::echelle::u;
use crate::flottant::texte;
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
    let (c, f) = (&t.palette, &t.forme);
    let trait_ = u(cx, f.border_width);
    let mut barre = div();
    {
        let s = barre.style();
        s.border_widths.top = Some(AbsoluteLength::from(trait_));
        s.border_widths.left = Some(AbsoluteLength::from(trait_));
    }
    let items = elements.iter().map(|e| {
        div()
            .flex()
            .items_center()
            .h_full()
            .px(u(cx, f.size_2_3))
            .whitespace_nowrap()
            .child(e.clone())
    });
    texte(barre, cx, t.typo.font_ui_smaller, t.typo.line_height_normal)
        .absolute()
        .right(u(cx, 0.0))
        .bottom(u(cx, 0.0))
        .flex()
        .items_center()
        .max_w(u(cx, LARGEUR_MAX))
        .overflow_hidden()
        .h(u(cx, HAUTEUR))
        .px(u(cx, f.size_2_3))
        .bg(couleur(c.tab_bar_bg))
        .border_color(couleur(c.tab_outline_color))
        .rounded_tl(u(cx, f.radius_sm))
        .text_color(couleur(c.text_muted))
        .children(items)
        .into_any_element()
}
