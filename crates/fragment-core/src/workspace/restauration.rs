// Port de app/src/ui/core/workspace/Workspace.ts (persistance du layout, §8.2 et §16.1 :
// getLayout, saveLayout, loadLayout, defaultLayout, changeLayout, la reconstruction et la
// matérialisation des feuilles visibles).
//
// Le disque est derrière un trait (`LayoutStore`) : `fragment-app` l'implémente sur
// `.fragment/workspace.json` ; les tests utilisent `LayoutMemoire`.
//
// ★ Tout ce qui peut mal tourner (fichier absent, JSON tronqué, format d'une autre version)
//   retombe sur le layout par défaut : un coffre qu'on n'arrive pas à restaurer s'ouvre comme
//   un coffre neuf, il ne refuse jamais de démarrer.
// ★ §8.4 — la restauration ne construit AUCUNE vue : chaque feuille reçoit son état en
//   coquille. Seules les feuilles réellement à l'écran se matérialisent, à la fin.

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use serde_json::Value;
use thiserror::Error;

use super::arbre::ItemId;
use super::espace::{LAYOUT_CHANGE, NouvelleFeuille, Workspace};
use super::layout::{LayoutNode, LeafLayout, SplitLayout, WorkspaceLayout};
use super::vue::EMPTY_VIEW_TYPE;
use super::{OpenViewState, ViewState};

/// Les échecs du stockage du layout.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ErreurLayout {
    #[error("lecture du layout impossible : {0}")]
    Lecture(String),
    #[error("écriture du layout impossible : {0}")]
    Ecriture(String),
    #[error("sérialisation du layout impossible : {0}")]
    Serialisation(String),
}

/// Où vit workspace.json. `lire` rend `None` si aucun layout n'a encore été écrit.
pub trait LayoutStore {
    fn lire(&self) -> Result<Option<String>, ErreurLayout>;
    fn ecrire(&self, json: &str) -> Result<(), ErreurLayout>;
}

/// Un stockage en mémoire, partagé entre ses clones : deux « sessions » successives d'un test
/// relisent le même fichier.
#[derive(Debug, Clone, Default)]
pub struct LayoutMemoire {
    contenu: Rc<RefCell<Option<String>>>,
}

impl LayoutMemoire {
    pub fn new() -> Self {
        Self::default()
    }

    /// Un stockage qui contient déjà ce texte (y compris un texte corrompu).
    pub fn avec(texte: &str) -> Self {
        let m = Self::new();
        *m.contenu.borrow_mut() = Some(texte.to_owned());
        m
    }

    pub fn contenu(&self) -> Option<String> {
        self.contenu.borrow().clone()
    }
}

impl LayoutStore for LayoutMemoire {
    fn lire(&self) -> Result<Option<String>, ErreurLayout> {
        Ok(self.contenu())
    }

    fn ecrire(&self, json: &str) -> Result<(), ErreurLayout> {
        *self.contenu.borrow_mut() = Some(json.to_owned());
        Ok(())
    }
}

impl Workspace {
    pub fn set_layout_store(&mut self, store: Rc<dyn LayoutStore>) {
        self.store = Some(store);
    }

    /// §8.2 — l'arbre entier réduit à du JSON : chaque racine se sérialise, chaque feuille
    /// aussi. `active` = l'id de la feuille active, ou `null`.
    pub fn get_layout(&self) -> WorkspaceLayout {
        let feuille = |_: ItemId, l: &super::feuille::WorkspaceLeaf| l.serialize();
        let racine = |id: ItemId| {
            self.arbre
                .serialiser_split(id, &feuille)
                .unwrap_or_else(|| SplitLayout {
                    direction: super::SplitDirection::Vertical,
                    children: Vec::new(),
                    collapsed: None,
                    width: None,
                    dimension: None,
                })
        };
        WorkspaceLayout {
            version: 1,
            main: racine(self.arbre.root()),
            left: racine(self.arbre.left()),
            right: racine(self.arbre.right()),
            active: self
                .active_leaf
                .and_then(|a| self.leaf(a))
                .map(|l| l.id.clone()),
        }
    }

    /// L'écriture (l'hôte la débounce). `Ok(false)` sans rien écrire tant que le layout n'est
    /// pas prêt, ou sans stockage.
    ///
    /// ★ Le garde `layout_ready` n'est pas une optimisation : pendant la reconstruction, chaque
    ///   ajout demande une sauvegarde, et écrire un arbre à moitié reconstruit par-dessus le
    ///   fichier qu'on est en train de relire PERDRAIT le layout.
    pub fn save_layout(&mut self) -> Result<bool, ErreurLayout> {
        if !self.layout_ready {
            return Ok(false);
        }
        let Some(store) = self.store.clone() else {
            return Ok(false);
        };
        let json = serde_json::to_string_pretty(&self.get_layout())
            .map_err(|e| ErreurLayout::Serialisation(e.to_string()))?;
        store.ecrire(&json)?;
        self.sauvegarde_due = false;
        Ok(true)
    }

    /// §16.1 — reconstruit l'arbre depuis le stockage, matérialise ce qui est visible, puis
    /// déclare le layout prêt (APRÈS le chargement des vues visibles, comme Obsidian : un
    /// plugin qui touche au layout dans `on_layout_ready` trouve un arbre complet).
    pub fn load_layout(&mut self) {
        let layout = self.lire_layout();
        match layout {
            Some(l) => self.change_layout(&l),
            None => self.default_layout(),
        }
        self.materialize_visible();
        self.layout_ready = true;
        for cb in std::mem::take(&mut self.layout_ready_callbacks) {
            cb();
        }
        // La structure est neuve : une seule émission, la nôtre.
        let _ = self.arbre.prendre_changement_de_structure();
        self.events.trigger(LAYOUT_CHANGE, &());
    }

    fn lire_layout(&self) -> Option<WorkspaceLayout> {
        let store = self.store.as_ref()?;
        let texte = match store.lire() {
            Ok(Some(t)) => t,
            Ok(None) => return None,
            Err(e) => {
                log::warn!("[workspace] workspace.json inexploitable — layout par défaut ({e})");
                return None;
            }
        };
        let valeur: Value = match serde_json::from_str(&texte) {
            Ok(v) => v,
            Err(e) => {
                log::warn!("[workspace] workspace.json inexploitable — layout par défaut ({e})");
                return None;
            }
        };
        let layout = WorkspaceLayout::from_value(&valeur);
        if layout.is_none() {
            log::warn!(
                "[workspace] workspace.json illisible ou d'un format inconnu — layout par défaut"
            );
        }
        layout
    }

    /// Le layout d'un coffre neuf : une feuille du type par défaut au dock gauche, le document
    /// initial (s'il y en a un) dans une pile du centre.
    fn default_layout(&mut self) {
        if let Ok(gauche) = self.get_left_leaf() {
            let type_ = self.default_side_view_type.clone();
            self.set_view_state(gauche, ViewState::new(type_), None);
        }
        let Ok(centre) = self.get_leaf(NouvelleFeuille::Split) else {
            return;
        };
        if let Some(doc) = self.document_initial.clone() {
            self.open_document_in(
                centre,
                &doc,
                Some(OpenViewState {
                    active: Some(true),
                    ..OpenViewState::default()
                }),
            );
        }
    }

    /// §8.2 — l'inverse de `get_layout` : vide les trois racines et les remplit. Aucune vue
    /// n'est construite ici.
    pub fn change_layout(&mut self, layout: &WorkspaceLayout) {
        self.set_active_hosted_leaf(None);
        if let Some(a) = self.active_leaf.take() {
            if let Some(s) = self.leaf(a).and_then(|l| l.view().scope()) {
                self.ctx.keymap.pop_scope(&s);
            }
            if let Some(f) = self.arbre.feuille_mut(a) {
                f.marquer_active(false);
            }
        }

        // Les ids du fichier d'abord : réservés AVANT de créer la moindre feuille, pour
        // qu'aucune feuille neuve ne reprenne l'un d'eux.
        fn reserver(n: &LayoutNode, ids: &mut super::feuille::GenerateurIds) {
            match n {
                LayoutNode::Leaf(l) => ids.reserver(&l.id),
                LayoutNode::Split(s) => s.children.iter().for_each(|c| reserver(c, ids)),
                LayoutNode::Tabs(t) => t.children.iter().for_each(|c| reserver(c, ids)),
            }
        }
        for r in [&layout.main, &layout.left, &layout.right] {
            for c in &r.children {
                reserver(c, &mut self.ids);
            }
        }
        let mut deja = HashSet::new();
        self.restore_into(self.arbre.root(), &layout.main, &mut deja);
        self.restore_into(self.arbre.left(), &layout.left, &mut deja);
        self.restore_into(self.arbre.right(), &layout.right, &mut deja);

        // L'active par son id, après reconstruction complète (n'importe quelle racine).
        if let Some(id) = &layout.active {
            self.active_leaf = self
                .iterate_all_leaves()
                .into_iter()
                .find(|&l| self.leaf(l).is_some_and(|f| &f.id == id));
        }
        // Repli : id périmé, ou layout sauvé avec `active: null`. Sans lui, l'actif resterait
        // vide au démarrage alors qu'un document est à l'écran, et toute commande « sur la vue
        // active » démarrerait cassée. Jamais un dock : le document regardé est au centre.
        if self.active_leaf.is_none() {
            self.active_leaf = self.premiere_feuille_visible(self.arbre.root());
        }
        // ★ L'active est l'onglet VISIBLE de sa pile : un layout sauvé avant le correctif
        //   pouvait désigner un onglet CACHÉ (sa portée était alors celle d'une note invisible).
        //   L'onglet montré gagne : c'est lui qu'on voit.
        if let Some(a) = self.active_leaf
            && let Some(p) = self.arbre.parent(a).filter(|&p| self.arbre.est_pile(p))
            && self.arbre.active_leaf(p) != Some(a)
        {
            self.active_leaf = self.arbre.active_leaf(p);
        }
    }

    /// Vide une racine PERMANENTE et la remplit depuis son nœud ; ses propriétés (direction,
    /// largeur et repli d'un dock) sont réappliquées.
    fn restore_into(&mut self, racine: ItemId, node: &SplitLayout, deja: &mut HashSet<String>) {
        for enfant in self.arbre.children(racine).to_vec() {
            let _ = self.arbre.remove_child(racine, enfant);
            self.liberer_avec_vues(enfant);
        }
        self.arbre.set_direction(racine, node.direction);
        let enfants = self.construire_enfants(racine, &node.children, deja);
        self.appliquer_geometrie(racine, &enfants);
        if self.arbre.sidedock(racine).is_some() {
            if let Some(w) = node.width {
                self.arbre.poser_largeur(racine, Some(w));
            }
            if node.collapsed == Some(true) {
                self.arbre.collapse(racine);
            } else {
                self.arbre.expand(racine);
            }
        }
    }

    /// Libère un sous-arbre détaché en fermant les vues de ses feuilles.
    fn liberer_avec_vues(&mut self, item: ItemId) {
        if let Ok(feuilles) = self.arbre.liberer(item) {
            for (_, mut f) in feuilles {
                f.view.on_close();
            }
        }
    }

    /// Construit les enfants d'un parent ; rend (nœud, dimension) de chaque enfant inséré. Un
    /// enfant que la forme du parent refuse (une feuille directement dans un split, écrite à la
    /// main) est écarté avec un avertissement : le reste du layout tient.
    fn construire_enfants(
        &mut self,
        parent: ItemId,
        noeuds: &[LayoutNode],
        deja: &mut HashSet<String>,
    ) -> Vec<(ItemId, Option<f64>)> {
        let mut out = Vec::new();
        for n in noeuds {
            let Some(item) = self.build_node(n, deja) else {
                continue;
            };
            match self.arbre.add_child(parent, item, None) {
                Ok(()) => out.push((item, dimension(n))),
                Err(e) => {
                    log::warn!("[workspace] nœud de layout écarté : {e}");
                    self.liberer_avec_vues(item);
                }
            }
        }
        out
    }

    /// Construit un sous-arbre détaché. Aucune vue n'est instanciée.
    fn build_node(&mut self, node: &LayoutNode, deja: &mut HashSet<String>) -> Option<ItemId> {
        match node {
            LayoutNode::Leaf(l) => Some(self.build_leaf(l, deja)),
            LayoutNode::Tabs(t) => {
                let pile = self.arbre.creer_pile();
                let enfants = self.construire_enfants(pile, &t.children, deja);
                // APRÈS les enfants : chaque ajout montre le dernier ajouté.
                if !enfants.is_empty() {
                    let i = t.current_tab.clamp(0, enfants.len() as i64 - 1) as usize;
                    self.arbre.select_tab(pile, i);
                }
                Some(pile)
            }
            LayoutNode::Split(s) => {
                let split = self.arbre.creer_split(s.direction);
                let enfants = self.construire_enfants(split, &s.children, deja);
                self.appliquer_geometrie(split, &enfants);
                Some(split)
            }
        }
    }

    fn build_leaf(&mut self, l: &LeafLayout, deja: &mut HashSet<String>) -> ItemId {
        let leaf = {
            let mut f = self.nouvelle_feuille();
            // ★ Un id déjà vu (fichier écrit avant la réservation) garde l'id NEUF : deux
            //   feuilles ne partagent jamais une identité.
            if !deja.contains(&l.id) {
                f.id = l.id.clone();
            }
            deja.insert(f.id.clone());
            self.arbre.creer_feuille(f)
        };
        // ★ Un onglet VIDE n'a rien à différer : la feuille neuve porte déjà la vue vide, et la
        //   différer donnerait une coquille intitulée « empty ».
        if l.state.type_ == EMPTY_VIEW_TYPE {
            if let Some(f) = self.arbre.feuille_mut(leaf) {
                f.pinned = l.state.pinned.unwrap_or(false);
            }
            return leaf;
        }
        // La lecture sauvée, si c'est un objet ; écrite à la main en autre chose : ignorée.
        let lecture = l.e_state.clone().filter(Value::is_object);
        self.set_deferred_state(leaf, l.state.clone(), lecture);
        leaf
    }

    /// Repose les proportions lues, puis l'unique propriétaire de l'invariant de géométrie
    /// (`reflow_geometry`). Conséquence gratuite : un fichier d'avant le correctif (somme < 1)
    /// est renormalisé au démarrage.
    fn appliquer_geometrie(&mut self, split: ItemId, enfants: &[(ItemId, Option<f64>)]) {
        for &(item, d) in enfants {
            if let Some(d) = d.filter(|d| d.is_finite()) {
                self.arbre.poser_grow(item, d);
            }
        }
        self.arbre.reflow_geometry(split);
    }

    /// L'écran entier, à la fin de la restauration : les feuilles visibles, puis l'active
    /// (elle est regardée), puis ses événements, émis UNE fois, quand tout est en place.
    fn materialize_visible(&mut self) {
        for r in [self.arbre.left(), self.arbre.root(), self.arbre.right()] {
            self.materialize_visible_in(r);
        }
        let Some(a) = self.active_leaf else {
            return;
        };
        self.load_if_deferred(a);
        // La portée de l'active (une vue jamais différée — un onglet vide — n'est passée par
        // aucun montage qui l'aurait empilée). L'insertion est idempotente.
        if let Some(s) = self.leaf(a).and_then(|l| l.view().scope()) {
            self.ctx.keymap.insert_scope(&s, &self.app_scope());
        }
        if let Some(f) = self.arbre.feuille_mut(a) {
            f.marquer_active(true);
        }
        let active = Some(a);
        self.events
            .trigger(super::espace::ACTIVE_LEAF_CHANGE, &active);
        let doc = self.document_de(a);
        self.events.trigger(super::espace::FILE_OPEN, &doc);
    }

    /// §16.1 pour les plugins : ne pas toucher au layout avant qu'il soit prêt. Les plugins
    /// se chargent AVANT `load_layout` : leur rappel part plus tard.
    pub fn on_layout_ready(&mut self, cb: impl FnOnce() + 'static) {
        if self.layout_ready {
            cb();
        } else {
            self.layout_ready_callbacks.push(Box::new(cb));
        }
    }
}

fn dimension(n: &LayoutNode) -> Option<f64> {
    match n {
        LayoutNode::Split(s) => s.dimension,
        LayoutNode::Tabs(t) => t.dimension,
        LayoutNode::Leaf(l) => l.dimension,
    }
}
