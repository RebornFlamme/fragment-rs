// Port de app/src/ui/core/layers/WidgetActivation.test.ts
//
// ★ Le TS dispatche de vrais événements DOM sous jsdom ; ici l'hôte est simulé :
//   `appui` = pointerdown, `relache` = pointerup, `clic` = appui + relâché + le
//   tour suivant (`vi.runAllTimers()` → `run_timers`). Le DOM de la page du test
//   (`#a > #a-btn`, `#b > #b-in`, `#dehors`) devient un arbre à parents.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use fragment_layers::types::{ElementId, LeafId};
use fragment_layers::widget_activation::{ElementTree, PointerDown, WidgetActivation};

const A: ElementId = ElementId(1);
const A_BTN: ElementId = ElementId(2);
const B: ElementId = ElementId(3);
const B_IN: ElementId = ElementId(4);
const DEHORS: ElementId = ElementId(5);

struct Arbre(HashMap<ElementId, ElementId>);

impl ElementTree for Arbre {
    fn contains(&self, ancestor: ElementId, node: ElementId) -> bool {
        let mut courant = Some(node);
        while let Some(n) = courant {
            if n == ancestor {
                return true;
            }
            courant = self.0.get(&n).copied();
        }
        false
    }
}

fn nouvelle() -> WidgetActivation {
    let parents = HashMap::from([(A_BTN, A), (B_IN, B)]);
    WidgetActivation::new(Rc::new(Arbre(parents)))
}

fn appui(act: &WidgetActivation, cible: ElementId, button: u8) {
    act.pointer_down(PointerDown {
        button,
        target: Some(cible),
        in_overlay: false,
    });
}

fn relache(act: &WidgetActivation) {
    act.pointer_up();
}

fn clic(act: &WidgetActivation, cible: ElementId, button: u8) {
    appui(act, cible, button);
    relache(act);
    act.run_timers();
}

type Journal<T> = Rc<RefCell<Vec<T>>>;

fn journal<T>() -> Journal<T> {
    Rc::new(RefCell::new(Vec::new()))
}

/// Combien d'éléments portent la classe `is-widget-active` (0 ou 1 : l'actif).
fn porteurs(act: &WidgetActivation) -> usize {
    usize::from(act.active_el().is_some())
}

#[test]
fn clic_dedans_actif_apres_le_geste_pas_a_l_appui_classe_posee_clic_dehors_aucun() {
    let act = nouvelle();
    let ea = act.track(A);
    let vus: Journal<bool> = journal();
    let v = Rc::clone(&vus);
    let _ = ea.on_change(move |actif| v.borrow_mut().push(actif));
    appui(&act, A_BTN, 0);
    assert!(!ea.is_active());
    relache(&act);
    assert!(!ea.is_active()); // tour suivant
    act.run_timers();
    assert!(ea.is_active());
    assert_eq!(act.active_el(), Some(A)); // la classe est posée sur A
    clic(&act, DEHORS, 0);
    assert!(!ea.is_active());
    assert_eq!(act.active_el(), None);
    assert_eq!(*vus.borrow(), vec![true, false]);
}

#[test]
fn un_seul_a_la_fois_la_vue_est_prevenue() {
    let act = nouvelle();
    let ea = act.track(A);
    let eb = act.track(B);
    let vue: Journal<Option<ElementId>> = journal();
    let v = Rc::clone(&vue);
    let _ = act.on_active_change(move |el| v.borrow_mut().push(el));
    clic(&act, A, 0);
    clic(&act, B, 0);
    assert!(!ea.is_active());
    assert!(eb.is_active());
    assert_eq!(porteurs(&act), 1);
    assert_eq!(*vue.borrow(), vec![Some(A), Some(B)]);
}

#[test]
fn focus_sans_geste_actif_tout_de_suite_pendant_un_geste_attend_le_relache() {
    let act = nouvelle();
    let eb = act.track(B);
    act.focus_in(B_IN);
    assert!(eb.is_active());
    clic(&act, DEHORS, 0);
    assert!(!eb.is_active());
    appui(&act, DEHORS, 0);
    // blur puis focus : seul le focusin compte pour le modèle.
    act.focus_in(B_IN);
    assert!(!eb.is_active()); // geste en cours : on ne tranche pas
}

#[test]
fn clic_droit_sans_effet() {
    let act = nouvelle();
    let ea = act.track(A);
    clic(&act, A, 2);
    assert!(!ea.is_active());
    clic(&act, A, 0);
    clic(&act, DEHORS, 2);
    assert!(ea.is_active());
}

#[test]
fn manuel_un_geste_en_lui_ne_l_active_pas_set_active_oui_le_clic_dehors_le_desactive() {
    let act = nouvelle();
    let ea = act.track(A);
    ea.set_manual(true);
    clic(&act, A, 0);
    assert!(!ea.is_active());
    ea.set_active(true);
    clic(&act, A, 0); // un geste en lui ne le désactive pas non plus
    assert!(ea.is_active());
    clic(&act, DEHORS, 0);
    assert!(!ea.is_active());
}

#[test]
fn inscriptions_multiples_du_meme_element_refcount_retrait_de_l_actif_aucun_actif() {
    let act = nouvelle();
    let e1 = act.track(A);
    let e2 = act.track(A);
    clic(&act, A, 0);
    assert!(e2.is_active());
    e1.release();
    e1.release(); // idempotent
    assert!(e2.is_active());
    let vue: Journal<Option<ElementId>> = journal();
    let v = Rc::clone(&vue);
    let _ = act.on_active_change(move |el| v.borrow_mut().push(el));
    e2.release();
    assert_eq!(*vue.borrow(), vec![None]);
    assert_eq!(porteurs(&act), 0);
    clic(&act, A, 0); // plus inscrit : rien
    assert_eq!(*vue.borrow(), vec![None]);
}

#[test]
fn widget_dans_un_widget_le_plus_profond_l_emporte() {
    let act = nouvelle();
    let ea = act.track(A);
    let ei = act.track(A_BTN);
    clic(&act, A_BTN, 0);
    assert!(ei.is_active());
    assert!(!ea.is_active());
}

#[test]
fn feuille_portee_active_leaf_suit_l_actif_set_leaf_sur_l_actif_previent_la_vue() {
    let feuille = LeafId(7);
    let act = nouvelle();
    let ea = act.track(A);
    let eb = act.track(B);
    let vue: Journal<Option<ElementId>> = journal();
    let v = Rc::clone(&vue);
    let _ = act.on_active_change(move |el| v.borrow_mut().push(el));
    eb.set_leaf(Some(feuille));
    assert_eq!(act.active_leaf(), None);
    clic(&act, B, 0);
    assert_eq!(act.active_leaf(), Some(feuille));
    eb.set_leaf(None);
    assert_eq!(act.active_leaf(), None);
    assert_eq!(*vue.borrow(), vec![Some(B), Some(B)]);
    clic(&act, A, 0);
    assert!(ea.is_active());
    assert_eq!(act.active_leaf(), None);
}

// ── cas ajoutés (le TS ne les teste pas) ────────────────────────────────────

#[test]
fn un_appui_dans_un_menu_ou_une_modale_ne_change_rien() {
    let act = nouvelle();
    let ea = act.track(A);
    clic(&act, A, 0);
    assert!(ea.is_active());
    // Le menu d'un widget : on y choisit POUR l'actif, ce n'est pas « dehors ».
    act.pointer_down(PointerDown {
        button: 0,
        target: Some(DEHORS),
        in_overlay: true,
    });
    act.pointer_up();
    act.run_timers();
    assert!(ea.is_active());
}

#[test]
fn un_element_retire_pendant_le_geste_n_est_pas_active() {
    let act = nouvelle();
    let ea = act.track(A);
    appui(&act, A, 0);
    ea.release();
    relache(&act);
    act.run_timers();
    assert_eq!(act.active_el(), None);
}

#[test]
fn les_abonnes_se_desabonnent() {
    let act = nouvelle();
    let ea = act.track(A);
    let vus: Journal<bool> = journal();
    let v = Rc::clone(&vus);
    let off = ea.on_change(move |actif| v.borrow_mut().push(actif));
    clic(&act, A, 0);
    off();
    clic(&act, DEHORS, 0);
    assert_eq!(*vus.borrow(), vec![true]);
}
