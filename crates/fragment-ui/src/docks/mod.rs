// Port de app/src/ui/core/workspace/WorkspaceRibbon.ts, de la poignée de Workspace.attachSidebarResize
// et de app/src/ui/workspace.css (`.workspace-ribbon`, `.workspace-sidedock*`,
// `.workspace-sidebar-resize-handle`).
//
//! Le ruban et les docks latéraux.
//!
//! - Le ruban borde le workspace à gauche : la case du repli du dock gauche (le « cadre »), puis les
//!   actions des plugins ([`ElementRuban`]). Il n'est PAS dans l'arbre des splits.
//! - Un dock replié, ou sans pile, fait 0 px : ni bordure, ni poignée (la géométrie ne le pose pas).
//!   ★ Pas de transition de largeur (Fragment l'a mesurée : chaque image relayait tout le workspace) :
//!   la largeur change d'un coup ; l'entrée animée du contenu n'est pas reprise en T1 (les scènes de
//!   référence coupent les animations).
//! - Les piles d'un dock sont les MÊMES piles, en habit de barre d'outils : l'icône devient l'onglet,
//!   sans titre, ni croix, ni « + », ni contour, et la vue n'a pas d'en-tête.

use fragment_core::{CommandManager, ItemId};
use gpui_kit::{
    AnyElement, App, InteractiveElement as _, IntoElement, MouseButton, ParentElement as _,
    StatefulInteractiveElement as _, Styled, Visibility, div, svg,
};

use crate::echelle::{Echelle, moitie, u};
use crate::entete;
use crate::glisser::{self, Apercu};
use crate::icones;
use crate::onglets::actions;
use crate::onglets::mesures::{OPACITE_REPOS, REMBOURRAGE_ENTETE};
use crate::splits::ContexteSplits;
use crate::splits::geometrie::{CadreDock, PoigneeDock, TRAIT_POIGNEE_ACTIVE};
use crate::splits::{elargi, quads_du_trait};
use crate::theme_global::{couleur, theme_ou_defaut};
use fragment_core::{Cadre, Cote};

/// Une action du ruban (`addRibbonIcon` de Fragment) : une icône, une infobulle, une commande.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElementRuban {
    /// Le nom de l'icône (trait fin : `square-pen-fin`, `sparkles-fin`, `terminal-fin`…).
    pub icone: String,
    pub titre: String,
    /// La commande exécutée au clic.
    pub commande: String,
}

/// Une bordure basse de `largeur` (GPUI n'a que des largeurs fixes : `border_b_1` ignorerait le zoom).
fn bordure_basse<E: Styled>(mut e: E, largeur: gpui_kit::Pixels) -> E {
    e.style().border_widths.bottom = Some(gpui_kit::AbsoluteLength::from(largeur));
    e
}

/// Une icône cliquable du ruban (`.clickable-icon`, 18 px au trait fin dans une case de 26).
fn icone_du_ruban(
    id: (&'static str, usize),
    nom: &str,
    cx: &App,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    let t = theme_ou_defaut(cx);
    let (c, f) = (&t.palette, &t.forme);
    let survol = couleur(c.background_modifier_hover);
    div()
        .id(id)
        .flex()
        .flex_none()
        .p(u(cx, f.size_4_1))
        .rounded(u(cx, f.radius_s))
        .opacity(OPACITE_REPOS)
        .hover(move |s| s.opacity(1.0).bg(survol))
        .child(
            svg()
                .path(icones::chemin(nom))
                .size(u(cx, f.icon_m))
                .flex_none()
                .text_color(couleur(c.text_normal)),
        )
}

/// Le ruban (`.workspace-ribbon.mod-left`) au cadre `cadre`.
pub fn rendre_ruban(ctx: &ContexteSplits, cadre: Cadre, cx: &App) -> AnyElement {
    let t = theme_ou_defaut(cx);
    let (c, f) = (&t.palette, &t.forme);
    let case = f.icon_m + 2.0 * f.size_4_1;
    let interieur = f.header_height - f.border_width;

    // La case du repli : le seul morceau du ruban qui appartient au CADRE (fond de la barre
    // d'onglets, même trait dessous, même hauteur), pour que son trait prolonge celui du bandeau.
    let ws = ctx.onglets.workspace.clone();
    let repli = bordure_basse(div(), u(cx, f.border_width))
        .flex_none()
        .w_full()
        .h(u(cx, f.header_height))
        .border_color(couleur(c.tab_outline_color))
        .bg(couleur(c.tab_bar_bg))
        .pl(moitie(cx, f.header_height - case))
        .pt(moitie(cx, interieur - case))
        .child(
            icone_du_ruban(("ruban-repli", 0), "panel-left-fin", cx).on_click(move |_, _, cx| {
                if let Ok(mut w) = ws.try_borrow_mut() {
                    let gauche = w.left_split();
                    w.toggle(gauche);
                }
                cx.refresh_windows();
            }),
        );

    let actions = ctx.ruban.iter().enumerate().map(|(i, e)| {
        let commandes = ctx.onglets.commandes.clone();
        let id = e.commande.clone();
        icone_du_ruban(("ruban-action", i), &e.icone, cx)
            .on_click(move |_, _, cx| {
                let m: &CommandManager = &commandes;
                m.execute_command_by_id(&id, None);
                cx.refresh_windows();
            })
            .into_any_element()
    });

    div()
        .absolute()
        .left(u(cx, cadre.x as f32))
        .top(u(cx, cadre.y as f32))
        .w(u(cx, cadre.largeur as f32))
        .h(u(cx, cadre.hauteur as f32))
        .flex()
        .flex_col()
        .items_center()
        .gap(u(cx, f.size_4_2))
        .pb(u(cx, f.size_4_2))
        .bg(couleur(c.background_secondary))
        .child(repli)
        .child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap(u(cx, f.size_4_2))
                .children(actions),
        )
        // La bordure droite COMMENCE sous la case du repli : le bandeau du haut reste d'un tenant.
        .child(
            div()
                .absolute()
                .top(u(cx, f.header_height))
                .right(u(cx, 0.0))
                .bottom(u(cx, 0.0))
                .w(u(cx, f.border_width))
                .bg(couleur(c.tab_outline_color)),
        )
        .into_any_element()
}

/// Le fond d'un dock visible (`--background-secondary`) et sa bordure côté centre.
pub fn rendre_fond_dock(dock: &CadreDock, cx: &App) -> AnyElement {
    let t = theme_ou_defaut(cx);
    let b = dock.boite;
    let bordure = dock.bordure;
    div()
        .absolute()
        .left(u(cx, b.x as f32))
        .top(u(cx, b.y as f32))
        .w(u(cx, b.largeur as f32))
        .h(u(cx, b.hauteur as f32))
        .bg(couleur(t.palette.background_secondary))
        .child(
            div()
                .absolute()
                .left(u(cx, (bordure.x - b.x) as f32))
                .top(u(cx, 0.0))
                .w(u(cx, bordure.largeur as f32))
                .h_full()
                .bg(couleur(t.palette.tab_outline_color)),
        )
        .into_any_element()
}

/// Une pile d'un dock : la barre d'icônes (sans titre, croix, « + » ni contour) et la feuille
/// visible, sans en-tête de vue. `retrait` : l'espace des boutons de fenêtre, si cette pile touche le
/// coin haut-droit (le bouton du dock droit s'y montre alors).
pub fn rendre_pile_laterale(
    ctx: &ContexteSplits,
    pile: ItemId,
    retrait: Option<f32>,
    _apercu: Option<Apercu>,
    cx: &App,
) -> AnyElement {
    let t = theme_ou_defaut(cx);
    let (c, f) = (&t.palette, &t.forme);
    let Ok(w) = ctx.onglets.workspace.try_borrow() else {
        return div().into_any_element();
    };
    let lignes = actions::lignes_de_la_pile(&w, pile);
    let visible = w.arbre().active_leaf(pile);
    drop(w);

    let case = f.icon_m + 2.0 * f.size_4_1 + 2.0 * REMBOURRAGE_ENTETE;
    let interieur = f.header_height - f.border_width;
    let onglets = lignes.iter().map(|l| {
        let (leaf, index) = (l.leaf, l.index);
        let ws = ctx.onglets.workspace.clone();
        let survol = couleur(c.background_modifier_hover);
        let groupe = format!("onglet-lateral-{}", leaf.index());
        div()
            .id(("onglet-lateral", leaf.index() as usize))
            .group(groupe.clone())
            .flex()
            .flex_none()
            .p(u(cx, REMBOURRAGE_ENTETE))
            .opacity(OPACITE_REPOS)
            .hover(|s| s.opacity(1.0))
            .on_mouse_down(MouseButton::Left, move |evt, _, cx| {
                if let Ok(mut w) = ws.try_borrow_mut() {
                    actions::presser_entete(&mut w, pile, index);
                }
                glisser::noter_appui(pile, leaf, evt.position, cx);
                cx.refresh_windows();
            })
            .child(
                div()
                    .flex()
                    .p(u(cx, f.size_4_1))
                    .rounded(u(cx, f.radius_s))
                    .group_hover(groupe, move |s| s.bg(survol))
                    .child(
                        svg()
                            .path(icones::chemin(&format!("{}-fin", l.icone)))
                            .size(u(cx, f.icon_m))
                            .flex_none()
                            .text_color(couleur(c.text_normal)),
                    ),
            )
            .into_any_element()
    });

    let mut barre = bordure_basse(div(), u(cx, f.border_width))
        .flex()
        .flex_none()
        .w_full()
        .h(u(cx, f.header_height))
        .border_color(couleur(c.tab_outline_color))
        .bg(couleur(c.tab_bar_bg))
        .child(
            div()
                .flex()
                .px(u(cx, f.size_2_1))
                .pt(moitie(cx, interieur - case))
                .children(onglets),
        )
        .child(div().flex_grow(1.0));
    if let Some(r) = retrait {
        let ws = ctx.onglets.workspace.clone();
        let survol = couleur(c.background_modifier_hover);
        let icone = f.icon_s + 2.0 * f.size_4_1;
        barre = barre
            .child(
                div()
                    .flex()
                    .flex_none()
                    .pl(u(cx, f.size_2_1))
                    .pr(u(cx, f.size_4_1))
                    .pt(moitie(cx, interieur - icone))
                    .child(
                        div()
                            .id(("onglet-lateral-dock-droit", pile.index() as usize))
                            .flex()
                            .p(u(cx, f.size_4_1))
                            .rounded(u(cx, f.radius_s))
                            .opacity(OPACITE_REPOS)
                            .hover(move |s| s.opacity(1.0).bg(survol))
                            .on_click(move |_, _, cx| {
                                if let Ok(mut w) = ws.try_borrow_mut() {
                                    actions::basculer_dock_droit(&mut w);
                                }
                                cx.refresh_windows();
                            })
                            .child(
                                svg()
                                    .path(icones::chemin("panel-right"))
                                    .size(u(cx, f.icon_s))
                                    .flex_none()
                                    .text_color(couleur(c.text_normal)),
                            ),
                    ),
            )
            .pr(u(cx, r));
    }

    let corps = visible.map(|leaf| {
        let ws = ctx.onglets.workspace.clone();
        div()
            .id(("feuille-laterale", leaf.index() as usize))
            .flex()
            .flex_col()
            .flex_1()
            .min_h(u(cx, 0.0))
            .capture_any_mouse_down(move |_, _, cx| {
                if let Ok(mut w) = ws.try_borrow_mut() {
                    actions::toucher_feuille(&mut w, leaf);
                }
                cx.refresh_windows();
            })
            // Pas d'en-tête de vue dans un dock : seul le contenu.
            .child(entete::rendre_contenu(&ctx.onglets, leaf, cx))
    });

    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_w(u(cx, 0.0))
        .min_h(u(cx, 0.0))
        .overflow_hidden()
        .child(barre)
        .children(corps)
        .into_any_element()
}

/// La poignée d'un dock : de largeur nulle sur la frontière, attrapable sur 6 px ; un trait d'accent
/// de 3 px, centré sur la frontière (donc à cheval sur des demi-pixels), apparaît au survol et pendant
/// le geste.
pub fn rendre_poignee_dock(
    ctx: &ContexteSplits,
    p: &PoigneeDock,
    active: bool,
    cx: &App,
) -> AnyElement {
    let t = theme_ou_defaut(cx);
    let zone = p.zone();
    let groupe: gpui_kit::SharedString = format!("poignee-dock-{}", p.dock.index()).into();
    let frontiere = Cadre::new(p.x, p.y, 0.0, p.hauteur);
    let trait_ = elargi(frontiere, true, TRAIT_POIGNEE_ACTIVE);
    let mut accent = div().absolute().size_full().children(quads_du_trait(
        trait_,
        zone,
        true,
        couleur(t.palette.color_accent),
        cx,
    ));
    if !active {
        accent.style().visibility = Some(Visibility::Hidden);
        accent = accent.group_hover(groupe.clone(), |mut s| {
            s.visibility = Some(Visibility::Visible);
            s
        });
    }
    let ctx = ctx.clone();
    let cote: Cote = p.cote;
    div()
        .id(("poignee-dock", p.dock.index() as usize))
        .group(groupe)
        .absolute()
        .left(u(cx, zone.x as f32))
        .top(u(cx, zone.y as f32))
        .w(u(cx, zone.largeur as f32))
        .h(u(cx, zone.hauteur as f32))
        .cursor_col_resize()
        .on_mouse_down(MouseButton::Left, move |evt, _, cx| {
            let zoom = f64::from(Echelle::courante(cx).zoom());
            ctx.commencer_dock(cote, f64::from(f32::from(evt.position.x)) / zoom);
            cx.stop_propagation();
            cx.refresh_windows();
        })
        .child(accent)
        .into_any_element()
}

/// Enregistre les commandes de scène `pixels:docks` : aucune, les docks des scènes arrivent par
/// l'action `layout` (`left` / `right`, voir `crate::splits::scene`).
pub fn commandes_de_scene(_commandes: &CommandManager) {}
