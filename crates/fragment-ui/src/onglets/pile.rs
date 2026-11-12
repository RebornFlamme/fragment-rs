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

use fragment_core::{DropZone, ItemId};
use fragment_theme::Couleur;
use gpui_kit::{
    AnyElement, App, Bounds, InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent,
    ParentElement as _, PathBuilder, Pixels, SharedString, StatefulInteractiveElement as _,
    Styled as _, Visibility, Window, canvas, div, fill, point, px, relative, svg,
};

use super::ContexteOnglets;
use super::actions::{self, LigneOnglet};
use super::mesures::{
    INTERIEUR_AUTRES_COTES, INTERIEUR_COTE_TITRE, LARGEUR_ONGLET, NOUVEL_ONGLET_MARGE_GAUCHE,
    OPACITE_REPOS, OngletActif, REMBOURRAGE_ENTETE, SEPARATEUR_LARGEUR, SEPARATEUR_MARGE, Segment,
    TRAIT_CONTOUR, bords_exacts, coin_concave,
};
use crate::echelle::{Echelle, u};
use crate::entete;
use crate::flottant::{ArrondiPeint, arrondir, declarer_arrondi, texte};
use crate::icones;
use crate::menu::Menu;
use crate::splits::geometrie;
use crate::theme_global::{couleur, theme_ou_defaut};

/// Ce qui change d'une pile à l'autre.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct OptionsPile {
    /// `Some(recouvrement)` : cette pile touche le coin haut-droit (`mod-top-right-space`). Elle
    /// réserve `recouvrement` px à droite pour les boutons de fenêtre et montre le bouton du dock
    /// droit. Le recouvrement vient de `Workspace::espace_des_boutons` (la réserve entière à
    /// défaut : `frame_right_space`).
    pub retrait_droit: Option<f32>,
    /// Le glisser d'onglet en cours vise cette pile (`crate::glisser`, fr-t1-splits) : la zone de
    /// dépôt sous le pointeur. Un bord : la feuille visible recule sur la moitié opposée et un bloc
    /// d'accent remplit la moitié libérée (`is-split-preview-*`). `Reorder` : la barre d'insertion,
    /// à l'abscisse [`Self::insertion`].
    pub apercu: Option<DropZone>,
    /// L'abscisse (px logiques, relative à la barre) de la barre d'insertion (`--drop-x`) ; lue
    /// seulement quand [`Self::apercu`] vaut `Reorder`.
    pub insertion: Option<f32>,
}

thread_local! {
    /// La largeur EXACTE (px logiques, avant calage) de chaque pile, notée par la mise en page
    /// pour son prochain rendu ([`noter_largeur_exacte`]) et consommée par [`rendre_pile`].
    static LARGEURS_EXACTES: std::cell::RefCell<std::collections::HashMap<ItemId, (f32, f32)>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

/// Note la largeur EXACTE de `pile` (px logiques, son cadre avant calage au pixel) pour son
/// prochain rendu : le trait du contour de l'onglet actif suit alors les bornes non calées, comme
/// celui de Fragment ([`super::mesures::bords_exacts`]). Sans note, il suit l'en-tête calé par GPUI.
/// `decalage` : le bord gauche exact moins le bord gauche calé (px logiques) ; la ligne de base
/// s'arrête au bord droit exact qui s'en déduit.
pub fn noter_largeur_exacte(pile: ItemId, largeur: f32, decalage: f32) {
    LARGEURS_EXACTES.with(|l| l.borrow_mut().insert(pile, (largeur, decalage)));
}

/// L'opacité du bloc d'accent de l'aperçu de split (workspace.css, `.is-split-preview-*`), en
/// pourcents.
const OPACITE_APERCU: f32 = 22.0;
/// La barre d'insertion du réordonnancement : 3 px de large, à 8 px du haut et du bas de la barre
/// (workspace.css, `.workspace-tab-header-container::after`).
const INSERTION_LARGEUR: f32 = 3.0;
const INSERTION_MARGE: f32 = 8.0;

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

/// Les couleurs du contour : le trait, le fond de la barre et celui du pane (fond de l'onglet actif
/// et des coins concaves).
#[derive(Clone, Copy)]
struct CouleursContour {
    trait_: Couleur,
    barre: Couleur,
    pane: Couleur,
}

/// Le nom de l'élément sous lequel les arcs du contour sont déclarés (exception nommée de T1,
/// `xtask/src/pixels/exceptions.rs`, `CONTOUR_ONGLET`).
const ELEMENT_CONTOUR: &str = "contour d'onglet actif";

/// Un trait droit d'épaisseur `[a, b]` (pixels physiques, en travers), sur les cellules `[d0, d1[`
/// (le long), peint en quads OPAQUES d'un pixel d'épaisseur : la couleur de chaque colonne (ou
/// ligne) est pré-composée par le mélange de Chromium ([`geometrie::melange_couverture`]) sur le
/// fond `fond(x, y)` du centre du pixel (coordonnées physiques).
///
/// ★ Le chemin de GPUI mélange l'anticrénelage au plus proche : le côté gauche du trait, à moitié
/// sur la barre claire, sortait à 222 au lieu des 221 de Chromium (232 + ⌊−21 × 128 / 256⌋).
fn trait_droit(
    window: &mut Window,
    k: f32,
    vertical: bool,
    travers: (f32, f32),
    long: (i32, i32),
    teinte: Couleur,
    fond: &dyn Fn(f32, f32) -> Couleur,
) {
    trait_droit_couvert(window, k, vertical, travers, long, 1.0, teinte, fond);
}

/// [`trait_droit`], chaque cellule couverte à `le_long` (0 à 1) dans le sens du trait : le bout
/// d'un trait qui s'arrête à une abscisse fractionnaire.
#[allow(clippy::too_many_arguments)]
fn trait_droit_couvert(
    window: &mut Window,
    k: f32,
    vertical: bool,
    (a, b): (f32, f32),
    (d0, d1): (i32, i32),
    le_long: f32,
    teinte: Couleur,
    fond: &dyn Fn(f32, f32) -> Couleur,
) {
    let carre = |x0: f32, y0: f32, x1: f32, y1: f32| {
        Bounds::from_corners(point(px(x0 / k), px(y0 / k)), point(px(x1 / k), px(y1 / k)))
    };
    if d1 <= d0 {
        return;
    }
    // ★ Plus fin qu'un pixel (zoom < 100 %) : un filet d'un pixel à l'alpha modulé, pas une surface
    //   ([`geometrie::filet`]) ; à 100 % et au-delà, rien ne change.
    let fin = b - a < 1.0 - 1e-4;
    let cellules: Vec<(i32, f32)> = if fin {
        geometrie::filet(f64::from(a), f64::from(b))
            .into_iter()
            .map(|(c, p)| (c, p as f32))
            .collect()
    } else {
        ((a.floor() as i32)..(b.ceil() as i32))
            .map(|c| (c, (b.min(c as f32 + 1.0) - a.max(c as f32)).max(0.0)))
            .collect()
    };
    for (cellule, part) in cellules {
        let c0 = cellule as f32;
        if part <= 0.0 {
            continue;
        }
        let couleur_en = |t: i32| {
            let (x, y) = if vertical {
                (c0 + 0.5, t as f32 + 0.5)
            } else {
                (t as f32 + 0.5, c0 + 0.5)
            };
            let f = fond(x, y);
            let m = |t: u8, f: u8| {
                if fin {
                    geometrie::melange_filet(t, f, f64::from(part * le_long))
                } else {
                    geometrie::melange_couverture(t, f, part * le_long)
                }
            };
            Couleur::octets(m(teinte.r, f.r), m(teinte.g, f.g), m(teinte.b, f.b), 255)
        };
        // Des suites de pixels de même fond : un quad par suite.
        let mut debut = d0;
        let mut courante = couleur_en(d0);
        for t in d0 + 1..=d1 {
            let suivante = (t < d1).then(|| couleur_en(t));
            if suivante == Some(courante) {
                continue;
            }
            let (t0, t1) = (debut as f32, t as f32);
            let bornes = if vertical {
                carre(c0, t0, c0 + 1.0, t1)
            } else {
                carre(t0, c0, t1, c0 + 1.0)
            };
            window.paint_quad(fill(bornes, couleur(courante)));
            if let Some(c) = suivante {
                (debut, courante) = (t, c);
            }
        }
    }
}

/// Un arc du contour : départ, raccord (début de l'arc), arrivée de l'arc, fin, rayon (px
/// physiques), sens horaire.
type ArcContour = ((f32, f32), (f32, f32), (f32, f32), (f32, f32), f32, bool);

/// Peint le fond des coins concaves, puis le contour, dans la barre `bornes`.
///
/// ★ Les droites (ligne de base, côtés, haut) sont des quads pré-composés, STRICTS ; seuls les
/// quatre arcs (et le bout de droite qui les raccorde au pixel entier suivant) sont des chemins
/// GPUI, déclarés coin par coin sous [`ELEMENT_CONTOUR`] : leur anticrénelage (MSAA 4x contre
/// Skia) est l'exception nommée de T1 rattachée à fr-p4.1.
///
/// Les rayons, puis le haut EXACT de l'onglet actif et la hauteur EXACTE de la barre (repère de la
/// barre, px de l'écran), que suivent les traits horizontaux.
///
/// `bords_exacts` : les bords gauche et droit EXACTS de l'actif (repère de la barre), quand ils
/// diffèrent de l'en-tête calé ; seul le TRAIT les suit. Le fond des coins concaves reste au ras de
/// l'en-tête calé, comme les pseudo-éléments de Fragment (des boîtes, calées elles aussi).
///
/// `droite_exacte` : le bord droit EXACT de la barre (repère de la barre) ; la ligne de base s'y
/// arrête, en cellule partielle.
/// ★ Les séparateurs des onglets inactifs, calés comme Chromium calerait leur `::after` : chaque
/// bord EXACT (sous-pixel) arrondi au pixel, `[⌊R − l⌉, ⌊R⌉)` × `[⌊haut + m⌉, ⌊bas − m⌉)`. Un `div`
/// de GPUI, posé dans l'en-tête calé, tombait à côté au zoom : 1 px au lieu de 2 à 150 %
/// (`t1-zoom-150`, (719,18)), 14 lignes au lieu de 12 à 80 % (`t1-zoom-80`, (526,9) et (526,22)).
///
/// `depart` : le bord gauche (calé) du conteneur des onglets ; `droite(i)` : le bord droit exact
/// de l'onglet `i` dans ce conteneur ; `(haut, hauteur, (marge, largeur))` : le haut exact des
/// onglets et la hauteur exacte de la barre (repère de la barre), la marge et la largeur du trait.
fn peindre_separateurs(
    bornes: Bounds<Pixels>,
    depart: f32,
    rangs: &[usize],
    droite: impl Fn(usize) -> f32,
    (haut, hauteur, (marge, largeur)): (f32, f32, (f32, f32)),
    teinte: Couleur,
    window: &mut Window,
) {
    let k = window.scale_factor();
    let oy = f32::from(bornes.origin.y);
    let (y0, y1) = (
        ((oy + haut + marge) * k).round(),
        ((oy + hauteur - marge) * k).round(),
    );
    // L'opacité de l'en-tête inactif, que le séparateur portait en enfant.
    let mut c = couleur(teinte);
    c.a *= OPACITE_REPOS;
    for &i in rangs {
        let r = depart + droite(i);
        let (x0, x1) = (((r - largeur) * k).round(), (r * k).round());
        if x1 <= x0 || y1 <= y0 {
            continue;
        }
        window.paint_quad(fill(
            Bounds::from_corners(point(px(x0 / k), px(y0 / k)), point(px(x1 / k), px(y1 / k))),
            c,
        ));
    }
}

#[allow(clippy::too_many_arguments)]
fn peindre_contour(
    bornes: Bounds<Pixels>,
    actif: Option<Bounds<Pixels>>,
    bords_exacts: Option<(f32, f32)>,
    droite_exacte: Option<f32>,
    couleurs: CouleursContour,
    (rayon_haut, rayon_bas, haut_exact, hauteur_exacte): (f32, f32, f32, f32),
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

    let k = window.scale_factor();
    let (ox, oy) = (f32::from(bornes.origin.x), f32::from(bornes.origin.y));
    // Repère de la barre (px logiques) vers l'écran (px physiques), et retour.
    let x_ = |x: f32| (ox + x) * k;
    let y_ = |y: f32| (oy + y) * k;
    let pt = |x: f32, y: f32| point(px(x / k), px(y / k));
    let demi = epaisseur * k / 2.0;
    // ★ Les traits horizontaux suivent la hauteur EXACTE de la barre et le haut EXACT de l'onglet
    //   (le rembourrage du conteneur), pas les bornes calées par GPUI : à 80 %, la barre fait 28,8 px
    //   (GPUI : 29) et l'onglet commence à 3,1875 (GPUI : 3). Identiques aux bornes à 100 et 150 %.
    let base = y_(hauteur_exacte) - demi;
    let (barre_g, barre_d) = (x_(0.0).round() as i32, x_(largeur).round() as i32);
    // Le fond sous un pixel : l'en-tête actif (quad de GPUI, calé au pixel) ou la barre.
    let entete = onglet.and(actif).map(|b| {
        let v = |p: Pixels| (f32::from(p) * k).round();
        (
            v(b.origin.x),
            v(b.origin.y),
            v(b.origin.x + b.size.width),
            v(b.origin.y + b.size.height),
        )
    });
    let fond = move |x: f32, y: f32| match entete {
        Some((x0, y0, x1, y1)) if x >= x0 && x < x1 && y >= y0 && y < y1 => couleurs.pane,
        _ => couleurs.barre,
    };
    // ★ Le bord droit EXACT de la barre (px physiques), quand il tombe dans un pixel : Chromium y
    //   arrête la ligne de base (une bordure, anticrénelée), alors que le fond de la barre est calé.
    //   La poignée de split se mélange ensuite par-dessus (`t1-split-vertical`, (657,35) : 42 = 52
    //   sur 33, et 33 = la ligne de base à moitié sur la barre).
    let bout = droite_exacte
        .map(x_)
        .filter(|d| d.fract() > 1e-3 && (d.floor() as i32) < barre_d)
        .map(|d| (d.floor() as i32, d.fract()));
    let ligne_de_base = |window: &mut Window, d0: i32, d1: i32| {
        let d1 = d1.min(barre_d);
        let (d1, reste) = match bout {
            Some((cellule, part)) if d1 == barre_d => (cellule, Some((cellule, part))),
            _ => (d1, None),
        };
        trait_droit(
            window,
            k,
            false,
            (base - demi, base + demi),
            (d0.max(barre_g), d1),
            couleurs.trait_,
            &fond,
        );
        if let Some((cellule, part)) = reste.filter(|&(c, _)| c >= d0.max(barre_g)) {
            trait_droit_couvert(
                window,
                k,
                false,
                (base - demi, base + demi),
                (cellule, cellule + 1),
                part,
                couleurs.trait_,
                &fond,
            );
        }
    };

    let Some(o) = onglet else {
        ligne_de_base(window, barre_g, barre_d);
        return;
    };

    for (bord, gauche) in [(o.gauche, true), (o.droite, false)] {
        let segments = coin_concave(bord, hauteur, rayon_bas, gauche);
        let mut p = PathBuilder::fill();
        tracer(&mut p, &segments, bornes.origin);
        p.close();
        if let Ok(chemin) = p.build() {
            window.paint_path(chemin, couleur(couleurs.pane));
        }
    }

    let (trait_g, trait_d) = bords_exacts.unwrap_or((o.gauche, o.droite));
    let (gx, dx) = (x_(trait_g), x_(trait_d));
    let haut = y_(haut_exact) + demi;
    let (rh, rb) = (rayon_haut * k, rayon_bas * k);
    // Les coupes : le premier pixel entier au-delà de chaque bout d'arc.
    let cote_haut = (haut + rh).ceil();
    let cote_bas = (base - rb).floor();
    let (haut_g, haut_d) = ((gx + rh).ceil(), (dx - rh).floor());
    let (base_g, base_d) = ((gx - rb).floor(), (dx + rb).ceil());

    ligne_de_base(window, barre_g, base_g as i32);
    ligne_de_base(window, base_d as i32, barre_d);
    for cote in [gx, dx] {
        trait_droit(
            window,
            k,
            true,
            (cote - demi, cote + demi),
            (cote_haut as i32, cote_bas as i32),
            couleurs.trait_,
            &fond,
        );
    }
    trait_droit(
        window,
        k,
        false,
        (haut - demi, haut + demi),
        (haut_g as i32, haut_d as i32),
        couleurs.trait_,
        &fond,
    );

    let arcs: [ArcContour; 4] = [
        (
            (gx, cote_haut),
            (gx, haut + rh),
            (gx + rh, haut),
            (haut_g, haut),
            rh,
            true,
        ),
        (
            (haut_d, haut),
            (dx - rh, haut),
            (dx, haut + rh),
            (dx, cote_haut),
            rh,
            true,
        ),
        (
            (base_g, base),
            (gx - rb, base),
            (gx, base - rb),
            (gx, cote_bas),
            rb,
            false,
        ),
        (
            (dx, cote_bas),
            (dx, base - rb),
            (dx + rb, base),
            (base_d, base),
            rb,
            false,
        ),
    ];
    for (depart, raccord, arrivee, fin, r, horaire) in arcs {
        let mut p = PathBuilder::stroke(px(epaisseur));
        p.move_to(pt(depart.0, depart.1));
        if depart != raccord {
            p.line_to(pt(raccord.0, raccord.1));
        }
        p.arc_to(
            point(px(r / k), px(r / k)),
            px(0.0),
            false,
            horaire,
            pt(arrivee.0, arrivee.1),
        );
        if fin != arrivee {
            p.line_to(pt(fin.0, fin.1));
        }
        if let Ok(chemin) = p.build() {
            window.paint_path(chemin, couleur(couleurs.trait_));
        }
    }

    // Les déclarations, en px physiques : le trait de 1 px est la bordure d'une boîte de rayon
    // extérieur `r + demi`, au demi-pixel. Le haut : les deux coins hauts de l'en-tête élargi ;
    // chaque pied : le coin d'une boîte de 2R × 2R posée hors de l'onglet, au ras du bas.
    let declarer = |x: f32, y: f32, cote: (f32, f32), r: f32, coins: [bool; 4]| {
        declarer_arrondi(ArrondiPeint {
            x,
            y,
            largeur: cote.0,
            hauteur: cote.1,
            rayon: r,
            bordure: 2.0 * demi,
            coins,
            element: ELEMENT_CONTOUR,
        });
    };
    let (rh2, rb2) = (rh + demi, rb + demi);
    declarer(
        gx - demi,
        haut - demi,
        (dx - gx + 2.0 * demi, base - haut + 2.0 * demi),
        rh2,
        [true, true, false, false],
    );
    let (bas, pied) = (base + demi, (2.0 * rb2, 2.0 * rb2));
    declarer(
        gx + demi - 2.0 * rb2,
        bas - 2.0 * rb2,
        pied,
        rb2,
        [false, false, false, true],
    );
    declarer(
        dx - demi,
        bas - 2.0 * rb2,
        pied,
        rb2,
        [false, false, true, false],
    );
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
    decalage: f32,
    separateur_par_la_barre: bool,
    titre_exact: Rc<dyn Fn() -> Option<f32>>,
    cx: &App,
) -> AnyElement {
    let t = theme_ou_defaut(cx);
    let (c, f) = (&t.palette, &t.forme);
    let actif = ligne.visible;
    // ★ Chromium pose le titre au bord gauche EXACT de la pile (sous-pixel ; seuls les fonds sont
    //   calés) : le titre du 2e pane de `t1-split-vertical` sort chez nous à +0,6 px (moitié gauche).
    //   Un `relative().left(−0,5)` n'y change RIEN (CI d4a5542, mesures identiques) : GPUI cale la
    //   mise en page au pixel : le texte est décalé à la peinture, et tronqué comme Chromium
    //   ([`super::titre`]).
    let titre = super::titre::titre(ligne.titre.clone().into(), decalage, titre_exact, cx);
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
        // Le titre prend ce qui reste et s'ellipse ; le statut, vide, garde son rang (et donc l'écart).
        .child(titre)
        .child(div().flex().items_center())
        .child(croix);
    if actif {
        interieur = interieur.rounded(u(cx, f.radius_s));
    } else {
        // Le fond de survol vit sur la pilule, pas sur la zone pleine hauteur ; réservé aux
        // inactifs (l'actif a déjà son fond fusionné avec le pane).
        // ★ Ses coins sont DÉCLARÉS (bord d'arrondi d'ADR-009) : non déclarés, l'anticrénelage de
        //   GPUI sortait hors boîte au delta 1 (`t1-onglet-survol`, les 4 coins de la pilule).
        let survol = couleur(c.background_modifier_hover);
        interieur =
            arrondir(interieur, cx, u(cx, f.radius_s), u(cx, 0.0)).hover(move |s| s.bg(survol));
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
            move |evt, _, cx| {
                if let Ok(mut w) = ws.try_borrow_mut() {
                    actions::presser_entete(&mut w, pile, index);
                }
                // L'appui est aussi le début possible d'un glisser d'onglet (fr-t1-splits) : le
                // seuil de 4 px et la suite vivent dans `crate::glisser`.
                crate::glisser::noter_appui(pile, leaf, evt.position, cx);
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
        if !est_dernier && !suivant_actif && !separateur_par_la_barre {
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

/// Un `canvas` qui note les bornes (calées) de son parent, à poser en enfant `absolute`.
fn mesurer(cellule: Rc<Cell<Option<Bounds<Pixels>>>>, cx: &App) -> impl IntoElement {
    canvas(
        move |bornes, _, _| cellule.set(Some(bornes)),
        |_, (), _, _| {},
    )
    .absolute()
    .top(u(cx, 0.0))
    .left(u(cx, 0.0))
    .right(u(cx, 0.0))
    .bottom(u(cx, 0.0))
}

/// La barre d'onglets d'une pile.
fn barre(
    ctx: &ContexteOnglets,
    pile: ItemId,
    lignes: &[LigneOnglet],
    options: OptionsPile,
    (largeur_exacte, decalage): (Option<f32>, f32),
    cx: &App,
) -> AnyElement {
    let t = theme_ou_defaut(cx);
    let (c, f) = (&t.palette, &t.forme);
    let mesure: Rc<Cell<Option<Bounds<Pixels>>>> = Rc::new(Cell::new(None));
    // Les bornes calées du conteneur des onglets et de l'espaceur : la place des onglets, à
    // laquelle s'ajoute la fraction de largeur que le calage a retirée à la pile.
    let place_onglets: Rc<Cell<Option<Bounds<Pixels>>>> = Rc::new(Cell::new(None));
    let place_espaceur: Rc<Cell<Option<Bounds<Pixels>>>> = Rc::new(Cell::new(None));
    let n = lignes.len();
    let rang_actif = lignes.iter().position(|l| l.visible);
    // Les rangs qui portent un séparateur : inactifs, ni le dernier, ni juste avant l'actif.
    let rangs_separes: Vec<usize> = (0..n.saturating_sub(1))
        .filter(|&i| !lignes[i].visible && !lignes[i + 1].visible)
        .collect();

    let zoom = Echelle::courante(cx).zoom();
    // Le séparateur (px logiques) : sa marge haute et basse, au 1/64, et sa largeur.
    let separateur = (
        geometrie::unite_de_mise_en_page(f64::from(f32::from(u(cx, SEPARATEUR_MARGE)))) as f32,
        f32::from(u(cx, SEPARATEUR_LARGEUR)),
    );
    let haut_exact =
        geometrie::unite_de_mise_en_page(f64::from(f32::from(u(cx, f.size_4_1)))) as f32;
    let hauteur_exacte = f32::from(u(cx, f.header_height));
    let (marge, largeur_max) = (
        f32::from(u(cx, f.size_4_1)),
        f32::from(u(cx, LARGEUR_ONGLET)),
    );
    // ★ Les largeurs EXACTES (px d'écran, chacune au 1/64 comme les `LayoutUnit` de Chromium, qui
    //   met en page en px zoomés) de ce qui, dans la barre, n'est pas le conteneur des onglets ni
    //   l'espaceur : « + », chevron, bouton du dock droit et retrait des boutons de fenêtre. GPUI
    //   cale chacune au pixel : à 150 %, la somme calée perdait 1 px et les côtés de l'actif
    //   tombaient 0,25 px à gauche (`t1-zoom-150`, (279,14) et (498,14)).
    let fixes = {
        let lu = |v: f32| geometrie::unite_de_mise_en_page(f64::from(v * zoom)) as f32;
        let bouton = lu(2.0 * f.size_4_1 + f.icon_s);
        let nouvel = lu(NOUVEL_ONGLET_MARGE_GAUCHE) + bouton + lu(f.size_2_1);
        let liste = lu(f.size_2_1) + bouton + lu(f.size_4_1);
        let dock = options.retrait_droit.map_or(0.0, |r| liste + lu(r));
        nouvel + liste + dock
    };
    // ★ Le bord gauche EXACT du titre de l'onglet `i` (px logiques de la fenêtre), quand la largeur
    //   exacte de la pile est connue : Chromium pose le texte au sous-pixel, onglet par onglet ; seuls
    //   les fonds sont calés. Les en-têtes calés de GPUI décalaient les titres de 0,9 à 1,6 px
    //   (`t1-zoom-150`). Lu à la peinture : la place des onglets est notée au prépeint.
    let titre_exact: Rc<dyn Fn(usize) -> Option<f32>> = {
        let po = place_onglets.clone();
        let interieur_titre =
            geometrie::unite_de_mise_en_page(f64::from(f32::from(u(cx, REMBOURRAGE_ENTETE))))
                as f32
                + geometrie::unite_de_mise_en_page(f64::from(f32::from(u(
                    cx,
                    INTERIEUR_COTE_TITRE,
                )))) as f32;
        Rc::new(move |i| {
            let (exacte, o) = (largeur_exacte?, po.get()?);
            let place = geometrie::unite_de_mise_en_page(f64::from(exacte * zoom)) as f32 - fixes;
            let (g, _) = bords_exacts(place, marge, n, i, largeur_max);
            Some(f32::from(o.origin.x) + decalage * zoom + g + interieur_titre)
        })
    };
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
                decalage,
                largeur_exacte.is_some(),
                {
                    let t = titre_exact.clone();
                    Rc::new(move || t(i)) as Rc<dyn Fn() -> Option<f32>>
                },
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
        .children(entetes)
        .child(mesurer(place_onglets.clone(), cx));

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
        .child(
            div()
                .relative()
                .flex_grow(1.0)
                .child(mesurer(place_espaceur.clone(), cx)),
        )
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

    // La barre d'insertion, centrée SUR l'interstice : peinte en dernier, au-dessus des onglets et
    // du contour.
    let insertion = options
        .insertion
        .filter(|_| options.apercu == Some(DropZone::Reorder))
        .map(|x| {
            div()
                .absolute()
                .top(u(cx, INSERTION_MARGE))
                .bottom(u(cx, INSERTION_MARGE))
                .left(u(cx, x - INSERTION_LARGEUR / 2.0))
                .w(u(cx, INSERTION_LARGEUR))
                .rounded(u(cx, f.radius_xxs))
                .bg(couleur(c.color_accent))
        });

    // Le contour : peint APRÈS tous les en-têtes, donc par-dessus le fond de l'actif.
    let couleurs = CouleursContour {
        trait_: c.tab_outline_color,
        barre: c.tab_bar_bg,
        pane: c.workspace_bg,
    };
    let (rayon_haut, rayon_bas) = (u(cx, f.input_radius), u(cx, f.radius_s));
    let epaisseur = u(cx, TRAIT_CONTOUR);
    barre
        .child(
            canvas(
                |_, _, _| {},
                move |bornes, (), window, _| {
                    // Les bords exacts de l'actif, si la mise en page a noté la largeur exacte.
                    let exacts = match (
                        largeur_exacte,
                        rang_actif,
                        place_onglets.get(),
                        place_espaceur.get(),
                    ) {
                        (Some(exacte), Some(rang), Some(o), Some(_)) => {
                            // La largeur exacte est LOGIQUE, les bornes sont à l'écran (zoomées) :
                            // la place des onglets est celle de la pile moins les largeurs exactes
                            // fixes, pas la somme des boîtes calées par GPUI.
                            let place = geometrie::unite_de_mise_en_page(f64::from(exacte * zoom))
                                as f32
                                - fixes;
                            let depart = f32::from(o.origin.x - bornes.origin.x);
                            let (g, d) = bords_exacts(place, marge, n, rang, largeur_max);
                            Some((depart + g, depart + d))
                        }
                        _ => None,
                    };
                    if let (Some(exacte), Some(o)) = (largeur_exacte, place_onglets.get()) {
                        let place = geometrie::unite_de_mise_en_page(f64::from(exacte * zoom))
                            as f32
                            - fixes;
                        peindre_separateurs(
                            bornes,
                            f32::from(o.origin.x),
                            &rangs_separes,
                            |i| bords_exacts(place, marge, n, i, largeur_max).1,
                            (haut_exact, hauteur_exacte, separateur),
                            couleurs.trait_,
                            window,
                        );
                    }
                    peindre_contour(
                        bornes,
                        mesure.get(),
                        exacts,
                        // Repère de la barre (son bord gauche calé) : le contour est un SVG, que
                        // Chromium pose au bord gauche CALÉ (`splits::origine_du_contour`) ; sa
                        // largeur reste exacte.
                        largeur_exacte.map(|e| e * zoom),
                        couleurs,
                        (
                            f32::from(rayon_haut),
                            f32::from(rayon_bas),
                            haut_exact,
                            hauteur_exacte,
                        ),
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
        .children(insertion)
        .into_any_element()
}

/// La feuille visible d'une pile : la vue (en-tête et contenu), avec l'activation au contact.
///
/// ★ Un appui N'IMPORTE OÙ dans la feuille (en capture, avant ses enfants) la rend active, pas
/// seulement son onglet : sinon, revenir écrire dans la note après le chat du dock la laissait
/// inactive, et ses raccourcis partaient au chat. Le focus (`focusin` de Fragment) viendra avec
/// les vues qui ont un champ de saisie.
fn feuille_visible(
    ctx: &ContexteOnglets,
    leaf: ItemId,
    apercu: Option<DropZone>,
    cx: &App,
) -> AnyElement {
    let ws = ctx.workspace.clone();
    let feuille = div()
        .id(("feuille", leaf.index() as usize))
        .absolute()
        .top(u(cx, 0.0))
        .left(u(cx, 0.0))
        .right(u(cx, 0.0))
        .bottom(u(cx, 0.0));
    // L'aperçu de split : la feuille recule, son bord côté cible va au milieu.
    let feuille = match apercu {
        Some(DropZone::SplitRight) => feuille.right(relative(0.5)),
        Some(DropZone::SplitLeft) => feuille.left(relative(0.5)),
        Some(DropZone::SplitTop) => feuille.top(relative(0.5)),
        Some(DropZone::SplitBottom) => feuille.bottom(relative(0.5)),
        _ => feuille,
    };
    feuille
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
    let note = LARGEURS_EXACTES.with(|l| l.borrow_mut().remove(&pile));
    let largeur_exacte = note.map(|(l, _)| l);
    let decalage = note.map_or(0.0, |(_, d)| d);
    let (lignes, visible) = match ctx.workspace.try_borrow() {
        Ok(w) => (
            actions::lignes_de_la_pile(&w, pile),
            w.arbre().active_leaf(pile),
        ),
        // Un emprunt en cours (un écouteur du Workspace qui redessine) : on ne peint rien.
        Err(_) => return div().into_any_element(),
    };
    let mut conteneur = div()
        .relative()
        .flex_1()
        .min_h(u(cx, 0.0))
        .bg(couleur(t.palette.workspace_bg))
        .children(visible.map(|l| feuille_visible(ctx, l, options.apercu, cx)));
    if let Some(zone) = options
        .apercu
        .filter(|z| !matches!(z, DropZone::Reorder | DropZone::Center))
    {
        // Le bloc d'accent remplit la moitié libérée : l'emplacement du futur pane.
        // ★ `opacity: 0.22` sur un fond connu (le pane, `--workspace-bg`) : l'aplat est PRÉ-COMPOSÉ
        //   comme Chromium (ADR-009 c). L'opacité de GPUI, mélangée en flottants, tombait un niveau
        //   à côté en clair (209 au lieu de 210, sur 472 000 px). Les coins sont déclarés (bord
        //   d'arrondi d'ADR-009).
        let p = &t.palette;
        let aplat = p.color_accent.sur_chromium(OPACITE_APERCU, p.workspace_bg);
        let bloc = div()
            .absolute()
            .top(u(cx, 0.0))
            .left(u(cx, 0.0))
            .right(u(cx, 0.0))
            .bottom(u(cx, 0.0))
            .bg(couleur(aplat));
        let bloc = match zone {
            DropZone::SplitRight => bloc.left(relative(0.5)),
            DropZone::SplitLeft => bloc.right(relative(0.5)),
            DropZone::SplitTop => bloc.bottom(relative(0.5)),
            _ => bloc.top(relative(0.5)),
        };
        let bloc = arrondir(bloc, cx, u(cx, t.forme.radius_m), u(cx, 0.0));
        conteneur = conteneur.child(bloc);
    }
    let pile_el = div()
        .flex()
        .flex_col()
        .flex_1()
        .min_w(u(cx, 0.0))
        .min_h(u(cx, 0.0))
        .overflow_hidden()
        .text_color(couleur(t.palette.text_normal))
        .child(barre(
            ctx,
            pile,
            &lignes,
            options,
            (largeur_exacte, decalage),
            cx,
        ))
        .child(conteneur);
    texte(pile_el, cx, t.typo.font_ui_small, 1.5).into_any_element()
}
