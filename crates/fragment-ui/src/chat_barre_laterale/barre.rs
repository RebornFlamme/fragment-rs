// Port de app/src/ui/views/chat/components/sidebar.ts et sidebar.css.
//! La barre des conversations du cowork : recherche, « Nouvelle conversation », liste groupée par
//! jour, conversation active surlignée. Elle ÉMET des intentions ([`EvenementBarre`]) ; la vue
//! (`fr-t7-vue`) les relie au fournisseur et lui passe la liste ([`BarreLaterale::mettre_a_jour`]).

use fragment_chat::api::{ChatConversationInfo, ChatProvider};
use fragment_theme::{Couleur, Theme};
use gpui_kit::base::input::{Input, InputEvent, InputState};
use gpui_kit::{
    AppContext as _, Context, ElementId, Entity, EventEmitter, Focusable as _,
    InteractiveElement as _, IntoElement, KeyDownEvent, ParentElement as _, Render, Role,
    SharedString, StatefulInteractiveElement as _, Styled as _, Subscription, Window, div, px, svg,
};

use super::horloge::Horloge;
use super::modele::{EtatBarre, LARGEUR_BARRE, Ligne, message_vide};
use crate::chat_fil::primitives::{Variante, bouton_icone};
use crate::echelle::u;
use crate::icones;
use crate::polices::FAMILLE_INTERFACE;
use crate::theme_global::{couleur, theme_ou_defaut};

/// Ce que la barre demande à la vue.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EvenementBarre {
    /// Ouvrir cette conversation (`onSelect`).
    Selectionner(String),
    /// « Nouvelle conversation » (`onNew`).
    Nouvelle,
    /// Masquer la barre (`onToggle`).
    Replier,
}

/// La barre des conversations (une entité GPUI).
pub struct BarreLaterale {
    etat: EtatBarre,
    horloge: Horloge,
    recherche: Entity<InputState>,
    _abonnements: Vec<Subscription>,
}

impl EventEmitter<EvenementBarre> for BarreLaterale {}

impl BarreLaterale {
    /// Une barre vide, lue à l'`horloge`.
    pub fn new(horloge: Horloge, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let recherche = cx.new(|cx| {
            let mut s = InputState::new(window, cx);
            s.set_placeholder("Rechercher", window, cx);
            s
        });
        let abonnements = vec![cx.subscribe(&recherche, Self::sur_saisie)];
        Self {
            etat: EtatBarre::default(),
            horloge,
            recherche,
            _abonnements: abonnements,
        }
    }

    fn sur_saisie(&mut self, champ: Entity<InputState>, ev: &InputEvent, cx: &mut Context<Self>) {
        if matches!(ev, InputEvent::Change) {
            self.etat.requete = champ.read(cx).value().to_string();
            cx.notify();
        }
    }

    /// La liste (plus récente d'abord) et la conversation affichée. Ne redessine que si la
    /// signature de la liste ou la conversation active a changé.
    pub fn mettre_a_jour(
        &mut self,
        items: &[ChatConversationInfo],
        active: &str,
        cx: &mut Context<Self>,
    ) {
        if self.etat.mettre_a_jour(items, active) {
            cx.notify();
        }
    }

    /// Lit la liste chez le fournisseur : à appeler à chaque `onChange` du fournisseur.
    pub fn suivre(&mut self, fournisseur: &dyn ChatProvider, active: &str, cx: &mut Context<Self>) {
        let items = fournisseur.list();
        self.mettre_a_jour(&items, active, cx);
    }

    /// Change l'horloge (la commande de scène `pixels:horloge`) et redessine.
    pub fn definir_horloge(&mut self, horloge: Horloge, cx: &mut Context<Self>) {
        self.horloge = horloge;
        cx.notify();
    }

    /// La requête de recherche courante.
    #[must_use]
    pub fn requete(&self) -> &str {
        &self.etat.requete
    }

    /// Pose la requête dans le champ (et filtre).
    pub fn definir_requete(&mut self, texte: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.recherche
            .update(cx, |s, cx| s.set_value(texte.to_owned(), window, cx));
        self.etat.requete = texte.to_owned();
        cx.notify();
    }

    /// Les lignes montrées (étiquettes et conversations), selon l'horloge et la requête.
    #[must_use]
    pub fn lignes(&self) -> Vec<Ligne> {
        self.etat
            .lignes(self.horloge.maintenant(), self.horloge.fuseau())
    }

    /// Le focus au champ de recherche.
    pub fn focus_recherche(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.recherche.update(cx, |s, cx| s.focus(window, cx));
    }

    /// Échap dans le champ : vide la requête (et garde l'événement quand il y a quelque chose à
    /// vider).
    fn sur_touche(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if ev.keystroke.key == "escape" && !self.etat.requete.is_empty() {
            cx.stop_propagation();
            self.definir_requete("", window, cx);
        }
    }
}

/// Un aplat : `text_normal` à `pourcent` % posé sur le fond de la barre (comme Chromium).
fn teinte(theme: &Theme, pourcent: f32, fond: Couleur) -> Couleur {
    theme.palette.text_normal.sur_chromium(pourcent, fond)
}

impl Render for BarreLaterale {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = theme_ou_defaut(cx);
        let (p, f) = (&theme.palette, &theme.forme);
        let fond = p.background_secondary;
        let focus_champ = self.recherche.focus_handle(cx).contains_focused(window, cx);
        let bordure_champ = if focus_champ {
            p.text_normal.sur_chromium(35.0, p.workspace_bg)
        } else {
            p.tab_outline_color
        };

        let haut = div().flex().items_center().min_h(u(cx, 28.0)).child(
            bouton_icone(
                "chat-sidebar-toggle",
                "panel-left",
                "Masquer la barre latérale",
                Variante::Discret,
                true,
                &theme,
                cx,
            )
            .on_click(cx.listener(|_, _, _, cx| cx.emit(EvenementBarre::Replier))),
        );

        let champ = div()
            .flex()
            .items_center()
            .gap(u(cx, f.size_4_2))
            .h(u(cx, 30.0))
            .px(u(cx, f.size_2_5))
            .border(px(f.border_width))
            .border_color(couleur(bordure_champ))
            .rounded(u(cx, f.radius_sm))
            .bg(couleur(p.workspace_bg))
            .text_color(couleur(p.text_muted))
            .on_key_down(cx.listener(|this, ev, window, cx| this.sur_touche(ev, window, cx)))
            .child(
                svg()
                    .path(icones::chemin("search"))
                    .size(u(cx, f.icon_xs))
                    .flex_none(),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(u(cx, theme.typo.font_ui_medium))
                    .text_color(couleur(p.text_normal))
                    .child(Input::new(&self.recherche)),
            );

        let survol_nouvelle = teinte(&theme, 8.0, fond);
        let nouvelle = div()
            .id("chat-sidebar-new")
            .role(Role::Button)
            .flex()
            .items_center()
            .gap(u(cx, f.size_4_2))
            .h(u(cx, 30.0))
            .px(u(cx, f.size_2_5))
            .rounded(u(cx, f.radius_sm))
            .text_size(u(cx, theme.typo.font_ui_medium))
            .text_color(couleur(p.text_normal))
            .cursor_pointer()
            .hover(move |s| s.bg(couleur(survol_nouvelle)))
            .on_click(cx.listener(|_, _, _, cx| cx.emit(EvenementBarre::Nouvelle)))
            .child(
                svg()
                    .path(icones::chemin("plus"))
                    .size(u(cx, 15.0))
                    .flex_none(),
            )
            .child(div().child("Nouvelle conversation"));

        let lignes = self.lignes();
        let mut liste = div()
            .id("chat-sidebar-list")
            .role(Role::List)
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .mt(u(cx, f.size_4_1))
            .pb(u(cx, f.size_4_2));
        if lignes.is_empty() {
            liste = liste.child(
                div()
                    .p(u(cx, f.size_2_5))
                    .text_size(u(cx, 12.5))
                    .text_color(couleur(p.text_muted))
                    .child(message_vide(&self.etat.requete)),
            );
        }
        for (rang, ligne) in lignes.into_iter().enumerate() {
            liste = match ligne {
                Ligne::Etiquette(texte) => liste.child(
                    div()
                        .pt(u(cx, if rang == 0 { f.size_4_1 } else { f.size_2_7 }))
                        .pb(u(cx, f.size_4_1))
                        .px(u(cx, f.size_2_5))
                        .text_size(u(cx, 11.5))
                        .text_color(couleur(p.text_muted))
                        .child(texte),
                ),
                Ligne::Conversation { id, titre, active } => {
                    let id_clic = id.clone();
                    let survol = teinte(&theme, 6.0, fond);
                    let mut item = div()
                        .id(ElementId::from(SharedString::from(format!(
                            "chat-history-{id}"
                        ))))
                        .role(Role::ListItem)
                        .aria_label(titre.clone())
                        .w_full()
                        .flex_none()
                        .py(px(7.0))
                        .px(u(cx, f.size_2_5))
                        .rounded(u(cx, f.radius_sm))
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .text_size(u(cx, theme.typo.font_ui_medium))
                        .text_color(couleur(p.text_normal))
                        .cursor_pointer()
                        .on_click(cx.listener(move |_, _, _, cx| {
                            cx.emit(EvenementBarre::Selectionner(id_clic.clone()));
                        }));
                    item = if active {
                        item.bg(couleur(teinte(&theme, 10.0, fond)))
                    } else {
                        item.hover(move |s| s.bg(couleur(survol)))
                    };
                    liste.child(item.child(titre))
                }
            };
        }

        div()
            .id("chat-sidebar")
            .role(Role::Navigation)
            .aria_label("Conversations")
            .flex()
            .flex_col()
            .gap(u(cx, f.size_2_3))
            .w(u(cx, LARGEUR_BARRE))
            .h_full()
            .min_w_0()
            .min_h_0()
            .p(u(cx, f.size_4_2))
            .overflow_hidden()
            .font_family(FAMILLE_INTERFACE)
            .bg(couleur(fond))
            .border_r(px(f.border_width))
            .border_color(couleur(p.tab_outline_color))
            .child(haut)
            .child(champ)
            .child(nouvelle)
            .child(liste)
    }
}
