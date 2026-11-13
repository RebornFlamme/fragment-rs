// Port de app/src/ui/views/chat/chat-view.css (`.chat-welcome`) et de la salutation de
// app/src/ui/views/chat/components/historyModel.ts.
//! L'accueil d'une conversation vide : l'icône du fournisseur et « Bon après-midi, Julien »,
//! en serif, centrés. Le composer et le centrage vertical sont ceux de la vue (`fr-t7-vue`) ;
//! ici le groupe « icône + salutation ».

use fragment_chat::history_model::salutation;
use fragment_theme::Theme;
use gpui_kit::{App, IntoElement, ParentElement as _, Styled as _, div, relative, svg};

use super::horloge::Horloge;
use crate::echelle::u;
use crate::icones;
use crate::theme_global::couleur;

/// La police serif de l'accueil (`--font-serif`, index.css:108 : Georgia d'abord).
pub const FAMILLE_SERIF: &str = "Georgia";

/// Le corps de la salutation : 34 px au centre (page), 22 px en panneau (`.chat-welcome`).
#[must_use]
pub fn corps_salutation(page: bool) -> f32 {
    if page { 34.0 } else { 22.0 }
}

/// La phrase d'accueil à l'heure de l'horloge : « Bonsoir, Julien » (« Bonjour » sans nom).
#[must_use]
pub fn texte_accueil(horloge: &Horloge, nom: &str) -> String {
    salutation(horloge.heure(), nom)
}

/// Le groupe « icône du fournisseur + salutation ». `icone` : le nom d'icône du fournisseur
/// (`ChatProvider::icon`), ignoré s'il n'existe pas ; `page` : la vue est au centre (grand corps).
pub fn accueil(
    nom: &str,
    horloge: &Horloge,
    icone: Option<&str>,
    page: bool,
    theme: &Theme,
    cx: &App,
) -> impl IntoElement {
    let p = &theme.palette;
    let corps = corps_salutation(page);
    let f = &theme.forme;
    let bas = if page { f.size_4_6 } else { f.size_4_5 };
    let mut groupe = div()
        .flex()
        .items_center()
        .justify_center()
        .gap(u(cx, f.size_4_3))
        .pt_0()
        .px(u(cx, f.size_4_4))
        .pb(u(cx, bas))
        .text_color(couleur(p.text_normal))
        .font_family(FAMILLE_SERIF)
        .text_size(u(cx, corps))
        .line_height(relative(1.2));
    if let Some(nom_icone) = icone.filter(|n| icones::svg(n).is_some()) {
        groupe = groupe.child(
            div()
                .flex()
                .flex_none()
                .text_color(couleur(p.color_accent))
                .child(
                    svg()
                        .path(icones::chemin(nom_icone))
                        .size(u(cx, corps))
                        .flex_none(),
                ),
        );
    }
    groupe.child(div().child(texte_accueil(horloge, nom)))
}
