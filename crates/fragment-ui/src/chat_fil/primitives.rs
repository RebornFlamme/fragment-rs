// Port de app/src/ui/views/chat/components/primitives.ts et primitives.css (le bouton-icône, l'avatar).
//! Les primitives du chat : un bouton-icône et un avatar, en jetons de thème.
//!
//! Les jetons `--chat-*` de primitives.css sont des alias de la palette : `--chat-bg` =
//! `--workspace-bg`, `--chat-subtle` = `--background-secondary`, `--chat-border` =
//! `--tab-outline-color`, `--chat-fg` = `--text-normal`, `--chat-muted` = `--text-muted`.

use fragment_chat::api::Participant;
use fragment_theme::{Couleur, Theme};
use gpui_kit::{
    App, ElementId, InteractiveElement as _, IntoElement, ParentElement as _, Role, SharedString,
    StatefulInteractiveElement as _, Styled as _, div, px, svg,
};

use super::modele::{ContenuAvatar, contenu_avatar, oklch_vers_rgb};
use crate::echelle::u;
use crate::icones;
use crate::polices::FAMILLE_INTERFACE;
use crate::theme_global::couleur;

/// La variante d'un bouton (`data-variant`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variante {
    /// Muet, un fond au survol.
    Discret,
    /// Un contour.
    Contour,
    /// Plein, rond : le bouton d'envoi.
    Primaire,
}

/// Le côté d'un bouton : 26 px (sm) ou 32 px (md) (`.chat-button[data-size]`, primitives.css:33-34).
#[must_use]
pub fn cote_bouton(grand: bool) -> f32 {
    if grand { 32.0 } else { 26.0 }
}

/// Le côté de l'icône d'un bouton (`.chat-button svg`, primitives.css:35).
pub const COTE_ICONE_BOUTON: f32 = 15.0;

/// Un bouton-icône : étiquette accessible obligatoire. `actif` faux : estompé (0,4) et inerte
/// (`.chat-button[disabled]`).
///
/// Le clic est branché par l'appelant (`.on_click(…)`) sur l'élément rendu.
pub fn bouton_icone(
    id: impl Into<SharedString>,
    icone: &str,
    libelle: &str,
    variante: Variante,
    actif: bool,
    theme: &Theme,
    cx: &App,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    let p = &theme.palette;
    let (fg, fond_survol) = (p.text_normal, p.background_secondary);
    let cote = u(cx, cote_bouton(false));
    let mut b = div()
        .id(ElementId::from(id.into()))
        .role(Role::Button)
        .aria_label(libelle.to_owned())
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(cote)
        .child(
            svg()
                .path(icones::chemin(icone))
                .size(u(cx, COTE_ICONE_BOUTON))
                .flex_none(),
        );
    b = match variante {
        Variante::Discret => b
            .rounded(u(cx, theme.forme.radius_sm))
            .text_color(couleur(p.text_muted))
            .hover(move |s| s.bg(couleur(fond_survol)).text_color(couleur(fg))),
        Variante::Contour => b
            .rounded(u(cx, theme.forme.radius_sm))
            .border(px(theme.forme.border_width))
            .border_color(couleur(p.tab_outline_color))
            .text_color(couleur(p.text_muted))
            .hover(move |s| s.bg(couleur(fond_survol)).text_color(couleur(fg))),
        Variante::Primaire => b
            .rounded(px(999.0))
            .bg(couleur(p.text_normal))
            .text_color(couleur(p.workspace_bg)),
    };
    if actif {
        b.cursor_pointer()
    } else {
        b.opacity(0.4)
    }
}

/// Le côté d'un avatar : 22 px (`.view-icon`), 28 px en grand (`.chat-avatar[data-size='md']`).
#[must_use]
pub fn cote_avatar(grand: bool) -> f32 {
    if grand { 28.0 } else { 22.0 }
}

/// La couleur d'une pastille d'avatar : `oklch(L C teinte)`, les luminosités et chromas du thème
/// (`--avatar-*`, index.css).
fn pastille(l: f32, c: f32, teinte: u32) -> Couleur {
    // Les teintes sont des angles (≤ 360) : exacts en f32.
    #[allow(clippy::cast_precision_loss)]
    let [r, g, b] = oklch_vers_rgb(l, c, teinte as f32);
    Couleur::octets(r, g, b, 255)
}

/// L'avatar d'un participant : la MÊME icône que la vue montre repliée (`renderViewIcon`) — un
/// nom d'icône, sinon des initiales sur une pastille teintée par le texte —, nommé par le
/// participant.
pub fn avatar(p: &Participant, grand: bool, theme: &Theme, cx: &App) -> impl IntoElement {
    let cote = u(cx, cote_avatar(grand));
    let corps = if grand {
        11.5
    } else {
        theme.typo.font_ui_smallest
    };
    let pal = &theme.palette;
    let base = div()
        .id(ElementId::from(SharedString::from(format!(
            "chat-avatar-{}",
            p.id
        ))))
        .role(Role::Image)
        .aria_label(p.name.clone())
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(cote)
        .rounded(px(999.0))
        .font_family(FAMILLE_INTERFACE);
    match contenu_avatar(p, |n| icones::svg(n).is_some()) {
        ContenuAvatar::Icone(nom) => base
            .bg(couleur(pal.text_normal))
            .text_color(couleur(pal.workspace_bg))
            .child(
                svg()
                    .path(icones::chemin(&nom))
                    .size(u(cx, 13.0))
                    .flex_none(),
            ),
        ContenuAvatar::Texte { texte, teinte } => base
            .bg(couleur(pastille(pal.avatar_bg_l, pal.avatar_bg_c, teinte)))
            .text_color(couleur(pastille(pal.avatar_fg_l, pal.avatar_fg_c, teinte)))
            .text_size(px(corps))
            .font_weight(gpui_kit::FontWeight(f32::from(
                u16::try_from(theme.typo.font_semibold).unwrap_or(600),
            )))
            .line_height(px(corps))
            .child(texte),
    }
}
