// Port de app/src/ui/views/agent/bar/AgentBar.ts (cible, lancer : ce que la barre demande au
// workspace) et registerBar.ts (estDocument).
//! Les branchements RÉELS de la barre : le workspace du cœur et le fournisseur agent.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use fragment_agent::fournisseur::AgentChatProvider;
use fragment_agent::fournisseur::agent_chat::ID_FOURNISSEUR;
use fragment_chat::api::{CHAT_VIEW_TYPE, ChatProvider};
use fragment_core::{ItemId, OpenViewState, SplitDirection, Workspace};
use serde_json::{Map, Value};

use super::modele::{FournisseurBarre, HoteBarre};

/// Le fournisseur agent, tel que la barre le voit.
impl FournisseurBarre for AgentChatProvider {
    fn id(&self) -> String {
        ID_FOURNISSEUR.to_owned()
    }

    fn create(&self) -> Option<String> {
        ChatProvider::create(self)
    }

    fn copy_pending(&self, de: &str, vers: &str) {
        AgentChatProvider::copy_pending(self, de, vers);
    }

    fn set_pending_meta(&self, conversation: &str, meta: Map<String, Value>) {
        AgentChatProvider::set_pending_meta(self, conversation, meta);
    }
}

/// Le workspace du cœur. `rendre_le_clavier` est celui du montage : le workspace ne connaît pas le
/// focus de GPUI (il blur le champ puis donne le focus à la surface du document).
pub struct HoteWorkspace {
    ws: Weak<RefCell<Workspace>>,
    clavier: Rc<dyn Fn(ItemId)>,
}

impl HoteWorkspace {
    #[must_use]
    pub fn new(ws: &Rc<RefCell<Workspace>>, clavier: Rc<dyn Fn(ItemId)>) -> Self {
        Self {
            ws: Rc::downgrade(ws),
            clavier,
        }
    }

    fn avec<R>(&self, f: impl FnOnce(&mut Workspace) -> R) -> Option<R> {
        let ws = self.ws.upgrade()?;
        let mut w = ws.try_borrow_mut().ok()?;
        Some(f(&mut w))
    }
}

impl HoteBarre for HoteWorkspace {
    fn feuille_active(&self) -> Option<ItemId> {
        self.avec(|w| w.active_leaf()).flatten()
    }

    fn est_document(&self, vue: ItemId) -> bool {
        self.avec(|w| {
            w.arbre().existe(vue)
                && w.leaf(vue)
                    .is_some_and(|f| f.get_view_type() != CHAT_VIEW_TYPE)
                && w.document_de(vue).is_some()
        })
        .unwrap_or(false)
    }

    fn document(&self, vue: ItemId) -> Option<String> {
        self.avec(|w| w.document_de(vue).map(|d| d.to_string()))
            .flatten()
    }

    fn ecran_partage(&self, depuis: ItemId, reutilisable: Option<ItemId>) -> Option<ItemId> {
        self.avec(|w| {
            // ★ L'écran partagé ouvert la dernière fois, s'il est toujours là, au centre, et montre
            //   une conversation.
            if let Some(s) = reutilisable
                && w.arbre().existe(s)
                && w.arbre().get_root(s) == w.root_split()
                && w.leaf(s)
                    .is_some_and(|f| f.get_view_type() == CHAT_VIEW_TYPE)
            {
                return Some(s);
            }
            w.create_leaf_by_split(depuis, SplitDirection::Vertical)
        })
        .flatten()
    }

    fn feuille_dock(&self) -> Option<ItemId> {
        self.avec(|w| {
            // Une conversation déjà au dock droit : on y va ; sinon une feuille du dock.
            let droite = w.right_split();
            w.get_leaves_of_type(CHAT_VIEW_TYPE)
                .into_iter()
                .find(|&l| w.arbre().get_root(l) == droite)
                .or_else(|| w.get_right_leaf().ok())
        })
        .flatten()
    }

    fn ouvrir(&self, feuille: ItemId, cle: &str, e_state: Map<String, Value>) -> bool {
        self.avec(|w| {
            w.open_document_in(
                feuille,
                cle,
                Some(OpenViewState {
                    e_state: Some(e_state),
                    active: Some(true),
                    ..OpenViewState::default()
                }),
            )
        })
        .unwrap_or(false)
    }

    fn reveler(&self, feuille: ItemId) {
        self.avec(|w| w.reveal_leaf(feuille));
    }

    fn rendre_le_clavier(&self, vue: ItemId) {
        (self.clavier)(vue);
    }
}
