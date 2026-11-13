// Port de `MessageList` (app/src/ui/views/chat/components/thread.ts) et de `.chat-messages`,
// `.chat-doc`, `.chat-column` (thread.css).
//! La liste des messages : ne touche que les messages dont la révision a changé — en place si
//! possible — et ne DÉFILE JAMAIS d'elle-même (c'est le contrôleur de défilement de la vue,
//! `fr-t7-defilement`).
//!
//! ★ Trois éléments STABLES, créés une fois et jamais remplacés, parce que la vue en fait le
//! support de ses calques : le défileur (la vue le pose autour), le PLAN du document (`.chat-doc`,
//! l'origine des coordonnées de texte) et la colonne des messages (`.chat-column`, les marges
//! l'entourent). Ici : la colonne est l'élément rendu ; l'identifiant du plan et celui de la colonne
//! sont [`ID_PLAN`] et [`ID_COLONNE`].

use std::collections::{HashMap, HashSet};

use fragment_chat::chat_session::LiveMessage;
use gpui_kit::{
    AppContext as _, Context, ElementId, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, Role, StatefulInteractiveElement as _, Styled as _, Window, div,
    px,
};

use super::message::{DepsFil, MessageVue, PartRendue};
use super::modele::{EtatMessage, etat};
use crate::echelle::u;

/// L'identifiant du plan du document (`.chat-doc`).
pub const ID_PLAN: &str = "chat-doc";
/// L'identifiant de la colonne des messages (`.chat-column`).
pub const ID_COLONNE: &str = "chat-column";

struct Rendu {
    rev: u32,
    etat: EtatMessage,
    vue: Entity<MessageVue>,
}

/// La liste des messages (une entité GPUI).
pub struct ListeMessages {
    deps: DepsFil,
    rendus: HashMap<String, Rendu>,
    ordre: Vec<String>,
    /// Combien de messages ont été (re)construits : les tests vérifient que seul le message en flux
    /// bouge.
    constructions: usize,
}

impl ListeMessages {
    #[must_use]
    pub fn new(deps: DepsFil) -> Self {
        ListeMessages {
            deps,
            rendus: HashMap::new(),
            ordre: Vec::new(),
            constructions: 0,
        }
    }

    /// Les dépendances (participants, contributions) ont changé : tout sera redessiné.
    pub fn poser_deps(&mut self, deps: DepsFil, cx: &mut Context<Self>) {
        self.deps = deps;
        self.reinitialiser(cx);
    }

    /// Les messages affichés, dans l'ordre : leur entité (`data-message-id`).
    #[must_use]
    pub fn messages(&self) -> Vec<(String, Entity<MessageVue>)> {
        self.ordre
            .iter()
            .filter_map(|id| self.rendus.get(id).map(|r| (id.clone(), r.vue.clone())))
            .collect()
    }

    /// Le message d'un id.
    #[must_use]
    pub fn message(&self, id: &str) -> Option<Entity<MessageVue>> {
        self.rendus.get(id).map(|r| r.vue.clone())
    }

    /// Les messages affichés avec leurs parts rendus (la surface de document les lit).
    #[must_use]
    pub fn entrees(&self, cx: &gpui_kit::App) -> Vec<(String, Vec<PartRendue>)> {
        self.ordre
            .iter()
            .filter_map(|id| {
                self.rendus
                    .get(id)
                    .map(|r| (id.clone(), r.vue.read(cx).parts(cx)))
            })
            .collect()
    }

    /// Combien de messages ont été construits depuis le début (créés ou reconstruits).
    #[must_use]
    pub fn constructions(&self) -> usize {
        self.constructions
    }

    /// Ne touche que les messages dont la révision ou l'état a changé : en place si la forme tient,
    /// sinon reconstruit. L'ordre suit celui des messages (un chargement peut préfixer l'historique).
    pub fn mettre_a_jour(
        &mut self,
        messages: &[LiveMessage],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut vus: HashSet<&str> = HashSet::new();
        for m in messages {
            vus.insert(m.id.as_str());
            let e = etat(m);
            let change = self
                .rendus
                .get(&m.id)
                .is_none_or(|r| r.rev != m.rev || r.etat != e);
            if !change {
                continue;
            }
            let en_place = match self.rendus.get(&m.id) {
                Some(r) => {
                    let vue = r.vue.clone();
                    vue.update(cx, |v, cx| v.mettre_a_jour(m, window, cx))
                }
                None => false,
            };
            if en_place {
                if let Some(r) = self.rendus.get_mut(&m.id) {
                    r.rev = m.rev;
                    r.etat = e;
                }
                continue;
            }
            if let Some(ancien) = self.rendus.remove(&m.id) {
                ancien.vue.update(cx, |v, cx| v.detruire(cx));
            }
            let deps = self.deps.clone();
            let vue = cx.new(|cx| MessageVue::new(m, deps, window, cx));
            self.constructions += 1;
            self.rendus.insert(
                m.id.clone(),
                Rendu {
                    rev: m.rev,
                    etat: e,
                    vue,
                },
            );
        }
        self.ordre = messages.iter().map(|m| m.id.clone()).collect();
        let partis: Vec<String> = self
            .rendus
            .keys()
            .filter(|id| !vus.contains(id.as_str()))
            .cloned()
            .collect();
        for id in partis {
            if let Some(r) = self.rendus.remove(&id) {
                r.vue.update(cx, |v, cx| v.detruire(cx));
            }
        }
        cx.notify();
    }

    /// Tout redessiner (contributions changées).
    pub fn reinitialiser(&mut self, cx: &mut Context<Self>) {
        for (_, r) in self.rendus.drain() {
            r.vue.update(cx, |v, cx| v.detruire(cx));
        }
        self.ordre.clear();
        cx.notify();
    }
}

impl Render for ListeMessages {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // La colonne : les messages à 14 px les uns des autres, 12 px au-dessus, 14 px de côté
        // (`.chat-column`, thread.css:28-34 : `gap: --size-2-7`, `padding: --size-4-3 --size-2-7`).
        let mut colonne = div()
            .id(ElementId::from(ID_COLONNE))
            .flex()
            .flex_col()
            .gap(u(cx, 14.0))
            .px(u(cx, 14.0))
            .py(u(cx, 12.0))
            .w_full();
        for id in &self.ordre {
            if let Some(r) = self.rendus.get(id) {
                colonne = colonne.child(r.vue.clone());
            }
        }
        // Un `log` SANS région vive : les événements passent par l'annonceur de la vue.
        div()
            .id(ElementId::from(ID_PLAN))
            .role(Role::Log)
            .relative()
            .w_full()
            .min_h(px(0.0))
            .child(colonne)
    }
}
