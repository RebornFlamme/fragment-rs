// Port de app/src/ui/core/workspace/WorkspaceTabs.ts (le rendu : `rebuildTabHeaders`,
// `updateTabOutline`) et de app/src/ui/workspace.css (`.workspace-tabs`, `.workspace-tab-header*`).
//
//! Le rendu d'une pile d'onglets : la barre (en-têtes, « + », chevron, bouton du dock droit), le
//! contour de l'onglet actif, et sous elle la feuille visible.
//!
//! Aucune règle ici : chaque geste appelle une fonction de [`super::actions`], puis demande un
//! redessin (`refresh_windows`). Les longueurs passent toutes par `u()` ; les couleurs sont des
//! jetons du thème.
//!
//! ★ L'onglet actif « fusionne » avec le pane : son fond est celui du pane, sans bordure ; le
//! trait de contour est UN chemin tracé par-dessus toute la barre (voir [`super::mesures::contour`]),
//! que Fragment relit dans le CSS (`--tab-radius-*`) pour que le trait et le fond ne divergent pas.
//! Ici le chemin est peint depuis les bornes mesurées de l'en-tête actif (`canvas`), lues à la
//! peinture : toute la barre est préparée avant d'être peinte, la mesure est donc déjà là.

use std::cell::Cell;
use std::rc::Rc;

use fragment_core::ItemId;
use gpui_kit::{
    AnyElement, App, Bounds, InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent,
    ParentElement as _, PathBuilder, Pixels, SharedString, StatefulInteractiveElement as _,
    Styled as _, Visibility, Window, canvas, div, point, px, svg,
};

use super::ContexteOnglets;
use super::actions::{self, LigneOnglet};
use super::mesures::{
    INTERIEUR_AUTRES_COTES, INTERIEUR_COTE_TITRE, LARGEUR_ONGLET, NOUVEL_ONGLET_MARGE_GAUCHE,
    OPACITE_REPOS, OngletActif, REMBOURRAGE_ENTETE, SEPARATEUR_LARGEUR, SEPARATEUR_MARGE, Segment,
    TRAIT_CONTOUR, coin_concave, contour,
};
use crate::echelle::{Echelle, u};
use crate::entete;
use crate::flottant::texte;
use crate::icones;
use crate::menu::Menu;
use crate::theme_global::{couleur, theme_ou_defaut};

/// Ce qui change d'une pile à l'autre.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct OptionsPile {
    /// `Some(recouvrement)` : cette pile touche le coin haut-droit (`mod-top-right-space`). Elle
    /// réserve `recouvrement` px à droite pour les boutons de fenêtre et montre le bouton du dock
    /// droit. Le recouvrement vient de `Workspace::espace_des_boutons` (la réserve entière à
    /// défaut : `frame_right_space`).
    pub retrait_droit: Option<f32>,
}

/// Peint `segments` dans `construit`, décalés de `origine` (le coin de la barre à l'écran).
fn tracer(construit: &mut PathBuilder, segments: &[Segment], origine: gpui_kit::Point<Pixels>) {
    let pt = |x: f32, y: f32| point(origine.x + px(x), origine.y + px(y));
    for s in segments {
        match *s {
            Segment::Debut(x, y) => construit.move_to(pt(x, y)),
            Segment::Ligne(x, y) => construit.line_to(pt(x, y)),
            Segment::Arc {
                rayon,
                vers_x,
                vers_y,
                horaire,
            } => construit.arc_to(
                point(px(rayon), px(rayon)),
                px(0.0),
                false,
                horaire,
                pt(vers_x, vers_y),
            ),
        }
    }
}

/// Peint le fond des coins concaves, puis le trait du contour, dans la barre `bornes`.
#[allow(clippy::too_many_arguments)]
fn peindre_contour(
    bornes: Bounds<Pixels>,
    actif: Option<Bounds<Pixels>>,
    fond_pane: gpui_kit::Rgba,
    trait_: gpui_kit::Rgba,
    rayon_haut: f32,
    rayon_bas: f32,
    epaisseur: f32,
    window: &mut Window,
) {
    let largeur = f32::from(bornes.size.width);
    let hauteur = f32::from(bornes.size.height);
    let onglet = actif.map(|b| OngletActif {
        gauche: f32::from(b.origin.x - bornes.origin.x),
        droite: f32::from(b.origin.x - bornes.origin.x + b.size.width),
        haut: f32::from(b.origin.y - bornes.origin.y),
    });
    // L'onglet sorti de la barre n'a plus de contour (son fond est rogné aussi).
    let onglet = onglet.filter(|o| o.droite >= 0.0 && o.gauche <= largeur);

    if let Some(o) = onglet {
        for (bord, gauche) in [(o.gauche, true), (o.droite, false)] {
            let segments = coin_concave(bord, hauteur, rayon_bas, gauche);
            let mut p = PathBuilder::fill();
            tracer(&mut p, &segments, bornes.origin);
            p.close();
            if let Ok(chemin) = p.build() {
                window.paint_path(chemin, fond_pane);
            }
        }
    }

    let segments = contour(
        largeur,
        hauteur,
        onglet,
        rayon_haut,
        rayon_bas,
        epaisseur / 2.0,
    );
    let mut p = PathBuilder::stroke(px(epaisseur));
    tracer(&mut p, &segments, bornes.origin);
    if let Ok(chemin) = p.build() {
        window.paint_path(chemin, trait_);
    }
}

/// L'icône cliquable générique (`.clickable-icon`, workspace.css:368) : 16 px dans une case de 24.
fn icone_cliquable(
    id: &'static str,
    nom: &str,
    taille: f32,
    cx: &App,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    let t = theme_ou_defaut(cx);
    let (c, f) = (&t.palette, &t.forme);
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .p(u(cx, f.size_4_1))
        .rounded(u(cx, f.radius_s))
        .opacity(OPACITE_REPOS)
        .hover({
            let survol = couleur(c.background_modifier_hover);
            move |s| s.opacity(1.0).bg(survol)
        })
        .child(
            svg()
                .path(icones::chemin(nom))
                .size(u(cx, taille))
                .flex_none()
                .text_color(couleur(c.text_normal)),
        )
}

/// Ouvre le menu contextuel d'un onglet (« Fermer ») au point de l'événement.
fn menu_de_l_onglet(ctx: &ContexteOnglets, leaf: ItemId, evt: &MouseDownEvent, cx: &App) {
    let zoom = Echelle::courante(cx).zoom();
    let ws = ctx.workspace.clone();
    let mut menu = Menu::new();
    menu.add_item(|i| {
        i.set_title("Fermer").set_icon("x").on_click(move || {
            if let Ok(mut w) = ws.try_borrow_mut() {
                actions::fermer_onglet(&mut w, leaf);
            }
        });
    });
    menu.show_at_position(
        f32::from(evt.position.x) / zoom,
        f32::from(evt.position.y) / zoom,
        &ctx.keymap,
    );
}

/// Le menu de la pile (le chevron) : actions sur les onglets, puis la liste de TOUS les onglets.
fn menu_de_la_pile(ctx: &ContexteOnglets, pile: ItemId, evt: &MouseDownEvent, cx: &App) {
    let zoom = Echelle::courante(cx).zoom();
    let lignes = match ctx.workspace.try_borrow() {
        Ok(w) => actions::lignes_de_la_pile(&w, pile),
        Err(_) => return,
    };
    let n = lignes.len();
    let mut menu = Menu::new();
    menu.add_sections(&["tabs", "close", "list"]);
    menu.add_item(|i| {
        // Désactivé pour l'instant, comme dans Fragment.
        i.set_title("Stack tabs")
            .set_icon("layers")
            .set_section("tabs")
            .set_disabled(true);
    });
    menu.add_item(|i| {
        i.set_title(&format!(
            "Bookmark {n} tab{}...",
            if n > 1 { "s" } else { "" }
        ))
        .set_icon("bookmark")
        .set_section("tabs");
    });
    let ws = ctx.workspace.clone();
    menu.add_item(|i| {
        i.set_title("Close all")
            .set_icon("x")
            .set_section("close")
            .on_click(move || {
                if let Ok(mut w) = ws.try_borrow_mut() {
                    actions::fermer_tout(&mut w, pile);
                }
            });
    });
    for l in lignes {
        let ws = ctx.workspace.clone();
        menu.add_item(|i| {
            i.set_title(&l.titre)
                .set_icon(&l.icone)
                .set_section("list")
                .set_checked(l.visible)
                .on_click(move || {
                    if let Ok(mut w) = ws.try_borrow_mut() {
                        actions::montrer_onglet(&mut w, pile, l.index);
                    }
                });
        });
    }
    menu.show_at_position(
        f32::from(evt.position.x) / zoom,
        f32::from(evt.position.y) / zoom,
        &ctx.keymap,
    );
}

/// Un en-tête d'onglet : zone tappable de 206 × 32, et dedans la « pilule » icône | titre | statut
/// | croix.
#[allow(clippy::too_many_arguments)]
fn entete_d_onglet(
    ctx: &ContexteOnglets,
    pile: ItemId,
    ligne: &LigneOnglet,
    est_dernier: bool,
    suivant_actif: bool,
    mesure: Option<Rc<Cell<Option<Bounds<Pixels>>>>>,
    cx: &App,
) -> AnyElement {
    let t = theme_ou_defaut(cx);
    let (c, f) = (&t.palette, &t.forme);
    let actif = ligne.visible;
    let leaf = ligne.leaf;
    let index = ligne.index;
    let rang = leaf.index() as usize;
    let groupe: SharedString = format!("onglet-{rang}").into();

    // La croix : invisible (mais présente : la largeur ne bouge pas) sauf sur l'actif et au survol.
    let mut croix = div()
        .id(("onglet-croix", rang))
        .flex()
        .items_center()
        .justify_center()
        .flex_shrink_0()
        .p(u(cx, f.size_2_1))
        .rounded(u(cx, f.radius_s))
        .opacity(OPACITE_REPOS)
        .on_mouse_down(MouseButton::Left, |_, _, cx| {
            // Presser « × » ne doit pas activer l'onglet qu'on ferme.
            cx.stop_propagation();
        })
        .on_click({
            let ws = ctx.workspace.clone();
            move |_, _, cx| {
                if let Ok(mut w) = ws.try_borrow_mut() {
                    actions::fermer_onglet(&mut w, leaf);
                }
                cx.refresh_windows();
            }
        })
        .child(
            svg()
                .path(icones::chemin("x"))
                .size(u(cx, f.icon_xs))
                .flex_none()
                .text_color(couleur(c.text_normal)),
        );
    if actif {
        let fond = couleur(c.background_modifier_active_hover);
        croix = croix.hover(move |s| s.opacity(1.0).bg(fond));
    } else {
        croix.style().visibility = Some(Visibility::Hidden);
        croix = croix
            .group_hover(groupe.clone(), |mut s| {
                s.visibility = Some(Visibility::Visible);
                s
            })
            .hover(|s| s.opacity(1.0));
    }

    let mut interieur = div()
        .id(("onglet-interieur", rang))
        .flex()
        .items_center()
        .flex_1()
        .min_w(u(cx, 0.0))
        .gap(u(cx, f.size_4_1))
        .pl(u(cx, INTERIEUR_COTE_TITRE))
        .pr(u(cx, INTERIEUR_AUTRES_COTES))
        .py(u(cx, INTERIEUR_AUTRES_COTES))
        .rounded(u(cx, f.radius_s))
        // Le titre prend ce qui reste et s'ellipse ; le statut, vide, garde son rang (et donc l'écart).
        .child(
            div()
                .flex_1()
                .min_w(u(cx, 0.0))
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .child(ligne.titre.clone()),
        )
        .child(div().flex().items_center())
        .child(croix);
    if !actif {
        // Le fond de survol vit sur la pilule, pas sur la zone pleine hauteur ; réservé aux
        // inactifs (l'actif a déjà son fond fusionné avec le pane).
        let survol = couleur(c.background_modifier_hover);
        interieur = interieur.hover(move |s| s.bg(survol));
    }

    let mut entete = div()
        .id(("onglet", rang))
        .group(groupe)
        .relative()
        .flex()
        .items_center()
        .w(u(cx, LARGEUR_ONGLET))
        .min_w(u(cx, 0.0))
        .p(u(cx, REMBOURRAGE_ENTETE))
        .rounded_tl(u(cx, f.input_radius))
        .rounded_tr(u(cx, f.input_radius))
        .whitespace_nowrap()
        .on_mouse_down(MouseButton::Left, {
            let ws = ctx.workspace.clone();
            move |_, _, cx| {
                if let Ok(mut w) = ws.try_borrow_mut() {
                    actions::presser_entete(&mut w, pile, index);
                }
                cx.refresh_windows();
            }
        })
        .on_mouse_down(MouseButton::Right, {
            let ctx = ctx.clone();
            move |evt, _, cx| {
                menu_de_l_onglet(&ctx, leaf, evt, cx);
                cx.refresh_windows();
            }
        })
        .child(interieur);
    entete = texte(entete, cx, t.typo.font_ui_small, 1.5);
    if actif {
        // Il peint au-dessus de ses voisins (ses coins concaves débordent) : c'est l'ordre
        // des enfants qui le garantit ici, le contour étant peint après tous les en-têtes.
        entete = entete.bg(couleur(c.workspace_bg));
    } else {
        entete = entete.opacity(OPACITE_REPOS);
        if !est_dernier && !suivant_actif {
            // Le trait fin au bord droit, plus court que la barre. Ni juste avant l'actif (son
            // fond ne le recouvrirait pas), ni après le dernier.
            entete = entete.child(
                div()
                    .absolute()
                    .top(u(cx, SEPARATEUR_MARGE))
                    .bottom(u(cx, SEPARATEUR_MARGE))
                    .right(u(cx, 0.0))
                    .w(u(cx, SEPARATEUR_LARGEUR))
                    .bg(couleur(c.tab_outline_color)),
            );
        }
    }
    if let Some(cellule) = mesure.filter(|_| actif) {
        // La mesure de l'en-tête actif, pour le contour (bornes de la boîte de bordure).
        entete = entete.child(
            canvas(
                move |bornes, _, _| cellule.set(Some(bornes)),
                |_, (), _, _| {},
            )
            .absolute()
            .top(u(cx, 0.0))
            .left(u(cx, 0.0))
            .right(u(cx, 0.0))
            .bottom(u(cx, 0.0)),
        );
    }
    entete.into_any_element()
}

/// La barre d'onglets d'une pile.
fn barre(
    ctx: &ContexteOnglets,
    pile: ItemId,
    lignes: &[LigneOnglet],
    options: OptionsPile,
    cx: &App,
) -> AnyElement {
    let t = theme_ou_defaut(cx);
    let (c, f) = (&t.palette, &t.forme);
    let mesure: Rc<Cell<Option<Bounds<Pixels>>>> = Rc::new(Cell::new(None));
    let n = lignes.len();

    let entetes: Vec<AnyElement> = lignes
        .iter()
        .enumerate()
        .map(|(i, l)| {
            let suivant_actif = lignes.get(i + 1).is_some_and(|s| s.visible);
            entete_d_onglet(
                ctx,
                pile,
                l,
                i + 1 == n,
                suivant_actif,
                Some(mesure.clone()),
                cx,
            )
        })
        .collect();

    // Les onglets rétrécissent pour tenir (jamais de barre de défilement) ; le rembourrage
    // horizontal vit ICI : le coin concave du premier ou du dernier onglet déborde de 4 px.
    let interieur = div()
        .flex()
        .px(u(cx, f.size_4_1))
        .overflow_hidden()
        .min_w(u(cx, 0.0))
        .children(entetes);

    let nouvel_onglet = div()
        .flex()
        .items_center()
        .pr(u(cx, f.size_2_1))
        .ml(u(cx, NOUVEL_ONGLET_MARGE_GAUCHE))
        .child(
            icone_cliquable("onglet-nouveau", "plus", f.icon_s, cx).on_click({
                let ws = ctx.workspace.clone();
                move |_, _, cx| {
                    if let Ok(mut w) = ws.try_borrow_mut() {
                        actions::nouvel_onglet(&mut w, pile);
                    }
                    cx.refresh_windows();
                }
            }),
        );

    // Le chevron : le menu de la pile. L'appui (et non le clic) ouvre le menu à la position du
    // pointeur, comme le ferait l'événement de clic de Fragment.
    let liste = div()
        .flex()
        .items_center()
        .flex_shrink_0()
        .pl(u(cx, f.size_2_1))
        .pr(u(cx, f.size_4_1))
        .child(
            icone_cliquable("onglet-liste", "chevron-down", f.icon_s, cx).on_mouse_down(
                MouseButton::Left,
                {
                    let ctx = ctx.clone();
                    move |evt, _, cx| {
                        menu_de_la_pile(&ctx, pile, evt, cx);
                        cx.refresh_windows();
                    }
                },
            ),
        );

    let mut barre = div()
        .relative()
        .flex()
        .flex_shrink_0()
        .w_full()
        .h(u(cx, f.header_height))
        .pt(u(cx, f.size_4_1))
        .bg(couleur(c.tab_bar_bg))
        .child(interieur)
        .child(nouvel_onglet)
        // Le spacer : reste en zone de glisser de la fenêtre, et pousse le chevron au bord.
        .child(div().flex_grow(1.0))
        .child(liste);

    if let Some(retrait) = options.retrait_droit {
        let bouton = div()
            .flex()
            .items_center()
            .flex_shrink_0()
            .pl(u(cx, f.size_2_1))
            .pr(u(cx, f.size_4_1))
            .child(
                icone_cliquable("onglet-dock-droit", "panel-right", f.icon_s, cx).on_click({
                    let ws = ctx.workspace.clone();
                    move |_, _, cx| {
                        if let Ok(mut w) = ws.try_borrow_mut() {
                            actions::basculer_dock_droit(&mut w);
                        }
                        cx.refresh_windows();
                    }
                }),
            );
        barre = barre.child(bouton).pr(u(cx, retrait));
    }

    // Le contour : peint APRÈS tous les en-têtes, donc par-dessus le fond de l'actif.
    let (fond_pane, trait_) = (couleur(c.workspace_bg), couleur(c.tab_outline_color));
    let (rayon_haut, rayon_bas) = (u(cx, f.input_radius), u(cx, f.radius_s));
    let epaisseur = u(cx, TRAIT_CONTOUR);
    barre
        .child(
            canvas(
                |_, _, _| {},
                move |bornes, (), window, _| {
                    peindre_contour(
                        bornes,
                        mesure.get(),
                        fond_pane,
                        trait_,
                        f32::from(rayon_haut),
                        f32::from(rayon_bas),
                        f32::from(epaisseur),
                        window,
                    );
                },
            )
            .absolute()
            .top(u(cx, 0.0))
            .left(u(cx, 0.0))
            .right(u(cx, 0.0))
            .bottom(u(cx, 0.0)),
        )
        .into_any_element()
}

/// La feuille visible d'une pile : la vue (en-tête et contenu), avec l'activation au contact.
///
/// ★ Un appui N'IMPORTE OÙ dans la feuille (en capture, avant ses enfants) la rend active, pas
/// seulement son onglet : sinon, revenir écrire dans la note après le chat du dock la laissait
/// inactive, et ses raccourcis partaient au chat. Le focus (`focusin` de Fragment) viendra avec
/// les vues qui ont un champ de saisie.
fn feuille_visible(ctx: &ContexteOnglets, leaf: ItemId, cx: &App) -> AnyElement {
    let ws = ctx.workspace.clone();
    div()
        .id(("feuille", leaf.index() as usize))
        .absolute()
        .top(u(cx, 0.0))
        .left(u(cx, 0.0))
        .right(u(cx, 0.0))
        .bottom(u(cx, 0.0))
        .flex()
        .capture_any_mouse_down(move |_, _, cx| {
            if let Ok(mut w) = ws.try_borrow_mut() {
                actions::toucher_feuille(&mut w, leaf);
            }
            cx.refresh_windows();
        })
        .child(entete::rendre_vue(ctx, leaf, cx))
        .into_any_element()
}

/// Une pile d'onglets : la barre, puis la feuille visible (les autres sont cachées, pas détruites).
pub fn rendre_pile(
    ctx: &ContexteOnglets,
    pile: ItemId,
    options: OptionsPile,
    cx: &App,
) -> AnyElement {
    let t = theme_ou_defaut(cx);
    let (lignes, visible) = match ctx.workspace.try_borrow() {
        Ok(w) => (
            actions::lignes_de_la_pile(&w, pile),
            w.arbre().active_leaf(pile),
        ),
        // Un emprunt en cours (un écouteur du Workspace qui redessine) : on ne peint rien.
        Err(_) => return div().into_any_element(),
    };
    let conteneur = div()
        .relative()
        .flex_1()
        .min_h(u(cx, 0.0))
        .bg(couleur(t.palette.workspace_bg))
        .children(visible.map(|l| feuille_visible(ctx, l, cx)));
    let pile_el = div()
        .flex()
        .flex_col()
        .flex_1()
        .min_w(u(cx, 0.0))
        .min_h(u(cx, 0.0))
        .overflow_hidden()
        .text_color(couleur(t.palette.text_normal))
        .child(barre(ctx, pile, &lignes, options, cx))
        .child(conteneur);
    texte(pile_el, cx, t.typo.font_ui_small, 1.5).into_any_element()
}
