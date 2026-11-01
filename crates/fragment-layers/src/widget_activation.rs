// Port de app/src/ui/core/layers/WidgetActivation.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  L'ACTIF des widgets d'une vue — UN SEUL, tous calques confondus, porté par
//  l'OverlayHost, comme l'arbitre des marges qui le lit.
//
//  ★ Pourquoi ici : avant, trois « actifs » coexistaient sans se parler. L'état
//    vit maintenant au niveau de la VUE ; les widgets le PRÉSENTENT.
//
//  Règles :
//    - un geste pointeur (bouton principal) qui COMMENCE dans un élément inscrit
//      l'active ; un geste commencé DEHORS (texte, autre pane, dock…) désactive.
//      ★ Les deux APRÈS le geste (relâché → tour suivant), jamais à l'appui :
//      l'actif est épinglé par l'arbitre ; si un widget bouge entre l'appui et le
//      relâché, le `click` n'atteint plus sa cible (constaté).
//    - un focus SANS geste en cours (Tab, focus programmatique) active tout de suite ;
//    - clic droit / milieu : sans effet (menu contextuel) ;
//    - un élément MANUEL n'est (dés)activé par aucun geste en lui — seulement par
//      `set_active`. Le geste extérieur le désactive comme les autres ;
//    - l'élément actif porte la classe `is-widget-active` ; ses abonnés et ceux de
//      la vue sont prévenus ; retiré, il cesse d'être actif ;
//    - une entrée peut PORTER une feuille (`set_leaf`) — `active_leaf` est alors la
//      feuille hébergée active de la vue.
//
//  ★ Transposition : pas de DOM. L'hôte (fragment-ui) TRADUIT ses événements en
//    appels du modèle :
//      pointerdown (capture)  → `pointer_down`
//      pointerup / cancel     → `pointer_up`
//      le tour suivant (setTimeout 0) → `run_timers`
//      focusin                → `focus_in`
//    et lit `active_el` / `on_active_change` pour poser la classe
//    `WIDGET_ACTIVE_CLASS`. L'imbrication (« contient ») est fournie par
//    `ElementTree`.
// ═══════════════════════════════════════════════════════════════════════════

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use crate::types::{ElementId, LeafId};

/// La classe posée sur l'élément du widget actif — l'API des thèmes.
pub const WIDGET_ACTIVE_CLASS: &str = "is-widget-active";

/// L'arbre des éléments, vu du modèle : seulement « contient ».
pub trait ElementTree {
    /// `ancestor` contient-il `node` ? (Un élément se contient lui-même, comme
    /// `Node.contains`.)
    fn contains(&self, ancestor: ElementId, node: ElementId) -> bool;
}

/// Où commence un geste pointeur.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PointerDown {
    /// Le bouton (0 = principal).
    pub button: u8,
    /// L'élément visé ; `None` : hors de tout arbre connu (le document).
    pub target: Option<ElementId>,
    /// La cible est dans un menu ou une modale (éphémères, au-dessus de tout) : ce
    /// n'est pas « dehors » — on y choisit souvent pour le widget actif (le filtre
    /// d'une grappe) ; rien ne change.
    pub in_overlay: bool,
}

type Rappel = Rc<dyn Fn(bool)>;
type RappelVue = Rc<dyn Fn(Option<ElementId>)>;

struct Entry {
    /// Le numéro d'inscription (l'identité de l'« objet » entrée du TS).
    serie: u64,
    refs: usize,
    manual: bool,
    leaf: Option<LeafId>,
    cbs: Vec<(u64, Rappel)>,
}

struct State {
    entries: HashMap<ElementId, Entry>,
    actif: Option<ElementId>,
    view_cbs: Vec<(u64, RappelVue)>,
    /// Le geste en cours (son numéro) et la cible où il a commencé.
    geste: Option<u64>,
    cibles: HashMap<u64, Option<ElementId>>,
    /// Les gestes qui attendent leur relâché (`pointerup` / `pointercancel`).
    ecoutes: Vec<u64>,
    /// Les gestes relâchés, qui tranchent au tour suivant.
    minuteries: Vec<u64>,
    prochain: u64,
}

impl State {
    fn numero(&mut self) -> u64 {
        self.prochain += 1;
        self.prochain
    }
}

/// L'actif des widgets d'une vue. Clonable : une poignée sur un même état.
#[derive(Clone)]
pub struct WidgetActivation {
    st: Rc<RefCell<State>>,
    tree: Rc<dyn ElementTree>,
}

/// L'inscription d'UN élément (un même élément peut être inscrit plusieurs fois :
/// refcount).
#[derive(Clone)]
pub struct ActivationEntry {
    st: Weak<RefCell<State>>,
    el: ElementId,
    serie: u64,
    libere: Rc<std::cell::Cell<bool>>,
}

impl WidgetActivation {
    pub fn new(tree: Rc<dyn ElementTree>) -> Self {
        Self {
            st: Rc::new(RefCell::new(State {
                entries: HashMap::new(),
                actif: None,
                view_cbs: Vec::new(),
                geste: None,
                cibles: HashMap::new(),
                ecoutes: Vec::new(),
                minuteries: Vec::new(),
                prochain: 0,
            })),
            tree,
        }
    }

    /// L'élément actif de la vue, ou `None`.
    pub fn active_el(&self) -> Option<ElementId> {
        self.st.borrow().actif
    }

    /// La feuille hébergée par l'élément actif, ou `None`.
    pub fn active_leaf(&self) -> Option<LeafId> {
        let st = self.st.borrow();
        st.actif
            .and_then(|el| st.entries.get(&el))
            .and_then(|e| e.leaf)
    }

    /// Prévenu à chaque changement d'actif dans la vue (l'arbitre s'y replanifie).
    /// Rend le désabonnement.
    pub fn on_active_change(&self, cb: impl Fn(Option<ElementId>) + 'static) -> Box<dyn FnOnce()> {
        let id = self.st.borrow_mut().numero();
        self.st.borrow_mut().view_cbs.push((id, Rc::new(cb)));
        let st = Rc::downgrade(&self.st);
        Box::new(move || {
            if let Some(st) = st.upgrade() {
                st.borrow_mut().view_cbs.retain(|(i, _)| *i != id);
            }
        })
    }

    /// Inscrit `el` (ou compte une inscription de plus s'il l'est déjà).
    pub fn track(&self, el: ElementId) -> ActivationEntry {
        let mut st = self.st.borrow_mut();
        let serie = if let Some(e) = st.entries.get_mut(&el) {
            e.refs += 1;
            e.serie
        } else {
            let serie = st.numero();
            st.entries.insert(
                el,
                Entry {
                    serie,
                    refs: 1,
                    manual: false,
                    leaf: None,
                    cbs: Vec::new(),
                },
            );
            serie
        };
        ActivationEntry {
            st: Rc::downgrade(&self.st),
            el,
            serie,
            libere: Rc::new(std::cell::Cell::new(false)),
        }
    }

    /// Un `pointerdown` (en capture, sur tout le document). Hors bouton principal
    /// ou dans une couche éphémère : rien. Sinon le geste COMMENCE ; il tranchera
    /// après son relâché (`pointer_up`) puis `run_timers`.
    pub fn pointer_down(&self, ev: PointerDown) {
        if ev.button != 0 || ev.in_overlay {
            return;
        }
        let cible = ev.target.and_then(|n| self.entree_de(n));
        let mut st = self.st.borrow_mut();
        let id = st.numero();
        st.cibles.insert(id, cible);
        st.geste = Some(id);
        st.ecoutes.push(id);
    }

    /// Un `pointerup` ou `pointercancel` : tous les gestes en attente de relâché
    /// tranchent au tour suivant (`run_timers`).
    pub fn pointer_up(&self) {
        let mut st = self.st.borrow_mut();
        let ecoutes = std::mem::take(&mut st.ecoutes);
        st.minuteries.extend(ecoutes);
    }

    /// Le tour suivant (le `setTimeout(…, 0)` du TS) : tranche les gestes relâchés.
    pub fn run_timers(&self) {
        let minuteries = std::mem::take(&mut self.st.borrow_mut().minuteries);
        for id in minuteries {
            let cible = {
                let mut st = self.st.borrow_mut();
                if st.geste == Some(id) {
                    st.geste = None;
                }
                st.cibles.remove(&id).flatten()
            };
            if let Some(c) = cible {
                let st = self.st.borrow();
                // Retiré pendant le geste : rien ; manuel : il gère lui-même son activation.
                match st.entries.get(&c) {
                    None => continue,
                    Some(e) if e.manual => continue,
                    Some(_) => {}
                }
            }
            poser(&self.st, cible);
        }
    }

    /// Un `focusin` arrivé dans `target` : les entrées qui le contiennent s'activent,
    /// de l'extérieur vers l'intérieur (le dernier écouteur, le plus profond,
    /// l'emporte) — sauf pendant un geste (on ne tranche pas) ou si manuelles.
    pub fn focus_in(&self, target: ElementId) {
        let mut conteneurs: Vec<ElementId> = self
            .st
            .borrow()
            .entries
            .keys()
            .copied()
            .filter(|&e| self.tree.contains(e, target))
            .collect();
        // Du plus extérieur au plus profond (ordre d'une capture).
        conteneurs.sort_by(|&a, &b| {
            if a == b {
                std::cmp::Ordering::Equal
            } else if self.tree.contains(a, b) {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            }
        });
        for el in conteneurs {
            let ok = {
                let st = self.st.borrow();
                st.geste.is_none() && st.entries.get(&el).is_some_and(|e| !e.manual)
            };
            if ok {
                poser(&self.st, Some(el));
            }
        }
    }

    /// Désarme tout : plus d'inscription, plus d'actif, plus d'abonné de la vue.
    pub fn destroy(&self) {
        let mut st = self.st.borrow_mut();
        st.entries.clear();
        st.actif = None;
        st.view_cbs.clear();
        st.geste = None;
        st.cibles.clear();
        st.ecoutes.clear();
        st.minuteries.clear();
    }

    /// L'inscription la plus PROFONDE qui contient `n` (un widget dans un widget).
    fn entree_de(&self, n: ElementId) -> Option<ElementId> {
        let st = self.st.borrow();
        let mut best: Option<ElementId> = None;
        for &el in st.entries.keys() {
            if self.tree.contains(el, n) && best.is_none_or(|b| self.tree.contains(b, el)) {
                best = Some(el);
            }
        }
        best
    }
}

/// Change l'actif et prévient (l'ancien, le nouveau, puis la vue).
fn poser(st: &Rc<RefCell<State>>, e: Option<ElementId>) {
    let (avant_cbs, apres_cbs, vue_cbs) = {
        let mut s = st.borrow_mut();
        if s.actif == e {
            return;
        }
        let avant = s.actif;
        s.actif = e;
        let cbs = |s: &State, el: Option<ElementId>| -> Vec<Rappel> {
            el.and_then(|el| s.entries.get(&el))
                .map(|en| en.cbs.iter().map(|(_, c)| Rc::clone(c)).collect())
                .unwrap_or_default()
        };
        let a = cbs(&s, avant);
        let b = cbs(&s, e);
        let v: Vec<RappelVue> = s.view_cbs.iter().map(|(_, c)| Rc::clone(c)).collect();
        (a, b, v)
    };
    for cb in avant_cbs {
        cb(false);
    }
    for cb in apres_cbs {
        cb(true);
    }
    for cb in vue_cbs {
        cb(e);
    }
}

impl ActivationEntry {
    /// L'état, si l'inscription est encore la courante de son élément.
    fn vivante(&self) -> Option<Rc<RefCell<State>>> {
        let st = self.st.upgrade()?;
        let ok = st
            .borrow()
            .entries
            .get(&self.el)
            .is_some_and(|e| e.serie == self.serie);
        ok.then_some(st)
    }

    pub fn is_active(&self) -> bool {
        self.vivante()
            .is_some_and(|st| st.borrow().actif == Some(self.el))
    }

    /// Prévenu quand CET élément devient (ou cesse d'être) l'actif. Rend le désabonnement.
    pub fn on_change(&self, cb: impl Fn(bool) + 'static) -> Box<dyn FnOnce()> {
        let Some(st) = self.vivante() else {
            return Box::new(|| {});
        };
        let id = st.borrow_mut().numero();
        if let Some(e) = st.borrow_mut().entries.get_mut(&self.el) {
            e.cbs.push((id, Rc::new(cb)));
        }
        let weak = Rc::downgrade(&st);
        let el = self.el;
        Box::new(move || {
            if let Some(st) = weak.upgrade() {
                if let Some(e) = st.borrow_mut().entries.get_mut(&el) {
                    e.cbs.retain(|(i, _)| *i != id);
                }
            }
        })
    }

    /// Active (ou désactive) explicitement — seule voie pour un élément manuel.
    pub fn set_active(&self, active: bool) {
        if self.libere.get() {
            return;
        }
        let Some(st) = self.vivante() else { return };
        if active {
            poser(&st, Some(self.el));
        } else if st.borrow().actif == Some(self.el) {
            poser(&st, None);
        }
    }

    /// Manuel : un geste en lui ne l'active ni ne le désactive.
    pub fn set_manual(&self, manual: bool) {
        if let Some(st) = self.vivante() {
            if let Some(e) = st.borrow_mut().entries.get_mut(&self.el) {
                e.manual = manual;
            }
        }
    }

    /// La feuille que cet élément héberge (`None` : aucune) — lue par `active_leaf`.
    pub fn set_leaf(&self, leaf: Option<LeafId>) {
        let Some(st) = self.vivante() else { return };
        let (change, actif, vue_cbs) = {
            let mut s = st.borrow_mut();
            let Some(e) = s.entries.get_mut(&self.el) else {
                return;
            };
            if e.leaf == leaf {
                return;
            }
            e.leaf = leaf;
            let actif = s.actif == Some(self.el);
            let v: Vec<RappelVue> = s.view_cbs.iter().map(|(_, c)| Rc::clone(c)).collect();
            (true, actif, v)
        };
        // Déjà actif : la vue doit relire la feuille hébergée.
        if change && actif {
            for cb in vue_cbs {
                cb(Some(self.el));
            }
        }
    }

    /// Libère CETTE inscription (le même élément peut être inscrit plusieurs fois).
    /// Idempotent.
    pub fn release(&self) {
        if self.libere.replace(true) {
            return;
        }
        let Some(st) = self.vivante() else { return };
        // Comme le TS : l'inscription sort de la table, PUIS l'actif est relâché
        // (ses abonnés sont prévenus), puis ses abonnés sont vidés.
        let (retiree, etait_actif, vue_cbs) = {
            let mut s = st.borrow_mut();
            let Some(e) = s.entries.get_mut(&self.el) else {
                return;
            };
            e.refs -= 1;
            if e.refs > 0 {
                return;
            }
            let retiree = s.entries.remove(&self.el);
            let etait_actif = s.actif == Some(self.el);
            if etait_actif {
                s.actif = None;
            }
            // Le geste en cours perd sa cible si c'était cet élément.
            if let Some(g) = s.geste {
                if s.cibles.get(&g) == Some(&Some(self.el)) {
                    s.cibles.insert(g, None);
                }
            }
            let v: Vec<RappelVue> = s.view_cbs.iter().map(|(_, c)| Rc::clone(c)).collect();
            (retiree, etait_actif, v)
        };
        if etait_actif {
            if let Some(e) = retiree {
                for (_, cb) in e.cbs {
                    cb(false);
                }
            }
            for cb in vue_cbs {
                cb(None);
            }
        }
    }
}
