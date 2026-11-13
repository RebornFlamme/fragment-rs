// Port du rendu de app/src/ui/views/chat/components/quote.ts (`dessiner`, `quotePart`) et de quote.css.
//! Le rendu de la citation : la puce au-dessus du champ, et le part `quote` d'un message.
//!
//! Le trait à gauche est celui d'une citation Markdown (`color_accent_2`), la teinte 8 % de
//! `color_accent` sur transparent (`color-mix`). Constantes : `docs/constantes-rendu.md` (quote.css).

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use fragment_chat::api::{ChatAttachment, ChatPart, ChatSlotContext, QuotePartData};
use fragment_theme::{Couleur, Theme};
use gpui_kit::{
    AnyElement, AnyView, App, AppContext as _, Context, Div, ImageSource, InteractiveElement as _,
    IntoElement, ObjectFit, ParentElement as _, Render, RenderImage, Role, SharedString,
    StatefulInteractiveElement as _, Styled as _, StyledImage as _, Window, div, img, px,
};

use super::modele::{EXTRAIT_MAX, EtatPartage, extrait};
use crate::chat_fil::primitives::{Variante, bouton_icone};
use crate::chat_fil::{PartVivant, RenduDePart, ZoneChamp};
use crate::chat_pieces::donnees::octets_de_url;
use crate::chat_pieces::vue::vignette_de;
use crate::echelle::u;
use crate::icones;
use crate::polices::FAMILLE_INTERFACE;
use crate::theme_global::{couleur, theme_ou_defaut};

/// Les bornes de la vignette d'une zone citée (`.chat-quote-thumb`).
pub const VIGNETTE_MAX_H: f32 = 40.0;
pub const VIGNETTE_MAX_L: f32 = 80.0;

/// La taille de la vignette d'une zone citée : naturelle, réduite pour tenir dans 80 × 40.
#[must_use]
pub fn taille_vignette(l: f32, h: f32) -> (f32, f32) {
    if l <= 0.0 || h <= 0.0 {
        return (VIGNETTE_MAX_H, VIGNETTE_MAX_H);
    }
    let e = (VIGNETTE_MAX_L / l).min(VIGNETTE_MAX_H / h).min(1.0);
    (l * e, h * e)
}

/// La teinte de fond d'une citation : `color-mix(accent 8 %, transparent)`.
#[must_use]
pub fn fond_citation(theme: &Theme) -> Couleur {
    theme
        .palette
        .color_accent
        .melange(8.0, Couleur::TRANSPARENT)
}

fn vignette_de_donnees(d: &QuotePartData) -> Option<Arc<RenderImage>> {
    let (mime, octets) = d.image.as_ref()?.url.as_deref().and_then(octets_de_url)?;
    vignette_de(&ChatAttachment {
        name: "zone".into(),
        mime,
        data: octets,
    })
}

/// La boîte d'une citation (`.chat-quote`) et son contenu : la vignette de la zone, puis l'extrait.
fn dessiner(
    d: &QuotePartData,
    vignette: Option<&Arc<RenderImage>>,
    theme: &Theme,
    cx: &App,
) -> Div {
    let p = &theme.palette;
    let f = &theme.forme;
    let mut boite = div()
        .flex()
        .items_center()
        .gap(u(cx, f.size_2_3))
        .min_w(px(0.0))
        // ★ `padding: var(--size-2-2) var(--size-2-4)` : `--size-2-4` n'existe pas dans Fragment, la
        // déclaration entière est invalide à l'usage et le remplissage retombe à 0.
        .border_l(px(f.border_width))
        .border_color(couleur(p.color_accent_2))
        .rounded(u(cx, f.radius_xs))
        .bg(couleur(fond_citation(theme)))
        .text_color(couleur(p.text_muted))
        .text_size(px(theme.typo.font_ui_small))
        .font_family(FAMILLE_INTERFACE);
    if let Some(i) = vignette {
        let t = i.size(0);
        let (l, h) = taille_vignette(t.width.0 as f32, t.height.0 as f32);
        boite = boite.child(
            img(ImageSource::Render(i.clone()))
                .w(px(l))
                .h(px(h))
                .flex_none()
                .rounded(u(cx, f.radius_xs))
                .object_fit(ObjectFit::Contain),
        );
    }
    if let Some(t) = d.text.as_deref().filter(|t| !t.is_empty()) {
        boite = boite.child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .overflow_hidden()
                .text_ellipsis()
                .whitespace_nowrap()
                .child(extrait(t, EXTRAIT_MAX)),
        );
    }
    boite
}

/// La zone de la puce, au-dessus du texte du champ : cachée sans citation.
#[must_use]
pub fn zone_citation(courant: Rc<RefCell<Option<EtatPartage>>>) -> ZoneChamp {
    Rc::new(move |_window: &mut Window, cx: &mut App| -> AnyElement {
        let Some(etat) = courant.borrow().clone() else {
            return div().hidden().into_any_element();
        };
        let Some(q) = etat.borrow().citation().cloned() else {
            return div().hidden().into_any_element();
        };
        let theme = theme_ou_defaut(cx);
        let f = &theme.forme;
        let donnees = super::modele::donnees(&q);
        let vignette = vignette_de_donnees(&donnees);
        let retrait = bouton_icone(
            "chat-quote-remove",
            "x",
            "Retirer la citation",
            Variante::Discret,
            true,
            &theme,
            cx,
        )
        .on_click(move |_, window, _| {
            etat.borrow_mut().retirer();
            window.refresh();
        });
        // `.chat-composer-quote` : padding 2-2 2-3 0 ; `.chat-quote-chip` : flex 1.
        div()
            .id("chat-composer-quote")
            .flex()
            .pt(u(cx, f.size_2_2))
            .px(u(cx, f.size_2_3))
            .child(
                dessiner(&donnees, vignette.as_ref(), &theme, cx)
                    .flex_1()
                    .child(
                        crate::icone::icone_image()
                            .path(icones::chemin("reply"))
                            .size(u(cx, f.icon_s))
                            .flex_none()
                            .text_color(couleur(theme.palette.color_accent_2)),
                    )
                    .child(retrait),
            )
            .into_any_element()
    })
}

/// Ce que fait un clic sur une citation d'un message : la vue révèle le passage cité (relocalisé par
/// son texte s'il a bougé).
pub type AllerAuPassage = Rc<dyn Fn(&QuotePartData, &mut Window, &mut App)>;

struct CitationVue {
    data: QuotePartData,
    vignette: Option<Arc<RenderImage>>,
    aller: AllerAuPassage,
}

impl Render for CitationVue {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = theme_ou_defaut(cx);
        let f = &theme.forme;
        let (donnees, aller) = (self.data.clone(), self.aller.clone());
        let survol = theme.palette.text_normal;
        // `.chat-part[data-part='quote']` : marge basse 2-3, curseur main, texte normal au survol.
        div()
            .id(SharedString::from(format!(
                "chat-citation-{}-{}",
                self.data.from, self.data.to
            )))
            .role(Role::Button)
            .aria_label("Aller au passage cité")
            .mb(u(cx, f.size_2_3))
            .cursor_pointer()
            .hover(move |s| s.text_color(couleur(survol)))
            .on_click(move |_, window, cx| aller(&donnees, window, cx))
            .child(dessiner(&self.data, self.vignette.as_ref(), &theme, cx))
    }
}

struct CitationVivante(AnyView);

impl PartVivant for CitationVivante {
    fn vue(&self) -> AnyView {
        self.0.clone()
    }
}

/// Le rendu du part `quote` (`chat:part@1`).
#[derive(Clone)]
pub struct RenduCitation {
    aller: AllerAuPassage,
}

impl RenduCitation {
    /// `aller` : ce que fait un clic (la vue chat révèle le passage).
    #[must_use]
    pub fn new(aller: AllerAuPassage) -> Self {
        Self { aller }
    }
}

impl RenduDePart for RenduCitation {
    fn type_part(&self) -> &str {
        "quote"
    }

    fn creer(
        &self,
        part: &ChatPart,
        _ctx: &ChatSlotContext,
        _window: &mut Window,
        cx: &mut App,
    ) -> Option<Box<dyn PartVivant>> {
        let data: QuotePartData = serde_json::from_value(part.data.clone()?).ok()?;
        let vignette = vignette_de_donnees(&data);
        let aller = self.aller.clone();
        let vue = cx.new(|_| CitationVue {
            data,
            vignette,
            aller,
        });
        Some(Box::new(CitationVivante(vue.into())))
    }
}
