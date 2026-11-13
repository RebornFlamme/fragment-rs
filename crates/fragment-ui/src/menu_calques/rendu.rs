// Port de app/src/ui/core/workspace/layer-menu.css (les constantes sont lues dans
// docs/constantes-rendu.md, section `ui/core/workspace/layer-menu.css`).
//! Le dessin du menu « Calques » : la boîte du menu ordinaire (`menu::couche`), des lignes d'arbre.
//!
//! ★ Une ligne de calque est DEUX zones : la zone de ligne (chevron, icône, titre), qui déplie, et
//! à sa droite l'œil et les boutons des contributeurs, qui n'y déplient pas. Le TS arrête la
//! propagation du clic des boutons ; ici ce sont des frères de la zone, jamais ses enfants, ce qui
//! rend le même comportement sans dépendre de la propagation des clics de GPUI.

use gpui_kit::{
    AnyElement, App, InteractiveElement as _, IntoElement, MouseDownEvent, ParentElement as _,
    Pixels, StatefulInteractiveElement as _, Styled as _, Window, anchored, deferred, div, point,
    svg,
};

use crate::echelle::{calees, moitie, u};
use crate::flottant::texte;
use crate::icones;
use crate::rendu::melange::{aplat_exact, bord_unique};
use crate::rendu::ombre::{ombrer_au_calque, voile};
use crate::theme_global::{couleur, theme_ou_defaut};
use fragment_theme::{Aplat, Couleur};

use super::{
    Genre, LigneVue, cliquer_action, cliquer_enfant, cliquer_ligne, cliquer_oeil,
    fermer_menu_calques, lignes_visibles, position,
};

/// `.menu.layer-menu` : largeur, bordure et rembourrage compris (`box-sizing: border-box`).
const LARGEUR: f32 = 280.0;
/// `.layer-menu-chevron` : largeur de la zone, côté du triangle, retrait à droite.
const ZONE_CHEVRON: f32 = 12.0;
const COTE_CHEVRON: f32 = 10.0;
const RETRAIT_CHEVRON: f32 = -2.0;
/// `.layer-menu-action` : côté du bouton et marge droite (`calc(var(--size-4-1) * -1)`).
const COTE_ACTION: f32 = 22.0;
/// Côté de l'icône d'une ligne (comme `menu`).
const ICONE: f32 = 15.0;
/// Opacités : chevron, icône, bouton au repos, calque masqué, ligne désactivée.
const OPACITE_CHEVRON: f32 = 0.5;
const OPACITE_ICONE: f32 = 0.7;
const OPACITE_ACTION: f32 = 0.7;
const OPACITE_MASQUE: f32 = 0.45;
const OPACITE_DESACTIVE: f32 = 0.4;

const VOILE_TRANSPARENT: Couleur = Couleur::TRANSPARENT;

/// Une icône du registre, ou `layers` si un contributeur en nomme une qu'il n'a pas.
fn icone_ou<'a>(nom: &'a str, repli: &'a str) -> &'a str {
    if icones::svg(nom).is_some() {
        nom
    } else {
        repli
    }
}

fn icone_oeil(voulu: bool) -> &'static str {
    if voulu { "eye" } else { "eye-off" }
}

/// Un appui hors de la boîte ferme le menu.
fn appui_exterieur(_: &MouseDownEvent, _: &mut Window, app: &mut App) {
    fermer_menu_calques();
    app.refresh_windows();
}

/// La couche du menu : rien s'il n'est pas ouvert.
pub fn couche(cx: &App) -> AnyElement {
    let Some((x, y)) = position() else {
        return div().into_any_element();
    };
    let boite = boite(&lignes_visibles(), y, cx);
    deferred(
        div()
            .absolute()
            .size_full()
            .child(voile(VOILE_TRANSPARENT))
            .child(
                anchored()
                    .position(point(u(cx, x), u(cx, y)))
                    .snap_to_window_with_margin(u(cx, 8.0))
                    .child(boite),
            ),
    )
    .into_any_element()
}

fn boite(lignes: &[LigneVue], origine_y: f32, cx: &App) -> AnyElement {
    let t = theme_ou_defaut(cx);
    let (c, f) = (&t.palette, &t.forme);
    // ★ 29,5 px par ligne (2 × 5 + interligne 1,5 × 13) : arêtes calées comme Chromium (`calees`).
    let ligne_exacte = 2.0 * 5.0 + 1.5 * t.typo.font_ui_medium;
    let debut = origine_y + f.border_width + f.size_4_1;
    let exactes = vec![ligne_exacte; lignes.len()];
    let hauteurs = calees(cx, debut, &exactes);
    let rendues: Vec<AnyElement> = lignes
        .iter()
        .zip(hauteurs)
        .enumerate()
        .map(|(i, (l, h))| ligne(i, l, h, cx))
        .collect();
    let boite = div()
        .id("menu-calques")
        .relative()
        .flex()
        .flex_col()
        .w(u(cx, LARGEUR))
        .max_w(u(cx, LARGEUR))
        .p(u(cx, f.size_4_1))
        .text_color(couleur(c.text_normal))
        .on_mouse_down_out(appui_exterieur);
    let boite = bord_unique(
        boite,
        cx,
        (f.radius_m, f.border_width),
        aplat_exact(c.background_modifier_border, c.opacites.bordure, c.menu_bg),
        c.menu_bg,
        true,
    )
    .children(rendues);
    let boite = ombrer_au_calque(
        boite,
        cx,
        &c.menu_shadow,
        "--menu-shadow",
        u(cx, f.radius_m),
        u(cx, f.border_width),
        VOILE_TRANSPARENT,
    );
    texte(boite, cx, t.typo.font_ui_medium, 1.5).into_any_element()
}

/// Le gabarit commun : une icône dans sa colonne de `icon_m`, comme `menu`.
fn icone_de_ligne(nom: Option<&str>, opacite: f32, cx: &App) -> AnyElement {
    let t = theme_ou_defaut(cx);
    let f = &t.forme;
    div()
        .w(u(cx, f.icon_m))
        .flex_none()
        .flex()
        .pl(moitie(cx, f.icon_m - ICONE))
        .opacity(opacite)
        .children(nom.map(|n| {
            svg()
                .path(icones::chemin(icone_ou(n, "layers")))
                .size(u(cx, ICONE))
                .flex_none()
                .text_color(couleur(t.palette.text_normal))
        }))
        .into_any_element()
}

fn titre_de_ligne(texte: &str, opacite: f32) -> AnyElement {
    div()
        .flex_1()
        .min_w(Pixels::ZERO)
        .overflow_hidden()
        .text_ellipsis()
        .whitespace_nowrap()
        .opacity(opacite)
        .child(texte.to_owned())
        .into_any_element()
}

/// Un bouton de `.layer-menu-action` : l'œil ou un bouton de contributeur.
fn bouton(
    id: (&'static str, usize),
    icone: &str,
    avertissement: bool,
    cx: &App,
    clic: impl Fn() + 'static,
) -> AnyElement {
    let t = theme_ou_defaut(cx);
    let (c, f) = (&t.palette, &t.forme);
    let survol = couleur(c.aplat(Aplat::SurvolActif, c.menu_bg));
    let rouge = couleur(c.text_error);
    div()
        .id(id)
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .w(u(cx, COTE_ACTION))
        .h(u(cx, COTE_ACTION))
        .mr(u(cx, -f.size_4_1))
        .rounded(u(cx, f.radius_s))
        .cursor_default()
        .opacity(OPACITE_ACTION)
        .text_color(couleur(c.text_normal))
        .hover(move |s| {
            let s = s.opacity(1.0).bg(survol);
            if avertissement {
                s.text_color(rouge)
            } else {
                s
            }
        })
        .child(
            svg()
                .path(icones::chemin(icone_ou(icone, "layers")))
                .size(u(cx, f.icon_xs))
                .flex_none()
                .text_color(couleur(c.text_normal)),
        )
        .on_click(move |_, _: &mut Window, app: &mut App| {
            clic();
            app.refresh_windows();
        })
        .into_any_element()
}

fn ligne(i: usize, l: &LigneVue, hauteur: Pixels, cx: &App) -> AnyElement {
    let t = theme_ou_defaut(cx);
    let (c, f) = (&t.palette, &t.forme);
    let survol = couleur(c.aplat(Aplat::SurvolActif, c.menu_bg));
    let base = div()
        .id(("calque-ligne", i))
        .relative()
        .flex()
        .items_center()
        .gap(u(cx, f.size_2_3))
        .px(u(cx, f.size_4_2))
        .h(hauteur)
        .rounded(u(cx, f.radius_s))
        .cursor_default()
        .text_color(couleur(c.text_normal));
    let cle = l.cle.clone();
    match l.genre {
        Genre::Vide => base
            .opacity(OPACITE_DESACTIVE)
            .child(icone_de_ligne(None, OPACITE_ICONE, cx))
            .child(titre_de_ligne(&l.titre, 1.0))
            .into_any_element(),
        Genre::Enfant => {
            let rang = l.rang;
            base.hover(move |s| s.bg(survol))
                // `.layer-menu-child` : retrait sous le NOM du calque.
                .pl(u(cx, f.size_4_12))
                .child(icone_de_ligne(l.icone.as_deref(), OPACITE_ICONE, cx))
                .child(titre_de_ligne(&l.titre, 1.0))
                .on_click(move |_, _: &mut Window, app: &mut App| {
                    cliquer_enfant(&cle, rang);
                    app.refresh_windows();
                })
                .into_any_element()
        }
        Genre::Calque => {
            // Un calque masqué : icône et titre s'éteignent, l'œil reste net.
            let teinte = if l.oeil { 1.0 } else { OPACITE_MASQUE };
            let opacite_icone = if l.oeil {
                OPACITE_ICONE
            } else {
                OPACITE_MASQUE
            };
            // Sans enfants : la place du chevron reste, le triangle disparaît.
            let chevron = div()
                .w(u(cx, ZONE_CHEVRON))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .mr(u(cx, RETRAIT_CHEVRON))
                .opacity(OPACITE_CHEVRON)
                .children(l.a_enfants.then(|| {
                    svg()
                        .path(icones::chemin(if l.depliee {
                            "right-triangle"
                        } else {
                            "right-triangle-replie"
                        }))
                        .size(u(cx, COTE_CHEVRON))
                        .flex_none()
                        .text_color(couleur(c.text_normal))
                }));
            let zone = div()
                .id(("calque-zone", i))
                .flex_1()
                .min_w(Pixels::ZERO)
                .h_full()
                .flex()
                .items_center()
                .gap(u(cx, f.size_2_3))
                .child(chevron)
                .child(icone_de_ligne(l.icone.as_deref(), opacite_icone, cx))
                .child(titre_de_ligne(&l.titre, teinte))
                .on_click({
                    let cle = cle.clone();
                    move |_, _: &mut Window, app: &mut App| {
                        cliquer_ligne(&cle);
                        app.refresh_windows();
                    }
                });
            let mut ligne = base.hover(move |s| s.bg(survol)).child(zone).child(bouton(
                ("calque-oeil", i),
                icone_oeil(l.oeil),
                false,
                cx,
                {
                    let cle = cle.clone();
                    move || {
                        cliquer_oeil(&cle);
                    }
                },
            ));
            for (j, b) in l.boutons.iter().enumerate() {
                let cle = cle.clone();
                ligne = ligne.child(bouton(
                    ("calque-action", i * 100 + j),
                    &b.icone,
                    b.avertissement,
                    cx,
                    move || {
                        cliquer_action(&cle, j);
                    },
                ));
            }
            ligne.into_any_element()
        }
    }
}
