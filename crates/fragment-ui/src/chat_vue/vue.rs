// Port de app/src/ui/views/chat/ChatView.ts (la part cœur : type, titre, icônes, adresse, état,
// état éphémère, portée clavier, menu ⋯).
//! [`VueChat`] : la vue `chat` d'une feuille, sans GPUI. Elle tient l'état partagé
//! ([`EtatChat`]) que le contenu GPUI ([`super::contenu::ContenuChat`]) dessine.
//!
//! ★ Les touches de la portée de la vue (Alt+↑ / Alt+↓ : le tour voisin, Fin : le plus récent) ne
//!   peuvent pas agir sur le fil (elles n'ont pas le contexte GPUI) : elles déposent un geste dans
//!   la file que le contenu joue à l'image suivante. Elles ne valent qu'en page et en panneau —
//!   ancrée, la frappe n'est pas à la vue (la portée rend `None` : on laisse passer).

use std::any::Any;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use fragment_chat::api::ChatMode;
use fragment_core::{
    ContexteVue, DocumentKey, ItemView, ItemViewBase, MenuPane, Modifier, Scope, View, ViewCreator,
    ViewStateResult,
};
use serde_json::{Map, Value};

use super::modele::{ContexteChat, EtatChat, TYPE_VUE_CHAT};

/// Un geste du clavier de la vue, joué par le contenu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToucheChat {
    /// Alt+↑ : le tour précédent.
    TourPrecedent,
    /// Alt+↓ : le tour suivant.
    TourSuivant,
    /// Fin : le plus récent.
    PlusRecent,
}

/// Ce que le contenu partage avec la vue.
#[derive(Clone)]
pub struct LiensChat {
    pub etat: Rc<RefCell<EtatChat>>,
    pub touches: Rc<RefCell<VecDeque<ToucheChat>>>,
}

/// La vue `chat`.
pub struct VueChat {
    base: ItemViewBase,
    liens: LiensChat,
}

impl VueChat {
    /// Une vue neuve pour la feuille du contexte.
    pub fn new(contexte: &ContexteVue, ctx: ContexteChat) -> Self {
        let base = ItemViewBase::new(&contexte.app_scope);
        let etat = Rc::new(RefCell::new(EtatChat::new(contexte.leaf, ctx)));
        let touches: Rc<RefCell<VecDeque<ToucheChat>>> = Rc::default();
        for (mods, touche, geste) in [
            (
                Some(&[Modifier::Alt][..]),
                "ArrowUp",
                ToucheChat::TourPrecedent,
            ),
            (
                Some(&[Modifier::Alt][..]),
                "ArrowDown",
                ToucheChat::TourSuivant,
            ),
            (None, "End", ToucheChat::PlusRecent),
        ] {
            let (e, file) = (etat.clone(), touches.clone());
            base.scope.register(mods, Some(touche), move |_, _| {
                // Seulement là où il y a un fil à lire (page, panneau).
                let fil = e
                    .try_borrow()
                    .is_ok_and(|e| matches!(e.mode(), ChatMode::Page | ChatMode::Panel));
                if !fil {
                    return None;
                }
                file.borrow_mut().push_back(geste);
                Some(false)
            });
        }
        Self {
            base,
            liens: LiensChat { etat, touches },
        }
    }

    /// Les poignées du contenu.
    #[must_use]
    pub fn liens(&self) -> &LiensChat {
        &self.liens
    }

    /// L'état partagé.
    #[must_use]
    pub fn etat(&self) -> &Rc<RefCell<EtatChat>> {
        &self.liens.etat
    }
}

impl View for VueChat {
    fn get_view_type(&self) -> String {
        TYPE_VUE_CHAT.to_owned()
    }

    fn get_display_text(&self) -> String {
        self.liens.etat.borrow().titre()
    }

    fn get_icon(&self) -> String {
        self.liens.etat.borrow().icone()
    }

    fn get_icons(&self) -> Vec<String> {
        self.liens.etat.borrow().icones()
    }

    /// L'adresse de la conversation : `chat:<fournisseur>/<id>`.
    fn get_document(&self) -> Option<DocumentKey> {
        self.liens.etat.borrow().document().map(DocumentKey::new)
    }

    /// On navigue d'une conversation à l'autre (historique ← →), comme entre notes.
    fn navigation(&self) -> bool {
        true
    }

    fn scope(&self) -> Option<Scope> {
        Some(self.base.scope.clone())
    }

    /// Le ⋯ du chat (en onglet, dans le dock) et le ⋮ de sa ligne dans une grappe : télécharger ou
    /// copier la conversation en Markdown, la supprimer.
    fn on_pane_menu(&self, menu: &mut MenuPane, _source: &str) {
        self.liens.etat.borrow().contribuer_au_menu(menu);
    }

    fn get_state(&self) -> Map<String, Value> {
        self.liens.etat.borrow().etat()
    }

    fn set_state(&mut self, state: &Map<String, Value>, _result: ViewStateResult) {
        self.liens.etat.borrow_mut().poser_etat(state);
    }

    fn set_ephemeral_state(&mut self, state: &Value) {
        self.liens.etat.borrow_mut().poser_etat_ephemere(state);
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl ItemView for VueChat {
    fn item_scope(&self) -> &Scope {
        &self.base.scope
    }
}

/// La fabrique du type `chat`.
#[must_use]
pub fn fabrique(ctx: ContexteChat) -> ViewCreator {
    Rc::new(move |c: &ContexteVue| Box::new(VueChat::new(c, ctx.clone())) as Box<dyn View>)
}
