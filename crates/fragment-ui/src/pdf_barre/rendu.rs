// Port de app/src/ui/views/pdf/pdf-view.css (`.pdf-toolbar*`, `.pdf-page-*`)
//
//! La barre d'outils PDF dessinée : 36 px de haut, boutons de 26 px (20 px pour un chevron de
//! menu), séparateurs de 1 × 16 px, champ de page de 44 × 24 px. Les mesures sont celles du CSS
//! (`docs/constantes-rendu.md`) ; les jetons de forme (marges, rayons, icônes) viennent du thème.

use std::rc::Rc;

use fragment_pdf::vue_modele::VueModele;
use gpui_kit::{
    AnyElement, App, Div, InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent,
    ParentElement as _, Stateful, Styled as _, div,
};

use super::modele::{BarrePdf, Bouton};
use crate::echelle::u;
use crate::flottant::texte;
use crate::icones;
use crate::onglets::mesures::OPACITE_REPOS;
use crate::theme_global::{couleur, theme_ou_defaut};

/// `.pdf-toolbar { height: 36px }` (pdf-view.css:39).
pub const HAUTEUR_BARRE: f32 = 36.0;
/// `.pdf-toolbar .clickable-icon { width/height: 26px }` (pdf-view.css:58).
pub const BOUTON: f32 = 26.0;
/// `.pdf-toolbar .clickable-icon.pdf-toolbar-dropdown { width: 20px }` (pdf-view.css:66).
pub const BOUTON_MENU: f32 = 20.0;
/// `.pdf-toolbar-separator` : 1 × 16 px (pdf-view.css:81-82).
pub const SEPARATEUR_LARGEUR: f32 = 1.0;
pub const SEPARATEUR_HAUTEUR: f32 = 16.0;
/// `.pdf-page-input` : 44 × 24 px (pdf-view.css:96-97).
pub const CHAMP_LARGEUR: f32 = 44.0;
pub const CHAMP_HAUTEUR: f32 = 24.0;

/// Ce que fait le rendu d'un appui sur un bouton : le bouton, puis la position du pointeur en
/// pixels LOGIQUES (déjà divisée par le zoom de l'app) pour placer le menu sous lui.
pub type SurBouton = Rc<dyn Fn(Bouton, f32, f32, &mut App)>;

/// Le caret du champ : 1 px de large, 14 px de haut (le curseur de texte d'un `<input>`).
pub const CARET_LARGEUR: f32 = 1.0;
pub const CARET_HAUTEUR: f32 = 14.0;

/// La barre d'outils de la vue, champ de page inerte (sans focus ni frappe : les scènes et les
/// tests de la barre seule).
pub fn rendre_barre(
    barre: &BarrePdf,
    vue: &VueModele,
    cx: &App,
    sur_bouton: &SurBouton,
) -> AnyElement {
    rendre_barre_avec_champ(barre, vue, cx, sur_bouton, &|c| c)
}

/// La barre d'outils de la vue. `habiller_champ` équipe le champ de page (poignée de focus,
/// écouteur de frappes, clic) : la barre dessine, la vue montée route le clavier.
pub fn rendre_barre_avec_champ(
    barre: &BarrePdf,
    vue: &VueModele,
    cx: &App,
    sur_bouton: &SurBouton,
    habiller_champ: &dyn Fn(Stateful<Div>) -> Stateful<Div>,
) -> AnyElement {
    let t = theme_ou_defaut(cx);
    let (c, f) = (&t.palette, &t.forme);
    let zoom = crate::echelle::Echelle::courante(cx).zoom();

    let separateur = || {
        div()
            .flex_none()
            .w(u(cx, SEPARATEUR_LARGEUR))
            .h(u(cx, SEPARATEUR_HAUTEUR))
            .mx(u(cx, f.size_2_3))
            .bg(couleur(c.background_modifier_active_hover))
    };

    let bouton = |b: Bouton| {
        let actif = barre.bouton_actif(b);
        let largeur = if b.est_menu() { BOUTON_MENU } else { BOUTON };
        let taille_icone = if b.est_menu() { f.icon_xs } else { f.icon_m };
        // Actif : accent à 18 % sur transparent (`color-mix`), et pleine opacité.
        let fond_actif = couleur(
            c.color_accent
                .melange(18.0, fragment_theme::Couleur::TRANSPARENT),
        );
        let sur = sur_bouton.clone();
        let el = div()
            .id(("pdf-barre", b as usize))
            .flex()
            .items_center()
            .justify_center()
            .flex_none()
            .w(u(cx, largeur))
            .h(u(cx, BOUTON))
            .rounded(u(cx, f.radius_s))
            .child(
                crate::icone::icone_image()
                    .path(icones::chemin(b.icone()))
                    .size(u(cx, taille_icone))
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
            el.bg(fond_actif)
        } else {
            let survol = couleur(c.background_modifier_active_hover);
            el.opacity(OPACITE_REPOS)
                .hover(move |s| s.opacity(1.0).bg(survol))
        }
    };

    let [bas, options_barre, moins, plus, options_zoom] = Bouton::TOUS;

    let champ = vue.champ();
    // Tout sélectionné : fond d'accent sur le texte ; sinon un caret après lui (au focus).
    let valeur = {
        let v = div().child(champ.valeur.clone());
        if champ.focus && champ.tout_selectionne && !champ.valeur.is_empty() {
            v.bg(couleur(
                c.color_accent
                    .melange(35.0, fragment_theme::Couleur::TRANSPARENT),
            ))
        } else {
            v
        }
    };
    let mut saisie = div()
        .id("pdf-page-input")
        .flex()
        .items_center()
        .justify_end()
        .flex_none()
        .w(u(cx, CHAMP_LARGEUR))
        .h(u(cx, CHAMP_HAUTEUR))
        .px(u(cx, f.size_2_3))
        .rounded(u(cx, f.radius_s))
        .border(u(cx, f.border_width))
        .border_color(fragment_theme_transparent())
        .bg(couleur(c.background_modifier_hover))
        .text_color(couleur(c.text_normal))
        .child(valeur);
    if champ.focus {
        saisie = saisie.border_color(couleur(c.color_accent));
        if !champ.tout_selectionne {
            saisie = saisie.child(
                div()
                    .flex_none()
                    .w(u(cx, CARET_LARGEUR))
                    .h(u(cx, CARET_HAUTEUR))
                    .bg(couleur(c.text_normal)),
            );
        }
    }
    let saisie = habiller_champ(saisie);

    let pages = div()
        .flex()
        .items_center()
        .gap(u(cx, f.size_2_3))
        .whitespace_nowrap()
        .child(saisie)
        .child(div().child(vue.total_texte()));

    let gauche = div()
        .flex()
        .items_center()
        .gap(u(cx, f.size_2_1))
        .min_w(u(cx, 0.0))
        .child(bouton(bas))
        .child(bouton(options_barre))
        .child(separateur())
        .child(bouton(moins))
        .child(bouton(plus))
        .child(bouton(options_zoom))
        .child(separateur())
        .child(pages);

    texte(
        div()
            .flex()
            .items_center()
            .flex_none()
            .h(u(cx, HAUTEUR_BARRE))
            .px(u(cx, f.size_4_2))
            .overflow_hidden()
            .bg(couleur(c.workspace_bg))
            .border_b(u(cx, f.border_width))
            .border_color(couleur(c.background_modifier_border))
            .text_color(couleur(c.text_muted))
            .child(gauche),
        cx,
        t.typo.font_ui_small,
        t.typo.line_height_normal,
    )
    .into_any_element()
}

/// `border: … solid transparent` du champ au repos.
fn fragment_theme_transparent() -> crate::theme_global::CouleurGpui {
    couleur(fragment_theme::Couleur::TRANSPARENT)
}
