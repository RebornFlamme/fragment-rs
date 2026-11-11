// Port de app/src/ui/core/workspace/ItemView.ts (l'en-tête : `makeHeaderButton`,
// `updateNavButtons`, le menu « ⋯ »), de EmptyView.ts (le panneau vide) et de app/src/ui/workspace.css
// (`.view`, `.view-header*`, `.view-action*`, `.view-content`).
//
//! L'en-tête d'une vue, en grille à TROIS zones (← → | titre | actions), et le panneau d'une
//! vue vide.
//!
//! ★ Trois colonnes `1fr auto 1fr` plutôt qu'un `space-between` : les deux côtés ont la MÊME
//! largeur, donc le titre est centré dans le pane même avec 2 boutons à gauche et 4 à droite.
//! Ici deux `flex: 1` encadrent le titre, ce qui donne le même partage.
//!
//! ★ Une feuille NEUVE (bouton « + ») ou restaurée vide porte une vue vide JAMAIS MONTÉE : un
//! panneau blanc, sans en-tête ni message. Seul `set_view_state` la monte ; l'en-tête et le
//! message apparaissent alors ([`a_un_entete`]). Les coquilles différées n'en ont pas non plus.
//!
//! Le titre central est vide pour une vue ordinaire : le fil d'Ariane des vues de fichier arrive
//! avec elles (T2).

use fragment_core::{CommandManager, DeferredView, EmptyView, ItemId, View};
use gpui_kit::{
    AnyElement, App, InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent,
    ParentElement as _, StatefulInteractiveElement as _, Styled as _, div, svg,
};

use crate::echelle::{Echelle, u};
use crate::flottant::texte;
use crate::icones;
use crate::menu::Menu;
use crate::onglets::ContexteOnglets;
use crate::onglets::mesures::{BOUTON_ENTETE_VUE, OPACITE_DESACTIVE, OPACITE_REPOS};
use crate::theme_global::{couleur, theme_ou_defaut};

/// La taille du message d'une vue vide : celle du corps de page (`font-size` par défaut, 16 px),
/// avec l'interligne 1,5 hérité de `:root` (24 px de haut, mesuré sur Fragment).
const TAILLE_MESSAGE: f32 = 16.0;

/// La vue montre-t-elle un en-tête ? Non pour une vue vide jamais montée (panneau blanc) et pour
/// une coquille différée.
pub fn a_un_entete(vue: &dyn View) -> bool {
    let any = vue.as_any();
    if any.is::<DeferredView>() {
        return false;
    }
    match any.downcast_ref::<EmptyView>() {
        // `message()` n'est vide que tant que la vue n'est pas montée.
        Some(v) => !v.message().is_empty(),
        None => true,
    }
}

/// Active la feuille puis exécute la commande : le contrat de tout bouton et de toute entrée de
/// menu de l'en-tête. ★ La commande agit sur la feuille ACTIVE : le « ⋯ » d'un pane inactif doit
/// agir sur CE pane-là.
pub fn executer_ici(ctx: &ContexteOnglets, leaf: ItemId, commande: &str) {
    if let Ok(mut w) = ctx.workspace.try_borrow_mut() {
        w.set_active_leaf(Some(leaf));
    }
    let commandes: &CommandManager = &ctx.commandes;
    commandes.execute_command_by_id(commande, None);
}

/// Le menu « ⋯ » : le menu du pane (ItemView déclare l'ordre des groupes, y compris ceux que seules
/// les sous-classes peupleront).
fn menu_plus_d_options(ctx: &ContexteOnglets, leaf: ItemId, evt: &MouseDownEvent, cx: &App) {
    let zoom = Echelle::courante(cx).zoom();
    let mut menu = Menu::new();
    menu.add_sections(&["pane", "action", "view", "info", "system", "danger"]);
    for (titre, icone, commande) in [
        (
            "Diviser à droite",
            "separator-vertical",
            "workspace:split-vertical",
        ),
        (
            "Diviser en bas",
            "separator-horizontal",
            "workspace:split-horizontal",
        ),
    ] {
        let ctx = ctx.clone();
        menu.add_item(|i| {
            i.set_title(titre)
                .set_icon(icone)
                .set_section("pane")
                .on_click(move || executer_ici(&ctx, leaf, commande));
        });
    }
    menu.add_item(|i| {
        // Pas de multi-fenêtre : désactivée, comme dans Fragment.
        i.set_title("Ouvrir dans une nouvelle fenêtre")
            .set_icon("app-window")
            .set_section("pane")
            .set_disabled(true);
    });
    menu.show_at_position(
        f32::from(evt.position.x) / zoom,
        f32::from(evt.position.y) / zoom,
        &ctx.keymap,
    );
}

/// Un bouton de l'en-tête (`.view-action`) : 26 × 26, icône de 18 px au trait de 1,5 px.
///
/// ★ Le trait de 1,5 px (`--icon-stroke`) n'est pas celui du jeu Lucide (2 px) : les icônes
/// `<nom>-fin` sont les mêmes tracés à 1,5.
fn bouton(
    id: (&'static str, usize),
    icone: &str,
    actif: bool,
    cx: &App,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    let t = theme_ou_defaut(cx);
    let (c, f) = (&t.palette, &t.forme);
    let b = div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .flex_none()
        .w(u(cx, BOUTON_ENTETE_VUE))
        .h(u(cx, BOUTON_ENTETE_VUE))
        .rounded(u(cx, f.radius_s))
        .child(
            svg()
                .path(icones::chemin(icone))
                .size(u(cx, f.icon_m))
                .flex_none()
                .text_color(couleur(c.text_normal)),
        );
    if actif {
        let survol = couleur(c.background_modifier_active_hover);
        b.opacity(OPACITE_REPOS)
            .hover(move |s| s.opacity(1.0).bg(survol))
    } else {
        // ← / → sans historique : présents (la grille ne bouge pas), mais éteints et inertes.
        b.opacity(OPACITE_DESACTIVE)
    }
}

/// L'en-tête de la feuille `leaf` : navigation ← →, titre, actions « ⋯ ».
pub fn rendre_entete(ctx: &ContexteOnglets, leaf: ItemId, cx: &App) -> AnyElement {
    let t = theme_ou_defaut(cx);
    let f = &t.forme;
    let rang = leaf.index() as usize;
    let (peut_reculer, peut_avancer) = match ctx.workspace.try_borrow() {
        Ok(w) => w.leaf(leaf).map_or((false, false), |l| {
            (l.history.can_go_back(), l.history.can_go_forward())
        }),
        Err(_) => (false, false),
    };

    let mut arriere = bouton(("entete-arriere", rang), "arrow-left-fin", peut_reculer, cx);
    if peut_reculer {
        arriere = arriere.on_click({
            let ctx = ctx.clone();
            move |_, _, cx| {
                executer_ici(&ctx, leaf, "app:go-back");
                cx.refresh_windows();
            }
        });
    }
    let mut avant = bouton(("entete-avant", rang), "arrow-right-fin", peut_avancer, cx);
    if peut_avancer {
        avant = avant.on_click({
            let ctx = ctx.clone();
            move |_, _, cx| {
                executer_ici(&ctx, leaf, "app:go-forward");
                cx.refresh_windows();
            }
        });
    }
    // Le menu s'ouvre à l'APPUI : la position du pointeur est dans l'événement d'appui, pas
    // dans celui du clic.
    let plus = bouton(("entete-plus", rang), "ellipsis-vertical-fin", true, cx).on_mouse_down(
        MouseButton::Left,
        {
            let ctx = ctx.clone();
            move |evt, _, cx| {
                menu_plus_d_options(&ctx, leaf, evt, cx);
                cx.refresh_windows();
            }
        },
    );

    div()
        .flex()
        .items_center()
        .flex_shrink_0()
        .px(u(cx, f.size_2_3))
        .py(u(cx, f.size_2_1))
        .child(
            div()
                .flex()
                .flex_1()
                .justify_start()
                .gap(u(cx, f.size_2_1))
                .child(arriere)
                .child(avant),
        )
        // La colonne du milieu : vide pour une vue ordinaire, mais présente (la grille garde ses
        // trois colonnes). Redescendue d'un pixel (correction optique : le texte paraît plus haut
        // que les icônes voisines).
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .min_w(u(cx, 0.0))
                .mt(u(cx, 1.0))
                .overflow_hidden()
                .whitespace_nowrap(),
        )
        .child(
            div()
                .flex()
                .flex_1()
                .justify_end()
                .gap(u(cx, f.size_2_1))
                .child(plus),
        )
        .into_any_element()
}

/// Le contenu de la vue : le message d'une vue vide, sinon ce que donne le hook de la vue.
pub fn rendre_contenu(ctx: &ContexteOnglets, leaf: ItemId, cx: &App) -> AnyElement {
    let t = theme_ou_defaut(cx);
    let contenu = div()
        .flex()
        .flex_col()
        .flex_1()
        .min_h(u(cx, 0.0))
        .text_color(couleur(t.palette.text_normal));
    let Ok(w) = ctx.workspace.try_borrow() else {
        return contenu.into_any_element();
    };
    let Some(l) = w.leaf(leaf) else {
        return contenu.into_any_element();
    };
    let vue = l.view();
    if let Some(vide) = vue.as_any().downcast_ref::<EmptyView>() {
        // Du texte de page : 16 px sur un interligne de 24, tout contre le bord gauche.
        return contenu
            .child(texte(
                div().child(vide.message()),
                cx,
                TAILLE_MESSAGE,
                t.typo.line_height_normal,
            ))
            .into_any_element();
    }
    match ctx.contenu_de_vue.as_ref().and_then(|h| h(vue, cx)) {
        Some(el) => contenu.child(el).into_any_element(),
        None => contenu.into_any_element(),
    }
}

/// La vue entière (`.view`) : l'en-tête s'il y en a un, puis le contenu.
pub fn rendre_vue(ctx: &ContexteOnglets, leaf: ItemId, cx: &App) -> AnyElement {
    let avec_entete = ctx
        .workspace
        .try_borrow()
        .ok()
        .and_then(|w| w.leaf(leaf).map(|l| a_un_entete(l.view())))
        .unwrap_or(false);
    let mut vue = div().flex().flex_col().flex_1().min_w(u(cx, 0.0));
    if avec_entete {
        vue = vue.child(rendre_entete(ctx, leaf, cx));
    }
    vue.child(rendre_contenu(ctx, leaf, cx)).into_any_element()
}

/// Enregistre les commandes de scène `pixels:entete*` : vide, les scènes de l'en-tête passent par
/// `pixels:monter-vue` de `onglets` (monter la vue vide de la feuille visible).
pub fn commandes_de_scene(_commandes: &CommandManager) {}
