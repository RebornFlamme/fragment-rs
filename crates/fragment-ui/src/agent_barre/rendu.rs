// Port de app/src/ui/views/agent/bar/bar.css (la bande, la pastille, le champ) et de la pose par
// l'API des widgets d'AgentBar.ts.
//! La PRÉSENTATION de la barre : un widget posé par l'API des widgets, sans contournement.
//!
//! ★ `WidgetLayer::add_widget` avec l'ancre `Viewport { x: 0, y: 0, from: BottomLeft }` et
//!   `pass_through` : une BANDE de bord à bord collée en bas du pane, transparente aux clics à côté
//!   du champ ; seuls la pastille et le champ les captent (`occlude`). Un widget viewport n'a que la
//!   taille de son contenu : la bande prend la largeur du pane que l'hôte a mesurée
//!   ([`crate::calques::HoteCalques::largeur_pane`]), refaite à chaque image — un pane qui change de
//!   taille ne laisse donc aucun écart.
//! ★ Le champ est la vue chat hébergée en mode `bar` : le montage la rend (`ChampBarre`), la barre ne
//!   la connaît que par ses poignées. La barre RECOUVRE le bas du texte (décision : aucune réserve).
//!
//! Écart : l'animation d'entrée (`agent-bar-in`, 150 ms) n'est pas rejouée ; elle est coupée dans
//! les scènes (T7.md §5) et la bascule est immédiate.

use std::cell::RefCell;
use std::rc::Rc;

use fragment_chat::api::{ChatMode, ChatSlot, ChatSlotContext, SlotAlign};
use fragment_layers::types::ElementId as ElementCalque;
use fragment_layers::widget_layer::{
    ViewportCorner, WidgetAnchor, WidgetHandle, WidgetOptions, WidgetSource,
};
use gpui_kit::{
    AnyElement, App, ElementId, InteractiveElement as _, IntoElement, ParentElement as _, Role,
    StatefulInteractiveElement as _, Styled as _, Window, div, px, svg,
};

use super::modele::{BarreAgent, Crochets, EtatBarre, GOUTTIERE_CHAMP, LARGEUR_CHAMP};
use super::registre::Barres;
use crate::calques::{Contenu, HoteCalques};
use crate::chat_fil::primitives::{Variante, bouton_icone};
use crate::chat_fil::registre::SlotChat;
use crate::chat_vue::EtatChat;
use crate::echelle::u;
use crate::flottant::{ombrer, texte};
use crate::icones;
use crate::theme_global::{couleur, theme_ou_defaut};
use crate::widget_couche::{CalqueWidgets, CoucheWidgets, WidgetDeclare};

/// L'id de la pastille.
pub const ID_PASTILLE: &str = "agent-bar-pill";
/// Le libellé accessible de la pastille.
pub const LIBELLE_PASTILLE: &str = "Ouvrir la barre de l’agent";
/// L'espace sous le champ (`padding-bottom: var(--size-4-3)`).
pub const ESPACE_BAS: f32 = 12.0;
/// L'id du bouton de bascule de destination (le slot `agent-bar-target`).
pub const SLOT_CIBLE: &str = "agent-bar-target";
/// L'id du bouton « Replier » (le slot `agent-bar-collapse`).
pub const SLOT_REPLIER: &str = "agent-bar-collapse";

/// La largeur du champ dans un pane de `pane` px : `min(720, 100 % - 32)`.
#[must_use]
pub fn largeur_du_champ(pane: f32) -> f32 {
    LARGEUR_CHAMP.min((pane - GOUTTIERE_CHAMP).max(0.0))
}

/// Le champ que le montage fournit : la vue chat hébergée en mode `bar`, et ses poignées.
#[derive(Clone)]
pub struct ChampBarre {
    /// L'élément du champ (la feuille hébergée), dessiné à chaque image.
    pub rendre: Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>,
    /// L'état de la vue chat du champ : Entrée et Échap y sont déposés.
    pub etat: Option<Rc<RefCell<EtatChat>>>,
    /// Monte la vue chat sur la conversation modèle (premier dépli).
    pub monter: Rc<dyn Fn(&str, bool)>,
    pub focaliser: Rc<dyn Fn()>,
    pub a_le_focus: Rc<dyn Fn() -> bool>,
}

/// Une barre posée.
pub struct BarrePosee {
    pub handle: Rc<dyn WidgetHandle>,
    pub declare: Rc<WidgetDeclare>,
}

impl BarrePosee {
    /// L'élément construit, s'il y en a un (hors de la bande : aucun).
    #[must_use]
    pub fn element(&self) -> Option<ElementCalque> {
        self.declare.element()
    }
}

/// Pose la barre de `barre` dans le calque `calque` (celui du calque « Agent »).
pub fn poser_barre(
    couche: &Rc<CoucheWidgets>,
    calque: &CalqueWidgets,
    barre: &Rc<BarreAgent>,
    champ: ChampBarre,
) -> BarrePosee {
    let hote = Rc::clone(couche.hote());
    let (b, h) = (Rc::clone(barre), Rc::downgrade(&hote));
    let c = champ.clone();
    let fabrique: crate::widget_couche::Fabrique = Rc::new(
        move |_: &Rc<dyn WidgetHandle>, _el: ElementCalque| -> Contenu {
            let (b, h, c) = (Rc::clone(&b), h.clone(), c.clone());
            Rc::new(move |window, cx| bande(&b, &h, &c, window, cx))
        },
    );
    let declare = WidgetDeclare::new(&hote, fabrique);
    let handle = calque.layer().add_widget(
        WidgetSource::Lazy(declare.clone()),
        WidgetAnchor::Viewport {
            x: 0.0,
            y: 0.0,
            from: Some(ViewportCorner::BottomLeft),
        },
        WidgetOptions { pass_through: true },
    );
    let relacher = Rc::clone(&handle);
    barre.poser_crochets(Crochets {
        monter: Some(Rc::clone(&champ.monter)),
        focaliser: Some(Rc::clone(&champ.focaliser)),
        a_le_focus: Some(Rc::clone(&champ.a_le_focus)),
        relacher: Some(Rc::new(move || relacher.set_active(false))),
    });
    BarrePosee { handle, declare }
}

/// La bande : de bord à bord, le contenu centré, transparente à côté.
fn bande(
    barre: &Rc<BarreAgent>,
    hote: &std::rc::Weak<HoteCalques>,
    champ: &ChampBarre,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    if !barre.visible() {
        return div().into_any_element();
    }
    let Some(pane) = hote.upgrade().and_then(|h| h.largeur_pane()) else {
        return div().into_any_element();
    };
    let pane = pane as f32;
    if let Some(etat) = &champ.etat
        && barre.est_ouverte()
    {
        barre.pomper(etat);
    }
    let contenu = if barre.est_ouverte() {
        // Le champ capte la souris : seul lui (et la pastille) la prend à la bande.
        div()
            .w_full()
            .occlude()
            .child((champ.rendre)(window, cx))
            .into_any_element()
    } else {
        pastille(barre, cx)
    };
    div()
        .w(u(cx, pane))
        .flex()
        .justify_center()
        .child(
            div()
                .w(u(cx, largeur_du_champ(pane)))
                .pb(u(cx, ESPACE_BAS))
                .flex()
                .flex_col()
                .items_center()
                .child(contenu),
        )
        .into_any_element()
}

/// La pastille de la barre repliée : icône « sparkles » et « Agent ».
fn pastille(barre: &Rc<BarreAgent>, cx: &App) -> AnyElement {
    let t = theme_ou_defaut(cx);
    let p = &t.palette;
    let (muet, normal, survol) = (p.text_muted, p.text_normal, p.menu_bg);
    let b = Rc::clone(barre);
    let bouton = div()
        .id(ElementId::from(ID_PASTILLE))
        .role(Role::Button)
        .aria_label(LIBELLE_PASTILLE)
        .occlude()
        .flex()
        .items_center()
        .gap(u(cx, 6.0))
        .py(u(cx, 6.0))
        .px(u(cx, 12.0))
        .whitespace_nowrap()
        .text_color(couleur(muet))
        .bg(couleur(p.background_primary_alt))
        .border(u(cx, t.forme.border_width))
        .border_color(couleur(p.tab_outline_color))
        .cursor_pointer()
        .hover(move |s| s.bg(couleur(survol)).text_color(couleur(normal)))
        // Sans voler le focus : le champ le prendra à l'ouverture.
        .on_mouse_down(gpui_kit::MouseButton::Left, |_, window, _| {
            window.prevent_default();
        })
        .on_click(move |_, window, _| {
            b.definir_etat(EtatBarre::Ouverte, true);
            window.refresh();
        })
        .child(
            svg()
                .path(icones::chemin("sparkles"))
                .size(u(cx, t.forme.icon_s))
                .flex_none(),
        )
        .child(texte(div(), cx, t.typo.font_ui_small, 1.5).child("Agent"));
    ombrer(
        bouton,
        cx,
        &p.shadow_s,
        "--shadow-s",
        px(999.0),
        u(cx, t.forme.border_width),
    )
    .into_any_element()
}

/// Les deux boutons de la barre du champ (`chat:composer-tools@1`, fin, mode `bar` seulement) : la
/// bascule de destination (l'icône DIT où Entrée ouvre) et « Replier ».
#[must_use]
pub fn slots_barre(barres: &Rc<Barres>, fournisseur: &str) -> Vec<SlotChat> {
    let fournisseur = fournisseur.to_owned();
    let en_barre =
        move |ctx: &ChatSlotContext| ctx.mode == ChatMode::Bar && ctx.provider == fournisseur;
    let cible = {
        let barres = Rc::clone(barres);
        let rendre = move |_: &ChatSlotContext, window: &mut Window, cx: &mut App| {
            let c = barres.cible();
            let t = theme_ou_defaut(cx);
            let barres = Rc::clone(&barres);
            let _ = window;
            bouton_icone(
                SLOT_CIBLE,
                c.icone(),
                c.libelle(),
                Variante::Discret,
                true,
                &t,
                cx,
            )
            .on_click(move |_, window, _| {
                barres.basculer_cible();
                window.refresh();
            })
            .into_any_element()
        };
        SlotChat::new(
            ChatSlot::new(SLOT_CIBLE)
                .with_align(SlotAlign::End)
                .with_applies_to(en_barre.clone()),
            Rc::new(rendre),
        )
    };
    let replier = {
        let barres = Rc::clone(barres);
        let rendre = move |ctx: &ChatSlotContext, _: &mut Window, cx: &mut App| {
            let t = theme_ou_defaut(cx);
            let (barres, conversation) = (Rc::clone(&barres), ctx.conversation.clone());
            bouton_icone(
                SLOT_REPLIER,
                "chevron-down",
                "Replier la barre",
                Variante::Discret,
                true,
                &t,
                cx,
            )
            .on_click(move |_, window, _| {
                barres.replier_modele(&conversation);
                window.refresh();
            })
            .into_any_element()
        };
        SlotChat::new(
            ChatSlot::new(SLOT_REPLIER)
                .with_align(SlotAlign::End)
                .with_applies_to(en_barre),
            Rc::new(rendre),
        )
    };
    vec![cible, replier]
}
