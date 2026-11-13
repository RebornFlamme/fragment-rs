// Port de app/src/ui/core/workspace/view-icons.css (motif « bulles ») et de
// app/src/ui/views/doc-widget/doc-widget.css (la vue de grappe ouverte en place).
//! Le dessin GPUI d'une grappe : les bulles (carré ou disque) dans le bandeau de 55 px, SANS fond, et
//! la vue ouverte EN PLACE (en-tête de 54 px — titre centré à côté des bulles —, barre de recherche,
//! lignes avec « Aller à » et ⋮).
//!
//! ★ Les bulles sont UN élément posé au même endroit replié comme ouvert : elles ne bougent pas d'un
//!   pixel quand la grappe s'ouvre ([`origine_bulles`] ne dépend pas de l'état).
//! Les scènes de pixels la pilotent par `pixels:grappe` (`{n, open}`) ; les références des scènes
//! `t5-grappe-*` se capturent au montage.

use std::cell::RefCell;

use fragment_core::{Command, CommandManager};
use fragment_theme::Couleur;
use gpui_kit::{
    AnyElement, App, FontWeight, IntoElement, ParentElement as _, Styled as _, div, svg,
};
use serde_json::Value;

use super::bulles::{self, Bulle, Disposition, MARGE_GAUCHE, OPACITE_DISQUE, SVG_BULLE};
use super::vue::INVITE_RECHERCHE;
use crate::doc_widget::DocWidgetAction;
use crate::doc_widget::icones_vue::{self, HAUTEUR_BULLES, Pastille};
use crate::echelle::u;
use crate::flottant::{ecouter_args, ombrer, texte};
use crate::icones;
use crate::rendu::melange::{aplat_exact, bord_unique};
use crate::theme_global::{couleur, theme_ou_defaut};

/// La largeur de la vue ouverte d'une scène.
const LARGEUR_SCENE: f32 = 320.0;
/// Le côté d'un bouton de l'en-tête et du coin d'une ligne (`.doc-widget-cluster-corner` : 26 px).
const BOUTON: f32 = 26.0;
/// Le SVG du coin d'une ligne : 15 px, 17 pour ⋮.
const SVG_COIN: f32 = 15.0;
const SVG_PLUS: f32 = 17.0;
/// Le coin d'une ligne : 8 px du haut et de la droite.
const COIN: f32 = 8.0;

/// Ce que la scène montre.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneGrappe {
    pub icones: Vec<String>,
    pub ouverte: bool,
}

thread_local! {
    static SCENE: RefCell<Option<SceneGrappe>> = const { RefCell::new(None) };
}

/// Lit `{n, open}` : `n` membres dont la 1re icône est « J » (une conversation de Julien).
pub fn scene_depuis_args(a: &Value) -> SceneGrappe {
    let n = a.get("n").and_then(Value::as_u64).unwrap_or(2).max(2) as usize;
    SceneGrappe {
        icones: vec!["J".to_owned(); n],
        ouverte: a.get("open").and_then(Value::as_bool) == Some(true),
    }
}

/// Enregistre `pixels:grappe` (`{n, open}`).
pub fn commandes_de_scene(commandes: &CommandManager) {
    SCENE.with(|s| *s.borrow_mut() = None);
    let args = ecouter_args(commandes);
    let commande = Command::new("pixels:grappe", "Pixels : poser une grappe de la marge")
        .with_callback(move || {
            let scene = scene_depuis_args(&args.borrow());
            SCENE.with(|s| *s.borrow_mut() = Some(scene));
        });
    let _retrait = commandes.add_command(commande, "pixels");
}

/// La scène courante dessinée, ou rien tant qu'aucune commande n'est venue.
pub fn couche(cx: &App) -> Option<AnyElement> {
    SCENE.with(|s| s.borrow().clone()).map(|s| {
        if s.ouverte {
            ouverte(cx, &s.icones, &lignes_de_scene(s.icones.len()))
        } else {
            replie(cx, &s.icones)
        }
    })
}

fn lignes_de_scene(n: usize) -> Vec<(String, String)> {
    (0..n)
        .map(|i| ("J".to_owned(), format!("Fil {}", i + 1)))
        .collect()
}

/// L'icône embarquée, ou son remplaçant (`minimize-2`, `columns-2` et `file-plus` manquent encore au jeu).
fn icone(nom: &str) -> String {
    if icones::svg(nom).is_some() {
        return icones::chemin(nom);
    }
    let repli = match nom {
        "minimize-2" => "minus",
        "columns-2" => "panel-right",
        "file-plus" => "file-plus-2",
        autre => autre,
    };
    icones::chemin(repli)
}

/// Le coin haut-gauche de la boîte des bulles dans le widget : le même replié et ouvert.
pub fn origine_bulles(n: usize) -> (f32, f32) {
    (MARGE_GAUCHE, Disposition::de(n).haut_dans_bandeau())
}

/// Une bulle : un SVG sur l'encre du texte, un texte sur une pastille teintée, ou le « +N ».
fn bulle(cx: &App, b: &Bulle, disposition: Disposition) -> AnyElement {
    let t = theme_ou_defaut(cx);
    let c = &t.palette;
    let base = div()
        .absolute()
        .left(u(cx, b.x))
        .top(u(cx, b.y))
        .w(u(cx, b.d))
        .h(u(cx, b.d))
        .flex()
        .items_center()
        .justify_center()
        .rounded_full();
    let taille = b.taille_texte(t.typo.font_ui_smallest, disposition);
    let svg_cote = match disposition {
        Disposition::Carre => 13.0,
        Disposition::Disque => b.d * SVG_BULLE,
    };
    let e = match &b.pastille {
        Pastille::Icone(nom) => base.bg(couleur(c.text_normal)).child(
            svg()
                .path(icone(nom))
                .size(u(cx, svg_cote))
                .text_color(couleur(c.workspace_bg)),
        ),
        Pastille::Texte { texte: tx, teinte } => {
            let [r, g, bl] = icones_vue::oklch_vers_rgb(c.avatar_bg_l, c.avatar_bg_c, *teinte);
            let [fr, fg, fb] = icones_vue::oklch_vers_rgb(c.avatar_fg_l, c.avatar_fg_c, *teinte);
            base.bg(couleur(Couleur {
                r,
                g,
                b: bl,
                a: 255,
            }))
            .text_color(couleur(Couleur {
                r: fr,
                g: fg,
                b: fb,
                a: 255,
            }))
            .child(tx.clone())
        }
        Pastille::Plus(n) => {
            // Au disque, une bulle du fond ; au carré, le gris des « +N ».
            let fond = match disposition {
                Disposition::Disque => c.workspace_bg,
                Disposition::Carre => aplat_exact(c.text_normal, 12.0, c.workspace_bg),
            };
            base.bg(couleur(fond))
                .text_color(couleur(c.text_muted))
                .child(format!("+{n}"))
        }
    };
    let e = match disposition {
        Disposition::Disque => ombrer(
            e,
            cx,
            &c.shadow_xs,
            "--shadow-xs",
            u(cx, b.d / 2.0),
            u(cx, 0.0),
        ),
        Disposition::Carre => e,
    };
    texte(e, cx, taille, 1.0)
        .font_weight(FontWeight::SEMIBOLD)
        .into_any_element()
}

/// La boîte des bulles (41 px, ou le disque), sans fond hors le disque gris.
pub fn boite_bulles(cx: &App, icones: &[String]) -> AnyElement {
    let t = theme_ou_defaut(cx);
    let c = &t.palette;
    let disposition = Disposition::de(icones.len());
    let cote = disposition.cote();
    let mut boite = div().relative().flex_none().w(u(cx, cote)).h(u(cx, cote));
    if disposition == Disposition::Disque {
        let gris = aplat_exact(c.text_normal, OPACITE_DISQUE * 100.0, c.workspace_bg);
        boite = boite.child(
            div()
                .absolute()
                .left(u(cx, 0.0))
                .top(u(cx, 0.0))
                .w(u(cx, cote))
                .h(u(cx, cote))
                .rounded_full()
                .bg(couleur(gris)),
        );
    }
    for b in bulles::bulles(icones) {
        boite = boite.child(bulle(cx, &b, disposition));
    }
    boite.into_any_element()
}

/// Les bulles posées à leur place dans le widget (replié comme ouvert).
fn bulles_posees(cx: &App, icones: &[String], decalage: f32) -> AnyElement {
    let (x, y) = origine_bulles(icones.len());
    div()
        .absolute()
        .left(u(cx, x - decalage))
        .top(u(cx, y - decalage))
        .child(boite_bulles(cx, icones))
        .into_any_element()
}

/// La grappe REPLIÉE : le bandeau de 55 px, ses bulles, SANS fond.
pub fn replie(cx: &App, icones: &[String]) -> AnyElement {
    let largeur = icones_vue::largeur_boite(icones.len(), icones_vue::Motif::Bulles);
    div()
        .absolute()
        .top(u(cx, 0.0))
        .left(u(cx, 0.0))
        .w(u(cx, largeur))
        .h(u(cx, HAUTEUR_BULLES))
        .child(bulles_posees(cx, icones, 0.0))
        .into_any_element()
}

/// La grappe OUVERTE en place : l'en-tête (bulles, « N éléments », actions), la barre collante, une
/// ligne par membre (`(icône, titre)`).
pub fn ouverte(cx: &App, icones: &[String], lignes: &[(String, String)]) -> AnyElement {
    let t = theme_ou_defaut(cx);
    let (c, f) = (&t.palette, &t.forme);
    let largeur_icones = icones_vue::largeur_boite(icones.len(), icones_vue::Motif::Bulles);
    let bord = f.border_width;
    let gris_bord = aplat_exact(
        c.background_modifier_border,
        c.opacites.bordure,
        c.workspace_bg,
    );
    let mut actions = div().flex().items_center().gap(u(cx, f.size_2_1));
    for a in DocWidgetAction::TOUTES {
        let (nom, _) = a.icone_et_titre();
        actions = actions.child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .w(u(cx, BOUTON))
                .h(u(cx, BOUTON))
                .child(
                    svg()
                        .path(icone(nom))
                        .size(u(cx, f.icon_m))
                        .text_color(couleur(c.text_muted)),
                ),
        );
    }
    let entete = div()
        .flex()
        .items_center()
        .gap(u(cx, f.size_4_1))
        .h(u(cx, HAUTEUR_BULLES - 1.0))
        .pl(u(cx, largeur_icones))
        .pr(u(cx, f.size_2_3))
        .border_b(u(cx, bord))
        .border_color(couleur(c.tab_outline_color))
        .child(
            div()
                .flex_1()
                .overflow_hidden()
                .whitespace_nowrap()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(couleur(c.text_normal))
                .child(bulles::infobulle(icones.len())),
        )
        .child(actions);
    // La barre collante : la recherche (pilule) et le bouton Filtre.
    let recherche = div()
        .flex_1()
        .flex()
        .items_center()
        .gap(u(cx, f.size_4_1))
        .h(u(cx, BOUTON))
        .px(u(cx, f.size_4_2))
        .rounded_full()
        // ★ Le champ vient de `SearchComponent` (T4) : son habillage exact se vérifiera à la capture.
        .border(u(cx, bord))
        .border_color(couleur(gris_bord))
        .child(
            svg()
                .path(icone("search"))
                .size(u(cx, f.icon_s))
                .text_color(couleur(c.text_faint)),
        )
        .child(
            div()
                .text_color(couleur(c.text_faint))
                .child(INVITE_RECHERCHE),
        );
    let barre = div()
        .flex()
        .items_center()
        .gap(u(cx, f.size_2_3))
        .py(u(cx, f.size_4_2))
        .px(u(cx, f.size_4_3))
        .child(recherche)
        .child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .w(u(cx, BOUTON))
                .h(u(cx, BOUTON))
                .rounded_full()
                .child(
                    svg()
                        .path(icone("list-filter"))
                        .size(u(cx, f.icon_s))
                        .text_color(couleur(c.text_muted)),
                ),
        );
    let mut liste = div().flex().flex_col().child(barre);
    for (i, (ic, titre)) in lignes.iter().enumerate() {
        let avatar = boite_avatar(cx, ic);
        let coin = div()
            .absolute()
            .top(u(cx, COIN))
            .right(u(cx, COIN))
            .flex()
            .gap(u(cx, f.size_2_1))
            .child(bouton_coin(cx, "locate", SVG_COIN))
            .child(bouton_coin(cx, "ellipsis-vertical", SVG_PLUS));
        let mut ligne = div()
            .relative()
            .flex()
            .items_center()
            .gap(u(cx, f.size_4_2))
            .pt(u(cx, f.size_4_3))
            .pb(u(cx, f.size_4_3))
            .pl(u(cx, f.size_4_4))
            .pr(u(cx, f.size_4_10))
            .child(avatar)
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(couleur(c.text_normal))
                    .child(titre.clone()),
            )
            .child(coin);
        if i > 0 {
            ligne = ligne
                .border_t(u(cx, bord))
                .border_color(couleur(c.tab_outline_color));
        }
        liste = liste.child(ligne);
    }
    let boite = div()
        .absolute()
        .top(u(cx, 0.0))
        .left(u(cx, 0.0))
        .w(u(cx, LARGEUR_SCENE))
        .flex()
        .flex_col();
    let boite = bord_unique(
        boite,
        cx,
        (f.radius_ml, bord),
        gris_bord,
        c.workspace_bg,
        false,
    );
    let boite = ombrer(
        boite,
        cx,
        &c.shadow_card,
        "--shadow-card",
        u(cx, f.radius_ml),
        u(cx, bord),
    );
    texte(
        boite
            .child(entete)
            .child(liste)
            .child(bulles_posees(cx, icones, bord)),
        cx,
        t.typo.font_ui_medium,
        1.5,
    )
    .into_any_element()
}

/// L'avatar d'une ligne : la 1re icône du membre, une pastille de 22 px.
fn boite_avatar(cx: &App, ic: &str) -> AnyElement {
    let une = vec![ic.to_owned()];
    let mut b = bulles::bulles(&une);
    let mut boite = div()
        .relative()
        .flex_none()
        .w(u(cx, icones_vue::PASTILLE))
        .h(u(cx, icones_vue::PASTILLE));
    if let Some(p) = b.pop() {
        boite = boite.child(bulle(cx, &p, Disposition::Carre));
    }
    boite.into_any_element()
}

fn bouton_coin(cx: &App, nom: &str, cote: f32) -> AnyElement {
    let t = theme_ou_defaut(cx);
    div()
        .flex()
        .items_center()
        .justify_center()
        .w(u(cx, BOUTON))
        .h(u(cx, BOUTON))
        .rounded_full()
        .child(
            svg()
                .path(icone(nom))
                .size(u(cx, cote))
                .text_color(couleur(t.palette.text_muted)),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn la_scene_lit_ses_arguments() {
        let s = scene_depuis_args(&json!({"n": 6, "open": true}));
        assert_eq!(s.icones.len(), 6);
        assert!(s.ouverte);
        let d = scene_depuis_args(&json!({}));
        assert_eq!(d.icones.len(), 2);
        assert!(!d.ouverte);
    }

    #[test]
    fn les_icones_manquantes_ont_un_remplacant_embarque() {
        for nom in [
            "locate",
            "list-filter",
            "ellipsis-vertical",
            "search",
            "minimize-2",
        ] {
            let chemin = icone(nom);
            let n = chemin.trim_start_matches("icons/").trim_end_matches(".svg");
            assert!(icones::svg(n).is_some(), "{nom} → {chemin}");
        }
    }
}
