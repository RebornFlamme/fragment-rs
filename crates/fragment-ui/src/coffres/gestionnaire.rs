// Port de app/src/ui/vault-manager/VaultManager.ts et de sa feuille vault-manager.css.
//
//! La fenêtre du gestionnaire de coffres : une barre de titre maison (la croix seule), la colonne
//! des coffres connus à gauche, l'identité et les deux actions (Créer, Ouvrir) à droite.
//!
//! ★ Elle ne construit ni App, ni Vault, ni Workspace : au premier lancement il n'y a aucun coffre
//!   à lire, et en inventer un pour « afficher quelque chose » ouvrirait un dossier que personne
//!   n'a demandé. Tout ce qu'elle fait passe par [`CommandeGestionnaire`].
//! ★ Un coffre introuvable est MONTRÉ (estompé, « — introuvable », non cliquable) : c'est
//!   justement là que l'utilisateur cherche des notes qu'il croit perdues.

use std::rc::Rc;

use fragment_cli::coffres::{
    LigneGestionnaire, TEXTE_AUCUN_COFFRE, VaultInfo, lignes_gestionnaire,
};
use gpui_kit::{
    AnyElement, App, Context, FontWeight, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement as _, Render, Styled, Window, div,
};

use crate::echelle::u;
use crate::flottant;
use crate::theme_global::{couleur, theme_ou_defaut};

/// La taille de la fenêtre du gestionnaire (`vaults.ts`, fenêtre de gestion).
pub const LARGEUR: f32 = 840.0;
pub const HAUTEUR: f32 = 560.0;
/// Le nom accessible du titre (le logo le dessine).
pub const TITRE: &str = "Fragment";
pub const BOUTON_CREER: &str = "Créer";
pub const BOUTON_OUVRIR: &str = "Ouvrir";

/// Les deux actions du panneau : titre, description, bouton.
pub const ACTIONS: [(&str, &str, &str); 2] = [
    (
        "Créer un nouveau coffre",
        "Choisissez un dossier : il deviendra un coffre.",
        BOUTON_CREER,
    ),
    (
        "Ouvrir un dossier comme coffre",
        "Choisissez un dossier de fichiers Markdown existant.",
        BOUTON_OUVRIR,
    ),
];

// Constantes lues dans vault-manager.css.
/// `.vault-manager-titlebar, .vault-manager-close { height: 32px }`.
const HAUTEUR_BARRE: f32 = 32.0;
/// `.vault-manager-close { opacity: 0.6 }`.
const OPACITE_CROIX: f32 = 0.6;
/// `.vault-manager-list { flex: 0 0 300px }`.
const LARGEUR_LISTE: f32 = 300.0;
/// `.vault-manager-item.is-missing .vault-manager-item-infos { opacity: 0.45 }`.
const OPACITE_INTROUVABLE: f32 = 0.45;
/// `.vault-manager-item-path { -webkit-line-clamp: 2 }`.
const LIGNES_CHEMIN: usize = 2;
/// `.vault-manager-actions { max-width: 420px }`.
const LARGEUR_MAX_ACTIONS: f32 = 420.0;
/// `.vault-manager-button { padding: 7px … }`.
const REMBOURRAGE_BOUTON: f32 = 7.0;
/// `.vault-manager-logo { height: 34px }`.
const HAUTEUR_LOGO: f32 = 34.0;

/// Ce que le gestionnaire demande à l'application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandeGestionnaire {
    Fermer,
    Creer,
    Ouvrir,
    Choisir(String),
    Oublier(String),
}

/// Le relais des commandes vers l'application.
pub type Relais = Rc<dyn Fn(CommandeGestionnaire, &mut App)>;

/// La vue de la fenêtre du gestionnaire.
pub struct VueGestionnaire {
    lister: Rc<dyn Fn() -> Vec<VaultInfo>>,
    relais: Relais,
    lignes: Vec<LigneGestionnaire>,
}

impl VueGestionnaire {
    pub fn new(lister: Rc<dyn Fn() -> Vec<VaultInfo>>, relais: Relais) -> Self {
        let lignes = lignes_gestionnaire(&lister());
        Self {
            lister,
            relais,
            lignes,
        }
    }

    /// Les lignes affichées.
    pub fn lignes(&self) -> &[LigneGestionnaire] {
        &self.lignes
    }

    /// Relit la liste (après « Retirer de la liste », ou quand la fenêtre revient devant).
    pub fn rafraichir(&mut self) {
        self.lignes = lignes_gestionnaire(&(self.lister)());
    }
}

fn graisse(poids: u32) -> FontWeight {
    FontWeight(f32::from(u16::try_from(poids).unwrap_or(600)))
}

/// La Mosaïque : une grille de cellules arrondies (pas de 12, côté 10, rayon 2, unités du lockup),
/// dont deux sortent dans la marge. ★ Géométrie décrite par le logo de Fragment, pas recopiée.
fn mosaique(cx: &App, k: f32, c: fragment_theme::Couleur) -> AnyElement {
    let cellules: [(f32, f32); 12] = [
        (0.0, 0.0),
        (12.0, 0.0),
        (0.0, 12.0),
        (12.0, 12.0),
        (24.0, 12.0),
        (0.0, 24.0),
        (12.0, 24.0),
        (0.0, 36.0),
        (12.0, 36.0),
        (24.0, 36.0),
        (38.0, 0.0),
        (38.0, 24.0),
    ];
    let mut m = div().relative().w(u(cx, 48.0 * k)).h(u(cx, 46.0 * k));
    for (x, y) in cellules {
        m = m.child(
            div()
                .absolute()
                .left(u(cx, x * k))
                .top(u(cx, y * k))
                .size(u(cx, 10.0 * k))
                .rounded(u(cx, 2.0 * k))
                .bg(couleur(c)),
        );
    }
    m.into_any_element()
}

impl Render for VueGestionnaire {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme_ou_defaut(cx);
        let (c, f, ty) = (&t.palette, &t.forme, &t.typo);
        let texte =
            |e: gpui_kit::Div, taille: f32| flottant::texte(e, cx, taille, ty.line_height_normal);

        // ── La barre de titre : la croix seule, à droite.
        let relais = self.relais.clone();
        let croix = texte(div(), ty.font_ui_small)
            .id("vault-manager-close")
            .w(u(cx, f.frame_button_width))
            .h(u(cx, HAUTEUR_BARRE))
            .flex()
            .items_center()
            .justify_center()
            .opacity(OPACITE_CROIX)
            .hover(|s| {
                s.opacity(1.0)
                    .bg(couleur(c.window_close_bg))
                    .text_color(couleur(c.text_on_accent))
            })
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                relais(CommandeGestionnaire::Fermer, cx);
            })
            .child("✕");
        let barre = div()
            .flex_none()
            .h(u(cx, HAUTEUR_BARRE))
            .flex()
            .justify_end()
            .items_center()
            .child(croix);

        // ── La colonne des coffres.
        let mut liste = div()
            .flex_none()
            .w(u(cx, LARGEUR_LISTE))
            .flex()
            .flex_col()
            .gap(u(cx, f.size_2_1))
            .py(u(cx, f.size_4_3))
            .px(u(cx, f.size_4_2))
            .overflow_hidden()
            .bg(couleur(c.background_secondary))
            .border_r(u(cx, f.border_width))
            .border_color(couleur(c.tab_outline_color));
        if self.lignes.is_empty() {
            liste = liste.child(
                texte(div(), ty.font_ui_small)
                    .p(u(cx, f.size_4_3))
                    .text_color(couleur(c.text_muted))
                    .child(TEXTE_AUCUN_COFFRE),
            );
        }
        for (i, l) in self.lignes.iter().enumerate() {
            let mut nom = texte(div(), ty.font_ui_small)
                .font_weight(graisse(ty.font_semibold))
                .whitespace_nowrap()
                .overflow_hidden()
                .text_ellipsis()
                .child(l.nom.clone());
            if l.courant {
                nom = nom.child(div().text_color(couleur(c.color_accent)).child(" ✓"));
            }
            let chemin = texte(div(), ty.font_ui_smaller)
                .text_color(couleur(c.text_muted))
                .line_clamp(LIGNES_CHEMIN)
                .overflow_hidden()
                .child(l.chemin_affiche.clone());
            let mut infos = div()
                .id(("vault-manager-item-infos", i))
                .flex_1()
                .min_w(u(cx, 0.0))
                .child(nom)
                .child(chemin);
            if l.introuvable {
                infos = infos.opacity(OPACITE_INTROUVABLE);
            }
            if l.cliquable {
                let (relais, p) = (self.relais.clone(), l.chemin.clone());
                infos = infos.on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    relais(CommandeGestionnaire::Choisir(p.clone()), cx);
                });
            }
            let (relais, p) = (self.relais.clone(), l.chemin.clone());
            let groupe = gpui_kit::SharedString::from(format!("vault-manager-item-{i}"));
            let oublier = texte(div(), ty.font_ui_small)
                .id(("vault-manager-item-forget", i))
                .flex_none()
                .px(u(cx, f.size_2_3))
                .text_color(couleur(c.text_muted))
                .opacity(0.0)
                .group_hover(groupe.clone(), |s| s.opacity(1.0))
                .hover(|s| s.text_color(couleur(c.text_normal)))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        relais(CommandeGestionnaire::Oublier(p.clone()), cx);
                        this.rafraichir();
                        cx.notify();
                    }),
                )
                .child("⋯");
            liste = liste.child(
                div()
                    .id(("vault-manager-item", i))
                    .group(groupe)
                    .flex()
                    .items_center()
                    .gap(u(cx, f.size_4_1))
                    .rounded(u(cx, f.radius_sm))
                    .py(u(cx, f.size_4_2))
                    .px(u(cx, f.size_2_5))
                    .hover(|s| s.bg(couleur(c.background_modifier_hover)))
                    .child(infos)
                    .child(oublier),
            );
        }

        // ── Le panneau : l'identité, puis les actions.
        let k = HAUTEUR_LOGO / 59.033;
        let mot = flottant::texte(div(), cx, 46.0 * k * 1.4, 1.0)
            .font_weight(graisse(ty.font_semibold))
            .text_color(couleur(c.text_normal))
            .child("fragment");
        let titre = div()
            .flex()
            .items_center()
            .h(u(cx, HAUTEUR_LOGO))
            .gap(u(cx, 12.0 * k))
            .child(mosaique(cx, k, c.text_normal))
            .child(mot);
        let mut actions = div()
            .w_full()
            .max_w(u(cx, LARGEUR_MAX_ACTIONS))
            .flex()
            .flex_col()
            .gap(u(cx, f.size_2_7))
            .p(u(cx, f.size_4_5))
            .border(u(cx, f.border_width))
            .border_color(couleur(c.tab_outline_color))
            .rounded(u(cx, f.radius_ml))
            .bg(couleur(c.background_secondary));
        for (i, (titre_action, description, bouton)) in ACTIONS.into_iter().enumerate() {
            let relais = self.relais.clone();
            let commande = if bouton == BOUTON_CREER {
                CommandeGestionnaire::Creer
            } else {
                CommandeGestionnaire::Ouvrir
            };
            actions = actions.child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(u(cx, f.size_4_4))
                    .child(
                        div()
                            .child(
                                texte(div(), ty.font_ui_small)
                                    .font_weight(graisse(ty.font_semibold))
                                    .child(titre_action),
                            )
                            .child(
                                texte(div(), ty.font_ui_small)
                                    .text_color(couleur(c.text_muted))
                                    .mt(u(cx, f.size_2_1))
                                    .child(description),
                            ),
                    )
                    .child(
                        texte(div(), ty.font_ui_small)
                            .id(("vault-manager-button", i))
                            .flex_none()
                            .py(u(cx, REMBOURRAGE_BOUTON))
                            .px(u(cx, f.size_2_9))
                            .rounded(u(cx, f.radius_sm))
                            .bg(couleur(c.color_accent))
                            .text_color(couleur(c.text_on_accent))
                            .font_weight(graisse(ty.font_semibold))
                            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                relais(commande.clone(), cx);
                            })
                            .child(bouton),
                    ),
            );
        }
        let panneau = div()
            .flex_1()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(u(cx, f.size_4_7))
            .py(u(cx, f.size_4_6))
            .px(u(cx, f.size_4_10))
            .child(titre)
            .child(actions);

        texte(div(), ty.font_ui_small)
            .size_full()
            .flex()
            .flex_col()
            .bg(couleur(c.workspace_bg))
            .text_color(couleur(c.text_normal))
            .child(barre)
            .child(
                div()
                    .flex_1()
                    .flex()
                    .min_h(u(cx, 0.0))
                    .child(liste)
                    .child(panneau),
            )
    }
}
