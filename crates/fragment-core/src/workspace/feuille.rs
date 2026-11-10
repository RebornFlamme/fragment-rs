// Port de app/src/ui/core/workspace/WorkspaceLeaf.ts (la part qui ne touche ni au DOM ni au
// Workspace).
//
// La feuille est la charge des nœuds `Leaf` de l'arbre : elle héberge exactement une vue, et
// se souvient de ce qu'elle DEMANDE (l'état de vue voulu), qui peut différer de ce qu'elle
// MONTRE (une coquille différée, une vue vide de remplacement). Tout ce qui demande
// l'application — monter une vue par le registre, empiler une portée clavier, activer — est
// une méthode du Workspace qui prend l'identifiant de la feuille.
//
// ★ La discipline du §8.4, héritée d'Obsidian (qui l'a découverte après coup, en 1.7.2) : le
//   cœur interroge la FEUILLE (`get_view_type`, `get_document`, `get_view_state`), jamais sa
//   vue. La vue peut être une coquille ou un remplaçant ; la feuille sait ce qu'elle veut.

use serde_json::Value;

use super::ViewState;
use super::arbre::ItemId;
use super::layout::LeafLayout;
use super::vue::{EMPTY_VIEW_TYPE, EmptyView, View};
use crate::document_key::DocumentKey;
use crate::documents::DocumentManager;
use crate::events::Events;
use crate::leaf_history::{HistoryEntry, LeafHistory};

/// Les noms des événements d'une feuille.
pub const ACTIVE_CHANGE: &str = "active-change";
pub const HISTORY_CHANGE: &str = "history-change";
pub const PINNED_CHANGE: &str = "pinned-change";
pub const HEADER_CHANGE: &str = "header-change";

/// Le générateur des identités de feuille (`leaf-<base 36>`), propre à un Workspace.
///
/// ★ Un id lu dans workspace.json RÉSERVE sa place : le compteur repart au-delà. Sans ça, il
///   recommençait à zéro à chaque session et la prochaine feuille créée reprenait l'id d'une
///   feuille restaurée — un fichier réel contenait « leaf-7 » trois fois, et `active`
///   désignait n'importe laquelle. (Fragment tenait ce compteur en statique ; ici, il
///   appartient au Workspace : deux workspaces de test ne se marchent pas dessus.)
#[derive(Debug, Clone, Default)]
pub struct GenerateurIds {
    compteur: u64,
}

impl GenerateurIds {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn suivant(&mut self) -> String {
        let id = format!("leaf-{}", base36(self.compteur));
        self.compteur += 1;
        id
    }

    /// Réserve un id venu du disque : s'il est de la forme `leaf-<base 36>`, le compteur passe
    /// au-delà. Toute autre forme est acceptée telle quelle et ne réserve rien.
    pub fn reserver(&mut self, id: &str) {
        let Some(n) = id
            .strip_prefix("leaf-")
            .filter(|r| {
                !r.is_empty()
                    && r.chars()
                        .all(|c| c.is_ascii_digit() || c.is_ascii_lowercase())
            })
            .and_then(|r| u64::from_str_radix(r, 36).ok())
        else {
            return;
        };
        if n >= self.compteur {
            self.compteur = n.saturating_add(1);
        }
    }
}

fn base36(mut n: u64) -> String {
    const CHIFFRES: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    if n == 0 {
        return "0".to_owned();
    }
    let mut out = Vec::new();
    while n > 0 {
        out.push(CHIFFRES[(n % 36) as usize]);
        n /= 36;
    }
    out.reverse();
    String::from_utf8_lossy(&out).into_owned()
}

/// Une feuille : le seul nœud qui héberge une vue.
pub struct WorkspaceLeaf {
    /// L'identité dans workspace.json, stable pour la vie de la feuille, reposée à la
    /// restauration (pour que `active` désigne la bonne feuille d'une session à l'autre).
    pub id: String,
    pub(super) view: Box<dyn View>,
    /// Ce que la feuille VEUT montrer. Jamais nul : une feuille neuve demande la vue vide.
    pub(super) requested_state: ViewState,
    pub(super) pinned: bool,
    /// L'hôte d'une feuille détachée hébergée dans un widget (chat ancré, note flottante).
    pub(super) host_leaf: Option<ItemId>,
    /// L'historique ← → de l'onglet, par CLÉ de document.
    pub history: LeafHistory,
    /// §8.4 — l'état est connu, la vue n'est pas construite.
    pub(super) is_deferred: bool,
    pub(super) deferred_e_state: Option<Value>,
    /// `mod-active` : la feuille est active (onglet OU hébergée).
    pub(super) active: bool,
    events: Events,
}

impl std::fmt::Debug for WorkspaceLeaf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkspaceLeaf")
            .field("id", &self.id)
            .field("requested", &self.requested_state.type_)
            .field("vue", &self.view.get_view_type())
            .field("deferred", &self.is_deferred)
            .field("pinned", &self.pinned)
            .finish()
    }
}

impl WorkspaceLeaf {
    /// Une feuille neuve : elle porte une vue vide NON montée (un panneau blanc).
    pub fn new(id: String, vide: EmptyView, history: LeafHistory) -> Self {
        Self {
            id,
            view: Box::new(vide),
            requested_state: ViewState::new(EMPTY_VIEW_TYPE),
            pinned: false,
            host_leaf: None,
            history,
            is_deferred: false,
            deferred_e_state: None,
            active: false,
            events: Events::new(),
        }
    }

    /// La vue montée (peut être une coquille ou un remplaçant : voir l'en-tête).
    pub fn view(&self) -> &dyn View {
        self.view.as_ref()
    }

    pub fn view_mut(&mut self) -> &mut dyn View {
        self.view.as_mut()
    }

    /// Les événements de la feuille : `active-change` (bool), `history-change`,
    /// `pinned-change` (bool), `header-change`.
    pub fn events(&self) -> &Events {
        &self.events
    }

    /// Le type que la feuille VEUT afficher, pas celui qu'elle affiche.
    pub fn get_view_type(&self) -> &str {
        &self.requested_state.type_
    }

    pub fn is_deferred(&self) -> bool {
        self.is_deferred
    }

    pub fn pinned(&self) -> bool {
        self.pinned
    }

    pub fn host_leaf(&self) -> Option<ItemId> {
        self.host_leaf
    }

    /// `mod-active`.
    pub fn est_active(&self) -> bool {
        self.active
    }

    /// La feuille attend-elle un type pas encore enregistré ?
    ///
    /// ★ `!is_deferred` est la condition qui fait marcher le §8.4 : une coquille satisfait les
    ///   deux autres termes, et sans cette exclusion la reconstruction des remplaçants (au
    ///   premier `changed` du registre des vues, donc au chargement du premier plugin)
    ///   matérialiserait TOUTES les feuilles différées.
    pub fn is_placeholder(&self) -> bool {
        !self.is_deferred
            && self.view.as_any().is::<EmptyView>()
            && self.requested_state.type_ != EMPTY_VIEW_TYPE
    }

    /// L'état sérialisable.
    ///
    /// ★ Lu dans l'état DEMANDÉ pour une coquille ou un remplaçant : interroger la vue
    ///   sérialiserait `empty` et PERDRAIT ce que l'onglet voulait — un plugin désactivé au
    ///   moment de quitter effacerait ses onglets du layout. Une vue matérialisée, elle, a
    ///   l'état frais (on a pu naviguer depuis) : on la préfère.
    pub fn get_view_state(&self) -> ViewState {
        if self.is_deferred || self.is_placeholder() {
            return ViewState {
                pinned: Some(self.pinned),
                ..self.requested_state.clone()
            };
        }
        let state = self.view.get_state();
        ViewState {
            type_: self.view.get_view_type(),
            state: Some(state),
            active: None,
            pinned: Some(self.pinned),
        }
    }

    /// L'état éphémère courant : celui mis en attente pour une coquille jamais regardée,
    /// celui de la vue sinon.
    pub fn get_ephemeral_state(&self) -> Option<Value> {
        if self.is_deferred {
            return self.deferred_e_state.clone();
        }
        Some(Value::Object(self.view.get_ephemeral_state()))
    }

    /// La feuille dans workspace.json. La lecture (défilement, page, calques) voyage avec elle,
    /// seulement si c'est un objet non vide.
    pub fn serialize(&self) -> LeafLayout {
        let e_state = self
            .get_ephemeral_state()
            .filter(|v| v.as_object().is_some_and(|m| !m.is_empty()));
        LeafLayout {
            id: self.id.clone(),
            state: self.get_view_state(),
            e_state,
            dimension: None,
        }
    }

    /// La clé du document que la feuille montre — ou VEUT montrer (coquille, remplaçant),
    /// résolue sans matérialiser la vue.
    pub fn get_document(&self, documents: &DocumentManager) -> Option<DocumentKey> {
        if !self.is_deferred && !self.is_placeholder() {
            return self.view.get_document();
        }
        documents.key_of_state(Some(&self.requested_state))
    }

    pub fn get_display_text(&self) -> String {
        self.view.get_display_text()
    }

    pub fn get_icon(&self) -> String {
        self.view.get_icon()
    }

    /// Ce que la feuille affiche maintenant, en entrée d'historique.
    pub(super) fn snapshot(&self, documents: &DocumentManager) -> HistoryEntry {
        HistoryEntry {
            state: self.get_view_state(),
            e_state: Some(Value::Object(self.view.get_ephemeral_state())),
            doc: self.get_document(documents),
        }
    }

    /// `mod-active` + `active-change`, sans effet si l'état ne change pas.
    pub(super) fn marquer_active(&mut self, actif: bool) {
        if self.active == actif {
            return;
        }
        self.active = actif;
        self.events.trigger(ACTIVE_CHANGE, &actif);
    }

    /// Le chemin d'une feuille hébergée : déclare son hôte.
    pub(super) fn hosted_in(&mut self, hote: Option<ItemId>) {
        self.host_leaf = hote;
    }

    /// Change l'épinglage ; `true` s'il a changé (`pinned-change` émis).
    pub(super) fn poser_epingle(&mut self, pinned: bool) -> bool {
        if self.pinned == pinned {
            return false;
        }
        self.pinned = pinned;
        self.events.trigger(PINNED_CHANGE, &pinned);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── Ajoutés : reserverId n'a pas de test TS ───

    #[test]
    fn les_ids_suivent_le_compteur_en_base_36() {
        let mut g = GenerateurIds::new();
        assert_eq!(g.suivant(), "leaf-0");
        for _ in 0..34 {
            g.suivant();
        }
        assert_eq!(g.suivant(), "leaf-z");
        assert_eq!(g.suivant(), "leaf-10");
    }

    #[test]
    fn reserver_un_id_du_disque_fait_repartir_le_compteur_au_dela() {
        let mut g = GenerateurIds::new();
        g.reserver("leaf-7");
        g.reserver("leaf-3"); // plus petit : rien ne recule
        g.reserver("onglet-perso"); // autre forme : rien
        g.reserver("leaf-"); // vide : rien
        assert_eq!(g.suivant(), "leaf-8");
    }
}
