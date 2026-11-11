// Port de app/src/ui/core/WindowControls.ts et de app/src/ui/core/window-controls.css.
//
//! Les boutons de fenêtre (réduire, agrandir, fermer) : posés par-dessus le coin haut-droit, ils ne
//! participent à AUCUNE mise en page. C'est la pile qui touche le coin qui s'écarte d'eux
//! (`OptionsPile::retrait_droit`, d'après `Workspace::espace_des_boutons`), jamais l'inverse.
//!
//! Le langage visuel de Windows, pas celui des icônes cliquables : 46 px de large, collés, sans
//! arrondi ; la croix vire au rouge au survol, icône blanche.

use gpui_kit::{
    AnyElement, App, InteractiveElement as _, IntoElement, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, div, svg,
};

use crate::echelle::{moitie, u};
use crate::icones;
use crate::onglets::mesures::OPACITE_REPOS;
use crate::theme_global::{couleur, theme_ou_defaut};

/// La hauteur d'un bouton : celle de la barre d'onglets sous lui, 4 + 3 + 3 + 3 + 3 + 18 = 34 px
/// (window-controls.css:31).
pub const HAUTEUR_BOUTON: f32 = 34.0;

/// Ce que fait un bouton.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoutonFenetre {
    Reduire,
    Agrandir,
    Fermer,
}

impl BoutonFenetre {
    /// Les trois, de gauche à droite.
    pub const TOUS: [Self; 3] = [Self::Reduire, Self::Agrandir, Self::Fermer];

    /// L'icône (trait fin, `--icon-stroke`).
    pub fn icone(self) -> &'static str {
        match self {
            Self::Reduire => "minus-fin",
            Self::Agrandir => "square-fin",
            Self::Fermer => "x-fin",
        }
    }
}

/// Le conteneur des trois boutons, en haut à droite de la fenêtre.
pub fn rendre_boutons(cx: &App) -> AnyElement {
    let t = theme_ou_defaut(cx);
    let (c, f) = (&t.palette, &t.forme);
    let boutons = BoutonFenetre::TOUS.into_iter().map(|b| {
        let survol = if b == BoutonFenetre::Fermer {
            (couleur(c.window_close_bg), Some(couleur(c.text_on_accent)))
        } else {
            (couleur(c.background_modifier_hover), None)
        };
        let normal = couleur(c.text_normal);
        let icone = svg()
            .path(icones::chemin(b.icone()))
            .size(u(cx, f.icon_xs))
            .flex_none()
            .text_color(normal);
        let groupe = format!("bouton-fenetre-{}", b as u8);
        let icone = match survol.1 {
            Some(blanc) => icone.group_hover(groupe.clone(), move |s| s.text_color(blanc)),
            None => icone,
        };
        let fond = survol.0;
        div()
            .id(("bouton-fenetre", b as usize))
            .group(groupe)
            .flex()
            .flex_none()
            .w(u(cx, f.frame_button_width))
            .h(u(cx, HAUTEUR_BOUTON))
            // Le centrage à la Chromium (l'égalité vers le haut) : 16 px et 10 px pour une icône de 14.
            .pl(moitie(cx, f.frame_button_width - f.icon_xs))
            .pt(moitie(cx, HAUTEUR_BOUTON - f.icon_xs))
            .opacity(OPACITE_REPOS)
            .hover(move |s| s.opacity(1.0).bg(fond))
            .on_click(move |_, window, _| match b {
                BoutonFenetre::Reduire => window.minimize_window(),
                BoutonFenetre::Agrandir => window.zoom_window(),
                BoutonFenetre::Fermer => window.remove_window(),
            })
            .child(icone)
    });
    div()
        .absolute()
        .top(u(cx, 0.0))
        .right(u(cx, 0.0))
        .flex()
        .children(boutons)
        .into_any_element()
}
