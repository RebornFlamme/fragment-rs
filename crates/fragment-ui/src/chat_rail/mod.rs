// Port de app/src/ui/views/chat/scroll/{rail,search,searchModel}.ts et de ChatView.ts / ChatPlugin.ts
// (non-lus, lien d'un message)
//! Se repérer dans une longue conversation (tâche `fr-t7-rail-recherche`).
//!
//! - [`modele`] : les décisions pures (tours du rail, première ligne « Nouveau », lien `#mN`) ;
//! - [`rail`] : le rail des tours ([`RailChat`]) ;
//! - [`recherche`] : Ctrl+F ([`RechercheChat`]), surlignages par le calque de la surface ;
//! - la ligne « Nouveau » ([`ligne_nouveau`]).
//!
//! Le montage (le rail et la barre posés dans la vue, Ctrl+F lié, l'écouteur du lien `#mN`, l'action
//! « Copier le lien du message ») est `fr-t7-vue` / `fr-t7-montage` : ici les pièces, testées seules.

pub mod modele;
pub mod rail;
pub mod recherche;

pub use modele::{lien_du_message, premier_non_lu, rang_du_lien, tours_du_rail};
pub use rail::{AuClic, RailChat};
pub use recherche::{EtatRecherche, HoteChat, HoteRecherche, RechercheChat};

use fragment_core::CommandManager;
use gpui_kit::{App, IntoElement, ParentElement as _, Styled as _, div};

use crate::echelle::u;
use crate::flottant::texte;
use crate::theme_global::{couleur, theme_ou_defaut};

/// Le texte de la ligne posée avant le premier message non lu.
pub const LIBELLE_NOUVEAU: &str = "Nouveau";

/// La ligne « Nouveau » : `--font-ui-smaller`, semi-grasse, en accent, soulignée d'un filet
/// d'accent à 40 %, 8 px au-dessus du message (`.is-first-unread::before`). La liste des messages la
/// pose avant [`premier_non_lu`] ; elle est hors du texte de la surface.
pub fn ligne_nouveau(cx: &App) -> impl IntoElement {
    let t = theme_ou_defaut(cx);
    let (c, f) = (&t.palette, &t.forme);
    texte(
        div()
            .flex()
            .items_center()
            .gap(u(cx, f.size_4_2))
            .mb(u(cx, f.size_4_2))
            .pb(u(cx, f.size_2_1))
            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
            .text_color(couleur(c.color_accent))
            .border_b(u(cx, f.border_width))
            .border_color(couleur(
                c.color_accent
                    .melange(40.0, fragment_theme::Couleur::TRANSPARENT),
            ))
            .child(LIBELLE_NOUVEAU),
        cx,
        t.typo.font_ui_smaller,
        t.typo.line_height_normal,
    )
}

/// Enregistre les commandes de scène `pixels:chat_rail` : aucune pour l'instant (les scènes du rail
/// et de la recherche appartiennent aux scènes de la vue chat, `fr-t7-scenes-a`).
pub fn commandes_de_scene(_commandes: &CommandManager) {}
