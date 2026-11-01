// Port de app/src/ui/core/layers/GutterArbiter.test.ts
//
// ★ Le TS monte de faux éléments jsdom (`getClientRects`, `offsetHeight`) ; ici
//   `Elements` implémente `SlotElements` : hauteur et affichage déclarés, hauts,
//   places et cache enregistrés. `attendre()` (le tour de microtâches) devient
//   `arb.run_microtasks()`.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use fragment_layers::gutter_arbiter::{GutterArbiter, GutterArbiterHost, SlotElements};
use fragment_layers::types::{CompactInfo, ElementId, GutterCluster, GutterMember, GutterSide};

#[derive(Default)]
struct Elements {
    hauteurs: RefCell<HashMap<ElementId, f64>>,
    caches: RefCell<HashSet<ElementId>>,
    tops: RefCell<HashMap<ElementId, f64>>,
    places: RefCell<HashMap<ElementId, f64>>,
    masques: RefCell<HashSet<ElementId>>,
    suivant: RefCell<u64>,
}

impl Elements {
    /// Un élément « affiché » de hauteur `h`.
    fn element(&self, h: f64) -> ElementId {
        let mut n = self.suivant.borrow_mut();
        *n += 1;
        let el = ElementId(*n);
        self.hauteurs.borrow_mut().insert(el, h);
        el
    }

    fn est_cache(&self, el: ElementId) -> bool {
        self.caches.borrow().contains(&el)
    }

    fn top(&self, el: ElementId) -> Option<f64> {
        self.tops.borrow().get(&el).copied()
    }
}

impl SlotElements for Elements {
    fn is_displayed(&self, el: ElementId) -> bool {
        !self.masques.borrow().contains(&el)
    }

    fn height(&self, el: ElementId) -> f64 {
        self.hauteurs.borrow().get(&el).copied().unwrap_or(0.0)
    }

    fn set_top(&self, el: ElementId, top: f64) {
        self.tops.borrow_mut().insert(el, top);
    }

    fn set_room(&self, el: ElementId, room: Option<f64>) {
        match room {
            Some(r) => self.places.borrow_mut().insert(el, r),
            None => self.places.borrow_mut().remove(&el),
        };
    }

    fn set_clustered(&self, el: ElementId, clustered: bool) {
        if clustered {
            self.caches.borrow_mut().insert(el);
        } else {
            self.caches.borrow_mut().remove(&el);
        }
    }
}

struct Annonces(Rc<RefCell<Vec<Vec<GutterCluster>>>>);

impl GutterArbiterHost for Annonces {
    fn clusters_change(&self, _side: GutterSide, clusters: Vec<GutterCluster>) {
        self.0.borrow_mut().push(clusters);
    }
}

fn membre(label: &str, groupable: bool) -> Rc<GutterMember> {
    let mut info = CompactInfo::new(label, vec![label.into()], || {});
    info.groupable = groupable;
    Rc::new(GutterMember::new(Rc::new(info), || 0))
}

fn arbitre(
    elements: &Rc<Elements>,
    avec_hote: bool,
) -> (GutterArbiter, Rc<RefCell<Vec<Vec<GutterCluster>>>>) {
    let annonces = Rc::new(RefCell::new(Vec::new()));
    let mut arb = GutterArbiter::new(Rc::clone(elements) as Rc<dyn SlotElements>)
        .with_visible_height(|| 1000.0)
        .with_active_el(|| None);
    if avec_hote {
        arb = arb.with_host(Rc::new(Annonces(Rc::clone(&annonces))));
    }
    (arb, annonces)
}

#[test]
fn deux_reduits_voisins_caches_hors_du_placement_annonces_une_fois_composition() {
    let els = Rc::new(Elements::default());
    let (arb, annonces) = arbitre(&els, true);
    let (a, b, c) = (els.element(30.0), els.element(30.0), els.element(200.0));
    let sa = arb.slot(a, GutterSide::Right);
    let sb = arb.slot(b, GutterSide::Right);
    let sc = arb.slot(c, GutterSide::Right);
    sa.set_compact(Some(membre("A", true)));
    let mb = membre("B", true);
    sb.set_compact(Some(Rc::clone(&mb)));
    sa.set_wanted(Some(100.0));
    sb.set_wanted(Some(110.0));
    sc.set_wanted(Some(105.0)); // c : entier, entre les deux
    arb.flush();
    arb.run_microtasks();
    assert!(els.est_cache(a));
    assert!(els.est_cache(b));
    assert!(!els.est_cache(c));
    // c, seul placé, reste sur son ancre (les cachés ne le poussent pas).
    assert_eq!(els.top(c), Some(105.0));
    assert_eq!(annonces.borrow().len(), 1);
    let etiquettes: Vec<Vec<String>> = annonces.borrow()[0]
        .iter()
        .map(|g| g.members.iter().map(|m| m.label().to_string()).collect())
        .collect();
    assert_eq!(etiquettes, vec![vec!["A".to_string(), "B".to_string()]]);
    // Rien n'a changé : pas de nouvelle annonce.
    sc.set_wanted(Some(106.0));
    arb.flush();
    arb.run_microtasks();
    assert_eq!(annonces.borrow().len(), 1);
    // Un membre redevient entier : la grappe se défait, annoncée vide.
    sb.set_compact(None);
    arb.flush();
    arb.run_microtasks();
    assert!(annonces.borrow().last().is_some_and(Vec::is_empty));
    assert!(!els.est_cache(a));
}

#[test]
fn un_reduit_non_regroupable_il_presente_une_grappe_ne_se_regroupe_jamais() {
    let els = Rc::new(Elements::default());
    let (arb, annonces) = arbitre(&els, true);
    let sa = arb.slot(els.element(30.0), GutterSide::Right);
    let sb = arb.slot(els.element(30.0), GutterSide::Right);
    sa.set_compact(Some(membre("A", true)));
    sb.set_compact(Some(membre("G", false)));
    sa.set_wanted(Some(100.0));
    sb.set_wanted(Some(100.0));
    arb.flush();
    arb.run_microtasks();
    assert!(annonces.borrow().is_empty());
}

#[test]
fn sans_presentateur_pas_d_hote_aucune_grappe_rien_de_cache() {
    let els = Rc::new(Elements::default());
    let (arb, _) = arbitre(&els, false);
    let (a, b) = (els.element(30.0), els.element(30.0));
    let sa = arb.slot(a, GutterSide::Right);
    let sb = arb.slot(b, GutterSide::Right);
    sa.set_compact(Some(membre("A", true)));
    sb.set_compact(Some(membre("B", true)));
    sa.set_wanted(Some(100.0));
    sb.set_wanted(Some(100.0));
    arb.flush();
    assert!(!els.est_cache(a));
}

// ── cas ajoutés : le placement et la place (le TS les couvre via gutterStack et
//    les e2e, pas dans ce fichier) ───────────────────────────────────────────

#[test]
fn deux_entiers_en_conflit_se_partagent_le_decalage_et_on_move_ne_previent_que_d_un_vrai_deplacement()
 {
    let els = Rc::new(Elements::default());
    let (arb, _) = arbitre(&els, false);
    let (a, b) = (els.element(50.0), els.element(50.0));
    let sa = arb.slot(a, GutterSide::Left);
    let sb = arb.slot(b, GutterSide::Left);
    let deplacements = Rc::new(RefCell::new(0));
    let d = Rc::clone(&deplacements);
    let _ = sb.on_move(move || *d.borrow_mut() += 1);
    sa.set_wanted(Some(100.0));
    sb.set_wanted(Some(110.0));
    arb.flush();
    // Voulus 100 et 110, hauteur 50, gap 8 : il manque 48 → 24 chacun.
    assert_eq!(els.top(a), Some(76.0));
    assert_eq!(els.top(b), Some(134.0));
    assert_eq!(*deplacements.borrow(), 1);
    // Même voulu : rien ne bouge, personne n'est prévenu.
    sb.set_wanted(Some(110.0));
    arb.flush();
    assert_eq!(*deplacements.borrow(), 1);
}

#[test]
fn la_place_est_annoncee_avant_la_mesure_et_retiree_quand_elle_est_illimitee() {
    let els = Rc::new(Elements::default());
    let (arb, _) = arbitre(&els, false);
    let (a, b) = (els.element(30.0), els.element(30.0));
    let sa = arb.slot(a, GutterSide::Right);
    let sb = arb.slot(b, GutterSide::Right);
    let vues = Rc::new(RefCell::new(Vec::new()));
    let v = Rc::clone(&vues);
    let _ = sa.on_room(move |room| v.borrow_mut().push(room));
    sa.set_wanted(Some(100.0));
    sb.set_wanted(Some(300.0));
    // Lire la place force la passe.
    assert_eq!(sa.get_room(), Some(192.0));
    assert_eq!(sb.get_room(), Some(f64::INFINITY));
    assert_eq!(*vues.borrow(), vec![Some(192.0)]);
    // Illimitée : la variable est retirée de l'élément ; bornée : posée.
    assert_eq!(els.places.borrow().get(&a), Some(&192.0));
    assert!(!els.places.borrow().contains_key(&b));
    // Un slot sans voulu n'occupe rien : plus de place.
    sa.set_wanted(None);
    assert_eq!(sa.get_room(), None);
    assert_eq!(*vues.borrow(), vec![Some(192.0), None]);
}

#[test]
fn un_element_non_affiche_n_occupe_aucune_place_un_actif_reste_pile_sur_son_ancre() {
    let els = Rc::new(Elements::default());
    let actif = Rc::new(RefCell::new(None));
    let act = Rc::clone(&actif);
    let arb = GutterArbiter::new(Rc::clone(&els) as Rc<dyn SlotElements>)
        .with_visible_height(|| 1000.0)
        .with_active_el(move || *act.borrow());
    let (a, b, cache) = (els.element(50.0), els.element(50.0), els.element(500.0));
    let sa = arb.slot(a, GutterSide::Right);
    let sb = arb.slot(b, GutterSide::Right);
    let sc = arb.slot(cache, GutterSide::Right);
    els.masques.borrow_mut().insert(cache);
    sa.set_wanted(Some(100.0));
    sb.set_wanted(Some(110.0));
    sc.set_wanted(Some(105.0)); // masqué : ne pousse personne
    arb.flush();
    assert_eq!(els.top(a), Some(76.0));
    // L'actif change : la vue prévient l'arbitre, `b` est épinglé sur son ancre.
    *actif.borrow_mut() = Some(b);
    arb.invalidate();
    arb.flush();
    assert_eq!(els.top(b), Some(110.0));
    assert_eq!(els.top(a), Some(110.0 - 8.0 - 50.0));
}

#[test]
fn retirer_un_slot_rend_sa_place_et_son_cache_et_replanifie() {
    let els = Rc::new(Elements::default());
    let (arb, _) = arbitre(&els, true);
    let (a, b) = (els.element(30.0), els.element(30.0));
    let sa = arb.slot(a, GutterSide::Right);
    let sb = arb.slot(b, GutterSide::Right);
    sa.set_compact(Some(membre("A", true)));
    sb.set_compact(Some(membre("B", true)));
    sa.set_wanted(Some(100.0));
    sb.set_wanted(Some(100.0));
    arb.run_microtasks();
    assert!(els.est_cache(a));
    sa.remove();
    sa.remove(); // idempotent
    assert!(!els.est_cache(a));
    assert!(!els.places.borrow().contains_key(&a));
    assert!(arb.needs_flush());
    arb.run_microtasks();
    assert!(!els.est_cache(b));
}
