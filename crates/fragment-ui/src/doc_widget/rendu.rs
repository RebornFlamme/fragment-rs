// Port de app/src/ui/views/doc-widget/doc-widget.css (la présentation du cadre).
//! Le dessin GPUI du cadre : en-tête de 37 px, bandeau replié de 38 px, anneau d'actif.
//!
//! Le MODÈLE (titre, icônes, actions, replié, actif) vit dans [`super::cadre`] ; ici, une fonction
//! pure [`cadre`] le met en boîtes. Les scènes de pixels la pilotent par `pixels:doc-widget`
//! (`{from, to, open, actions}`) : l'état de scène est posé dans [`SceneCadre`], lu par [`couche`].

use std::cell::RefCell;

use fragment_core::{Command, CommandManager};
use fragment_theme::Couleur;
use gpui_kit::{
    AnyElement, App, FontWeight, IntoElement, ParentElement as _, Styled as _, div, svg,
};
use serde_json::Value;

use super::icones_vue::{self, CHEVAUCHEMENT, HAUTEUR_LIGNE, Motif, PASTILLE, Pastille};
use crate::echelle::u;
use crate::flottant::{chaine, ecouter_args, ombrer, texte};
use crate::icones;
use crate::rendu::melange::{aplat_exact, bord_unique};
use crate::theme_global::{couleur, theme_ou_defaut};

/// La hauteur de l'en-tête ouvert : la ligne des icônes moins 1 px de cadre (`.view-header`).
pub const HAUTEUR_ENTETE: f32 = HAUTEUR_LIGNE - 1.0;
/// La marge gauche de l'en-tête (`--doc-widget-icons-width` par défaut).
const MARGE_GAUCHE: f32 = 10.0;
/// Le côté d'un bouton d'action de l'en-tête.
const BOUTON: f32 = 26.0;
/// La largeur de la boîte d'un widget de scène.
const LARGEUR_SCENE: f32 = 320.0;
/// La hauteur du contenu de scène (la vue hébergée est un vide).
const CONTENU_SCENE: f32 = 120.0;

/// Ce que la scène montre.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneCadre {
    pub replie: bool,
    pub actif: bool,
    pub titre: String,
    pub icones: Vec<String>,
    /// Les actions de la vue (leurs noms d'icône) ; « Réduire » est ajouté en dernier.
    pub actions: Vec<String>,
}

impl Default for SceneCadre {
    fn default() -> Self {
        Self {
            replie: false,
            actif: false,
            titre: "Vue de test".to_owned(),
            icones: vec!["message-circle".to_owned()],
            actions: Vec::new(),
        }
    }
}

thread_local! {
    static SCENE: RefCell<Option<SceneCadre>> = const { RefCell::new(None) };
}

/// Lit `{open, actions, active, title}` des arguments de scène.
pub fn scene_depuis_args(a: &Value) -> SceneCadre {
    let mut s = SceneCadre {
        replie: a.get("open").and_then(Value::as_bool) == Some(false),
        actif: a.get("active").and_then(Value::as_bool) == Some(true),
        ..SceneCadre::default()
    };
    if let Some(t) = chaine(a, "title") {
        s.titre = t.to_owned();
    }
    let n = a.get("actions").and_then(Value::as_u64).unwrap_or(0) as usize;
    s.actions = ["search", "pin", "settings"]
        .iter()
        .take(n)
        .map(|x| (*x).to_owned())
        .collect();
    s
}

/// Enregistre `pixels:doc-widget` (`{from, to, open, actions}`).
pub fn commandes_de_scene(commandes: &CommandManager) {
    SCENE.with(|s| *s.borrow_mut() = None);
    let args = ecouter_args(commandes);
    let commande = Command::new("pixels:doc-widget", "Pixels : poser un cadre DocWidget")
        .with_callback(move || {
            let scene = scene_depuis_args(&args.borrow());
            SCENE.with(|s| *s.borrow_mut() = Some(scene));
        });
    let _retrait = commandes.add_command(commande, "pixels");
}

/// La scène courante dessinée, ou rien tant qu'aucune commande n'est venue.
pub fn couche(cx: &App) -> Option<AnyElement> {
    SCENE.with(|s| s.borrow().clone()).map(|s| cadre(cx, &s))
}

/// Une pastille de 22 px : un SVG sobre, ou un texte sur un fond teinté.
fn pastille(cx: &App, p: &Pastille, rang: usize) -> AnyElement {
    let t = theme_ou_defaut(cx);
    let c = &t.palette;
    let base = div()
        .absolute()
        .top(u(cx, 0.0))
        .left(u(cx, rang as f32 * (PASTILLE - CHEVAUCHEMENT)))
        .w(u(cx, PASTILLE))
        .h(u(cx, PASTILLE))
        .flex()
        .items_center()
        .justify_center()
        .rounded_full();
    let enfant = match p {
        Pastille::Icone(nom) => base.bg(couleur(c.background_secondary_alt)).child(
            svg()
                .path(icones::chemin(nom))
                .size(u(cx, t.forme.icon_s))
                .text_color(couleur(c.text_normal)),
        ),
        Pastille::Texte { texte: tx, teinte } => {
            let [r, g, b] = icones_vue::oklch_vers_rgb(c.avatar_bg_l, c.avatar_bg_c, *teinte);
            let [fr, fg, fb] = icones_vue::oklch_vers_rgb(c.avatar_fg_l, c.avatar_fg_c, *teinte);
            base.bg(couleur(Couleur { r, g, b, a: 255 }))
                .text_color(couleur(Couleur {
                    r: fr,
                    g: fg,
                    b: fb,
                    a: 255,
                }))
                .child(tx.clone())
        }
        Pastille::Plus(n) => base
            .bg(couleur(c.background_secondary_alt))
            .text_color(couleur(c.text_muted))
            .child(format!("+{n}")),
    };
    texte(enfant, cx, t.typo.font_ui_small, 1.0).into_any_element()
}

/// La pile d'icônes (un seul élément, au même endroit ouvert ou replié).
fn pile(cx: &App, icones: &[String]) -> AnyElement {
    let ps = icones_vue::pastilles(icones, Motif::Pile);
    let largeur = icones_vue::largeur_pile(icones.len());
    let mut boite = div()
        .relative()
        .flex_none()
        .w(u(cx, largeur - 18.0))
        .h(u(cx, PASTILLE));
    for (i, p) in ps.iter().enumerate() {
        boite = boite.child(pastille(cx, p, i));
    }
    boite.into_any_element()
}

/// Le cadre entier de la scène `s` : bandeau replié (pastille ronde de 38 px) ou widget ouvert
/// (en-tête de 37 px, trait, contenu), avec l'anneau accent de 2 px quand il est actif.
pub fn cadre(cx: &App, s: &SceneCadre) -> AnyElement {
    let t = theme_ou_defaut(cx);
    let (c, f) = (&t.palette, &t.forme);
    let largeur_icones = icones_vue::largeur_pile(s.icones.len());
    if s.replie {
        let fond = c.background_secondary_alt;
        let b = div()
            .absolute()
            .top(u(cx, 0.0))
            .left(u(cx, 0.0))
            .w(u(cx, largeur_icones))
            .h(u(cx, HAUTEUR_LIGNE))
            .flex()
            .items_center()
            .pl(u(cx, 11.0));
        let b = bord_unique(
            b,
            cx,
            (HAUTEUR_LIGNE / 2.0, f.border_width),
            aplat_exact(c.background_modifier_border, c.opacites.bordure, fond),
            fond,
            true,
        );
        return b.child(pile(cx, &s.icones)).into_any_element();
    }
    // Ouvert : l'anneau d'actif remplace le trait (2 px accent), sinon 1 px de bordure.
    let (largeur_bord, bord) = if s.actif {
        (2.0 * f.border_width, c.color_accent)
    } else {
        (
            f.border_width,
            aplat_exact(
                c.background_modifier_border,
                c.opacites.bordure,
                c.workspace_bg,
            ),
        )
    };
    let mut actions = div().flex().items_center().gap(u(cx, f.size_2_1));
    for nom in s
        .actions
        .iter()
        .map(String::as_str)
        .chain(["chevron-up"])
    {
        actions = actions.child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .w(u(cx, BOUTON))
                .h(u(cx, BOUTON))
                .child(
                    svg()
                        .path(icones::chemin(nom))
                        .size(u(cx, f.icon_m))
                        .text_color(couleur(c.text_muted)),
                ),
        );
    }
    let entete = div()
        .flex()
        .items_center()
        .gap(u(cx, f.size_4_1))
        .h(u(cx, HAUTEUR_ENTETE))
        .pl(u(cx, MARGE_GAUCHE))
        .pr(u(cx, f.size_2_3))
        .border_b(u(cx, f.border_width))
        .border_color(couleur(c.tab_outline_color))
        .child(
            div()
                .w(u(cx, largeur_icones - MARGE_GAUCHE - 7.0))
                .h(u(cx, PASTILLE))
                .flex_none(),
        )
        .child(
            div()
                .flex_1()
                .overflow_hidden()
                .whitespace_nowrap()
                .font_weight(FontWeight::SEMIBOLD)
                .text_size(u(cx, t.typo.font_ui_medium))
                .text_color(couleur(c.text_normal))
                .child(s.titre.clone()),
        )
        .child(actions);
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
        (f.radius_ml, largeur_bord),
        bord,
        c.workspace_bg,
        false,
    );
    let boite = ombrer(
        boite,
        cx,
        &c.shadow_card,
        "--shadow-card",
        u(cx, f.radius_ml),
        u(cx, largeur_bord),
    );
    // La pile d'icônes est posée AU-DESSUS de l'en-tête, à sa place (11 px de marge, centre à 19).
    let pile_posee = div()
        .absolute()
        .top(u(cx, (HAUTEUR_LIGNE - PASTILLE) / 2.0 - largeur_bord))
        .left(u(cx, 11.0 - largeur_bord))
        .child(pile(cx, &s.icones));
    texte(
        boite
            .child(entete)
            .child(div().h(u(cx, CONTENU_SCENE)))
            .child(pile_posee),
        cx,
        t.typo.font_ui_medium,
        1.5,
    )
    .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn la_scene_lit_ses_arguments() {
        let s = scene_depuis_args(&json!({"from": 0, "to": 5, "open": false, "actions": 2}));
        assert!(s.replie);
        assert_eq!(s.actions.len(), 2);
        let d = scene_depuis_args(&json!({}));
        assert!(!d.replie && d.actions.is_empty());
    }

    #[test]
    fn l_entete_fait_la_ligne_des_icones_moins_le_cadre() {
        assert!((HAUTEUR_ENTETE - 37.0).abs() < f32::EPSILON);
    }
}
