// Port de app/src/ui/views/pdf/pdf-view.css (`.pdf-find-bar*`, `.pdf-find-highlight`)
//
//! La barre de recherche PDF dessinée, et les surlignages des résultats.
//!
//! Les mesures sont celles du CSS (`docs/constantes-rendu.md`) : barre de 36 px, boutons de 26 px
//! (20 px pour Aa), champ de 24 px de haut ; les jetons de forme (marges, rayons, icônes) et les
//! couleurs viennent du thème.
//!
//! ★ Les surlignages de Fragment portent `mix-blend-mode: multiply` pour laisser lire le texte
//!   imprimé à travers. GPUI n'a pas de mode de fusion : la couleur est posée avec son
//!   opacité, qui laisse lire le texte de la même façon sur la page blanche.

use std::rc::Rc;

use fragment_text::Rect;
use fragment_theme::Couleur;
use gpui_kit::{
    AnyElement, App, InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent,
    ParentElement as _, Styled as _, div, svg,
};

use super::modele::BarreRecherche;
use crate::echelle::{Echelle, u};
use crate::flottant::texte;
use crate::icones;
use crate::onglets::mesures::OPACITE_REPOS;
use crate::pdf_barre::rendu::{BOUTON, HAUTEUR_BARRE, SEPARATEUR_HAUTEUR, SEPARATEUR_LARGEUR};
use crate::theme_global::{couleur, theme_ou_defaut};

/// `.pdf-find-input-container { height: 24px }` (pdf-view.css:514).
pub const CHAMP_HAUTEUR: f32 = 24.0;
/// `.pdf-find-input-container { flex: 0 1 440px }` (pdf-view.css:509).
pub const CHAMP_LARGEUR: f32 = 440.0;
/// `.pdf-find-bar .clickable-icon.pdf-find-case { width/height: 20px }` (pdf-view.css:562).
pub const BOUTON_CASSE: f32 = 20.0;
/// Le texte du champ vide (`placeholder`).
pub const MARQUE: &str = "Search...";
/// Un contrôle de la barre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bouton {
    /// Aa.
    Casse,
    /// ↑
    Precedent,
    /// ↓
    Suivant,
    /// Le menu des réglages.
    Options,
    /// ✕
    Fermer,
}

impl Bouton {
    /// Dans l'ordre d'affichage.
    pub const TOUS: [Bouton; 5] = [
        Bouton::Casse,
        Bouton::Precedent,
        Bouton::Suivant,
        Bouton::Options,
        Bouton::Fermer,
    ];

    /// L'`aria-label` du bouton (contrat de la spec e2e).
    pub fn libelle(self) -> &'static str {
        super::modele::LIBELLES[self as usize]
    }

    /// Le nom de l'icône du registre.
    pub fn icone(self) -> &'static str {
        match self {
            Bouton::Casse => "case-sensitive",
            Bouton::Precedent => "arrow-up",
            Bouton::Suivant => "arrow-down",
            Bouton::Options => "sliders-horizontal",
            Bouton::Fermer => "x",
        }
    }
}

/// Ce que fait le rendu d'un appui sur un bouton : le bouton, puis la position du pointeur en
/// pixels LOGIQUES (déjà divisée par le zoom de l'app) pour placer un menu sous lui.
pub type SurBouton = Rc<dyn Fn(Bouton, f32, f32, &mut App)>;

/// La barre de recherche. `focus` : le champ a le focus (bordure d'accent).
pub fn rendre_barre(
    barre: &BarreRecherche,
    focus: bool,
    cx: &App,
    sur_bouton: &SurBouton,
) -> AnyElement {
    let t = theme_ou_defaut(cx);
    let (c, f) = (&t.palette, &t.forme);
    let zoom = Echelle::courante(cx).zoom();
    let accent_fond = couleur(c.color_accent.melange(18.0, Couleur::TRANSPARENT));

    let bouton = |b: Bouton| {
        let actif = match b {
            Bouton::Casse => barre.casse_active(),
            Bouton::Options => barre.menu_ouvert(),
            _ => false,
        };
        let cote = if b == Bouton::Casse {
            BOUTON_CASSE
        } else {
            BOUTON
        };
        let sur = sur_bouton.clone();
        let el = div()
            .id(("pdf-recherche", b as usize))
            .flex()
            .items_center()
            .justify_center()
            .flex_none()
            .w(u(cx, cote))
            .h(u(cx, cote))
            .rounded(u(cx, f.radius_s))
            .child(
                svg()
                    .path(icones::chemin(b.icone()))
                    .size(u(cx, f.icon_m))
                    .flex_none()
                    .text_color(couleur(if actif { c.color_accent } else { c.text_muted })),
            )
            .on_mouse_down(MouseButton::Left, move |evt: &MouseDownEvent, _, cx| {
                sur(
                    b,
                    f32::from(evt.position.x) / zoom,
                    f32::from(evt.position.y) / zoom,
                    cx,
                );
                cx.refresh_windows();
            });
        if actif {
            el.bg(accent_fond)
        } else {
            let survol = couleur(c.background_modifier_active_hover);
            el.opacity(OPACITE_REPOS)
                .hover(move |s| s.opacity(1.0).bg(survol))
        }
    };

    // Le contenu du champ : la marque grisée si vide, sinon le texte (sur fond d'accent s'il est
    // tout sélectionné).
    let contenu = if barre.valeur().is_empty() {
        div()
            .text_color(couleur(c.text_faint))
            .child(MARQUE.to_string())
    } else if barre.tout_selectionne() {
        div()
            .bg(couleur(c.color_accent.melange(35.0, Couleur::TRANSPARENT)))
            .text_color(couleur(c.text_normal))
            .child(barre.valeur().to_string())
    } else {
        div()
            .text_color(couleur(c.text_normal))
            .child(barre.valeur().to_string())
    };
    let saisie = div()
        .flex()
        .items_center()
        .flex_1()
        .min_w(u(cx, 0.0))
        .h_full()
        .overflow_hidden()
        .whitespace_nowrap()
        .text_size(u(cx, t.typo.font_ui_medium))
        .child(contenu);

    let compteur = div()
        .flex_none()
        .whitespace_nowrap()
        .text_size(u(cx, t.typo.font_ui_small))
        .child(barre.compteur());
    // `has-no-results` : rouge et opaque ; sinon à 80 %.
    let compteur = if barre.aucun_resultat() {
        compteur.text_color(couleur(c.text_error))
    } else {
        compteur.opacity(0.8)
    };

    let loupe = svg()
        .path(icones::chemin("search"))
        .size(u(cx, f.icon_xs))
        .flex_none()
        .opacity(0.7)
        .text_color(couleur(c.text_muted));

    let contour = if focus {
        couleur(c.color_accent)
    } else {
        couleur(c.background_modifier_border_hover)
    };
    let champ = div()
        .flex()
        .items_center()
        .gap(u(cx, f.size_2_3))
        .flex_shrink(1.0)
        .flex_basis(u(cx, CHAMP_LARGEUR))
        .min_w(u(cx, 0.0))
        .h(u(cx, CHAMP_HAUTEUR))
        .mr(u(cx, f.size_2_3))
        .pl(u(cx, f.size_2_5))
        .pr(u(cx, f.size_4_1))
        .rounded(u(cx, f.radius_sm))
        .border(u(cx, f.border_width))
        .border_color(contour)
        .bg(couleur(c.background_modifier_hover))
        .child(loupe)
        .child(saisie)
        .child(compteur)
        .child(bouton(Bouton::Casse));

    let separateur = div()
        .flex_none()
        .w(u(cx, SEPARATEUR_LARGEUR))
        .h(u(cx, SEPARATEUR_HAUTEUR))
        .mx(u(cx, f.size_2_3))
        .bg(couleur(c.background_modifier_active_hover));

    texte(
        div()
            .id("pdf-recherche-barre")
            .flex()
            .items_center()
            .justify_end()
            .flex_none()
            .gap(u(cx, f.size_2_1))
            .h(u(cx, HAUTEUR_BARRE))
            .px(u(cx, f.size_4_2))
            .overflow_hidden()
            .bg(couleur(c.workspace_bg))
            .border_b(u(cx, f.border_width))
            .border_color(couleur(c.background_modifier_border))
            .text_color(couleur(c.text_muted))
            .child(champ)
            .child(bouton(Bouton::Precedent))
            .child(bouton(Bouton::Suivant))
            .child(bouton(Bouton::Options))
            .child(separateur)
            .child(bouton(Bouton::Fermer)),
        cx,
        t.typo.font_ui_small,
        t.typo.line_height_normal,
    )
    .into_any_element()
}

/// Les couleurs d'un surlignage : (les résultats, le résultat courant), lues dans le thème.
fn couleurs_surlignage(cx: &App) -> (gpui_kit::Hsla, gpui_kit::Hsla) {
    let c = &theme_ou_defaut(cx).palette;
    (
        gpui_kit::Hsla::from(couleur(c.pdf_find_bg)),
        gpui_kit::Hsla::from(couleur(c.pdf_find_bg_current)),
    )
}

/// Un rect de surlignage du plan (unités non zoomées) posé dans la zone des pages : `echelle` est
/// celle du plan, `defilement` le décalage (haut, gauche) en px CSS.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RectSurlignage {
    /// Le rect, en unités du plan non zoomées (les points de la page posés à son coin).
    pub rect: Rect,
    /// C'est celui du résultat courant.
    pub courant: bool,
}

/// Les surlignages en éléments absolus, à poser dans un parent `relative` de la zone des pages.
pub fn rendre_surlignages(
    rects: &[RectSurlignage],
    echelle: f64,
    defilement: (f64, f64),
    cx: &App,
) -> Vec<AnyElement> {
    let t = theme_ou_defaut(cx);
    let rayon = u(cx, t.forme.radius_xxs);
    let (normal, courant) = couleurs_surlignage(cx);
    rects
        .iter()
        .enumerate()
        .map(|(i, r)| {
            div()
                .id(("pdf-recherche-surlignage", i))
                .absolute()
                .left(u(cx, (r.rect.left * echelle - defilement.1) as f32))
                .top(u(cx, (r.rect.top * echelle - defilement.0) as f32))
                .w(u(cx, ((r.rect.right - r.rect.left) * echelle) as f32))
                .h(u(cx, ((r.rect.bottom - r.rect.top) * echelle) as f32))
                .rounded(rayon)
                .bg(if r.courant { courant } else { normal })
                .into_any_element()
        })
        .collect()
}
