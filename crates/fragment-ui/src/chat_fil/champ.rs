// Port de `Composer` (app/src/ui/views/chat/components/thread.ts) et de `.chat-composer*`
// (thread.css:~236-330), avec les extensions `chat:composer@1` (api.ts).
//! Le champ de la conversation : il grandit avec le texte ; Entrée envoie, Maj+Entrée va à la
//! ligne ; le bouton passe d'« Envoyer » à « Arrêter » pendant un tour.
//!
//! - **zones** : des bandes au-dessus du texte, posées par les extensions ([`ExtensionChamp`]) ;
//! - **extensions** (`chat:composer@1`) : `pending` (on peut envoyer sans texte), `take` (ce que
//!   l'envoi emporte : options du fournisseur et parts du message), `save` / `restore` (l'état gardé
//!   quand la vue reconstruit le champ). Une extension que le fournisseur refuse (`without`) ou qui
//!   ne s'applique pas n'est pas montée ; à id égal, la plus haute priorité gagne ;
//! - **slots** `composer-tools` : au début de la barre, ou à sa fin (`SlotAlign::End`), à côté du
//!   bouton d'envoi.
//!
//! Le champ ÉMET [`EvenementChamp`] ; la vue s'y abonne et pilote la session. ★ Écart au TS : `take`
//! est appelé ici, à l'envoi, et le résultat voyage dans l'événement ([`EnvoiChamp`]) — la vue n'a
//! plus à connaître les extensions.

use std::collections::HashMap;

use fragment_chat::api::{ChatComposerHandle, ChatPart, ChatSlotContext, SendOpts, SlotAlign};
use gpui_kit::base::input::{InputEvent, Textarea, TextareaState};
use gpui_kit::{
    AppContext as _, Context, ElementId, Entity, EventEmitter, InteractiveElement as _,
    IntoElement, KeyDownEvent, MouseButton, ParentElement as _, Render, Role, SharedString,
    StatefulInteractiveElement as _, Styled as _, Subscription, Window, div, px,
};
use serde_json::Value;

use super::modele::libelle_bouton;
use super::primitives::{Variante, bouton_icone};
use super::registre::{
    ExtensionChamp, RegistreFil, ZoneChamp, extensions_retenues, slots_applicables,
};
use crate::echelle::u;
use crate::polices::FAMILLE_INTERFACE;
use crate::theme_global::{couleur, theme_ou_defaut};

/// Ce que l'envoi emporte : le texte, et ce que les extensions ajoutent.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EnvoiChamp {
    pub texte: String,
    /// Les options fusionnées des extensions, pour `provider.send`.
    pub opts: Option<SendOpts>,
    /// Les parts ajoutés au message de l'utilisateur, APRÈS le texte.
    pub parts: Vec<ChatPart>,
}

/// Ce que le champ dit à la vue.
#[derive(Clone, Debug, PartialEq)]
pub enum EvenementChamp {
    Envoyer(EnvoiChamp),
    /// Le bouton « Arrêter » (ou le geste équivalent) pendant un tour.
    Arreter,
    /// Échap dans un champ vide.
    Echap,
}

/// Ce que le champ reçoit de la vue.
pub struct DepsChamp {
    pub placeholder: String,
    pub contexte: ChatSlotContext,
    pub registre: RegistreFil,
    /// Les extensions (par id) dont le fournisseur ne veut pas (`ChatProvider::without`).
    pub sans: Vec<String>,
    /// L'état à rendre aux extensions (ce que `save` avait rendu), par id.
    pub etat_initial: HashMap<String, Value>,
}

struct Active {
    id: String,
    handle: Box<dyn ChatComposerHandle>,
    zone: Option<ZoneChamp>,
}

/// L'état du bouton : ce qu'il montre et s'il agit (`majBouton`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EtatBouton {
    pub icone: &'static str,
    pub libelle: &'static str,
    pub actif: bool,
}

/// L'état du bouton d'envoi : « Arrêter » (carré) pendant un tour, sinon « Envoyer » (flèche),
/// inerte sans texte ni rien d'autre prêt à partir.
#[must_use]
pub fn etat_bouton(en_cours: bool, texte_vide: bool, pending: bool) -> EtatBouton {
    EtatBouton {
        icone: if en_cours { "square" } else { "arrow-up" },
        libelle: libelle_bouton(en_cours),
        actif: en_cours || !texte_vide || pending,
    }
}

/// Le champ de saisie (une entité GPUI).
pub struct Champ {
    etat: Entity<TextareaState>,
    en_cours: bool,
    contexte: ChatSlotContext,
    registre: RegistreFil,
    extensions: Vec<Active>,
    placeholder: String,
    _abonnements: Vec<Subscription>,
}

impl EventEmitter<EvenementChamp> for Champ {}

/// Le nombre de lignes à partir duquel le champ défile : `max-height: 200px` pour des lignes de
/// 13,5 × 1,5 px (thread.css:`.chat-composer-input`).
const LIGNES_MAX: usize = 9;

impl Champ {
    /// Un champ vide, avec ses extensions.
    pub fn new(deps: DepsChamp, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let placeholder = deps.placeholder.clone();
        let etat = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(1, LIGNES_MAX)
                .placeholder(placeholder)
                .submit_on_enter(true)
        });
        let abonnements = vec![
            cx.observe(&etat, |_, _, cx| cx.notify()),
            cx.subscribe_in(&etat, window, Self::sur_evenement),
        ];
        let retenues = {
            let r = deps.registre.borrow();
            extensions_retenues(&r.extensions, &deps.contexte, &deps.sans)
        };
        let mut extensions: Vec<Active> = Vec::new();
        for e in retenues {
            extensions.push(Self::monter(&e, &deps.contexte, &deps.etat_initial));
        }
        Champ {
            etat,
            en_cours: false,
            contexte: deps.contexte,
            registre: deps.registre,
            extensions,
            placeholder: deps.placeholder,
            _abonnements: abonnements,
        }
    }

    fn monter(
        e: &ExtensionChamp,
        ctx: &ChatSlotContext,
        initial: &HashMap<String, Value>,
    ) -> Active {
        let mut handle = e.extension.create(ctx);
        let id = e.extension.id().to_owned();
        if let Some(s) = initial.get(&id) {
            handle.restore(s.clone());
        }
        Active {
            id,
            handle,
            zone: e.zone.clone(),
        }
    }

    fn sur_evenement(
        &mut self,
        _etat: &Entity<TextareaState>,
        ev: &InputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Entrée envoie (jamais en cours de tour : l'arrêt est celui du bouton), Maj+Entrée a déjà
        // inséré la ligne.
        if let InputEvent::PressEnter { shift: false, .. } = ev
            && !self.en_cours
        {
            self.agir(window, cx);
        }
    }

    /// Le texte saisi.
    #[must_use]
    pub fn valeur(&self, cx: &gpui_kit::App) -> String {
        self.etat.read(cx).value().to_string()
    }

    /// Pose un texte (un brouillon) dans le champ.
    pub fn definir_valeur(&mut self, texte: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.etat
            .update(cx, |s, cx| s.set_value(texte.to_owned(), window, cx));
        cx.notify();
    }

    /// Le focus au champ, curseur à la fin.
    pub fn focus(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.etat.update(cx, |s, cx| s.focus(window, cx));
    }

    /// Un tour est en cours (ou non) : le bouton bascule.
    pub fn set_running(&mut self, en_cours: bool, cx: &mut Context<Self>) {
        if self.en_cours != en_cours {
            self.en_cours = en_cours;
            cx.notify();
        }
    }

    /// Un tour est en cours.
    #[must_use]
    pub fn en_cours(&self) -> bool {
        self.en_cours
    }

    /// Une extension a changé hors du texte (une pièce ajoutée ou retirée) : le bouton se remet à
    /// jour.
    pub fn rafraichir(&mut self, cx: &mut Context<Self>) {
        cx.notify();
    }

    /// Quelque chose d'autre que le texte est prêt à partir.
    #[must_use]
    pub fn pending(&self) -> bool {
        self.extensions.iter().any(|a| a.handle.pending())
    }

    /// L'état actuel du bouton.
    #[must_use]
    pub fn bouton(&self, cx: &gpui_kit::App) -> EtatBouton {
        etat_bouton(
            self.en_cours,
            self.valeur(cx).trim().is_empty(),
            self.pending(),
        )
    }

    /// L'état des extensions, par id (ce que `save` rend), à rendre au champ suivant.
    #[must_use]
    pub fn sauver(&self) -> HashMap<String, Value> {
        self.extensions
            .iter()
            .filter_map(|a| a.handle.save().map(|s| (a.id.clone(), s)))
            .collect()
    }

    /// Rend à chaque extension l'état gardé (`restore`).
    pub fn restaurer(&mut self, etat: &HashMap<String, Value>) {
        for a in &mut self.extensions {
            if let Some(s) = etat.get(&a.id) {
                a.handle.restore(s.clone());
            }
        }
    }

    /// L'id des extensions montées, dans l'ordre.
    #[must_use]
    pub fn ids_extensions(&self) -> Vec<String> {
        self.extensions.iter().map(|a| a.id.clone()).collect()
    }

    /// Le geste du bouton : arrêter pendant un tour, sinon envoyer (le texte rogné, ou rien si rien
    /// d'autre n'est prêt).
    pub fn agir(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.en_cours {
            cx.emit(EvenementChamp::Arreter);
            return;
        }
        let texte = self.valeur(cx).trim().to_owned();
        if texte.is_empty() && !self.pending() {
            return;
        }
        let mut opts: Option<SendOpts> = None;
        let mut parts: Vec<ChatPart> = Vec::new();
        for a in &mut self.extensions {
            if let Some(prise) = a.handle.take() {
                if let Some(o) = prise.opts {
                    opts.get_or_insert_with(SendOpts::new).extend(o);
                }
                parts.extend(prise.parts);
            }
        }
        self.etat
            .update(cx, |s, cx| s.set_value(String::new(), window, cx));
        cx.emit(EvenementChamp::Envoyer(EnvoiChamp { texte, opts, parts }));
        cx.notify();
    }
}

impl Render for Champ {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = theme_ou_defaut(cx);
        let p = &theme.palette;
        let bouton = self.bouton(cx);

        // Les zones au-dessus du texte.
        let mut zones = Vec::new();
        for a in &self.extensions {
            if let Some(z) = &a.zone {
                zones.push(z(window, cx));
            }
        }
        // Les outils : au début de la barre, ou à sa fin.
        let outils = slots_applicables(&self.registre.borrow().outils, &self.contexte);
        let mut debut = div().flex().items_center().gap(u(cx, 4.0)).min_w(px(0.0));
        let mut fin = div().flex().items_center().gap(u(cx, 4.0)).ml_auto();
        for s in &outils {
            let el = (s.rendre)(&self.contexte, window, cx);
            if s.align() == SlotAlign::End {
                fin = fin.child(el);
            } else {
                debut = debut.child(el);
            }
        }

        let etat = self.etat.clone();
        let envoyer = bouton_icone(
            "chat-envoyer",
            bouton.icone,
            bouton.libelle,
            Variante::Primaire,
            bouton.actif,
            &theme,
            cx,
        )
        .on_click(cx.listener(move |this, _, window, cx| {
            if this.bouton(cx).actif {
                this.agir(window, cx);
            }
        }));

        // La carte du champ : 8 px / 12 px de marge, bordure, rayon 12, fond de l'espace de travail
        // (`.chat-composer`).
        let champ = div()
            .id(ElementId::from(SharedString::from("chat-champ")))
            .flex()
            .flex_col()
            .gap(u(cx, 6.0))
            .mt(u(cx, 8.0))
            .mx(u(cx, 12.0))
            .mb(u(cx, 12.0))
            .pt(u(cx, 8.0))
            .pr(u(cx, 8.0))
            .pb(u(cx, 6.0))
            .pl(u(cx, 12.0))
            .border(px(theme.forme.border_width))
            .border_color(couleur(p.tab_outline_color))
            .rounded(u(cx, theme.forme.radius_ml + 2.0))
            .bg(couleur(p.workspace_bg))
            .font_family(FAMILLE_INTERFACE)
            .text_color(couleur(p.text_normal))
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| {
                // Échap dans un champ vide : à la vue (replier une barre).
                if ev.keystroke.key == "escape" && this.valeur(cx).is_empty() {
                    cx.emit(EvenementChamp::Echap);
                }
            }))
            // Un clic dans la zone (hors boutons) donne le focus au champ.
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, window, cx| this.focus(window, cx)),
            );
        let champ = zones.into_iter().fold(champ, |c, z| c.child(z));
        let champ = champ
            .child(
                div()
                    .id("chat-saisie")
                    .aria_label(self.placeholder.clone())
                    .w_full()
                    .min_h(px(22.0))
                    .text_size(px(13.5))
                    .child(Textarea::new(&etat)),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(u(cx, 6.0))
                    .child(debut)
                    .child(fin)
                    .child(envoyer),
            );
        champ.role(Role::Group)
    }
}
