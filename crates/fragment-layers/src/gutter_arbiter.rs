// Port de app/src/ui/core/layers/GutterArbiter.ts
//
// ═══════════════════════════════════════════════════════════════════════════
//  L'ARBITRE DES MARGES — un par vue, porté par son OverlayHost.
//
//  ★ Pourquoi au niveau de la VUE : chaque calque crée son propre WidgetLayer. Un
//    empilement par WidgetLayer ne voit que SES widgets : deux calques, deux
//    piles, qui se chevauchent. La marge est une ressource PARTAGÉE de la vue —
//    elle s'arbitre là où tous les calques se rejoignent.
//
//  Contrat : un widget de marge prend un SLOT (`slot(el, side)`), y déclare son
//  haut voulu (`set_wanted`, `None` = n'occupe rien), et l'arbitre écrit LUI-MÊME
//  le haut de tous les slots d'un côté (gutter_stack : au plus près des ancres,
//  sans recouvrement, ordre du texte gardé).
//
//  ★ LA PLACE DISPONIBLE (`compute_rooms`) : à chaque passe, chaque slot reçoit la
//    hauteur dont il dispose avant de gêner le suivant — posée sur son élément
//    (`SlotElements::set_room`, la variable CSS `--gutter-room` du TS ; retirée
//    quand elle est illimitée) et annoncée à ses abonnés `on_room`,
//    SYNCHRONEMENT, avant qu'on le mesure. C'est une INFORMATION : le widget s'y
//    plie à SES seuils ou l'ignore (il sera décalé).
//
//  Règles :
//    - un élément NON AFFICHÉ (masqué par son propriétaire) n'occupe aucune
//      place ; un élément affiché de hauteur 0 garde son slot ;
//    - le slot ACTIF reste pile sur son ancre. L'actif n'est PAS décidé ici :
//      c'est celui de la vue (WidgetActivation, lu par `active_el`) ; l'arbitre
//      se replanifie quand il change (`invalidate`) ;
//    - recalcul regroupé (N `set_wanted` → une passe) ; `flush()` force la passe
//      pour qui lit une position ; `on_move` ne prévient que si le haut a changé.
//
//  ★ LES GRAPPES — le cœur GROUPE, CACHE, ANNONCE ; il ne dessine rien. Un slot
//    RÉDUIT regroupable et non actif se regroupe avec les réduits voisins. Dès 2
//    membres, TOUS sont cachés (`set_clustered` : invisibles mais mesurés — les
//    sortir de l'affichage les sortirait de la passe suivante, ça oscillerait) et
//    sortent du placement ; l'arbitre ANNONCE les grappes (`clusters_change`,
//    seulement si leur composition change).
//
//  ★ Transposition du temps : le TS planifie par `queueMicrotask`. Ici l'arbitre
//    pose un drapeau (`needs_flush`) et la boucle de l'hôte appelle
//    `run_microtasks()` à son point de contrôle (fin du tour) : passe en attente,
//    puis annonces des grappes (qui sont HORS de la passe — le présentateur monte
//    et retire des widgets, ce qui replanifie). Le `ResizeObserver` du TS devient
//    l'appel de l'hôte à `invalidate()` quand un élément de slot change de taille.
// ═══════════════════════════════════════════════════════════════════════════

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::{Rc, Weak};

use crate::gutter_stack::{
    ArrangeOptions, ClusterItem, GUTTER_STACK_GAP, GutterItem, RoomOptions, arrange_gutter,
    cluster_gutter, compute_rooms,
};
use crate::types::{ElementId, GutterCluster, GutterMember, GutterSide};

/// La distance d'ancre sous laquelle deux réduits se regroupent (≈ une hauteur d'icône).
const RAYON_GRAPPE: f64 = 24.0;

/// Ce que l'arbitre lit et écrit sur les éléments de ses slots (le DOM du TS).
pub trait SlotElements {
    /// L'élément est-il affiché ? (`getClientRects().length > 0` : faux pour un
    /// `display: none`, vrai pour une hauteur nulle.)
    fn is_displayed(&self, el: ElementId) -> bool;
    /// Sa hauteur MESURÉE (`offsetHeight`) — après que ses abonnés `on_room` ont réagi.
    fn height(&self, el: ElementId) -> f64;
    /// Pose son haut (repère du plan document).
    fn set_top(&self, el: ElementId, top: f64);
    /// Pose sa place disponible (`--gutter-room`) ; `None` la retire (illimitée ou
    /// non placé).
    fn set_room(&self, el: ElementId, room: Option<f64>);
    /// Le cache (`is-gutter-clustered`) : membre d'une grappe, invisible mais mesuré.
    fn set_clustered(&self, el: ElementId, clustered: bool);
}

/// Ce que la vue donne à l'arbitre pour ses grappes (optionnel : sans, pas de grappe).
pub trait GutterArbiterHost {
    /// Les grappes d'un côté ont changé (composition). Appelé hors de la passe.
    fn clusters_change(&self, side: GutterSide, clusters: Vec<GutterCluster>);
}

type RappelMove = Rc<dyn Fn()>;
type RappelPlace = Rc<dyn Fn(Option<f64>)>;

struct Slot {
    id: u64,
    el: ElementId,
    side: GutterSide,
    wanted: Option<f64>,
    /// Le dernier haut écrit par l'arbitre (pour ne prévenir que d'un vrai déplacement).
    placed: Option<f64>,
    listeners: Vec<(u64, RappelMove)>,
    /// La dernière place annoncée.
    room: Option<f64>,
    room_listeners: Vec<(u64, RappelPlace)>,
    /// Réduit (regroupable) ?
    compact: Option<Rc<GutterMember>>,
}

#[derive(Default)]
struct State {
    slots: Vec<Slot>,
    planifie: bool,
    /// La composition annoncée de chaque côté (on n'annonce que les changements).
    annonces: [String; 2],
    /// Un numéro par membre : la signature d'une composition.
    numeros: Vec<(Weak<GutterMember>, u64)>,
    prochain: u64,
    /// Les annonces à faire hors de la passe.
    differees: Vec<(GutterSide, Vec<GutterCluster>)>,
}

impl State {
    fn nouveau_id(&mut self) -> u64 {
        self.prochain += 1;
        self.prochain
    }

    fn slot_mut(&mut self, id: u64) -> Option<&mut Slot> {
        self.slots.iter_mut().find(|s| s.id == id)
    }

    fn numero(&mut self, m: &Rc<GutterMember>) -> u64 {
        for (w, n) in &self.numeros {
            if w.upgrade().is_some_and(|x| Rc::ptr_eq(&x, m)) {
                return *n;
            }
        }
        self.numeros.retain(|(w, _)| w.strong_count() > 0);
        let n = self.nouveau_id();
        self.numeros.push((Rc::downgrade(m), n));
        n
    }
}

struct Inner {
    st: RefCell<State>,
    elements: Rc<dyn SlotElements>,
    /// La hauteur VISIBLE du pane (px du plan) : le plafond de la place de l'actif.
    hauteur_visible: Box<dyn Fn() -> f64>,
    /// L'élément actif de la VUE (WidgetActivation) : le slot qui le porte est épinglé.
    actif_el: Box<dyn Fn() -> Option<ElementId>>,
    host: Option<Rc<dyn GutterArbiterHost>>,
}

/// L'arbitre des marges d'une vue. Clonable : une poignée sur un même arbitre.
#[derive(Clone)]
pub struct GutterArbiter {
    inner: Rc<Inner>,
}

/// La poignée d'UN slot, rendue par `GutterArbiter::slot`.
#[derive(Clone)]
pub struct GutterSlot {
    arb: Weak<Inner>,
    id: u64,
}

fn indice(side: GutterSide) -> usize {
    match side {
        GutterSide::Left => 0,
        GutterSide::Right => 1,
    }
}

impl GutterArbiter {
    /// Un arbitre sans plafond de place (hauteur visible infinie), sans actif, sans
    /// présentateur de grappes.
    pub fn new(elements: Rc<dyn SlotElements>) -> Self {
        Self {
            inner: Rc::new(Inner {
                st: RefCell::new(State::default()),
                elements,
                hauteur_visible: Box::new(|| f64::INFINITY),
                actif_el: Box::new(|| None),
                host: None,
            }),
        }
    }

    /// La hauteur visible du pane (plafond de la place de l'actif).
    pub fn with_visible_height(self, f: impl Fn() -> f64 + 'static) -> Self {
        self.reconfigurer(|i| i.hauteur_visible = Box::new(f))
    }

    /// L'élément actif de la vue (le slot qui le porte est épinglé).
    pub fn with_active_el(self, f: impl Fn() -> Option<ElementId> + 'static) -> Self {
        self.reconfigurer(|i| i.actif_el = Box::new(f))
    }

    /// Le présentateur des grappes : sans lui, aucune grappe n'est formée.
    pub fn with_host(self, host: Rc<dyn GutterArbiterHost>) -> Self {
        self.reconfigurer(|i| i.host = Some(host))
    }

    /// Les `with_*` s'appellent à la construction, avant tout partage de la poignée.
    fn reconfigurer(mut self, f: impl FnOnce(&mut Inner)) -> Self {
        if let Some(inner) = Rc::get_mut(&mut self.inner) {
            f(inner);
        }
        self
    }

    /// Prend un slot pour `el`, du côté `side`.
    pub fn slot(&self, el: ElementId, side: GutterSide) -> GutterSlot {
        let mut st = self.inner.st.borrow_mut();
        let id = st.nouveau_id();
        st.slots.push(Slot {
            id,
            el,
            side,
            wanted: None,
            placed: None,
            listeners: Vec::new(),
            room: None,
            room_listeners: Vec::new(),
            compact: None,
        });
        GutterSlot {
            arb: Rc::downgrade(&self.inner),
            id,
        }
    }

    /// Une passe est-elle en attente ? (L'hôte appelle alors `run_microtasks` à la
    /// fin du tour.)
    pub fn needs_flush(&self) -> bool {
        let st = self.inner.st.borrow();
        st.planifie || !st.differees.is_empty()
    }

    /// Recalcule MAINTENANT si une passe est en attente.
    pub fn flush(&self) {
        self.inner.flush();
    }

    /// Le point de contrôle de microtâches : la passe en attente, puis les annonces
    /// de grappes (hors passe), jusqu'à ce que plus rien ne soit en attente.
    pub fn run_microtasks(&self) {
        loop {
            self.inner.flush();
            let differees = std::mem::take(&mut self.inner.st.borrow_mut().differees);
            if differees.is_empty() {
                if self.inner.st.borrow().planifie {
                    continue;
                }
                break;
            }
            if let Some(host) = &self.inner.host {
                for (side, clusters) in differees {
                    host.clusters_change(side, clusters);
                }
            }
        }
    }

    /// L'actif de la vue a changé (ou un élément de slot a changé de taille) :
    /// nouvelle passe (épingle et places).
    pub fn invalidate(&self) {
        self.inner.schedule();
    }

    pub fn destroy(&self) {
        self.inner.st.borrow_mut().slots.clear();
    }
}

impl Inner {
    fn schedule(&self) {
        self.st.borrow_mut().planifie = true;
    }

    fn flush(&self) {
        {
            let mut st = self.st.borrow_mut();
            if !st.planifie {
                return;
            }
            st.planifie = false;
        }
        self.arrange();
    }

    fn arrange(&self) {
        let mut moved: Vec<u64> = Vec::new();
        let mut annonces: Vec<(GutterSide, Vec<GutterCluster>)> = Vec::new();
        // La place de l'actif : la hauteur visible, moins un souffle en haut et en bas.
        let cap = ((self.hauteur_visible)() - 2.0 * GUTTER_STACK_GAP).max(0.0);
        for side in GutterSide::TOUS {
            // Un instantané du côté : les abonnés appelés pendant la passe peuvent
            // poser/retirer des slots (la passe suivante les verra).
            let du_cote: Vec<Instantane> = self
                .st
                .borrow()
                .slots
                .iter()
                .filter(|s| s.side == side)
                .map(|s| Instantane {
                    id: s.id,
                    el: s.el,
                    wanted: s.wanted,
                    compact: s.compact.clone(),
                })
                .collect();
            let pile: Vec<&Instantane> = du_cote.iter().filter(|s| s.wanted.is_some()).collect();
            // Non affiché = masqué par son propriétaire : aucune place.
            let visibles: Vec<&Instantane> = pile
                .iter()
                .copied()
                .filter(|s| self.elements.is_displayed(s.el))
                .collect();
            let actif_el = (self.actif_el)();
            // 0. Les grappes (sans hôte qui les présente : aucune). Leurs membres sont
            //    cachés et sortent du placement ; le reste se place comme toujours.
            let groupes: Vec<Vec<&Instantane>> = if self.host.is_some() {
                let items: Vec<ClusterItem> = visibles
                    .iter()
                    .map(|s| ClusterItem {
                        top: s.wanted.unwrap_or(0.0),
                        compact: s.compact.as_ref().is_some_and(|c| c.groupable())
                            && Some(s.el) != actif_el,
                    })
                    .collect();
                cluster_gutter(&items, RAYON_GRAPPE)
                    .into_iter()
                    .filter(|g| g.len() > 1)
                    .map(|g| g.into_iter().map(|i| visibles[i]).collect())
                    .collect()
            } else {
                Vec::new()
            };
            let caches: HashSet<u64> = groupes.iter().flatten().map(|s| s.id).collect();
            for s in &du_cote {
                self.elements.set_clustered(s.el, caches.contains(&s.id));
            }
            let pleins: Vec<&Instantane> = visibles
                .iter()
                .copied()
                .filter(|s| !caches.contains(&s.id))
                .collect();
            let pinned = actif_el.and_then(|el| pleins.iter().position(|s| s.el == el));
            // 1. La place de chacun, dans l'ordre du texte ; il y réagit, on le mesure.
            let voulus: Vec<f64> = pleins.iter().map(|s| s.wanted.unwrap_or(0.0)).collect();
            let rooms = compute_rooms(
                &voulus,
                RoomOptions {
                    pinned,
                    cap,
                    ..RoomOptions::default()
                },
                |i, room| {
                    self.annoncer_place(pleins[i].id, pleins[i].el, Some(room));
                    self.elements.height(pleins[i].el)
                },
            );
            for s in &du_cote {
                if !pleins.iter().any(|p| p.id == s.id) {
                    self.annoncer_place(s.id, s.el, None);
                }
            }
            // 2. Le placement au plus près, avec les hauteurs obtenues.
            let items: Vec<GutterItem> = pleins
                .iter()
                .zip(&rooms.heights)
                .map(|(s, &height)| GutterItem {
                    top: s.wanted.unwrap_or(0.0),
                    height,
                })
                .collect();
            let tops = arrange_gutter(
                &items,
                ArrangeOptions {
                    pinned,
                    min_top: 0.0,
                    ..ArrangeOptions::default()
                },
            );
            for s in &pile {
                let voulu = s.wanted.unwrap_or(0.0);
                let top = pleins
                    .iter()
                    .position(|p| p.id == s.id)
                    .map_or(voulu, |i| tops[i]);
                self.elements.set_top(s.el, top);
                let mut st = self.st.borrow_mut();
                if let Some(slot) = st.slot_mut(s.id) {
                    if slot.placed != Some(top) {
                        slot.placed = Some(top);
                        moved.push(s.id);
                    }
                }
            }
            // 3. L'annonce, si la composition a changé.
            let grappes: Vec<GutterCluster> = groupes
                .iter()
                .map(|g| GutterCluster {
                    side,
                    members: g.iter().filter_map(|s| s.compact.clone()).collect(),
                })
                .collect();
            let mut st = self.st.borrow_mut();
            let signature = grappes
                .iter()
                .map(|c| {
                    c.members
                        .iter()
                        .map(|m| st.numero(m).to_string())
                        .collect::<Vec<_>>()
                        .join(",")
                })
                .collect::<Vec<_>>()
                .join("|");
            if signature != st.annonces[indice(side)] {
                st.annonces[indice(side)] = signature;
                annonces.push((side, grappes));
            }
        }
        for id in moved {
            let cbs: Vec<RappelMove> = self
                .st
                .borrow()
                .slots
                .iter()
                .find(|s| s.id == id)
                .map(|s| s.listeners.iter().map(|(_, c)| Rc::clone(c)).collect())
                .unwrap_or_default();
            for cb in cbs {
                cb();
            }
        }
        // Hors de la passe : le présentateur monte / retire des widgets (nouvelle passe).
        self.st.borrow_mut().differees.extend(annonces);
    }

    /// Pose la place (élément + abonnés), seulement si elle a changé.
    fn annoncer_place(&self, id: u64, el: ElementId, room: Option<f64>) {
        let cbs: Vec<RappelPlace> = {
            let mut st = self.st.borrow_mut();
            let Some(s) = st.slot_mut(id) else { return };
            if s.room == room {
                return;
            }
            s.room = room;
            s.room_listeners.iter().map(|(_, c)| Rc::clone(c)).collect()
        };
        self.elements.set_room(el, room.filter(|r| r.is_finite()));
        for cb in cbs {
            cb(room);
        }
    }
}

/// Un slot tel qu'il était au début de la passe.
struct Instantane {
    id: u64,
    el: ElementId,
    wanted: Option<f64>,
    compact: Option<Rc<GutterMember>>,
}

impl GutterSlot {
    fn avec<R>(&self, f: impl FnOnce(&mut Slot) -> R) -> Option<R> {
        let inner = self.arb.upgrade()?;
        let mut st = inner.st.borrow_mut();
        let s = st.slot_mut(self.id)?;
        // L'emprunt de `st` couvre `f` : `f` ne rappelle jamais d'abonné.
        Some(f(s))
    }

    /// Le haut voulu (repère du plan document), ou `None` : non affiché, aucune place.
    pub fn set_wanted(&self, top: Option<f64>) {
        let change = self
            .avec(|s| {
                if s.wanted == top {
                    false
                } else {
                    s.wanted = top;
                    true
                }
            })
            .unwrap_or(false);
        if change {
            self.planifier();
        }
    }

    /// Prévenu quand l'arbitre déplace l'élément. Rend le désabonnement.
    pub fn on_move(&self, cb: impl Fn() + 'static) -> Box<dyn FnOnce()> {
        let rappel: RappelMove = Rc::new(cb);
        let cb_id = self
            .arb
            .upgrade()
            .map(|i| i.st.borrow_mut().nouveau_id())
            .unwrap_or(0);
        self.avec(|s| s.listeners.push((cb_id, rappel)));
        let (arb, id) = (self.arb.clone(), self.id);
        Box::new(move || {
            if let Some(inner) = arb.upgrade() {
                if let Some(s) = inner.st.borrow_mut().slot_mut(id) {
                    s.listeners.retain(|(i, _)| *i != cb_id);
                }
            }
        })
    }

    /// La place disponible (px du plan), `INFINITY` = illimitée, `None` = non placé.
    pub fn get_room(&self) -> Option<f64> {
        self.flush();
        self.avec(|s| s.room).flatten()
    }

    /// Prévenu quand la place change — PENDANT la passe d'arbitrage, avant la
    /// mesure : rendre le nouveau rendu SYNCHRONEMENT pour qu'il compte tout de suite.
    pub fn on_room(&self, cb: impl Fn(Option<f64>) + 'static) -> Box<dyn FnOnce()> {
        let rappel: RappelPlace = Rc::new(cb);
        let cb_id = self
            .arb
            .upgrade()
            .map(|i| i.st.borrow_mut().nouveau_id())
            .unwrap_or(0);
        self.avec(|s| s.room_listeners.push((cb_id, rappel)));
        let (arb, id) = (self.arb.clone(), self.id);
        Box::new(move || {
            if let Some(inner) = arb.upgrade() {
                if let Some(s) = inner.st.borrow_mut().slot_mut(id) {
                    s.room_listeners.retain(|(i, _)| *i != cb_id);
                }
            }
        })
    }

    /// Applique tout de suite la passe en attente (avant de LIRE une position).
    pub fn flush(&self) {
        if let Some(inner) = self.arb.upgrade() {
            inner.flush();
        }
    }

    /// RÉDUIT à des icônes (regroupable avec ses voisins réduits), ou `None` : entier.
    /// Comparé par identité (`Rc::ptr_eq`), comme le `===` du TS.
    pub fn set_compact(&self, member: Option<Rc<GutterMember>>) {
        let change = self
            .avec(|s| {
                let pareil = match (&s.compact, &member) {
                    (None, None) => true,
                    (Some(a), Some(b)) => Rc::ptr_eq(a, b),
                    _ => false,
                };
                if !pareil {
                    s.compact = member;
                }
                !pareil
            })
            .unwrap_or(false);
        if change {
            self.planifier();
        }
    }

    pub fn remove(&self) {
        let Some(inner) = self.arb.upgrade() else {
            return;
        };
        let el = {
            let mut st = inner.st.borrow_mut();
            let Some(pos) = st.slots.iter().position(|s| s.id == self.id) else {
                return;
            };
            st.slots.remove(pos).el
        };
        inner.elements.set_room(el, None);
        inner.elements.set_clustered(el, false);
        inner.schedule();
    }

    fn planifier(&self) {
        if let Some(inner) = self.arb.upgrade() {
            inner.schedule();
        }
    }
}
