// Port de app/src/ui/core/workspace/Workspace.ts (la partie vivante, hors persistance : voir
// restauration.rs) et des méthodes de WorkspaceLeaf.ts qui demandent l'application (monter une
// vue par le registre, empiler une portée, ouvrir un document, l'historique, fermer).
//
// Le Workspace possède l'arbre (`Arbre<WorkspaceLeaf>`) et la feuille active. Ses deux
// promesses, tenues après CHAQUE opération (`apres_operation`) :
//
// ★ LA FEUILLE ACTIVE EST L'ONGLET VISIBLE DE SA PILE. Sinon sa portée clavier — celle d'une
//   vue qu'on ne voit pas — reçoit Ctrl+F, Ctrl+Z… Montrer un autre onglet de la pile de
//   l'active (ajout, retrait, sélection) l'active ; activer une feuille cachée la montre.
// ★ ELLE NE DEVIENT JAMAIS NULLE tant qu'une feuille reste dans la zone principale : fermer
//   l'active passe le relais à l'onglet redevenu visible, et à défaut à la première feuille
//   visible du centre. Toute commande « sur la vue active » en dépend.
//
// Deux étages d'actif : `active_leaf` (la feuille d'onglet, l'HÔTE) et `active_hosted_leaf`
// (une feuille détachée hébergée dans un widget de l'hôte : chat ancré, note flottante). La
// portée de l'hôte est INSÉRÉE juste au-dessus de la racine ; celle de l'hébergée juste
// au-dessus de celle de l'hôte (mais chaînée à la racine) ; une modale ou un menu ouvert reste
// toujours au-dessus (`Keymap::insert_scope`).
//
// ★ Les événements partent PENDANT l'opération, le Workspace étant emprunté : un écouteur ne
//   doit pas réemprunter le Workspace, il lit la charge utile (un `ItemId`, une clé). Un
//   écouteur fautif est isolé par `Events` (il panique seul, journalisé).

use std::rc::Rc;

use serde_json::{Map, Value};

use super::arbre::{Arbre, Cote, ErreurArbre, EspaceBoutons, ItemId, MiseEnPage};
use super::feuille::{GenerateurIds, HEADER_CHANGE, HISTORY_CHANGE, WorkspaceLeaf};
use super::pile::DropZone;
use super::restauration::LayoutStore;
use super::vue::{ContexteVue, DeferredView, EMPTY_VIEW_TYPE, EmptyView, View, ViewCreator};
use super::{OpenViewState, PaneType, SplitDirection, ViewState, ViewStateResult};
use crate::document_key::DocumentKey;
use crate::documents::DocumentManager;
use crate::events::Events;
use crate::keymap::{Keymap, Scope};
use crate::leaf_history::{HistoryEntry, LeafHistory};
use crate::registry::Registry;

/// Les noms des événements du Workspace et leur charge utile.
/// `active-leaf-change` : `Option<ItemId>`.
pub const ACTIVE_LEAF_CHANGE: &str = "active-leaf-change";
/// `file-open` : `Option<DocumentKey>` (le document de la feuille active ; Fragment passait le
/// fichier, le Vault vient avec T2).
pub const FILE_OPEN: &str = "file-open";
/// `layout-change` : `()`.
pub const LAYOUT_CHANGE: &str = "layout-change";
/// `resize` : `()`.
pub const RESIZE: &str = "resize";

/// Ce que le Workspace emprunte à l'application : le routeur clavier (et sa racine), le
/// registre des vues par type, les documents. Des poignées partagées.
#[derive(Clone)]
pub struct ContexteApp {
    pub keymap: Keymap,
    pub views: Registry<ViewCreator>,
    pub documents: Rc<DocumentManager>,
}

/// Où `get_leaf` prend sa feuille (`getLeaf(false | 'tab' | 'split')`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NouvelleFeuille {
    /// Réutiliser la feuille active du centre si elle n'est pas épinglée.
    Non,
    /// Un nouvel onglet dans la pile de l'active du centre.
    Onglet,
    /// Une nouvelle pile dans la zone centrale.
    Split,
}

impl NouvelleFeuille {
    /// D'après le type de pane d'une frappe modifiée (`Keymap::is_mod_event`) : une fenêtre
    /// détachée n'existe pas encore, elle devient un onglet.
    pub fn depuis_pane(pane: Option<PaneType>) -> Self {
        match pane {
            None => Self::Non,
            Some(PaneType::Tab | PaneType::Window) => Self::Onglet,
            Some(PaneType::Split) => Self::Split,
        }
    }
}

/// Le Workspace : l'arbre, la feuille active, les vues.
pub struct Workspace {
    pub(super) arbre: Arbre<WorkspaceLeaf>,
    pub(super) ctx: ContexteApp,
    pub(super) ids: GenerateurIds,
    pub(super) events: Events,
    pub(super) active_leaf: Option<ItemId>,
    pub(super) active_hosted_leaf: Option<ItemId>,
    /// §16.1 — faux jusqu'à la fin de `load_layout` : l'invariant offert aux plugins
    /// (`on_layout_ready`) et le garde qui interdit d'écrire un arbre en reconstruction.
    pub(super) layout_ready: bool,
    pub(super) layout_ready_callbacks: Vec<Box<dyn FnOnce()>>,
    /// Le type de vue posé dans le dock gauche d'un coffre neuf. Vide par défaut : le cœur ne
    /// connaît AUCUNE vue par son nom, c'est l'assemblage qui le renseigne.
    pub default_side_view_type: String,
    /// Le document ouvert au centre d'un coffre neuf (Fragment prenait la première note du
    /// coffre ; sans Vault, c'est l'assemblage qui le dit).
    pub document_initial: Option<DocumentKey>,
    pub(super) store: Option<Rc<dyn LayoutStore>>,
    pub(super) sauvegarde_due: bool,
    en_operation: bool,
}

impl std::fmt::Debug for Workspace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Workspace")
            .field("feuilles", &self.arbre.feuilles().len())
            .field("active_leaf", &self.active_leaf)
            .field("layout_ready", &self.layout_ready)
            .finish()
    }
}

impl Workspace {
    pub fn new(ctx: ContexteApp) -> Self {
        Self {
            arbre: Arbre::new(),
            ctx,
            ids: GenerateurIds::new(),
            events: Events::new(),
            active_leaf: None,
            active_hosted_leaf: None,
            layout_ready: false,
            layout_ready_callbacks: Vec::new(),
            default_side_view_type: EMPTY_VIEW_TYPE.to_owned(),
            document_initial: None,
            store: None,
            sauvegarde_due: false,
            en_operation: false,
        }
    }

    // ─── Lecture ───

    /// L'arbre, en lecture (la couche de rendu le parcourt). Toute mutation passe par le
    /// Workspace, qui tient ses invariants.
    pub fn arbre(&self) -> &Arbre<WorkspaceLeaf> {
        &self.arbre
    }

    pub fn events(&self) -> &Events {
        &self.events
    }

    pub fn root_split(&self) -> ItemId {
        self.arbre.root()
    }

    pub fn left_split(&self) -> ItemId {
        self.arbre.left()
    }

    pub fn right_split(&self) -> ItemId {
        self.arbre.right()
    }

    pub fn leaf(&self, id: ItemId) -> Option<&WorkspaceLeaf> {
        self.arbre.feuille(id)
    }

    /// La vue d'une feuille, en écriture : ce qu'une commande « sur la vue active » ou la
    /// couche de rendu modifient. Remplacer la vue, lui, passe par `set_view_state`.
    pub fn view_mut(&mut self, leaf: ItemId) -> Option<&mut dyn View> {
        self.arbre.feuille_mut(leaf).map(WorkspaceLeaf::view_mut)
    }

    pub fn active_leaf(&self) -> Option<ItemId> {
        self.active_leaf
    }

    pub fn active_hosted_leaf(&self) -> Option<ItemId> {
        self.active_hosted_leaf
    }

    pub fn layout_ready(&self) -> bool {
        self.layout_ready
    }

    /// La portée racine (`app.scope`).
    pub fn app_scope(&self) -> Scope {
        self.ctx.keymap.get_root_scope()
    }

    /// Toutes les feuilles de l'arbre (`iterateAllLeaves`) : dock gauche, centre, dock droit.
    pub fn iterate_all_leaves(&self) -> Vec<ItemId> {
        self.arbre.feuilles()
    }

    /// ★ Par le type DEMANDÉ de la feuille, jamais celui de sa vue : un onglet restauré mais
    ///   pas encore regardé doit être trouvé par le plugin qui cherche ses vues.
    pub fn get_leaves_of_type(&self, type_: &str) -> Vec<ItemId> {
        self.iterate_all_leaves()
            .into_iter()
            .filter(|&l| self.leaf(l).is_some_and(|f| f.get_view_type() == type_))
            .collect()
    }

    /// Les feuilles qui montrent (ou veulent montrer) ce document.
    pub fn get_leaves_of_document(&self, key: &str) -> Vec<ItemId> {
        self.iterate_all_leaves()
            .into_iter()
            .filter(|&l| self.document_de(l).as_deref() == Some(key))
            .collect()
    }

    /// Le document de la feuille (`leaf.file` / `getDocument()`), résolu sans la matérialiser.
    pub fn document_de(&self, leaf: ItemId) -> Option<DocumentKey> {
        self.leaf(leaf)?.get_document(&self.ctx.documents)
    }

    /// Une sauvegarde du layout est-elle due ? Vrai une fois, puis faux. L'hôte la débounce
    /// (2 s dans Fragment) et appelle `save_layout`.
    pub fn prendre_sauvegarde_due(&mut self) -> bool {
        std::mem::take(&mut self.sauvegarde_due)
    }

    pub fn request_save_layout(&mut self) {
        self.sauvegarde_due = true;
    }

    // ─── Création ───

    pub(super) fn nouvelle_feuille(&mut self) -> WorkspaceLeaf {
        let docs = self.ctx.documents.clone();
        let history = LeafHistory::with_is_alive(move |d| docs.exists(d));
        let scope = self.app_scope();
        WorkspaceLeaf::new(self.ids.suivant(), EmptyView::new(&scope, None), history)
    }

    /// Une feuille neuve (vue vide non montée) dans `parent` (`createLeafInParent`) ; `None`
    /// pour l'index : à la fin.
    pub fn create_leaf_in_parent(
        &mut self,
        parent: ItemId,
        index: Option<usize>,
    ) -> Result<ItemId, ErreurArbre> {
        let f = self.nouvelle_feuille();
        let leaf = self.arbre.creer_feuille(f);
        if let Err(e) = self.arbre.add_child(parent, leaf, index) {
            let _ = self.arbre.liberer(leaf);
            return Err(e);
        }
        self.apres_operation();
        Ok(leaf)
    }

    /// Une feuille DÉTACHÉE hébergée par `hote` (un widget : chat ancré, note flottante). Son
    /// contenu vit dans l'hôte : un contact dedans active l'HÔTE.
    pub fn create_hosted_leaf(&mut self, hote: ItemId) -> ItemId {
        let mut f = self.nouvelle_feuille();
        f.hosted_in(Some(hote));
        self.arbre.creer_feuille(f)
    }

    /// La feuille active si elle est dans la zone centrale (les docks hébergent des outils,
    /// pas des documents : ouvrir une note depuis l'explorateur ne doit pas le remplacer).
    fn active_du_centre(&self) -> Option<ItemId> {
        self.active_leaf
            .filter(|&a| self.arbre.get_root(a) == self.arbre.root())
    }

    /// Rend une feuille utilisable (`getLeaf`).
    pub fn get_leaf(&mut self, nouvelle: NouvelleFeuille) -> Result<ItemId, ErreurArbre> {
        let active = self.active_du_centre();
        if nouvelle == NouvelleFeuille::Non
            && let Some(a) = active
            && !self.leaf(a).is_some_and(WorkspaceLeaf::pinned)
        {
            return Ok(a);
        }
        let root = self.arbre.root();
        if nouvelle == NouvelleFeuille::Split {
            let pile = self.arbre.creer_pile();
            self.arbre.add_child(root, pile, None)?;
            return self.create_leaf_in_parent(pile, None);
        }
        // Un onglet : dans la pile de l'active du centre ; sinon une pile existante du centre
        // (sinon chaque ouverture depuis l'explorateur ajouterait une colonne) ; sinon une neuve.
        let pile = active
            .and_then(|a| self.arbre.parent(a))
            .filter(|&p| self.arbre.est_pile(p))
            .or_else(|| {
                self.arbre
                    .children(root)
                    .iter()
                    .copied()
                    .find(|&c| self.arbre.est_pile(c))
            });
        let pile = match pile {
            Some(p) => p,
            None => {
                let p = self.arbre.creer_pile();
                self.arbre.add_child(root, p, None)?;
                p
            }
        };
        self.create_leaf_in_parent(pile, None)
    }

    fn feuille_de_dock(&mut self, cote: Cote) -> Result<ItemId, ErreurArbre> {
        let dock = self.arbre.dock(cote);
        let existante = self
            .arbre
            .children(dock)
            .iter()
            .copied()
            .find(|&c| self.arbre.est_pile(c));
        let pile = match existante {
            Some(p) => p,
            None => {
                let p = self.arbre.creer_pile();
                self.arbre.add_child(dock, p, None)?;
                p
            }
        };
        self.create_leaf_in_parent(pile, None)
    }

    /// Une feuille dans le dock gauche (l'explorateur, la recherche…).
    pub fn get_left_leaf(&mut self) -> Result<ItemId, ErreurArbre> {
        self.feuille_de_dock(Cote::Gauche)
    }

    /// Une feuille dans le dock droit : `get_right_leaf` puis `set_view_state`, c'est ainsi
    /// qu'un plugin monte sa vue.
    pub fn get_right_leaf(&mut self) -> Result<ItemId, ErreurArbre> {
        self.feuille_de_dock(Cote::Droite)
    }

    /// Garantit une pile au dock droit, pour qu'il s'affiche quand on l'ouvre vide (et
    /// devienne une cible de dépôt).
    pub fn ensure_right_pile(&mut self) -> Result<(), ErreurArbre> {
        let right = self.arbre.right();
        if self
            .arbre
            .children(right)
            .iter()
            .any(|&c| self.arbre.est_pile(c))
        {
            return Ok(());
        }
        let leaf = self.get_right_leaf()?;
        self.set_view_state(leaf, ViewState::new(EMPTY_VIEW_TYPE), None);
        Ok(())
    }

    /// Rend une feuille VISIBLE et ACTIVE : déplie son dock, montre son onglet, l'active.
    pub fn reveal_leaf(&mut self, leaf: ItemId) {
        let racine = self.arbre.get_root(leaf);
        if self.arbre.sidedock(racine).is_some_and(|d| d.collapsed) {
            self.expand(racine);
        }
        self.arbre.select_leaf(leaf);
        self.set_active_leaf(Some(leaf));
    }

    /// Une nouvelle feuille à côté de `leaf` (`Vertical` : à droite ; `Horizontal` : en
    /// dessous), devenue active. Délègue à la découpe de la pile.
    pub fn create_leaf_by_split(
        &mut self,
        leaf: ItemId,
        direction: SplitDirection,
    ) -> Option<ItemId> {
        let f = self.nouvelle_feuille();
        let nouvelle = self.arbre.create_leaf_by_split(leaf, direction, f)?;
        self.set_active_leaf(Some(nouvelle));
        self.apres_operation();
        Some(nouvelle)
    }

    /// Le commit d'un dépôt de feuille (glisser d'onglet) ; une découpe active la feuille.
    pub fn drop_leaf(
        &mut self,
        pile: ItemId,
        leaf: ItemId,
        zone: DropZone,
        interstice: usize,
    ) -> Result<(), ErreurArbre> {
        let r = self.arbre.drop_leaf(pile, leaf, zone, interstice);
        if r.is_ok() && !matches!(zone, DropZone::Reorder | DropZone::Center) {
            self.set_active_leaf(Some(leaf));
        }
        self.apres_operation();
        r
    }

    /// Déplace un nœud (pile, split) dans l'arbre (`addChild` d'un nœud existant).
    pub fn move_item(
        &mut self,
        parent: ItemId,
        item: ItemId,
        index: Option<usize>,
    ) -> Result<(), ErreurArbre> {
        let r = self.arbre.add_child(parent, item, index);
        self.apres_operation();
        r
    }

    /// Pose la part de flex-grow d'un pane (la fin d'un geste de poignée).
    pub fn poser_grow(&mut self, item: ItemId, grow: f64) {
        self.arbre.poser_grow(item, grow);
        self.request_save_layout();
    }

    // ─── Onglets ───

    /// Montre l'onglet `index` (`selectTab`). Si l'active est dans cette pile, l'onglet montré
    /// devient l'actif ; une feuille différée qui devient visible se matérialise.
    pub fn select_tab(&mut self, pile: ItemId, index: usize) -> bool {
        let ok = self.arbre.select_tab(pile, index);
        self.apres_operation();
        ok
    }

    /// L'appui sur un en-tête d'onglet : montrer ET activer, dès l'appui (un clic humain bouge
    /// souvent de quelques pixels, franchit le seuil du glisser et avalait le `click`).
    pub fn activer_onglet(&mut self, pile: ItemId, index: usize) {
        if self.arbre.select_tab(pile, index)
            && let Some(leaf) = self.arbre.active_leaf(pile)
        {
            self.set_active_leaf(Some(leaf));
        }
        self.apres_operation();
    }

    /// Un appui ou un focus N'IMPORTE OÙ dans une feuille (en capture) la rend active, comme
    /// chez Obsidian. ★ Avant, seul l'onglet activait : une fois le chat du dock actif,
    /// revenir écrire dans la note la laissait inactive, et ses raccourcis partaient au chat.
    /// Une feuille hébergée (sans parent) vit DANS son hôte : c'est l'hôte qui s'active.
    pub fn activer_par_contact(&mut self, leaf: ItemId) {
        let cible = if self.arbre.parent(leaf).is_some() {
            Some(leaf)
        } else {
            self.leaf(leaf).and_then(WorkspaceLeaf::host_leaf)
        };
        if let Some(c) = cible
            && self.active_leaf != Some(c)
        {
            self.set_active_leaf(Some(c));
        }
    }

    // ─── L'actif ───

    /// Change la feuille active.
    ///
    /// - L'hébergée active tombe si son hôte n'est plus l'actif (sa portée est au-dessus).
    /// - La portée de l'ancienne vue est retirée, celle de la nouvelle INSÉRÉE à l'étage des
    ///   vues (juste au-dessus de la racine) : une modale ouverte reste au-dessus.
    /// - ★ Une feuille cachée dans sa pile est d'abord MONTRÉE (l'invariant de l'en-tête).
    /// - Devenir active, c'est être regardée : une feuille différée se matérialise.
    /// - `active-leaf-change` puis `file-open` (un plugin d'interface veut le premier, un
    ///   indexeur le second).
    pub fn set_active_leaf(&mut self, leaf: Option<ItemId>) {
        let leaf = leaf.filter(|&l| self.arbre.est_feuille(l));
        if self.active_leaf == leaf {
            return;
        }
        if let Some(h) = self.active_hosted_leaf
            && self.leaf(h).and_then(WorkspaceLeaf::host_leaf) != leaf
        {
            self.set_active_hosted_leaf(None);
        }
        let ancienne = std::mem::replace(&mut self.active_leaf, leaf);
        if let Some(l) = leaf {
            self.arbre.select_leaf(l);
        }
        if let Some(a) = ancienne.and_then(|a| self.arbre.feuille_mut(a)) {
            a.marquer_active(false);
        }
        if let Some(l) = leaf.and_then(|l| self.arbre.feuille_mut(l)) {
            l.marquer_active(true);
        }
        if let Some(s) = ancienne.and_then(|a| self.scope_de(a)) {
            self.ctx.keymap.pop_scope(&s);
        }
        if let Some(s) = leaf.and_then(|l| self.scope_de(l)) {
            self.ctx.keymap.insert_scope(&s, &self.app_scope());
        }
        if let Some(l) = leaf
            && self.leaf(l).is_some_and(WorkspaceLeaf::is_deferred)
        {
            self.load_if_deferred(l);
        }
        self.events.trigger(ACTIVE_LEAF_CHANGE, &leaf);
        let doc = leaf.and_then(|l| self.document_de(l));
        self.events.trigger(FILE_OPEN, &doc);
        self.apres_operation();
    }

    /// Le SECOND étage de l'actif : seul mutateur. N'accepte qu'une feuille hébergée PAR la
    /// feuille active (sinon : aucune). Sa portée est insérée juste au-dessus de celle de
    /// l'hôte ; elle reste chaînée à la RACINE (sinon Ctrl+Z dans un chat ancré sur un PDF
    /// partait aux traits du PDF).
    pub fn set_active_hosted_leaf(&mut self, leaf: Option<ItemId>) {
        let leaf = leaf.filter(|&l| {
            let hote = self.leaf(l).and_then(WorkspaceLeaf::host_leaf);
            hote.is_some() && hote == self.active_leaf
        });
        if self.active_hosted_leaf == leaf {
            return;
        }
        let ancienne = std::mem::replace(&mut self.active_hosted_leaf, leaf);
        if let Some(s) = ancienne.and_then(|a| self.scope_de(a)) {
            self.ctx.keymap.pop_scope(&s);
        }
        if let Some(l) = leaf
            && let Some(s) = self.scope_de(l)
        {
            let dessous = self
                .leaf(l)
                .and_then(WorkspaceLeaf::host_leaf)
                .and_then(|h| self.scope_de(h))
                .unwrap_or_else(|| self.app_scope());
            self.ctx.keymap.insert_scope(&s, &dessous);
        }
        if let Some(a) = ancienne.and_then(|a| self.arbre.feuille_mut(a)) {
            a.marquer_active(false);
        }
        if let Some(l) = leaf.and_then(|l| self.arbre.feuille_mut(l)) {
            l.marquer_active(true);
        }
    }

    fn scope_de(&self, leaf: ItemId) -> Option<Scope> {
        self.leaf(leaf)?.view().scope()
    }

    // ─── Les vues d'une feuille ───

    /// Remplace la vue d'une feuille. ★ Si la feuille est active (ou l'hébergée active), sa
    /// portée suit la vue : `set_active_leaf` n'empile qu'au changement de FEUILLE, et ouvrir
    /// un document dans l'onglet courant laissait sinon l'ancienne portée empilée.
    fn poser_vue(&mut self, leaf: ItemId, vue: Box<dyn View>) {
        if let Some(h) = self.active_hosted_leaf
            && self.leaf(h).and_then(WorkspaceLeaf::host_leaf) == Some(leaf)
        {
            self.set_active_hosted_leaf(None);
        }
        let empilee = self.active_leaf == Some(leaf) || self.active_hosted_leaf == Some(leaf);
        if empilee {
            if let Some(s) = self.scope_de(leaf) {
                self.ctx.keymap.pop_scope(&s);
            }
            if let Some(s) = vue.scope() {
                let dessous = self
                    .leaf(leaf)
                    .and_then(WorkspaceLeaf::host_leaf)
                    .and_then(|h| self.scope_de(h))
                    .unwrap_or_else(|| self.app_scope());
                self.ctx.keymap.insert_scope(&s, &dessous);
            }
        }
        if let Some(f) = self.arbre.feuille_mut(leaf) {
            f.view = vue;
        }
    }

    fn fermer_vue(&mut self, leaf: ItemId) {
        if let Some(f) = self.arbre.feuille_mut(leaf) {
            f.view.on_close();
        }
    }

    /// Monte la vue demandée par `vs` (`setViewState`) : la fabrique du registre, ou — type
    /// inconnu — une vue vide qui MÉMORISE ce qu'elle voulait (un état normal, pas une erreur).
    ///
    /// ★ `empty` n'est pas un type inconnu, c'est l'onglet vide : personne ne l'enregistre, et
    ///   le passer comme type demandé ferait dire à la vue vide « aucune vue pour empty ».
    pub fn set_view_state(&mut self, leaf: ItemId, vs: ViewState, e_state: Option<Value>) {
        let Some(f) = self.arbre.feuille_mut(leaf) else {
            return;
        };
        f.requested_state = vs.clone();
        f.is_deferred = false;
        f.deferred_e_state = None;
        self.fermer_vue(leaf);

        let contexte = ContexteVue {
            leaf,
            app_scope: self.app_scope(),
        };
        let vue: Box<dyn View> = match self.ctx.views.get(&vs.type_) {
            Some(fabrique) => fabrique(&contexte),
            None => Box::new(EmptyView::new(
                &contexte.app_scope,
                (vs.type_ != EMPTY_VIEW_TYPE).then(|| vs.type_.clone()),
            )),
        };
        self.poser_vue(leaf, vue);
        if let Some(f) = self.arbre.feuille_mut(leaf) {
            f.view.on_open();
            if let Some(state) = &vs.state {
                f.view.set_state(state, ViewStateResult { history: false });
            }
            if let Some(e) = e_state.as_ref().filter(|e| !e.is_null()) {
                f.view.set_ephemeral_state(e);
            }
            f.pinned = vs.pinned.unwrap_or(false);
        }
        self.events.trigger(LAYOUT_CHANGE, &());
        if vs.active == Some(true) {
            self.set_active_leaf(Some(leaf));
        }
        self.apres_operation();
    }

    /// §8.4 — pose l'état SANS construire la vue : une coquille qui sait se nommer et se
    /// sérialiser, et ne paiera l'instanciation qu'au premier regard.
    pub fn set_deferred_state(&mut self, leaf: ItemId, vs: ViewState, e_state: Option<Value>) {
        let coquille = DeferredView::new(vs.clone(), &self.ctx.documents);
        let Some(f) = self.arbre.feuille_mut(leaf) else {
            return;
        };
        f.pinned = vs.pinned.unwrap_or(false);
        f.requested_state = vs;
        f.deferred_e_state = e_state;
        f.is_deferred = true;
        self.fermer_vue(leaf);
        self.poser_vue(leaf, Box::new(coquille));
        if let Some(f) = self.arbre.feuille_mut(leaf) {
            f.view.on_open();
        }
    }

    /// §8.4 — matérialise, si besoin. Idempotent : sans effet sur une feuille chargée.
    ///
    /// ★ `file-open` est réémis si la feuille est l'active : c'est précisément l'activation qui
    ///   a demandé la matérialisation, et `set_active_leaf` ne réémet rien pour une feuille déjà
    ///   active — les consommateurs croiraient qu'aucun document n'est ouvert.
    pub fn load_if_deferred(&mut self, leaf: ItemId) {
        let Some(f) = self.arbre.feuille_mut(leaf) else {
            return;
        };
        if !f.is_deferred {
            return;
        }
        // Levé AVANT le montage : un second appel pendant le montage ne relance rien.
        f.is_deferred = false;
        let e_state = f.deferred_e_state.take();
        let vs = f.requested_state.clone();
        self.set_view_state(leaf, vs, e_state);
        if self.active_leaf == Some(leaf) {
            let doc = self.document_de(leaf);
            self.events.trigger(FILE_OPEN, &doc);
        }
    }

    /// Rejoue la demande en cours (un registre a changé : un remplaçant peut devenir la vraie
    /// vue).
    pub fn rebuild_view(&mut self, leaf: ItemId) {
        let Some(f) = self.leaf(leaf) else {
            return;
        };
        let e_state = Some(Value::Object(f.view().get_ephemeral_state()));
        let vs = f.requested_state.clone();
        self.set_view_state(leaf, vs, e_state);
    }

    /// Le registre des vues a changé : chaque remplaçant retente sa vue. Les coquilles
    /// différées, elles, attendent toujours qu'on les regarde.
    pub fn rebuild_placeholders(&mut self) {
        let attente: Vec<ItemId> = self
            .iterate_all_leaves()
            .into_iter()
            .filter(|&l| self.leaf(l).is_some_and(WorkspaceLeaf::is_placeholder))
            .collect();
        for l in attente {
            self.rebuild_view(l);
        }
    }

    /// Ouvre un DOCUMENT par sa clé dans une feuille (`leaf.openDocument`) : le geste de
    /// navigation, qui écrit l'historique. `false` si aucun fournisseur ne sait l'ouvrir.
    ///
    /// ★ Même type de vue que celle montée : bascule EN PLACE (la vue reste montée — passer
    ///   d'une conversation à l'autre ne reconstruit pas la vue). Sinon, une vue neuve.
    pub fn open_document_in(
        &mut self,
        leaf: ItemId,
        key: &str,
        open_state: Option<OpenViewState>,
    ) -> bool {
        let Some(vs) = self.ctx.documents.view_state_for(key) else {
            log::warn!("[workspace] aucun fournisseur ne sait ouvrir « {key} »");
            return false;
        };
        let open_state = open_state.unwrap_or_default();
        self.noter_depart(leaf, key);
        let mut state = vs.state.clone().unwrap_or_default();
        if let Some(s) = &open_state.state {
            state.extend(s.clone());
        }
        let en_place = self.leaf(leaf).is_some_and(|f| {
            !f.is_deferred && !f.is_placeholder() && f.view().get_view_type() == vs.type_
        });
        if en_place {
            if let Some(f) = self.arbre.feuille_mut(leaf) {
                let mut fusion: Map<String, Value> = f.view.get_state();
                fusion.extend(state.clone());
                f.requested_state = ViewState {
                    state: Some(fusion),
                    ..vs.clone()
                };
                f.view.set_state(&state, ViewStateResult { history: true });
                if let Some(e) = &open_state.e_state {
                    f.view.set_ephemeral_state(&Value::Object(e.clone()));
                }
            }
            self.events.trigger(LAYOUT_CHANGE, &());
            self.update_header(leaf);
            if open_state.active == Some(true) {
                self.set_active_leaf(Some(leaf));
            }
            self.apres_operation();
            return true;
        }
        let demande = ViewState {
            state: Some(state),
            active: open_state.active,
            ..vs
        };
        self.set_view_state(leaf, demande, open_state.e_state.map(Value::Object));
        true
    }

    /// Ouvre un document dans une feuille choisie par `get_leaf(nouvelle)` ; rend la feuille.
    pub fn open_document(
        &mut self,
        key: &str,
        nouvelle: NouvelleFeuille,
        open_state: Option<OpenViewState>,
    ) -> Option<ItemId> {
        let leaf = self.get_leaf(nouvelle).ok()?;
        self.open_document_in(leaf, key, open_state).then_some(leaf)
    }

    /// ★ Seule l'OUVERTURE écrit l'historique (le geste de navigation) : la restauration, la
    ///   matérialisation et la reconstruction n'en créent pas — rouvrir l'app n'est pas
    ///   « naviguer ».
    fn noter_depart(&mut self, leaf: ItemId, suivant: &str) {
        let docs = self.ctx.documents.clone();
        let Some(f) = self.arbre.feuille_mut(leaf) else {
            return;
        };
        let quitte = f.view().navigation()
            && !f.is_deferred
            && !f.is_placeholder()
            && f.get_document(&docs).as_deref() != Some(suivant);
        if !quitte {
            return;
        }
        let entree = f.snapshot(&docs);
        f.history.push(entree);
        f.events().trigger(HISTORY_CHANGE, &());
    }

    /// ← : revient à l'état précédent de l'onglet. Faux s'il n'y en a pas.
    pub fn go_back(&mut self, leaf: ItemId) -> bool {
        self.naviguer(leaf, true)
    }

    /// → : repart vers l'état suivant de l'onglet. Faux s'il n'y en a pas.
    pub fn go_forward(&mut self, leaf: ItemId) -> bool {
        self.naviguer(leaf, false)
    }

    fn naviguer(&mut self, leaf: ItemId, arriere: bool) -> bool {
        let docs = self.ctx.documents.clone();
        let Some(f) = self.arbre.feuille_mut(leaf) else {
            return false;
        };
        let courant = f.snapshot(&docs);
        let entree = if arriere {
            f.history.take_back(courant)
        } else {
            f.history.take_forward(courant)
        };
        let Some(entree) = entree else {
            return false;
        };
        self.rejouer(leaf, entree);
        true
    }

    /// Rejoue une entrée : l'état se RELIT chez le fournisseur (le document a pu être renommé,
    /// la clé a suivi) ; le type de vue de l'entrée est gardé. Ne réécrit pas l'historique.
    fn rejouer(&mut self, leaf: ItemId, entree: HistoryEntry) {
        let frais = entree
            .doc
            .as_deref()
            .and_then(|d| self.ctx.documents.view_state_for(d));
        let mut vs = entree.state;
        if let Some(frais) = frais {
            let mut s = vs.state.take().unwrap_or_default();
            s.extend(frais.state.unwrap_or_default());
            vs.state = Some(s);
        }
        vs.active = Some(true);
        self.set_view_state(leaf, vs, entree.e_state);
        if let Some(f) = self.leaf(leaf) {
            f.events().trigger(HISTORY_CHANGE, &());
        }
    }

    /// Un document (ou un dossier) a été renommé : l'historique de chaque feuille suit la clé.
    pub fn rename_document(&mut self, ancienne: &str, nouvelle: &str) {
        for l in self.iterate_all_leaves() {
            if let Some(f) = self.arbre.feuille_mut(l) {
                f.history.rename_document(ancienne, nouvelle);
            }
        }
    }

    /// La vue signale que ce qui la présente (titre, icônes) a changé : `header-change`.
    pub fn update_header(&mut self, leaf: ItemId) {
        if let Some(f) = self.leaf(leaf) {
            f.events().trigger(HEADER_CHANGE, &());
        }
    }

    pub fn set_pinned(&mut self, leaf: ItemId, pinned: bool) {
        if self
            .arbre
            .feuille_mut(leaf)
            .is_some_and(|f| f.poser_epingle(pinned))
        {
            self.request_save_layout();
        }
    }

    pub fn toggle_pinned(&mut self, leaf: ItemId) {
        let p = self.leaf(leaf).is_some_and(WorkspaceLeaf::pinned);
        self.set_pinned(leaf, !p);
    }

    // ─── Fermer ───

    /// Retire la feuille de l'arbre et détruit sa vue (`leaf.detach`).
    ///
    /// ★ Une FERMETURE passe le relais de l'actif ; un déplacement, lui, ne passe pas par ici :
    ///   on ne vole jamais l'actif à une feuille déplacée.
    pub fn detach(&mut self, leaf: ItemId) {
        if !self.arbre.est_feuille(leaf) {
            return;
        }
        self.fermer_vue(leaf);
        // La pile AVANT le retrait : c'est son onglet redevenu visible qui reçoit l'actif.
        let pile = self.arbre.parent(leaf);
        if let Some(p) = pile {
            let _ = self.arbre.remove_child(p, leaf);
        }
        self.traiter_piles_videes();
        self.on_leaf_detached(leaf, pile);
        let _ = self.arbre.liberer(leaf);
        self.apres_operation();
    }

    fn on_leaf_detached(&mut self, leaf: ItemId, pile: Option<ItemId>) {
        if self.active_hosted_leaf == Some(leaf)
            || self
                .active_hosted_leaf
                .and_then(|h| self.leaf(h))
                .and_then(WorkspaceLeaf::host_leaf)
                == Some(leaf)
        {
            self.set_active_hosted_leaf(None);
        }
        if self.active_leaf == Some(leaf) {
            // ★ Fragment rendait `null` quand la pile disparaissait avec son dernier onglet
            //   (dans un split du centre). Repli sur la première feuille visible du centre :
            //   l'actif ne tombe à `None` que si le centre est vide.
            let relais = pile
                .and_then(|p| self.arbre.active_leaf(p))
                .filter(|&l| l != leaf)
                .or_else(|| self.premiere_feuille_visible(self.arbre.root()));
            self.set_active_leaf(relais);
            if self.active_leaf == Some(leaf) {
                self.active_leaf = None;
            }
        }
        self.events.trigger(LAYOUT_CHANGE, &());
        self.request_save_layout();
    }

    /// Une pile vient de perdre son dernier onglet : dans le centre, s'il reste d'AUTRES piles,
    /// elle se ferme ; sinon (dernière pile du centre, ou pile d'un dock) un onglet vide s'y
    /// rouvre. Une pile détachée vide est simplement rendue à l'arène.
    fn on_pile_emptied(&mut self, pile: ItemId) {
        let racine = self.arbre.get_root(pile);
        if racine == pile {
            let _ = self.arbre.liberer(pile);
            return;
        }
        let root = self.arbre.root();
        if racine == root && self.arbre.piles_sous(root).len() > 1 {
            if let Some(p) = self.arbre.parent(pile) {
                let _ = self.arbre.remove_child(p, pile);
            }
            let _ = self.arbre.liberer(pile);
        } else if let Ok(leaf) = self.create_leaf_in_parent(pile, None) {
            self.set_active_leaf(Some(leaf));
        }
    }

    fn traiter_piles_videes(&mut self) -> bool {
        let mut traite = false;
        loop {
            let piles = self.arbre.prendre_piles_videes();
            if piles.is_empty() {
                return traite;
            }
            traite = true;
            for p in piles {
                self.on_pile_emptied(p);
            }
        }
    }

    /// La feuille visible la plus « en amont » d'un sous-arbre.
    pub(super) fn premiere_feuille_visible(&self, item: ItemId) -> Option<ItemId> {
        if self.arbre.est_pile(item) {
            return self.arbre.active_leaf(item);
        }
        self.arbre
            .children(item)
            .iter()
            .find_map(|&c| self.premiere_feuille_visible(c))
    }

    // ─── Les docks ───

    /// Replie un dock (`sidedock.collapse`) : `resize`, sauvegarde.
    pub fn collapse(&mut self, dock: ItemId) {
        if self.arbre.collapse(dock) {
            self.events.trigger(RESIZE, &());
        }
        self.apres_operation();
    }

    /// Déplie un dock. ★ Déplier, c'est regarder : un dock restauré REPLIÉ n'a pas matérialisé
    /// ses feuilles, et le bouton de repli n'est pas un clic sur un onglet — sans ceci, il
    /// s'ouvrirait sur une coquille que rien ne viendrait remplir.
    pub fn expand(&mut self, dock: ItemId) {
        if self.arbre.expand(dock) {
            self.materialize_visible_in(dock);
            self.events.trigger(RESIZE, &());
        }
        self.apres_operation();
    }

    pub fn toggle(&mut self, dock: ItemId) {
        match self.arbre.sidedock(dock).map(|d| d.collapsed) {
            Some(true) => self.expand(dock),
            Some(false) => self.collapse(dock),
            None => {}
        }
    }

    /// Pose la largeur choisie d'un dock (fin du geste de la poignée).
    pub fn poser_largeur_dock(&mut self, dock: ItemId, largeur: Option<f64>) {
        self.arbre.poser_largeur(dock, largeur);
        self.events.trigger(RESIZE, &());
        self.request_save_layout();
    }

    /// La pile qui porte l'espace des boutons de fenêtre (`updateFrameSpacing`), d'après la
    /// mise en page fournie par le rendu (ou la règle de structure, sans elle).
    pub fn espace_des_boutons(
        &self,
        mise_en_page: Option<&MiseEnPage<'_>>,
        largeur_boutons: f64,
    ) -> Option<EspaceBoutons> {
        self.arbre.espace_des_boutons(mise_en_page, largeur_boutons)
    }

    /// Les feuilles visibles d'un sous-arbre (l'onglet courant de chaque pile, hors dock
    /// replié), matérialisées. Public : un dock qu'on déplie le redemande.
    pub fn materialize_visible_in(&mut self, racine: ItemId) {
        let mut visibles = Vec::new();
        self.visibles_sous(racine, false, &mut visibles);
        for l in visibles {
            self.load_if_deferred(l);
        }
    }

    pub(super) fn visibles_sous(&self, item: ItemId, cache: bool, out: &mut Vec<ItemId>) {
        let cache = cache || self.arbre.sidedock(item).is_some_and(|d| d.collapsed);
        if self.arbre.est_pile(item) {
            if let Some(l) = self.arbre.active_leaf(item)
                && !cache
            {
                out.push(l);
            }
            return;
        }
        for &c in self.arbre.children(item) {
            self.visibles_sous(c, cache, out);
        }
    }

    // ─── Après chaque opération ───

    /// Rétablit les promesses du Workspace et vide ce que l'arbre a demandé : piles vidées,
    /// « l'active est l'onglet visible », matérialisation de ce qui devient visible (une fois
    /// le layout prêt), puis UNE émission de `layout-change` si la structure a changé, et la
    /// sauvegarde due. Réentrant sans danger : un appel imbriqué rend la main à l'appel en cours.
    pub(super) fn apres_operation(&mut self) {
        if self.en_operation {
            return;
        }
        self.en_operation = true;
        // Chaque tour converge (une pile vidée se ferme ou se remplit, l'actif rejoint
        // l'onglet visible) ; la borne n'est qu'un garde-fou.
        for _ in 0..8 {
            let mut stable = !self.traiter_piles_videes();
            if self.layout_ready {
                if let Some(a) = self.active_leaf {
                    let visible = self
                        .arbre
                        .parent(a)
                        .filter(|&p| self.arbre.est_pile(p))
                        .and_then(|p| self.arbre.active_leaf(p));
                    if let Some(v) = visible
                        && v != a
                    {
                        // L'appel imbriqué à `apres_operation` rend la main : ce tour-ci et
                        // le suivant finissent le travail.
                        self.set_active_leaf(Some(v));
                        stable = false;
                    }
                    if !self.arbre.existe(a) {
                        self.active_leaf = None;
                    }
                }
                let mut visibles = Vec::new();
                for r in [self.arbre.left(), self.arbre.root(), self.arbre.right()] {
                    self.visibles_sous(r, false, &mut visibles);
                }
                for l in visibles {
                    if self.leaf(l).is_some_and(WorkspaceLeaf::is_deferred) {
                        self.load_if_deferred(l);
                        stable = false;
                    }
                }
            }
            if stable {
                break;
            }
        }
        let structure = self.arbre.prendre_changement_de_structure();
        if self.arbre.prendre_sauvegarde_demandee() {
            self.sauvegarde_due = true;
        }
        self.en_operation = false;
        if structure && self.layout_ready {
            self.events.trigger(LAYOUT_CHANGE, &());
        }
    }
}
