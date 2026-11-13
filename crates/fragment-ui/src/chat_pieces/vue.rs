// Port du rendu de app/src/ui/views/chat/components/attachments.ts (`dessiner`, `attachmentPart`)
// et de attachments.css.
//! Le rendu des pièces : puces du champ (vignette de 56 px, ✕ au survol), part `attachment` d'un
//! message, lecture du presse-papiers.
//!
//! Les constantes sont celles de `docs/constantes-rendu.md` (attachments.css) ; les couleurs sont
//! des jetons du thème (`--chat-border` = `tab_outline_color`, fond d'une pièce =
//! `background_secondary`).

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use fragment_chat::api::{AttachmentPartData, ChatAttachment, ChatPart, ChatSlotContext};
use fragment_theme::Theme;
use gpui_kit::{
    AnyElement, AnyView, App, AppContext as _, ClipboardEntry, ClipboardItem, Context, Div,
    ElementId, ImageSource, InteractiveElement as _, IntoElement, ObjectFit, ParentElement as _,
    Render, RenderImage, SharedString, StatefulInteractiveElement as _, Styled as _,
    StyledImage as _, Window, div, img, px, svg,
};

use super::donnees::{est_image, octets_de_url, piece_depuis_octets, pieces_de_chemins};
use super::etat::EtatPartage;
use crate::chat_fil::primitives::{Variante, bouton_icone};
use crate::chat_fil::{PartVivant, RenduDePart, ZoneChamp};
use crate::echelle::u;
use crate::icones;
use crate::polices::FAMILLE_INTERFACE;
use crate::theme_global::{couleur, theme_ou_defaut};

/// La vignette d'un champ : un carré de 56 px (`.chat-composer-files .chat-file-thumb`).
pub const COTE_VIGNETTE_CHAMP: f32 = 56.0;
/// Les bornes d'une vignette dans un message (`.chat-file-thumb`).
pub const VIGNETTE_MIN: f32 = 32.0;
pub const VIGNETTE_MAX_L: f32 = 220.0;
pub const VIGNETTE_MAX_H: f32 = 160.0;
/// Une puce : largeur max 220 px, hauteur mini 56 px dans le champ (`.chat-file`, `.chat-file-chip`).
pub const PUCE_MAX_L: f32 = 220.0;
pub const PUCE_MIN_H: f32 = 56.0;
/// Le ✕ : 20 px, à 2 px du coin haut droit (`.chat-file-remove`).
pub const COTE_RETRAIT: f32 = 20.0;
pub const DECALAGE_RETRAIT: f32 = 2.0;

/// La vignette décodée d'une image (PNG ; les autres formats ne sont pas décodés ici : la pièce
/// garde alors l'icône et le nom). GPUI peint du BGRA.
#[must_use]
pub fn vignette_de(att: &ChatAttachment) -> Option<Arc<RenderImage>> {
    if !est_image(&att.mime) {
        return None;
    }
    let mut rgba = image::load_from_memory(&att.data).ok()?.into_rgba8();
    for p in rgba.pixels_mut() {
        p.0.swap(0, 2);
    }
    Some(Arc::new(RenderImage::new(vec![image::Frame::new(rgba)])))
}

/// La taille d'une vignette DANS UN MESSAGE : la taille naturelle réduite pour tenir dans
/// 220 × 160 en gardant les proportions, jamais sous 32 px (`.chat-file-thumb`).
#[must_use]
pub fn taille_vignette_message(l: f32, h: f32) -> (f32, f32) {
    if l <= 0.0 || h <= 0.0 {
        return (VIGNETTE_MIN, VIGNETTE_MIN);
    }
    let echelle = (VIGNETTE_MAX_L / l).min(VIGNETTE_MAX_H / h).min(1.0);
    (
        (l * echelle).max(VIGNETTE_MIN),
        (h * echelle).max(VIGNETTE_MIN),
    )
}

/// L'icône d'une pièce sans vignette : `file-text` pour un PDF, `file` sinon.
#[must_use]
pub fn icone_de_piece(mime: &str) -> &'static str {
    if mime == "application/pdf" {
        "file-text"
    } else {
        "file"
    }
}

/// Les pièces d'un collage : les images et les fichiers copiés ; vide si le presse-papiers ne porte
/// que du texte (le collage normal du champ).
#[must_use]
pub fn pieces_du_presse_papiers(item: &ClipboardItem) -> Vec<ChatAttachment> {
    let mut pieces = Vec::new();
    for e in item.entries() {
        match e {
            ClipboardEntry::Image(i) => pieces.push(piece_depuis_octets(
                None,
                Some(i.format.mime_type()),
                i.bytes.clone(),
            )),
            ClipboardEntry::ExternalPaths(p) => pieces.extend(pieces_de_chemins(p.paths())),
            ClipboardEntry::String(_) => {}
        }
    }
    pieces
}

/// Le dessin d'une pièce : la boîte (`.chat-file`) et son contenu. `dans_champ` : la vignette
/// carrée de 56 px ; sinon celle d'un message.
fn dessiner(
    d: &AttachmentPartData,
    vignette: Option<&Arc<RenderImage>>,
    dans_champ: bool,
    theme: &Theme,
    cx: &App,
) -> Div {
    let p = &theme.palette;
    let boite = div()
        .relative()
        .flex()
        .items_center()
        .gap(u(cx, theme.forme.size_2_3))
        .max_w(px(PUCE_MAX_L))
        .border(px(theme.forme.border_width))
        .border_color(couleur(p.tab_outline_color))
        .rounded(u(cx, theme.forme.radius_m))
        .bg(couleur(p.background_secondary))
        .text_color(couleur(p.text_normal))
        .text_size(px(theme.typo.font_ui_small))
        .font_family(FAMILLE_INTERFACE)
        .overflow_hidden();
    let boite = if dans_champ {
        boite.min_h(px(PUCE_MIN_H))
    } else {
        boite
    };
    if let Some(i) = vignette {
        let image = if dans_champ {
            img(ImageSource::Render(i.clone()))
                .w(px(COTE_VIGNETTE_CHAMP))
                .h(px(COTE_VIGNETTE_CHAMP))
                .object_fit(ObjectFit::Cover)
        } else {
            let t = i.size(0);
            let (l, h) = taille_vignette_message(t.width.0 as f32, t.height.0 as f32);
            img(ImageSource::Render(i.clone()))
                .w(px(l))
                .h(px(h))
                .object_fit(ObjectFit::Contain)
        };
        return boite.child(image.flex_none());
    }
    boite
        .px(u(cx, theme.forme.size_4_2))
        .py(u(cx, theme.forme.size_2_3))
        .child(
            svg()
                .path(icones::chemin(icone_de_piece(&d.mime)))
                .size(u(cx, theme.forme.icon_s))
                .flex_none()
                .text_color(couleur(p.text_muted)),
        )
        .child(
            div()
                .min_w(px(0.0))
                .overflow_hidden()
                .text_ellipsis()
                .whitespace_nowrap()
                .child(d.name.clone()),
        )
}

/// La zone des puces, au-dessus du texte du champ : cachée sans pièce ; un ✕ par puce, visible au
/// survol.
#[must_use]
pub fn zone_puces(courant: Rc<RefCell<Option<EtatPartage>>>) -> ZoneChamp {
    Rc::new(move |_window: &mut Window, cx: &mut App| -> AnyElement {
        let Some(etat) = courant.borrow().clone() else {
            return div().hidden().into_any_element();
        };
        let pieces: Vec<_> = etat.borrow().pieces().to_vec();
        if pieces.is_empty() {
            return div().hidden().into_any_element();
        }
        let theme = theme_ou_defaut(cx);
        let f = &theme.forme;
        let mut zone = div()
            .id("chat-composer-files")
            .flex()
            .flex_wrap()
            .gap(u(cx, f.size_2_3))
            .pt(u(cx, f.size_2_1))
            .px(u(cx, f.size_2_1))
            .pb(u(cx, f.size_2_3));
        for (i, piece) in pieces.iter().enumerate() {
            let groupe = SharedString::from(format!("chat-file-chip-{i}"));
            let etat = etat.clone();
            let retrait = bouton_icone(
                format!("chat-file-remove-{i}"),
                "x",
                &format!("Retirer {}", piece.att.name),
                Variante::Discret,
                true,
                &theme,
                cx,
            )
            .absolute()
            .top(px(DECALAGE_RETRAIT))
            .right(px(DECALAGE_RETRAIT))
            .size(px(COTE_RETRAIT))
            .bg(couleur(theme.palette.background_primary))
            .opacity(0.0)
            .group_hover(groupe.clone(), |s| s.opacity(1.0))
            .on_click(move |_, window, _| {
                etat.borrow_mut().retirer(i);
                window.refresh();
            });
            zone = zone.child(
                dessiner(&piece.data, piece.vignette.as_ref(), true, &theme, cx)
                    .id(ElementId::from(groupe.clone()))
                    .group(groupe)
                    .child(retrait),
            );
        }
        zone.into_any_element()
    })
}

/// La vue d'un part `attachment` dans un message.
struct PieceVue {
    data: AttachmentPartData,
    vignette: Option<Arc<RenderImage>>,
}

impl Render for PieceVue {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = theme_ou_defaut(cx);
        let f = &theme.forme;
        // `.chat-part[data-part='attachment']` : marge 2-3 2-3 0 0, au début de la ligne.
        div()
            .id(SharedString::from(format!("chat-piece-{}", self.data.name)))
            .self_start()
            .mt(u(cx, f.size_2_3))
            .mr(u(cx, f.size_2_3))
            .aria_label(self.data.name.clone())
            .child(dessiner(
                &self.data,
                self.vignette.as_ref(),
                false,
                &theme,
                cx,
            ))
    }
}

struct PieceVivante(AnyView);

impl PartVivant for PieceVivante {
    fn vue(&self) -> AnyView {
        self.0.clone()
    }
}

/// Le rendu du part `attachment` (`chat:part@1`).
#[derive(Clone, Copy, Debug, Default)]
pub struct RenduPieceJointe;

impl RenduDePart for RenduPieceJointe {
    fn type_part(&self) -> &str {
        "attachment"
    }

    fn creer(
        &self,
        part: &ChatPart,
        _ctx: &ChatSlotContext,
        _window: &mut Window,
        cx: &mut App,
    ) -> Option<Box<dyn PartVivant>> {
        let data: AttachmentPartData = serde_json::from_value(part.data.clone()?).ok()?;
        // L'image d'une relecture arrive en URL `data:` : on la décode une fois.
        let vignette = data
            .url
            .as_deref()
            .and_then(octets_de_url)
            .and_then(|(mime, octets)| {
                vignette_de(&ChatAttachment {
                    name: data.name.clone(),
                    mime,
                    data: octets,
                })
            });
        let vue = cx.new(|_| PieceVue { data, vignette });
        Some(Box::new(PieceVivante(vue.into())))
    }
}
